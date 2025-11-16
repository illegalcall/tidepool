use crate::errors::TidePoolError;
use anchor_lang::prelude::*;

/// Maximum fee rate: 100% (10000 hundredths of a bip = 100bps = 1%)
pub const MAX_FEE_RATE: u16 = 10000;

/// Calculate fee amount from a gross amount.
/// fee = amount * fee_rate / 1_000_000
/// (fee_rate is in hundredths of a basis point, so 1_000_000 = 100%)
pub fn calculate_fee_amount(amount: u64, fee_rate: u16) -> Result<u64> {
    if fee_rate == 0 {
        return Ok(0);
    }

    let fee = (amount as u128)
        .checked_mul(fee_rate as u128)
        .ok_or(TidePoolError::MathOverflow)?
        / 1_000_000u128;

    Ok(fee as u64)
}

/// Calculate the amount after fees.
/// amount_after = amount - fee
pub fn calculate_amount_after_fees(amount: u64, fee_rate: u16) -> Result<(u64, u64)> {
    let fee = calculate_fee_amount(amount, fee_rate)?;
    let amount_after = amount
        .checked_sub(fee)
        .ok_or(TidePoolError::MathOverflow)?;
    Ok((amount_after, fee))
}

/// Update global fee growth given a fee amount and current liquidity.
/// fee_growth_delta = (fee_amount << 64) / liquidity (Q64.64)
pub fn calculate_fee_growth_delta(fee_amount: u64, liquidity: u128) -> u128 {
    if liquidity == 0 || fee_amount == 0 {
        return 0;
    }
    ((fee_amount as u128) << 64) / liquidity
}

/// Calculate protocol fee from total fee.
pub fn calculate_protocol_fee(fee_amount: u64, protocol_fee_rate: u16) -> u64 {
    if protocol_fee_rate == 0 {
        return 0;
    }
    ((fee_amount as u128) * (protocol_fee_rate as u128) / 10_000u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_fee_rate() {
        let fee = calculate_fee_amount(1000, 0).unwrap();
        assert_eq!(fee, 0);
    }

    #[test]
    fn test_fee_calculation() {
        // 3000 hundredths of bip = 30 bps = 0.3%
        let fee = calculate_fee_amount(1_000_000, 3000).unwrap();
        assert_eq!(fee, 3000); // 0.3% of 1M
    }

    #[test]
    fn test_amount_after_fees() {
        let (after, fee) = calculate_amount_after_fees(1_000_000, 3000).unwrap();
        assert_eq!(fee, 3000);
        assert_eq!(after, 997_000);
    }

    #[test]
    fn test_protocol_fee() {
        // 10% of 3000 fee = 300
        let proto_fee = calculate_protocol_fee(3000, 1000);
        assert_eq!(proto_fee, 300);
    }

    #[test]
    fn test_fee_growth_delta() {
        let delta = calculate_fee_growth_delta(1000, 1u128 << 64);
        assert_eq!(delta, 1000); // 1000 << 64 / (1 << 64) = 1000
    }
}
