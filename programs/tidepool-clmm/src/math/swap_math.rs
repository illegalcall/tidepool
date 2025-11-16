use crate::errors::TidePoolError;
use crate::math::fee_math;
use crate::math::sqrt_price_math;
use anchor_lang::prelude::*;

#[derive(Debug, Clone)]
pub struct SwapStepResult {
    pub sqrt_price_next: u128,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
}

/// Compute a single swap step within a tick range.
///
/// Given the current price, target price, available liquidity, and remaining amount,
/// determines how much can be swapped and the resulting price.
pub fn compute_swap_step(
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: u64,
    fee_rate: u16,
    amount_specified_is_input: bool,
    a_to_b: bool,
) -> Result<SwapStepResult> {
    if liquidity == 0 {
        return Ok(SwapStepResult {
            sqrt_price_next: sqrt_price_target,
            amount_in: 0,
            amount_out: 0,
            fee_amount: 0,
        });
    }

    let (amount_after_fees, fee_from_remaining) = if amount_specified_is_input {
        fee_math::calculate_amount_after_fees(amount_remaining, fee_rate)?
    } else {
        (amount_remaining, 0u64)
    };

    // Calculate the max amount that can be swapped to reach the target price
    let amount_in_max = if a_to_b {
        sqrt_price_math::get_amount_a_delta(
            sqrt_price_target,
            sqrt_price_current,
            liquidity,
            true,
        )?
    } else {
        sqrt_price_math::get_amount_b_delta(
            sqrt_price_current,
            sqrt_price_target,
            liquidity,
            true,
        )?
    };

    // Determine if we can reach the target price
    let (sqrt_price_next, amount_in, amount_out) = if amount_after_fees >= amount_in_max {
        // We reach the target price
        let amount_out = if a_to_b {
            sqrt_price_math::get_amount_b_delta(
                sqrt_price_target,
                sqrt_price_current,
                liquidity,
                false,
            )?
        } else {
            sqrt_price_math::get_amount_a_delta(
                sqrt_price_current,
                sqrt_price_target,
                liquidity,
                false,
            )?
        };
        (sqrt_price_target, amount_in_max, amount_out)
    } else {
        // We can't reach the target; compute the price we do reach
        let next_price = if a_to_b {
            sqrt_price_math::get_next_sqrt_price_a_up(
                sqrt_price_current,
                liquidity,
                amount_after_fees,
                true,
            )?
        } else {
            sqrt_price_math::get_next_sqrt_price_b_down(
                sqrt_price_current,
                liquidity,
                amount_after_fees,
                true,
            )?
        };

        let actual_in = if a_to_b {
            sqrt_price_math::get_amount_a_delta(next_price, sqrt_price_current, liquidity, true)?
        } else {
            sqrt_price_math::get_amount_b_delta(sqrt_price_current, next_price, liquidity, true)?
        };

        let actual_out = if a_to_b {
            sqrt_price_math::get_amount_b_delta(next_price, sqrt_price_current, liquidity, false)?
        } else {
            sqrt_price_math::get_amount_a_delta(
                sqrt_price_current,
                next_price,
                liquidity,
                false,
            )?
        };

        (next_price, actual_in, actual_out)
    };

    // Calculate fee
    let fee_amount = if amount_specified_is_input && sqrt_price_next != sqrt_price_target {
        // Didn't reach target — fee is whatever is left
        amount_remaining
            .checked_sub(amount_in)
            .ok_or(TidePoolError::MathOverflow)?
    } else {
        fee_math::calculate_fee_amount(amount_in, fee_rate)?
    };

    Ok(SwapStepResult {
        sqrt_price_next,
        amount_in,
        amount_out,
        fee_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_Q64: u128 = 1u128 << 64;

    #[test]
    fn test_zero_liquidity_step() {
        let result = compute_swap_step(
            ONE_Q64,
            ONE_Q64 * 2,
            0,    // zero liquidity
            1000,
            3000,
            true,
            false,
        )
        .unwrap();

        assert_eq!(result.sqrt_price_next, ONE_Q64 * 2);
        assert_eq!(result.amount_in, 0);
        assert_eq!(result.amount_out, 0);
    }
}
