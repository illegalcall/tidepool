use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};

use crate::errors::VaultError;
use crate::events::VaultDeposited;
use crate::state::Vault;

#[derive(Accounts)]
pub struct VaultDeposit<'info> {
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

    /// User's share token account — receives minted shares
    #[account(mut)]
    pub user_share_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(
    ctx: Context<VaultDeposit>,
    max_amount_a: u64,
    max_amount_b: u64,
) -> Result<()> {
    require!(
        max_amount_a > 0 || max_amount_b > 0,
        VaultError::ZeroDeposit
    );

    let vault = &ctx.accounts.vault;
    require!(!vault.paused, VaultError::VaultPaused);

    // Calculate shares to mint
    let shares_to_mint = vault.calculate_shares_to_mint(max_amount_a, max_amount_b);
    require!(shares_to_mint > 0, VaultError::ZeroDeposit);

    // Transfer tokens from user to vault
    if max_amount_a > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_a.to_account_info(),
                    to: ctx.accounts.token_vault_a.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            max_amount_a,
        )?;
    }

    if max_amount_b > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_b.to_account_info(),
                    to: ctx.accounts.token_vault_b.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            max_amount_b,
        )?;
    }

    // Mint share tokens to user
    let pool_key = vault.pool;
    let bump = vault.bump;
    let seeds = &[b"vault".as_ref(), pool_key.as_ref(), &[bump]];
    let signer_seeds = &[&seeds[..]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        shares_to_mint,
    )?;

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares_to_mint)
        .ok_or(VaultError::MathOverflow)?;
    vault.total_value_a = vault
        .total_value_a
        .checked_add(max_amount_a)
        .ok_or(VaultError::MathOverflow)?;
    vault.total_value_b = vault
        .total_value_b
        .checked_add(max_amount_b)
        .ok_or(VaultError::MathOverflow)?;

    let share_price = if vault.total_shares > 0 {
        ((vault.total_value_a as u128 + vault.total_value_b as u128) * 1_000_000
            / vault.total_shares as u128) as u64
    } else {
        1_000_000
    };

    emit!(VaultDeposited {
        vault: vault.key(),
        user: ctx.accounts.depositor.key(),
        amount_a: max_amount_a,
        amount_b: max_amount_b,
        shares_minted: shares_to_mint,
        share_price,
    });

    Ok(())
}
