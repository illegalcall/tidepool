use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserReceipt {
    pub vault: Pubkey,
    pub owner: Pubkey,
    pub shares: u64,
    pub deposited_a: u64,
    pub deposited_b: u64,
    pub deposit_timestamp: i64,
    pub bump: u8,
}
