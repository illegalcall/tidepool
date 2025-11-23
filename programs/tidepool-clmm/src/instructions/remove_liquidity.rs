use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TidePoolError;
use crate::events::LiquidityRemoved;
use crate::math::liquidity_math::{add_liquidity_delta, calculate_fee_growth_inside};
use crate::math::sqrt_price_math::{get_amount_a_delta, get_amount_b_delta};
use crate::math::tick_math::tick_to_sqrt_price;
use crate::state::{Pool, Position, TickArray};

use super::ModifyLiquidity;

pub fn handler(
    ctx: Context<ModifyLiquidity>,
    liquidity_amount: u128,
    token_min_a: u64,
    token_min_b: u64,
) -> Result<()> {
    require!(liquidity_amount > 0, TidePoolError::ZeroLiquidity);

    let position = &ctx.accounts.position;
    require!(
        position.liquidity >= liquidity_amount,
        TidePoolError::ZeroLiquidity
    );

    let pool = &ctx.accounts.pool;
    let tick_lower = position.tick_lower_index;
    let tick_upper = position.tick_upper_index;

    let sqrt_price_lower = tick_to_sqrt_price(tick_lower)?;
    let sqrt_price_upper = tick_to_sqrt_price(tick_upper)?;
    let sqrt_price_current = pool.sqrt_price;

    // Calculate token amounts to return
    let (amount_a, amount_b) = if sqrt_price_current < sqrt_price_lower {
        let a = get_amount_a_delta(sqrt_price_lower, sqrt_price_upper, liquidity_amount, false)?;
        (a, 0u64)
    } else if sqrt_price_current >= sqrt_price_upper {
        let b = get_amount_b_delta(sqrt_price_lower, sqrt_price_upper, liquidity_amount, false)?;
        (0u64, b)
    } else {
        let a = get_amount_a_delta(sqrt_price_current, sqrt_price_upper, liquidity_amount, false)?;
        let b = get_amount_b_delta(sqrt_price_lower, sqrt_price_current, liquidity_amount, false)?;
        (a, b)
    };

    // Check slippage
    require!(amount_a >= token_min_a, TidePoolError::TokenMinNotMet);
    require!(amount_b >= token_min_b, TidePoolError::TokenMinNotMet);

    // Update fee growth for position
    let tick_lower_data = ctx
        .accounts
        .tick_array_lower
        .get_tick(tick_lower, pool.tick_spacing)
        .ok_or(TidePoolError::InvalidTickArray)?;
    let tick_upper_data = ctx
        .accounts
        .tick_array_upper
        .get_tick(tick_upper, pool.tick_spacing)
        .ok_or(TidePoolError::InvalidTickArray)?;

    let fee_growth_inside_a = calculate_fee_growth_inside(
        pool.tick_current_index,
        tick_lower,
        tick_upper,
        pool.fee_growth_global_a,
        tick_lower_data.fee_growth_outside_a,
        tick_upper_data.fee_growth_outside_a,
    );
    let fee_growth_inside_b = calculate_fee_growth_inside(
        pool.tick_current_index,
        tick_lower,
        tick_upper,
        pool.fee_growth_global_b,
        tick_lower_data.fee_growth_outside_b,
        tick_upper_data.fee_growth_outside_b,
    );

    // Update position
    let position = &mut ctx.accounts.position;
    position.update_fees(fee_growth_inside_a, fee_growth_inside_b);
    position.liquidity = position
        .liquidity
        .checked_sub(liquidity_amount)
        .ok_or(TidePoolError::MathOverflow)?;

    // Update tick liquidity (reverse of add)
    let tick_array_lower = &mut ctx.accounts.tick_array_lower;
    if let Some(tick) = tick_array_lower.get_tick_mut(tick_lower, pool.tick_spacing) {
        tick.liquidity_net = tick
            .liquidity_net
            .checked_sub(liquidity_amount as i128)
            .ok_or(TidePoolError::MathOverflow)?;
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_sub(liquidity_amount)
            .ok_or(TidePoolError::MathOverflow)?;
        if tick.liquidity_gross == 0 {
            tick.initialized = false;
        }
    }

    let tick_array_upper = &mut ctx.accounts.tick_array_upper;
    if let Some(tick) = tick_array_upper.get_tick_mut(tick_upper, pool.tick_spacing) {
        tick.liquidity_net = tick
            .liquidity_net
            .checked_add(liquidity_amount as i128)
            .ok_or(TidePoolError::MathOverflow)?;
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_sub(liquidity_amount)
            .ok_or(TidePoolError::MathOverflow)?;
        if tick.liquidity_gross == 0 {
            tick.initialized = false;
        }
    }

    // Update pool liquidity
    let pool = &mut ctx.accounts.pool;
    if pool.is_price_in_range(tick_lower, tick_upper) {
        pool.liquidity = add_liquidity_delta(pool.liquidity, -(liquidity_amount as i128))?;
    }

    // Transfer tokens from pool vaults to owner (PDA signer)
    let pool_key = pool.key();
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

    if amount_a > 0 {
        let transfer_a = Transfer {
            from: ctx.accounts.token_vault_a.to_account_info(),
            to: ctx.accounts.token_owner_account_a.to_account_info(),
            authority: pool.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_a,
                signer_seeds,
            ),
            amount_a,
        )?;
    }
    if amount_b > 0 {
        let transfer_b = Transfer {
            from: ctx.accounts.token_vault_b.to_account_info(),
            to: ctx.accounts.token_owner_account_b.to_account_info(),
            authority: pool.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_b,
                signer_seeds,
            ),
            amount_b,
        )?;
    }

    emit!(LiquidityRemoved {
        pool: pool_key,
        position: ctx.accounts.position.key(),
        owner: ctx.accounts.owner.key(),
        tick_lower,
        tick_upper,
        liquidity_delta: liquidity_amount,
        amount_a,
        amount_b,
    });

    Ok(())
}
