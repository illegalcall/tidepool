use crate::errors::TidePoolError;
use anchor_lang::prelude::*;

/// Add a signed liquidity delta to an unsigned liquidity value.
pub fn add_liquidity_delta(liquidity: u128, delta: i128) -> Result<u128> {
    if delta >= 0 {
        liquidity
            .checked_add(delta as u128)
            .ok_or_else(|| error!(TidePoolError::MathOverflow))
    } else {
        let abs_delta = (-delta) as u128;
        liquidity
            .checked_sub(abs_delta)
            .ok_or_else(|| error!(TidePoolError::MathOverflow))
    }
}

/// Calculate fee growth inside a position's tick range.
///
/// fee_growth_inside = fee_growth_global
///     - fee_growth_below(tick_lower)
///     - fee_growth_above(tick_upper)
pub fn calculate_fee_growth_inside(
    tick_current: i32,
    tick_lower: i32,
    tick_upper: i32,
    fee_growth_global: u128,
    fee_growth_outside_lower: u128,
    fee_growth_outside_upper: u128,
) -> u128 {
    // Fee growth below tick_lower
    let fee_growth_below = if tick_current >= tick_lower {
        fee_growth_outside_lower
    } else {
        fee_growth_global.wrapping_sub(fee_growth_outside_lower)
    };

    // Fee growth above tick_upper
    let fee_growth_above = if tick_current < tick_upper {
        fee_growth_outside_upper
    } else {
        fee_growth_global.wrapping_sub(fee_growth_outside_upper)
    };

    fee_growth_global
        .wrapping_sub(fee_growth_below)
        .wrapping_sub(fee_growth_above)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive_delta() {
        let result = add_liquidity_delta(100, 50).unwrap();
        assert_eq!(result, 150);
    }

    #[test]
    fn test_add_negative_delta() {
        let result = add_liquidity_delta(100, -50).unwrap();
        assert_eq!(result, 50);
    }

    #[test]
    fn test_underflow_fails() {
        let result = add_liquidity_delta(50, -100);
        assert!(result.is_err());
    }

    #[test]
    fn test_fee_growth_inside_current_in_range() {
        let result = calculate_fee_growth_inside(
            5,    // current tick in range [0, 10]
            0,    // tick_lower
            10,   // tick_upper
            1000, // global
            200,  // outside lower
            300,  // outside upper
        );
        assert_eq!(result, 500); // 1000 - 200 - 300
    }

    #[test]
    fn test_fee_growth_inside_current_below() {
        let result = calculate_fee_growth_inside(
            -5,   // current tick below range [0, 10]
            0,    // tick_lower
            10,   // tick_upper
            1000, // global
            200,  // outside lower
            300,  // outside upper
        );
        // below = global - outside_lower = 1000 - 200 = 800
        // above = outside_upper = 300
        // inside = 1000 - 800 - 300 = wrapping sub
        let expected = 1000u128.wrapping_sub(800).wrapping_sub(300);
        assert_eq!(result, expected);
    }
}
