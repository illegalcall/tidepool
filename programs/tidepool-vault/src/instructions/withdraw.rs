use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::errors::VaultError;
use crate::events::VaultWithdrawn;
use crate::state::Vault;

#[derive(Accounts)]
pub struct VaultWithdraw<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.pool.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        constraint = share_mint.key() == vault.share_mint @ VaultError::UnauthorizedAuthority,
    )]
    pub share_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = token_vault_a.key() == vault.token_vault_a @ VaultError::UnauthorizedAuthority,
    )]
    pub token_vault_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_vault_b.key() == vault.token_vault_b @ VaultError::UnauthorizedAuthority,
    )]
    pub token_vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_share_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub withdrawer: Signer<'info>,

    pub token_program: Program<'info, Token>,
}


pub fn handler(ctx: Context<VaultWithdraw>, shares_to_burn: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    require!(shares_to_burn > 0, VaultError::InsufficientShares);
    require!(vault.total_shares > 0, VaultError::NoShares);
    require!(
        shares_to_burn <= ctx.accounts.user_share_account.amount,
        VaultError::InsufficientShares
    );

    // Calculate proportional token amounts
    let (amount_a, amount_b) = vault.calculate_withdrawal_amounts(shares_to_burn);

    // Burn share tokens
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.withdrawer.to_account_info(),
            },
        ),
        shares_to_burn,
    )?;

    // Transfer tokens from vault to user
    let pool_key = vault.pool;
    let bump = vault.bump;
    let seeds = &[b"vault".as_ref(), pool_key.as_ref(), &[bump]];
    let signer_seeds = &[&seeds[..]];

    if amount_a > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault_a.to_account_info(),
                    to: ctx.accounts.user_token_a.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            amount_a,
        )?;
    }

    if amount_b > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault_b.to_account_info(),
                    to: ctx.accounts.user_token_b.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            amount_b,
        )?;
    }

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares_to_burn)
        .ok_or(VaultError::MathOverflow)?;
    vault.total_value_a = vault
        .total_value_a
        .checked_sub(amount_a)
        .ok_or(VaultError::MathOverflow)?;
    vault.total_value_b = vault
        .total_value_b
        .checked_sub(amount_b)
        .ok_or(VaultError::MathOverflow)?;

    emit!(VaultWithdrawn {
        vault: vault.key(),
        user: ctx.accounts.withdrawer.key(),
        shares_burned: shares_to_burn,
        amount_a,
        amount_b,
    });

    Ok(())
}

