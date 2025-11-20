use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub position_mint: Pubkey,

    /// Lower tick boundary of the position
    pub tick_lower_index: i32,
    /// Upper tick boundary of the position
    pub tick_upper_index: i32,
    /// Amount of liquidity owned by this position
    pub liquidity: u128,

    /// Fee growth inside the position's range at last update (token A), Q64.64
    pub fee_growth_inside_last_a: u128,
    /// Fee growth inside the position's range at last update (token B), Q64.64
    pub fee_growth_inside_last_b: u128,
    /// Uncollected fees owed to this position (token A)
    pub fees_owed_a: u64,
    /// Uncollected fees owed to this position (token B)
    pub fees_owed_b: u64,

    pub bump: u8,
}

impl Position {
    pub fn update_fees(
        &mut self,
        fee_growth_inside_a: u128,
        fee_growth_inside_b: u128,
    ) {
        if self.liquidity > 0 {
            let delta_a = fee_growth_inside_a
                .wrapping_sub(self.fee_growth_inside_last_a);
            let delta_b = fee_growth_inside_b
                .wrapping_sub(self.fee_growth_inside_last_b);

            // fees = liquidity * fee_growth_delta / 2^64
            let fees_a = (delta_a as u128)
                .checked_mul(self.liquidity)
                .map(|v| (v >> 64) as u64)
                .unwrap_or(0);
            let fees_b = (delta_b as u128)
                .checked_mul(self.liquidity)
                .map(|v| (v >> 64) as u64)
                .unwrap_or(0);

            self.fees_owed_a = self.fees_owed_a.saturating_add(fees_a);
            self.fees_owed_b = self.fees_owed_b.saturating_add(fees_b);
        }

        self.fee_growth_inside_last_a = fee_growth_inside_a;
        self.fee_growth_inside_last_b = fee_growth_inside_b;
    }

    pub fn is_empty(&self) -> bool {
        self.liquidity == 0 && self.fees_owed_a == 0 && self.fees_owed_b == 0
    }
}
