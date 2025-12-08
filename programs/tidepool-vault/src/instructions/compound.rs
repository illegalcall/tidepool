use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

use crate::errors::VaultError;
use crate::events::VaultCompounded;
use crate::state::Vault;

use tidepool_clmm::cpi::accounts::CollectFees as ClmmCollectFees;
use tidepool_clmm::program::TidepoolClmm;
use tidepool_clmm::state::{Pool, Position, TickArray};

#[derive(Accounts)]
pub struct Compound<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.pool.as_ref()],
        bump = vault.bump,
        constraint = vault.keeper == keeper.key() @ VaultError::UnauthorizedKeeper,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        constraint = pool.key() == vault.pool @ VaultError::UnauthorizedAuthority,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        constraint = position.pool == pool.key(),
    )]
    pub position: Account<'info, Position>,

    #[account(constraint = tick_array_lower.pool == pool.key())]
    pub tick_array_lower: Account<'info, TickArray>,

    #[account(constraint = tick_array_upper.pool == pool.key())]
    pub tick_array_upper: Account<'info, TickArray>,

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

    #[account(mut, constraint = pool_vault_a.key() == pool.token_vault_a)]
    pub pool_vault_a: Account<'info, TokenAccount>,

    #[account(mut, constraint = pool_vault_b.key() == pool.token_vault_b)]
    pub pool_vault_b: Account<'info, TokenAccount>,

    pub keeper: Signer<'info>,
    pub clmm_program: Program<'info, TidepoolClmm>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Compound>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    require!(!vault.paused, VaultError::VaultPaused);
    require!(vault.has_active_position, VaultError::NoActivePosition);

    // Snapshot vault token balances before fee collection
    let balance_a_before = ctx.accounts.token_vault_a.amount;
    let balance_b_before = ctx.accounts.token_vault_b.amount;

    let pool_key = vault.pool;
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[b"vault", pool_key.as_ref(), &[bump]]];

    // CPI: collect accumulated trading fees from the CLMM position
    let cpi_accounts = ClmmCollectFees {
        pool: ctx.accounts.pool.to_account_info(),
        position: ctx.accounts.position.to_account_info(),
        tick_array_lower: ctx.accounts.tick_array_lower.to_account_info(),
        tick_array_upper: ctx.accounts.tick_array_upper.to_account_info(),
        token_vault_a: ctx.accounts.pool_vault_a.to_account_info(),
        token_vault_b: ctx.accounts.pool_vault_b.to_account_info(),
        token_owner_account_a: ctx.accounts.token_vault_a.to_account_info(),
        token_owner_account_b: ctx.accounts.token_vault_b.to_account_info(),
        owner: ctx.accounts.vault.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.clmm_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    tidepool_clmm::cpi::collect_fees(cpi_ctx)?;

    // Reload token accounts to get updated balances
    ctx.accounts.token_vault_a.reload()?;
    ctx.accounts.token_vault_b.reload()?;

    let collected_fees_a = ctx
        .accounts
        .token_vault_a
        .amount
        .saturating_sub(balance_a_before);
    let collected_fees_b = ctx
        .accounts
        .token_vault_b
        .amount
        .saturating_sub(balance_b_before);

    // Deduct performance fee
    let performance_fee_a = (collected_fees_a as u128)
        .checked_mul(vault.performance_fee_bps as u128)
        .unwrap_or(0)
        / 10_000;
    let performance_fee_b = (collected_fees_b as u128)
        .checked_mul(vault.performance_fee_bps as u128)
        .unwrap_or(0)
        / 10_000;

    let compound_a = collected_fees_a.saturating_sub(performance_fee_a as u64);
    let compound_b = collected_fees_b.saturating_sub(performance_fee_b as u64);

    // Update vault accounting
    let vault = &mut ctx.accounts.vault;
    vault.total_fees_earned_a = vault
        .total_fees_earned_a
        .saturating_add(collected_fees_a);
    vault.total_fees_earned_b = vault
        .total_fees_earned_b
        .saturating_add(collected_fees_b);
    vault.total_value_a = vault.total_value_a.saturating_add(compound_a);
    vault.total_value_b = vault.total_value_b.saturating_add(compound_b);
    vault.last_compound_slot = Clock::get()?.slot;

    emit!(VaultCompounded {
        vault: vault.key(),
        fees_a: collected_fees_a,
        fees_b: collected_fees_b,
        performance_fee_a: performance_fee_a as u64,
        performance_fee_b: performance_fee_b as u64,
    });

    Ok(())
}
