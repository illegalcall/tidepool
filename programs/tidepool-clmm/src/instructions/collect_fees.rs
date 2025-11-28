use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TidePoolError;
use crate::events::FeesCollected;
use crate::math::liquidity_math::calculate_fee_growth_inside;
use crate::state::{Pool, Position, TickArray};

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        has_one = pool,
        constraint = position.owner == owner.key() @ TidePoolError::Unauthorized,
    )]
    pub position: Account<'info, Position>,

    #[account(
        constraint = tick_array_lower.pool == pool.key() @ TidePoolError::InvalidTickArray,
    )]
    pub tick_array_lower: Account<'info, TickArray>,

    #[account(
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

    #[account(mut)]
    pub token_owner_account_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_owner_account_b: Account<'info, TokenAccount>,

    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<CollectFees>) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let position = &ctx.accounts.position;
    let tick_lower = position.tick_lower_index;
    let tick_upper = position.tick_upper_index;

    // Calculate current fee growth inside
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

    // Update position fee tracking
    let position = &mut ctx.accounts.position;
    position.update_fees(fee_growth_inside_a, fee_growth_inside_b);

    let fees_a = position.fees_owed_a;
    let fees_b = position.fees_owed_b;

    require!(
        fees_a > 0 || fees_b > 0,
        TidePoolError::NoFeesToCollect
    );

    // Reset owed fees
    position.fees_owed_a = 0;
    position.fees_owed_b = 0;

    // Transfer fees from pool vaults to owner
    let pool = &ctx.accounts.pool;
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

    if fees_a > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault_a.to_account_info(),
                    to: ctx.accounts.token_owner_account_a.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            fees_a,
        )?;
    }

    if fees_b > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault_b.to_account_info(),
                    to: ctx.accounts.token_owner_account_b.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            fees_b,
        )?;
    }

    emit!(FeesCollected {
        pool: pool.key(),
        position: ctx.accounts.position.key(),
        owner: ctx.accounts.owner.key(),
        amount_a: fees_a,
        amount_b: fees_b,
    });

    Ok(())
}
