use crate::errors::TidePoolError;
use crate::state::tick::{MAX_TICK, MIN_TICK};
use anchor_lang::prelude::*;

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
    // Generated from: floor(sqrt(1.0001)^(2^i) * 2^64)
    // Verified against Uniswap V3's TickMath.sol constants
    if abs_tick & 0x1 != 0 {
        ratio = mul_shr_64(ratio, 18_447_666_387_855_958_016); // sqrt(1.0001)^1
    }
    if abs_tick & 0x2 != 0 {
        ratio = mul_shr_64(ratio, 18_448_588_748_116_918_272); // sqrt(1.0001)^2
    }
    if abs_tick & 0x4 != 0 {
        ratio = mul_shr_64(ratio, 18_450_433_606_991_728_640); // sqrt(1.0001)^4
    }
    if abs_tick & 0x8 != 0 {
        ratio = mul_shr_64(ratio, 18_454_123_878_217_453_568); // sqrt(1.0001)^8
    }
    if abs_tick & 0x10 != 0 {
        ratio = mul_shr_64(ratio, 18_461_506_635_089_977_344); // sqrt(1.0001)^16
    }
    if abs_tick & 0x20 != 0 {
        ratio = mul_shr_64(ratio, 18_476_281_010_653_851_648); // sqrt(1.0001)^32
    }
    if abs_tick & 0x40 != 0 {
        ratio = mul_shr_64(ratio, 18_505_865_242_158_133_248); // sqrt(1.0001)^64
    }
    if abs_tick & 0x80 != 0 {
        ratio = mul_shr_64(ratio, 18_565_175_891_880_198_144); // sqrt(1.0001)^128
    }
    if abs_tick & 0x100 != 0 {
        ratio = mul_shr_64(ratio, 18_684_368_066_214_465_536); // sqrt(1.0001)^256
    }
    if abs_tick & 0x200 != 0 {
        ratio = mul_shr_64(ratio, 18_925_053_041_274_802_176); // sqrt(1.0001)^512
    }
    if abs_tick & 0x400 != 0 {
        ratio = mul_shr_64(ratio, 19_415_764_168_675_909_632); // sqrt(1.0001)^1024
    }
    if abs_tick & 0x800 != 0 {
        ratio = mul_shr_64(ratio, 20_435_687_552_629_014_528); // sqrt(1.0001)^2048
    }
    if abs_tick & 0x1000 != 0 {
        ratio = mul_shr_64(ratio, 22_639_080_592_215_080_960); // sqrt(1.0001)^4096
    }
    if abs_tick & 0x2000 != 0 {
        ratio = mul_shr_64(ratio, 27_784_196_929_975_758_848); // sqrt(1.0001)^8192
    }
    if abs_tick & 0x4000 != 0 {
        ratio = mul_shr_64(ratio, 41_848_122_137_926_787_072); // sqrt(1.0001)^16384
    }
    if abs_tick & 0x8000 != 0 {
        ratio = mul_shr_64(ratio, 94_936_283_577_910_951_936); // sqrt(1.0001)^32768
    }
    if abs_tick & 0x10000 != 0 {
        ratio = mul_shr_64(ratio, 488_590_176_324_437_606_400); // sqrt(1.0001)^65536
    }
    if abs_tick & 0x20000 != 0 {
        ratio = mul_shr_64(ratio, 12_941_056_668_150_515_367_936); // sqrt(1.0001)^131072
    }
    if abs_tick & 0x40000 != 0 {
        ratio = mul_shr_64(ratio, 9_078_618_265_592_131_460_530_176); // sqrt(1.0001)^262144
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

    #[test]
    fn test_tick_1_matches_sqrt_1_0001() {
        let price = tick_to_sqrt_price(1).unwrap();
        let one = 1u128 << 64;
        // sqrt(1.0001) ≈ 1.0000499987500624
        // At tick 1, price should be ~0.005% above 1.0
        let diff_pct = ((price as f64 - one as f64) / one as f64) * 100.0;
        assert!(diff_pct > 0.004, "tick 1 price should be ~0.005% above 1.0");
        assert!(diff_pct < 0.006, "tick 1 price should be ~0.005% above 1.0");
    }

    #[test]
    fn test_tick_symmetry() {
        // tick_to_sqrt_price(n) * tick_to_sqrt_price(-n) ≈ 1.0^2 = 2^128
        let pos = tick_to_sqrt_price(1000).unwrap();
        let neg = tick_to_sqrt_price(-1000).unwrap();
        let product = crate::math::u256::U256::mul_u128(pos, neg);
        let one_squared = 1u128 << 64; // 1.0 in Q64.64
        let result = product.shr_64();
        // Should be close to 1.0 in Q64.64
        let error_pct = ((result as f64 - one_squared as f64) / one_squared as f64).abs() * 100.0;
        assert!(error_pct < 0.01, "pos*neg product should be ~1.0, got {}% error", error_pct);
    }

    #[test]
    fn test_roundtrip_tick_100() {
        let price = tick_to_sqrt_price(100).unwrap();
        let tick = sqrt_price_to_tick(price).unwrap();
        assert_eq!(tick, 100);
    }

    #[test]
    fn test_roundtrip_tick_negative() {
        let price = tick_to_sqrt_price(-500).unwrap();
        let tick = sqrt_price_to_tick(price).unwrap();
        assert_eq!(tick, -500);
    }
}
