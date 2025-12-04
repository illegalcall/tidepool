use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::VaultError;
use crate::events::VaultInitialized;
use crate::state::Vault;

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", pool.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,

    /// The CLMM pool this vault manages liquidity for.
    /// Validated by deserializing as a Pool account owned by the CLMM program.
    #[account(
        owner = tidepool_clmm::ID,
    )]
    pub pool: Account<'info, tidepool_clmm::state::Pool>,

    /// Mint for vault share tokens
    #[account(
        init,
        payer = authority,
        mint::decimals = 6,
        mint::authority = vault,
        seeds = [b"share_mint", vault.key().as_ref()],
        bump,
    )]
    pub share_mint: Account<'info, Mint>,

    /// Vault's token A holdings
    #[account(
        init,
        payer = authority,
        token::mint = token_mint_a,
        token::authority = vault,
        seeds = [b"vault_token_a", vault.key().as_ref()],
        bump,
    )]
    pub token_vault_a: Account<'info, TokenAccount>,

    /// Vault's token B holdings
    #[account(
        init,
        payer = authority,
        token::mint = token_mint_b,
        token::authority = vault,
        seeds = [b"vault_token_b", vault.key().as_ref()],
        bump,
    )]
    pub token_vault_b: Account<'info, TokenAccount>,

    pub token_mint_a: Account<'info, Mint>,
    pub token_mint_b: Account<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<InitializeVault>,
    rebalance_threshold_bps: u16,
    tick_range_multiplier: u8,
    performance_fee_bps: u16,
    management_fee_bps: u16,
) -> Result<()> {
    require!(
        performance_fee_bps <= 3000, // Max 30% performance fee
        VaultError::InvalidFeeConfig
    );
    require!(
        management_fee_bps <= 500, // Max 5% management fee
        VaultError::InvalidFeeConfig
    );
    require!(
        tick_range_multiplier > 0 && tick_range_multiplier <= 100,
        VaultError::InvalidFeeConfig
    );

    let vault = &mut ctx.accounts.vault;
    vault.authority = ctx.accounts.authority.key();
    vault.keeper = ctx.accounts.authority.key(); // Initially, authority is also keeper
    vault.pool = ctx.accounts.pool.key();
    vault.share_mint = ctx.accounts.share_mint.key();
    vault.token_vault_a = ctx.accounts.token_vault_a.key();
    vault.token_vault_b = ctx.accounts.token_vault_b.key();
    vault.active_position = Pubkey::default();
    vault.active_tick_lower = 0;
    vault.active_tick_upper = 0;
    vault.has_active_position = false;
    vault.rebalance_threshold_bps = rebalance_threshold_bps;
    vault.tick_range_multiplier = tick_range_multiplier;
    vault.max_slippage_bps = 100; // 1% default max slippage
    vault.total_shares = 0;
    vault.total_value_a = 0;
    vault.total_value_b = 0;
    vault.total_fees_earned_a = 0;
    vault.total_fees_earned_b = 0;
    vault.last_rebalance_slot = Clock::get()?.slot;
    vault.last_compound_slot = Clock::get()?.slot;
    vault.rebalance_count = 0;
    vault.performance_fee_bps = performance_fee_bps;
    vault.management_fee_bps = management_fee_bps;
    vault.paused = false;
    vault.bump = ctx.bumps.vault;

    emit!(VaultInitialized {
        vault: vault.key(),
        pool: vault.pool,
        authority: vault.authority,
        keeper: vault.keeper,
        rebalance_threshold_bps,
        tick_range_multiplier,
    });

    Ok(())
}
