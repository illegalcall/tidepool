use anchor_lang::prelude::*;

#[event]
pub struct VaultInitialized {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub keeper: Pubkey,
    pub rebalance_threshold_bps: u16,
    pub tick_range_multiplier: u8,
}

#[event]
pub struct VaultDeposited {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
    pub shares_minted: u64,
    pub share_price: u64,
}

#[event]
pub struct VaultWithdrawn {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub shares_burned: u64,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct VaultRebalanced {
    pub vault: Pubkey,
    pub old_tick_lower: i32,
    pub old_tick_upper: i32,
    pub new_tick_lower: i32,
    pub new_tick_upper: i32,
    pub rebalance_count: u32,
}

#[event]
pub struct VaultCompounded {
    pub vault: Pubkey,
    pub fees_a: u64,
    pub fees_b: u64,
    pub performance_fee_a: u64,
    pub performance_fee_b: u64,
}
