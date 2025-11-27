use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TidePoolError;
use crate::events::SwapExecuted;
use crate::math::fee_math::{calculate_fee_growth_delta, calculate_protocol_fee};
use crate::math::swap_math::compute_swap_step;
use crate::math::tick_math::tick_to_sqrt_price;
use crate::state::{Pool, TickArray};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        constraint = token_vault_a.key() == pool.token_vault_a @ TidePoolError::Unauthorized,
    )]
    pub token_vault_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_vault_b.key() == pool.token_vault_b @ TidePoolError::Unauthorized,
    )]
    pub token_vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_owner_account_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_owner_account_b: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = tick_array_0.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_0: Account<'info, TickArray>,

    #[account(
        mut,
        constraint = tick_array_1.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_1: Account<'info, TickArray>,

    #[account(
        mut,
        constraint = tick_array_2.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_2: Account<'info, TickArray>,

    pub trader: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(
    ctx: Context<Swap>,
    amount: u64,
    other_amount_threshold: u64,
    sqrt_price_limit: u128,
    amount_specified_is_input: bool,
    a_to_b: bool,
) -> Result<()> {
    require!(amount > 0, TidePoolError::ZeroSwapAmount);

    let pool = &ctx.accounts.pool;
    require!(!pool.paused, TidePoolError::PoolPaused);

    // Validate sqrt_price_limit direction
    if a_to_b {
        require!(
            sqrt_price_limit < pool.sqrt_price,
            TidePoolError::SqrtPriceLimitReached
        );
    } else {
        require!(
            sqrt_price_limit > pool.sqrt_price,
            TidePoolError::SqrtPriceLimitReached
        );
    }

    let mut current_sqrt_price = pool.sqrt_price;
    let mut current_tick_index = pool.tick_current_index;
    let mut current_liquidity = pool.liquidity;
    let mut amount_remaining = amount;
    let mut amount_calculated: u64 = 0;
    let mut total_fee_amount: u64 = 0;
    let fee_rate = pool.fee_rate;

    let tick_spacing = pool.tick_spacing;

    // Swap loop: iterate through tick ranges
    let tick_arrays = [
        &mut ctx.accounts.tick_array_0,
        &mut ctx.accounts.tick_array_1,
        &mut ctx.accounts.tick_array_2,
    ];

    // Process swap across tick arrays
    let mut array_idx = 0;
    let mut steps = 0;
    const MAX_STEPS: usize = 100;

    while amount_remaining > 0 && current_sqrt_price != sqrt_price_limit && steps < MAX_STEPS {
        steps += 1;

        // Find next initialized tick in the current tick array
        let next_tick_index = find_next_initialized_tick(
            &tick_arrays,
            array_idx,
            current_tick_index,
            tick_spacing,
            a_to_b,
        );

        let sqrt_price_next_tick = match next_tick_index {
            Some(tick) => tick_to_sqrt_price(tick)?,
            None => sqrt_price_limit,
        };

        // Clamp to price limit
        let sqrt_price_target = if a_to_b {
            sqrt_price_next_tick.max(sqrt_price_limit)
        } else {
            sqrt_price_next_tick.min(sqrt_price_limit)
        };

        // Compute swap step
        let step = compute_swap_step(
            current_sqrt_price,
            sqrt_price_target,
            current_liquidity,
            amount_remaining,
            fee_rate,
            amount_specified_is_input,
            a_to_b,
        )?;

        // Update running totals
        if amount_specified_is_input {
            amount_remaining = amount_remaining
                .checked_sub(step.amount_in)
                .ok_or(TidePoolError::MathOverflow)?
                .checked_sub(step.fee_amount)
                .ok_or(TidePoolError::MathOverflow)?;
            amount_calculated = amount_calculated
                .checked_add(step.amount_out)
                .ok_or(TidePoolError::MathOverflow)?;
        } else {
            amount_remaining = amount_remaining
                .checked_sub(step.amount_out)
                .ok_or(TidePoolError::MathOverflow)?;
            amount_calculated = amount_calculated
                .checked_add(step.amount_in)
                .ok_or(TidePoolError::MathOverflow)?
                .checked_add(step.fee_amount)
                .ok_or(TidePoolError::MathOverflow)?;
        }

        total_fee_amount = total_fee_amount
            .checked_add(step.fee_amount)
            .ok_or(TidePoolError::MathOverflow)?;
        current_sqrt_price = step.sqrt_price_next;

        // If we reached the next tick, cross it
        if let Some(next_tick) = next_tick_index {
            if current_sqrt_price == sqrt_price_next_tick {
                if let Some(tick_data) =
                    get_tick_from_arrays(&tick_arrays, array_idx, next_tick, tick_spacing)
                {
                    // liquidity_net is i128: positive at lower bound, negative at upper.
                    // When crossing downward (a_to_b), we negate the net before applying.
                    let liquidity_delta = if a_to_b {
                        -tick_data.liquidity_net
                    } else {
                        tick_data.liquidity_net
                    };

                    if liquidity_delta >= 0 {
                        current_liquidity = current_liquidity
                            .checked_add(liquidity_delta as u128)
                            .ok_or(TidePoolError::MathOverflow)?;
                    } else {
                        current_liquidity = current_liquidity
                            .checked_sub(liquidity_delta.unsigned_abs())
                            .ok_or(TidePoolError::MathOverflow)?;
                    }

                    current_tick_index = if a_to_b {
                        next_tick - 1
                    } else {
                        next_tick
                    };
                }
            }
        } else {
            // Move to next tick array
            if array_idx < 2 {
                array_idx += 1;
            } else {
                break;
            }
        }
    }

    // Determine final amounts
    let (amount_in, amount_out) = if amount_specified_is_input {
        (amount - amount_remaining, amount_calculated)
    } else {
        (amount_calculated, amount - amount_remaining)
    };

    // Check slippage
    if amount_specified_is_input {
        require!(
            amount_out >= other_amount_threshold,
            TidePoolError::SlippageExceeded
        );
    } else {
        require!(
            amount_in <= other_amount_threshold,
            TidePoolError::SlippageExceeded
        );
    }

    // Update pool state
    let pool = &mut ctx.accounts.pool;

    // Update fee growth
    if current_liquidity > 0 && total_fee_amount > 0 {
        let protocol_fee = calculate_protocol_fee(total_fee_amount, pool.protocol_fee_rate);
        let lp_fee = total_fee_amount.saturating_sub(protocol_fee);

        let fee_growth_delta = calculate_fee_growth_delta(lp_fee, current_liquidity);

        if a_to_b {
            pool.fee_growth_global_a = pool
                .fee_growth_global_a
                .wrapping_add(fee_growth_delta);
            pool.protocol_fees_owed_a = pool
                .protocol_fees_owed_a
                .saturating_add(protocol_fee);
        } else {
            pool.fee_growth_global_b = pool
                .fee_growth_global_b
                .wrapping_add(fee_growth_delta);
            pool.protocol_fees_owed_b = pool
                .protocol_fees_owed_b
                .saturating_add(protocol_fee);
        }
    }

    pool.sqrt_price = current_sqrt_price;
    pool.tick_current_index = current_tick_index;
    pool.liquidity = current_liquidity;

    // Execute token transfers
    let mint_a = pool.token_mint_a;
    let mint_b = pool.token_mint_b;
    let tick_spacing_bytes = pool.tick_spacing.to_le_bytes();
    let bump = pool.bump;

    let seeds = &[
        b"pool".as_ref(),
        mint_a.as_ref(),
        mint_b.as_ref(),
        tick_spacing_bytes.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]];

    if a_to_b {
        // Trader sends token A, receives token B
        if amount_in > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.token_owner_account_a.to_account_info(),
                        to: ctx.accounts.token_vault_a.to_account_info(),
                        authority: ctx.accounts.trader.to_account_info(),
                    },
                ),
                amount_in,
            )?;
        }
        if amount_out > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.token_vault_b.to_account_info(),
                        to: ctx.accounts.token_owner_account_b.to_account_info(),
                        authority: pool.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount_out,
            )?;
        }
    } else {
        // Trader sends token B, receives token A
        if amount_in > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.token_owner_account_b.to_account_info(),
                        to: ctx.accounts.token_vault_b.to_account_info(),
                        authority: ctx.accounts.trader.to_account_info(),
                    },
                ),
                amount_in,
            )?;
        }
        if amount_out > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.token_vault_a.to_account_info(),
                        to: ctx.accounts.token_owner_account_a.to_account_info(),
                        authority: pool.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount_out,
            )?;
        }
    }

    emit!(SwapExecuted {
        pool: pool.key(),
        trader: ctx.accounts.trader.key(),
        a_to_b,
        amount_in,
        amount_out,
        fee_amount: total_fee_amount,
        sqrt_price_after: current_sqrt_price,
        tick_after: current_tick_index,
    });

    Ok(())
}

/// Find next initialized tick in the given direction.
/// For a_to_b (price decreasing): scan backwards from current_tick to find
/// the nearest initialized tick below.
/// For b_to_a (price increasing): scan forwards from current_tick to find
/// the nearest initialized tick above.
fn find_next_initialized_tick(
    tick_arrays: &[&mut Account<'_, TickArray>; 3],
    array_idx: usize,
    current_tick: i32,
    tick_spacing: u16,
    a_to_b: bool,
) -> Option<i32> {
    if array_idx >= tick_arrays.len() {
        return None;
    }
    let ta = &tick_arrays[array_idx];
    let spacing = tick_spacing as i32;
    let num_ticks = ta.ticks.len();

    if a_to_b {
        // Scan backwards: find the highest initialized tick <= current_tick
        let mut best: Option<i32> = None;
        for i in 0..num_ticks {
            let tick_index = ta.start_tick_index + (i as i32) * spacing;
            if tick_index >= current_tick {
                break;
            }
            if let Some(t) = ta.ticks.get(i) {
                if t.initialized {
                    best = Some(tick_index);
                }
            }
        }
        best
    } else {
        // Scan forwards: find the lowest initialized tick > current_tick
        for i in 0..num_ticks {
            let tick_index = ta.start_tick_index + (i as i32) * spacing;
            if tick_index <= current_tick {
                continue;
            }
            if let Some(t) = ta.ticks.get(i) {
                if t.initialized {
                    return Some(tick_index);
                }
            }
        }
        None
    }
}

/// Get tick data from arrays by tick index.
fn get_tick_from_arrays<'a>(
    tick_arrays: &'a [&mut Account<'_, TickArray>; 3],
    array_idx: usize,
    tick_index: i32,
    tick_spacing: u16,
) -> Option<&'a crate::state::tick::Tick> {
    if array_idx >= tick_arrays.len() {
        return None;
    }
    tick_arrays[array_idx].get_tick(tick_index, tick_spacing)
}
