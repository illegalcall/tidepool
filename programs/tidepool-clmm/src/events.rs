use anchor_lang::prelude::*;

#[event]
pub struct PoolInitialized {
    pub pool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub tick_spacing: u16,
    pub initial_sqrt_price: u128,
    pub fee_rate: u16,
}

#[event]
pub struct LiquidityAdded {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity_delta: u128,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity_delta: u128,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct SwapExecuted {
    pub pool: Pubkey,
    pub trader: Pubkey,
    pub a_to_b: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub sqrt_price_after: u128,
    pub tick_after: i32,
}

#[event]
pub struct FeesCollected {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct ProtocolFeesCollected {
    pub pool: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
}
