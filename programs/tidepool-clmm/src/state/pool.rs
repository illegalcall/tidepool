use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub authority: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_vault_b: Pubkey,

    /// Current sqrt(price) in Q64.64 format
    pub sqrt_price: u128,
    /// Current tick index derived from sqrt_price
    pub tick_current_index: i32,
    /// Minimum tick spacing for positions
    pub tick_spacing: u16,

    /// Currently in-range liquidity
    pub liquidity: u128,

    /// Fee rate in hundredths of a basis point (e.g., 3000 = 30bps = 0.3%)
    pub fee_rate: u16,
    /// Protocol's share of fees in basis points (e.g., 1000 = 10%)
    pub protocol_fee_rate: u16,

    /// Global cumulative fee growth per unit of liquidity (token A), Q64.64
    pub fee_growth_global_a: u128,
    /// Global cumulative fee growth per unit of liquidity (token B), Q64.64
    pub fee_growth_global_b: u128,

    /// Accumulated protocol fees available for collection
    pub protocol_fees_owed_a: u64,
    pub protocol_fees_owed_b: u64,

    pub total_liquidity_provided: u128,
    pub num_positions: u32,
    pub paused: bool,
    pub bump: u8,
}

impl Pool {
    pub fn is_price_in_range(&self, tick_lower: i32, tick_upper: i32) -> bool {
        self.tick_current_index >= tick_lower && self.tick_current_index < tick_upper
    }
}
