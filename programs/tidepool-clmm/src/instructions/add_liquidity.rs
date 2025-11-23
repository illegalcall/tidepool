use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TidePoolError;
use crate::events::LiquidityAdded;
use crate::math::liquidity_math::{add_liquidity_delta, calculate_fee_growth_inside};
use crate::math::sqrt_price_math::{get_amount_a_delta, get_amount_b_delta};
use crate::math::tick_math::tick_to_sqrt_price;
use crate::state::{Pool, Position, TickArray};

#[derive(Accounts)]
pub struct ModifyLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        has_one = pool,
        constraint = position.owner == owner.key() @ TidePoolError::Unauthorized,
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        constraint = tick_array_lower.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_lower: Account<'info, TickArray>,

    #[account(
        mut,
        constraint = tick_array_upper.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_upper: Account<'info, TickArray>,

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

    #[account(
        mut,
        constraint = token_owner_account_a.owner == owner.key() @ TidePoolError::Unauthorized,
    )]
    pub token_owner_account_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_owner_account_b.owner == owner.key() @ TidePoolError::Unauthorized,
    )]
    pub token_owner_account_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(
    ctx: Context<ModifyLiquidity>,
    liquidity_amount: u128,
    token_max_a: u64,
    token_max_b: u64,
) -> Result<()> {
    require!(liquidity_amount > 0, TidePoolError::ZeroLiquidity);

    let pool = &ctx.accounts.pool;
    require!(!pool.paused, TidePoolError::PoolPaused);

    let tick_lower = ctx.accounts.position.tick_lower_index;
    let tick_upper = ctx.accounts.position.tick_upper_index;

    let sqrt_price_lower = tick_to_sqrt_price(tick_lower)?;
    let sqrt_price_upper = tick_to_sqrt_price(tick_upper)?;
    let sqrt_price_current = pool.sqrt_price;

    // Calculate required token amounts based on current price vs position range
    let (amount_a, amount_b) = if sqrt_price_current < sqrt_price_lower {
        // Current price below range: only token A needed
        let a = get_amount_a_delta(sqrt_price_lower, sqrt_price_upper, liquidity_amount, true)?;
        (a, 0u64)
    } else if sqrt_price_current >= sqrt_price_upper {
        // Current price above range: only token B needed
        let b = get_amount_b_delta(sqrt_price_lower, sqrt_price_upper, liquidity_amount, true)?;
        (0u64, b)
    } else {
        // Current price in range: both tokens needed
        let a = get_amount_a_delta(sqrt_price_current, sqrt_price_upper, liquidity_amount, true)?;
        let b = get_amount_b_delta(sqrt_price_lower, sqrt_price_current, liquidity_amount, true)?;
        (a, b)
    };

    // Check slippage
    require!(amount_a <= token_max_a, TidePoolError::TokenMaxExceeded);
    require!(amount_b <= token_max_b, TidePoolError::TokenMaxExceeded);

    // Update fee growth for position before modifying liquidity
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

    // Update position fees
    let position = &mut ctx.accounts.position;
    position.update_fees(fee_growth_inside_a, fee_growth_inside_b);
    position.liquidity = position
        .liquidity
        .checked_add(liquidity_amount)
        .ok_or(TidePoolError::MathOverflow)?;

    // Update tick liquidity
    let tick_array_lower = &mut ctx.accounts.tick_array_lower;
    if let Some(tick) = tick_array_lower.get_tick_mut(tick_lower, pool.tick_spacing) {
        tick.liquidity_net = tick
            .liquidity_net
            .checked_add(liquidity_amount as i128)
            .ok_or(TidePoolError::MathOverflow)?;
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_add(liquidity_amount)
            .ok_or(TidePoolError::MathOverflow)?;
        if !tick.initialized {
            tick.initialized = true;
            // Initialize fee growth outside for the tick
            if pool.tick_current_index >= tick_lower {
                tick.fee_growth_outside_a = pool.fee_growth_global_a;
                tick.fee_growth_outside_b = pool.fee_growth_global_b;
            }
        }
    }

    let tick_array_upper = &mut ctx.accounts.tick_array_upper;
    if let Some(tick) = tick_array_upper.get_tick_mut(tick_upper, pool.tick_spacing) {
        tick.liquidity_net = tick
            .liquidity_net
            .checked_sub(liquidity_amount as i128)
            .ok_or(TidePoolError::MathOverflow)?;
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_add(liquidity_amount)
            .ok_or(TidePoolError::MathOverflow)?;
        if !tick.initialized {
            tick.initialized = true;
            if pool.tick_current_index >= tick_upper {
                tick.fee_growth_outside_a = pool.fee_growth_global_a;
                tick.fee_growth_outside_b = pool.fee_growth_global_b;
            }
        }
    }

    // Update pool liquidity if position is in range
    let pool = &mut ctx.accounts.pool;
    if pool.is_price_in_range(tick_lower, tick_upper) {
        pool.liquidity = add_liquidity_delta(pool.liquidity, liquidity_amount as i128)?;
    }
    pool.total_liquidity_provided = pool
        .total_liquidity_provided
        .checked_add(liquidity_amount)
        .ok_or(TidePoolError::MathOverflow)?;

    // Transfer tokens from owner to pool vaults
    if amount_a > 0 {
        let transfer_a = Transfer {
            from: ctx.accounts.token_owner_account_a.to_account_info(),
            to: ctx.accounts.token_vault_a.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), transfer_a),
            amount_a,
        )?;
    }
    if amount_b > 0 {
        let transfer_b = Transfer {
            from: ctx.accounts.token_owner_account_b.to_account_info(),
            to: ctx.accounts.token_vault_b.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), transfer_b),
            amount_b,
        )?;
    }

    emit!(LiquidityAdded {
        pool: pool.key(),
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
