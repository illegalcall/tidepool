use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("TVAULTxVRzBf7JqX77GEQpKTkNLBcEigvbUP5SNBhMn");

#[program]
pub mod tidepool_vault {
    use super::*;

    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        rebalance_threshold_bps: u16,
        tick_range_multiplier: u8,
        performance_fee_bps: u16,
        management_fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize_vault::handler(
            ctx,
            rebalance_threshold_bps,
            tick_range_multiplier,
            performance_fee_bps,
            management_fee_bps,
        )
    }

    pub fn deposit(ctx: Context<VaultDeposit>, max_amount_a: u64, max_amount_b: u64) -> Result<()> {
        instructions::deposit::handler(ctx, max_amount_a, max_amount_b)
    }

    pub fn withdraw(ctx: Context<VaultWithdraw>, shares_to_burn: u64) -> Result<()> {
        instructions::withdraw::handler(ctx, shares_to_burn)
    }

    pub fn rebalance(ctx: Context<Rebalance>, new_tick_lower: i32, new_tick_upper: i32) -> Result<()> {
        instructions::rebalance::handler(ctx, new_tick_lower, new_tick_upper)
    }

    pub fn compound(ctx: Context<Compound>) -> Result<()> {
        instructions::compound::handler(ctx)
    }
}
