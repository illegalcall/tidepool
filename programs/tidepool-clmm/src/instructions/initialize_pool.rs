use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::TidePoolError;
use crate::events::PoolInitialized;
use crate::math::tick_math::{sqrt_price_to_tick, MAX_SQRT_PRICE, MIN_SQRT_PRICE};
use crate::state::Pool;

#[derive(Accounts)]
#[instruction(tick_spacing: u16)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Pool::INIT_SPACE,
        seeds = [
            b"pool",
            token_mint_a.key().as_ref(),
            token_mint_b.key().as_ref(),
            &tick_spacing.to_le_bytes(),
        ],
        bump,
    )]
    pub pool: Account<'info, Pool>,

    pub token_mint_a: Account<'info, Mint>,
    pub token_mint_b: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = token_mint_a,
        token::authority = pool,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump,
    )]
    pub token_vault_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        token::mint = token_mint_b,
        token::authority = pool,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump,
    )]
    pub token_vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<InitializePool>,
    tick_spacing: u16,
    initial_sqrt_price: u128,
    fee_rate: u16,
) -> Result<()> {
    require!(
        tick_spacing > 0 && tick_spacing <= 200,
        TidePoolError::InvalidTickSpacing
    );
    require!(
        initial_sqrt_price >= MIN_SQRT_PRICE && initial_sqrt_price <= MAX_SQRT_PRICE,
        TidePoolError::SqrtPriceOutOfBounds
    );
    require!(
        fee_rate > 0 && fee_rate <= 10_000,
        TidePoolError::InvalidFeeRate
    );

    let initial_tick = sqrt_price_to_tick(initial_sqrt_price)?;

    let pool = &mut ctx.accounts.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.token_mint_a = ctx.accounts.token_mint_a.key();
    pool.token_mint_b = ctx.accounts.token_mint_b.key();
    pool.token_vault_a = ctx.accounts.token_vault_a.key();
    pool.token_vault_b = ctx.accounts.token_vault_b.key();
    pool.sqrt_price = initial_sqrt_price;
    pool.tick_current_index = initial_tick;
    pool.tick_spacing = tick_spacing;
    pool.liquidity = 0;
    pool.fee_rate = fee_rate;
    pool.protocol_fee_rate = 1000; // 10% of fees go to protocol
    pool.fee_growth_global_a = 0;
    pool.fee_growth_global_b = 0;
    pool.protocol_fees_owed_a = 0;
    pool.protocol_fees_owed_b = 0;
    pool.total_liquidity_provided = 0;
    pool.num_positions = 0;
    pool.paused = false;
    pool.bump = ctx.bumps.pool;

    emit!(PoolInitialized {
        pool: pool.key(),
        token_mint_a: pool.token_mint_a,
        token_mint_b: pool.token_mint_b,
        tick_spacing,
        initial_sqrt_price,
        fee_rate,
    });

    msg!(
        "Pool initialized: tick={}, sqrt_price={}",
        initial_tick,
        initial_sqrt_price
    );

    Ok(())
}
