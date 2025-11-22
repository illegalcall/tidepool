use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::TidePoolError;
use crate::state::tick::{check_tick_alignment, check_tick_bounds};
use crate::state::{Pool, Position};

#[derive(Accounts)]
#[instruction(tick_lower_index: i32, tick_upper_index: i32)]
pub struct OpenPosition<'info> {
    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [
            b"position",
            pool.key().as_ref(),
            position_mint.key().as_ref(),
        ],
        bump,
    )]
    pub position: Account<'info, Position>,

    pub pool: Account<'info, Pool>,

    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = position,
    )]
    pub position_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = owner,
        token::mint = position_mint,
        token::authority = owner,
    )]
    pub position_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<OpenPosition>,
    tick_lower_index: i32,
    tick_upper_index: i32,
) -> Result<()> {
    let pool = &ctx.accounts.pool;

    // Validate tick range
    require!(
        tick_lower_index < tick_upper_index,
        TidePoolError::InvalidTickRange
    );
    require!(
        check_tick_bounds(tick_lower_index) && check_tick_bounds(tick_upper_index),
        TidePoolError::TickOutOfRange
    );
    require!(
        check_tick_alignment(tick_lower_index, pool.tick_spacing)
            && check_tick_alignment(tick_upper_index, pool.tick_spacing),
        TidePoolError::TickNotAligned
    );

    let position = &mut ctx.accounts.position;
    position.pool = pool.key();
    position.owner = ctx.accounts.owner.key();
    position.position_mint = ctx.accounts.position_mint.key();
    position.tick_lower_index = tick_lower_index;
    position.tick_upper_index = tick_upper_index;
    position.liquidity = 0;
    position.fee_growth_inside_last_a = 0;
    position.fee_growth_inside_last_b = 0;
    position.fees_owed_a = 0;
    position.fees_owed_b = 0;
    position.bump = ctx.bumps.position;

    // Mint position NFT to owner
    let cpi_accounts = anchor_spl::token::MintTo {
        mint: ctx.accounts.position_mint.to_account_info(),
        to: ctx.accounts.position_token_account.to_account_info(),
        authority: position.to_account_info(),
    };
    let pool_key = pool.key();
    let mint_key = ctx.accounts.position_mint.key();
    let seeds = &[
        b"position".as_ref(),
        pool_key.as_ref(),
        mint_key.as_ref(),
        &[position.bump],
    ];
    let signer_seeds = &[&seeds[..]];
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    anchor_spl::token::mint_to(cpi_ctx, 1)?;

    msg!(
        "Position opened: ticks=[{}, {}], pool={}",
        tick_lower_index,
        tick_upper_index,
        pool_key
    );

    Ok(())
}
