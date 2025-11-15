use crate::errors::TidePoolError;
use crate::math::u256::U256;
use anchor_lang::prelude::*;

/// Calculate the amount of token A required for a given liquidity and price range.
/// delta_a = L * (1/sqrt_price_lower - 1/sqrt_price_upper)
///         = L * (sqrt_price_upper - sqrt_price_lower) / (sqrt_price_lower * sqrt_price_upper)
pub fn get_amount_a_delta(
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    if liquidity == 0 || sqrt_price_lower == sqrt_price_upper {
        return Ok(0);
    }

    let price_diff = sqrt_price_upper
        .checked_sub(sqrt_price_lower)
        .ok_or(TidePoolError::MathOverflow)?;

    // numerator = L * (sqrt_upper - sqrt_lower) << 64
    let numerator = U256::mul_u128(liquidity, price_diff);

    // denominator = sqrt_lower * sqrt_upper >> 64
    let denom_256 = U256::mul_u128(sqrt_price_lower, sqrt_price_upper);
    let denominator = denom_256.shr_64();

    if denominator == 0 {
        return err!(TidePoolError::DivisionByZero);
    }

    // result = numerator / denominator (with Q64.64 adjustments)
    let result_shifted = numerator.shr_64();
    let amount = result_shifted / denominator;

    if round_up && result_shifted % denominator != 0 {
        Ok(amount.checked_add(1).ok_or(TidePoolError::MathOverflow)? as u64)
    } else {
        Ok(amount as u64)
    }
}

/// Calculate the amount of token B required for a given liquidity and price range.
/// delta_b = L * (sqrt_price_upper - sqrt_price_lower)
pub fn get_amount_b_delta(
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    if liquidity == 0 || sqrt_price_lower == sqrt_price_upper {
        return Ok(0);
    }

    let price_diff = sqrt_price_upper
        .checked_sub(sqrt_price_lower)
        .ok_or(TidePoolError::MathOverflow)?;

    // amount = L * (sqrt_upper - sqrt_lower) >> 64
    let product = U256::mul_u128(liquidity, price_diff);
    let amount = product.shr_64();

    if round_up {
        let remainder_check = U256::mul_u128(liquidity, price_diff);
        let has_remainder = (remainder_check.0[0] & ((1u128 << 64) - 1)) != 0;
        if has_remainder {
            Ok(amount.checked_add(1).ok_or(TidePoolError::MathOverflow)? as u64)
        } else {
            Ok(amount as u64)
        }
    } else {
        Ok(amount as u64)
    }
}

/// Calculate the next sqrt price when swapping token A for token B (price decreases).
/// new_sqrt_price = L * sqrt_price / (L + amount * sqrt_price)
pub fn get_next_sqrt_price_a_up(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    add: bool,
) -> Result<u128> {
    if amount == 0 {
        return Ok(sqrt_price);
    }

    // product = amount * sqrt_price >> 64
    let product = U256::mul_u128(amount as u128, sqrt_price).shr_64();

    let (numerator, denominator) = if add {
        // Buying B with A: price goes down
        // new_price = L * price / (L + amount * price)
        let denom = liquidity.checked_add(product).ok_or(TidePoolError::MathOverflow)?;
        let num = U256::mul_u128(liquidity, sqrt_price);
        (num.shr_64(), denom)
    } else {
        // Removing A: price goes up
        let denom = liquidity.checked_sub(product).ok_or(TidePoolError::MathOverflow)?;
        let num = U256::mul_u128(liquidity, sqrt_price);
        (num.shr_64(), denom)
    };

    if denominator == 0 {
        return err!(TidePoolError::DivisionByZero);
    }

    // Round up to ensure we don't undercharge
    let result = numerator / denominator;
    if numerator % denominator != 0 {
        Ok(result.checked_add(1).ok_or(TidePoolError::MathOverflow)?)
    } else {
        Ok(result)
    }
}

/// Calculate the next sqrt price when swapping token B for token A (price increases).
/// new_sqrt_price = sqrt_price + amount / L
pub fn get_next_sqrt_price_b_down(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    add: bool,
) -> Result<u128> {
    if amount == 0 {
        return Ok(sqrt_price);
    }

    // quotient = (amount << 64) / liquidity
    let amount_shifted = (amount as u128) << 64;
    let quotient = amount_shifted / liquidity;

    if add {
        // Adding B: price increases
        sqrt_price
            .checked_add(quotient)
            .ok_or_else(|| error!(TidePoolError::MathOverflow))
    } else {
        // Removing B: price decreases
        sqrt_price
            .checked_sub(quotient)
            .ok_or_else(|| error!(TidePoolError::MathOverflow))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_Q64: u128 = 1u128 << 64;

    #[test]
    fn test_amount_a_delta_zero_liquidity() {
        let result = get_amount_a_delta(ONE_Q64, ONE_Q64 * 2, 0, false).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_amount_b_delta_zero_liquidity() {
        let result = get_amount_b_delta(ONE_Q64, ONE_Q64 * 2, 0, false).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_amount_a_positive_for_valid_range() {
        let result = get_amount_a_delta(ONE_Q64, ONE_Q64 * 2, 1_000_000, true).unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_amount_b_positive_for_valid_range() {
        let result = get_amount_b_delta(ONE_Q64, ONE_Q64 * 2, 1_000_000, true).unwrap();
        assert!(result > 0);
    }
}
