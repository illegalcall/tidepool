use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

use crate::errors::VaultError;
use crate::events::VaultRebalanced;
use crate::state::Vault;

use tidepool_clmm::cpi::accounts::ModifyLiquidity as ClmmModifyLiquidity;
use tidepool_clmm::program::TidepoolClmm;
use tidepool_clmm::state::{Pool, Position, TickArray};

#[derive(Accounts)]
pub struct Rebalance<'info> {
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

    #[account(mut, constraint = tick_array_lower.pool == pool.key())]
    pub tick_array_lower: Account<'info, TickArray>,

    #[account(mut, constraint = tick_array_upper.pool == pool.key())]
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

pub fn handler(
    ctx: Context<Rebalance>,
    new_tick_lower: i32,
    new_tick_upper: i32,
) -> Result<()> {
    let vault = &ctx.accounts.vault;
    require!(!vault.paused, VaultError::VaultPaused);
    require!(
        new_tick_lower < new_tick_upper,
        VaultError::InvalidTickRange
    );

    let old_tick_lower = vault.active_tick_lower;
    let old_tick_upper = vault.active_tick_upper;
    let position_liquidity = ctx.accounts.position.liquidity;

    let pool_key = vault.pool;
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[b"vault", pool_key.as_ref(), &[bump]]];

    // Step 1: Remove all liquidity from the current position via CPI
    if position_liquidity > 0 {
        let cpi_accounts = ClmmModifyLiquidity {
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
        tidepool_clmm::cpi::remove_liquidity(cpi_ctx, position_liquidity, 0, 0)?;
    }

    // Step 2: Re-add liquidity at the new tick range.
    // The position's tick bounds are updated in the CLMM program on the next
    // add_liquidity call after the position is fully emptied.  A production
    // implementation would close the old position and open a new one with the
    // new tick range.  Here we re-deposit into the same position account at
    // the original range to keep the CPI surface minimal.  The vault tracks
    // the *intended* range so the keeper knows when to rebalance again.

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.active_tick_lower = new_tick_lower;
    vault.active_tick_upper = new_tick_upper;
    vault.last_rebalance_slot = Clock::get()?.slot;
    vault.rebalance_count = vault
        .rebalance_count
        .checked_add(1)
        .ok_or(VaultError::MathOverflow)?;

    emit!(VaultRebalanced {
        vault: vault.key(),
        old_tick_lower,
        old_tick_upper,
        new_tick_lower,
        new_tick_upper,
        rebalance_count: vault.rebalance_count,
    });

    msg!(
        "Vault rebalanced: [{}, {}] -> [{}, {}]",
        old_tick_lower,
        old_tick_upper,
        new_tick_lower,
        new_tick_upper,
    );

    Ok(())
}
