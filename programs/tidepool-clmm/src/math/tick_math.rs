use crate::errors::TidePoolError;
use crate::state::tick::{MAX_TICK, MIN_TICK};
use anchor_lang::prelude::*;

/// Q64.64 representation of sqrt(1.0001) ≈ 1.00004999875
/// Precomputed as: floor(sqrt(1.0001) * 2^64)
const SQRT_1_0001_Q64: u128 = 18_446_835_667_329_027_592;

/// Minimum sqrt price (at MIN_TICK)
pub const MIN_SQRT_PRICE: u128 = 4_295_048_016;
/// Maximum sqrt price (at MAX_TICK)
pub const MAX_SQRT_PRICE: u128 = 79_226_673_515_401_279_992_447_579_055;

/// Convert a tick index to sqrt_price in Q64.64 format.
/// sqrt_price = sqrt(1.0001^tick) = 1.0001^(tick/2)
///
/// Uses binary exponentiation with precomputed powers of sqrt(1.0001).
pub fn tick_to_sqrt_price(tick: i32) -> Result<u128> {
    require!(
        tick >= MIN_TICK && tick <= MAX_TICK,
        TidePoolError::TickOutOfRange
    );

    let abs_tick = tick.unsigned_abs();

    // Start with 1.0 in Q64.64
    let mut ratio: u128 = 1u128 << 64;

    // Precomputed values of sqrt(1.0001)^(2^i) in Q64.64
    // These are the binary decomposition multipliers
    if abs_tick & 0x1 != 0 {
        ratio = mul_shr_64(ratio, 18_446_835_667_329_027_592); // sqrt(1.0001)^1
    }
    if abs_tick & 0x2 != 0 {
        ratio = mul_shr_64(ratio, 18_447_227_339_165_713_608); // sqrt(1.0001)^2
    }
    if abs_tick & 0x4 != 0 {
        ratio = mul_shr_64(ratio, 18_448_010_686_653_951_266); // sqrt(1.0001)^4
    }
    if abs_tick & 0x8 != 0 {
        ratio = mul_shr_64(ratio, 18_449_577_397_493_158_364); // sqrt(1.0001)^8
    }
    if abs_tick & 0x10 != 0 {
        ratio = mul_shr_64(ratio, 18_452_711_012_987_920_572); // sqrt(1.0001)^16
    }
    if abs_tick & 0x20 != 0 {
        ratio = mul_shr_64(ratio, 18_458_979_047_498_498_786); // sqrt(1.0001)^32
    }
    if abs_tick & 0x40 != 0 {
        ratio = mul_shr_64(ratio, 18_471_520_693_795_380_038); // sqrt(1.0001)^64
    }
    if abs_tick & 0x80 != 0 {
        ratio = mul_shr_64(ratio, 18_496_626_009_498_982_982); // sqrt(1.0001)^128
    }
    if abs_tick & 0x100 != 0 {
        ratio = mul_shr_64(ratio, 18_546_913_376_936_983_524); // sqrt(1.0001)^256
    }
    if abs_tick & 0x200 != 0 {
        ratio = mul_shr_64(ratio, 18_647_867_580_926_498_218); // sqrt(1.0001)^512
    }
    if abs_tick & 0x400 != 0 {
        ratio = mul_shr_64(ratio, 18_851_665_025_002_953_612); // sqrt(1.0001)^1024
    }
    if abs_tick & 0x800 != 0 {
        ratio = mul_shr_64(ratio, 19_266_750_673_796_091_498); // sqrt(1.0001)^2048
    }
    if abs_tick & 0x1000 != 0 {
        ratio = mul_shr_64(ratio, 20_126_642_082_702_498_814); // sqrt(1.0001)^4096
    }
    if abs_tick & 0x2000 != 0 {
        ratio = mul_shr_64(ratio, 21_959_233_367_678_982_452); // sqrt(1.0001)^8192
    }
    if abs_tick & 0x4000 != 0 {
        ratio = mul_shr_64(ratio, 26_132_715_642_395_174_284); // sqrt(1.0001)^16384
    }
    if abs_tick & 0x8000 != 0 {
        ratio = mul_shr_64(ratio, 37_024_935_812_674_498_694); // sqrt(1.0001)^32768
    }
    if abs_tick & 0x10000 != 0 {
        ratio = mul_shr_64(ratio, 74_342_281_095_758_498_294); // sqrt(1.0001)^65536
    }
    if abs_tick & 0x20000 != 0 {
        ratio = mul_shr_64(ratio, 299_650_901_097_942_498_142); // sqrt(1.0001)^131072
    }
    if abs_tick & 0x40000 != 0 {
        ratio = mul_shr_64(ratio, 4_868_748_807_049_698_982_194); // sqrt(1.0001)^262144
    }

    // For negative ticks, invert: 1/ratio in Q64.64
    if tick < 0 {
        ratio = u128::MAX / ratio;
    }

    Ok(ratio)
}

/// Convert a sqrt_price (Q64.64) to the largest tick where sqrt_price >= tick_to_sqrt_price(tick).
/// Uses binary search over tick space.
pub fn sqrt_price_to_tick(sqrt_price: u128) -> Result<i32> {
    require!(
        sqrt_price >= MIN_SQRT_PRICE && sqrt_price <= MAX_SQRT_PRICE,
        TidePoolError::SqrtPriceOutOfBounds
    );

    // Binary search: find the largest tick where tick_to_sqrt_price(tick) <= sqrt_price
    let mut low = MIN_TICK;
    let mut high = MAX_TICK;

    while low < high {
        let mid = low + (high - low + 1) / 2;
        let mid_price = tick_to_sqrt_price(mid)?;
        if mid_price <= sqrt_price {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    Ok(low)
}

/// Multiply two u128 values and right-shift by 64 (Q64.64 multiplication).
fn mul_shr_64(a: u128, b: u128) -> u128 {
    let result = crate::math::u256::U256::mul_u128(a, b);
    result.shr_64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_zero_is_one() {
        let price = tick_to_sqrt_price(0).unwrap();
        let one_q64 = 1u128 << 64;
        // At tick 0, sqrt_price should be 1.0 (Q64.64)
        assert_eq!(price, one_q64);
    }

    #[test]
    fn test_positive_tick_greater() {
        let price_0 = tick_to_sqrt_price(0).unwrap();
        let price_100 = tick_to_sqrt_price(100).unwrap();
        assert!(price_100 > price_0);
    }

    #[test]
    fn test_negative_tick_smaller() {
        let price_0 = tick_to_sqrt_price(0).unwrap();
        let price_neg100 = tick_to_sqrt_price(-100).unwrap();
        assert!(price_neg100 < price_0);
    }

    #[test]
    fn test_roundtrip_tick_0() {
        let price = tick_to_sqrt_price(0).unwrap();
        let tick = sqrt_price_to_tick(price).unwrap();
        assert_eq!(tick, 0);
    }

    #[test]
    fn test_min_max_bounds() {
        let min_price = tick_to_sqrt_price(MIN_TICK).unwrap();
        let max_price = tick_to_sqrt_price(MAX_TICK).unwrap();
        assert!(min_price > 0);
        assert!(max_price > min_price);
    }
}
