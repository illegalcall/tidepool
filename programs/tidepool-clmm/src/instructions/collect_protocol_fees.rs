use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TidePoolError;
use crate::events::ProtocolFeesCollected;
use crate::state::Pool;

#[derive(Accounts)]
pub struct CollectProtocolFees<'info> {
    #[account(
        mut,
        constraint = pool.authority == authority.key() @ TidePoolError::Unauthorized,
    )]
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
    pub destination_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub destination_b: Account<'info, TokenAccount>,

    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<CollectProtocolFees>) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let fees_a = pool.protocol_fees_owed_a;
    let fees_b = pool.protocol_fees_owed_b;

    require!(
        fees_a > 0 || fees_b > 0,
        TidePoolError::NoFeesToCollect
    );

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
                    to: ctx.accounts.destination_a.to_account_info(),
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
                    to: ctx.accounts.destination_b.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            fees_b,
        )?;
    }

    let pool = &mut ctx.accounts.pool;
    pool.protocol_fees_owed_a = 0;
    pool.protocol_fees_owed_b = 0;

    emit!(ProtocolFeesCollected {
        pool: pool.key(),
        amount_a: fees_a,
        amount_b: fees_b,
    });

    Ok(())
}
