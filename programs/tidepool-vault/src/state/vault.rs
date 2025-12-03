use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
    pub keeper: Pubkey,
    pub pool: Pubkey,
    pub share_mint: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_vault_b: Pubkey,

    /// Currently active CLMM position
    pub active_position: Pubkey,
    pub active_tick_lower: i32,
    pub active_tick_upper: i32,
    pub has_active_position: bool,

    /// Strategy configuration
    pub rebalance_threshold_bps: u16,
    pub tick_range_multiplier: u8,
    pub max_slippage_bps: u16,

    /// Accounting
    pub total_shares: u64,
    pub total_value_a: u64,
    pub total_value_b: u64,
    pub total_fees_earned_a: u64,
    pub total_fees_earned_b: u64,

    /// Operational tracking
    pub last_rebalance_slot: u64,
    pub last_compound_slot: u64,
    pub rebalance_count: u32,

    /// Fee configuration
    pub performance_fee_bps: u16,
    pub management_fee_bps: u16,

    pub paused: bool,
    pub bump: u8,
}

impl Vault {
    pub fn calculate_shares_to_mint(
        &self,
        amount_a: u64,
        amount_b: u64,
    ) -> u64 {
        if self.total_shares == 0 {
            // First deposit: shares = sqrt(amount_a * amount_b) for balanced representation
            let product = (amount_a as u128) * (amount_b as u128);
            if product == 0 {
                return amount_a.max(amount_b);
            }
            integer_sqrt(product) as u64
        } else {
            // Pro-rata shares based on existing vault composition
            let share_a = if self.total_value_a > 0 {
                (amount_a as u128) * (self.total_shares as u128) / (self.total_value_a as u128)
            } else {
                0
            };
            let share_b = if self.total_value_b > 0 {
                (amount_b as u128) * (self.total_shares as u128) / (self.total_value_b as u128)
            } else {
                0
            };
            // Take the minimum to prevent dilution
            share_a.min(share_b) as u64
        }
    }

    pub fn calculate_withdrawal_amounts(
        &self,
        shares: u64,
    ) -> (u64, u64) {
        if self.total_shares == 0 {
            return (0, 0);
        }
        let amount_a = (shares as u128) * (self.total_value_a as u128) / (self.total_shares as u128);
        let amount_b = (shares as u128) * (self.total_value_b as u128) / (self.total_shares as u128);
        (amount_a as u64, amount_b as u64)
    }
}

fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vault(total_shares: u64, total_a: u64, total_b: u64) -> Vault {
        Vault {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            share_mint: Pubkey::default(),
            token_vault_a: Pubkey::default(),
            token_vault_b: Pubkey::default(),
            active_position: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 0,
            has_active_position: false,
            rebalance_threshold_bps: 1000,
            tick_range_multiplier: 10,
            max_slippage_bps: 100,
            total_shares,
            total_value_a: total_a,
            total_value_b: total_b,
            total_fees_earned_a: 0,
            total_fees_earned_b: 0,
            last_rebalance_slot: 0,
            last_compound_slot: 0,
            rebalance_count: 0,
            performance_fee_bps: 1000,
            management_fee_bps: 200,
            paused: false,
            bump: 0,
        }
    }

    #[test]
    fn test_integer_sqrt() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(1), 1);
        assert_eq!(integer_sqrt(4), 2);
        assert_eq!(integer_sqrt(9), 3);
        assert_eq!(integer_sqrt(100), 10);
        assert_eq!(integer_sqrt(2), 1); // floor
    }

    #[test]
    fn test_first_deposit_uses_sqrt() {
        let vault = make_vault(0, 0, 0);
        // sqrt(1000 * 4000) = sqrt(4_000_000) = 2000
        let shares = vault.calculate_shares_to_mint(1000, 4000);
        assert_eq!(shares, 2000);
    }

    #[test]
    fn test_first_deposit_single_token() {
        let vault = make_vault(0, 0, 0);
        // only token A deposited, product is 0 -> fallback to max
        let shares = vault.calculate_shares_to_mint(500, 0);
        assert_eq!(shares, 500);
    }

    #[test]
    fn test_subsequent_deposit_proportional() {
        let vault = make_vault(1000, 500, 500);
        // deposit 100 of each: share_a = 100*1000/500 = 200, share_b = 200
        // min(200, 200) = 200
        let shares = vault.calculate_shares_to_mint(100, 100);
        assert_eq!(shares, 200);
    }

    #[test]
    fn test_subsequent_deposit_imbalanced() {
        let vault = make_vault(1000, 500, 500);
        // deposit 200 A and 50 B -> share_a=400, share_b=100 -> min=100
        let shares = vault.calculate_shares_to_mint(200, 50);
        assert_eq!(shares, 100);
    }

    #[test]
    fn test_withdrawal_proportional() {
        let vault = make_vault(1000, 500, 800);
        let (a, b) = vault.calculate_withdrawal_amounts(100);
        // 100/1000 * 500 = 50, 100/1000 * 800 = 80
        assert_eq!(a, 50);
        assert_eq!(b, 80);
    }

    #[test]
    fn test_withdrawal_all_shares() {
        let vault = make_vault(1000, 500, 800);
        let (a, b) = vault.calculate_withdrawal_amounts(1000);
        assert_eq!(a, 500);
        assert_eq!(b, 800);
    }

    #[test]
    fn test_withdrawal_zero_shares() {
        let vault = make_vault(0, 0, 0);
        let (a, b) = vault.calculate_withdrawal_amounts(100);
        assert_eq!(a, 0);
        assert_eq!(b, 0);
    }
}
