/// Minimal 256-bit unsigned integer for intermediate Q64.64 multiplication.
/// Represented as [lo, hi] where value = lo + hi * 2^128.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U256(pub [u128; 2]);

impl U256 {
    pub const ZERO: Self = U256([0, 0]);

    pub fn from_u128(v: u128) -> Self {
        U256([v, 0])
    }

    /// Full 128x128 → 256-bit multiplication.
    pub fn mul_u128(a: u128, b: u128) -> Self {
        let a_lo = a as u64 as u128;
        let a_hi = a >> 64;
        let b_lo = b as u64 as u128;
        let b_hi = b >> 64;

        let ll = a_lo * b_lo;
        let lh = a_lo * b_hi;
        let hl = a_hi * b_lo;
        let hh = a_hi * b_hi;

        let mid_sum = lh.checked_add(hl).unwrap_or(u128::MAX);
        let (lo, carry1) = ll.overflowing_add(mid_sum << 64);
        let hi = hh + (mid_sum >> 64) + if carry1 { 1 } else { 0 };

        // Handle potential carry from lh + hl overflow
        let hi = if lh > u128::MAX - hl {
            hi + (1u128 << 64)
        } else {
            hi
        };

        U256([lo, hi])
    }

    /// Right shift by 64 bits (used for Q64.64 multiplication result).
    pub fn shr_64(&self) -> u128 {
        (self.0[0] >> 64) | (self.0[1] << 64)
    }

    /// Right shift by 128 bits.
    pub fn shr_128(&self) -> u128 {
        self.0[1]
    }

    /// Check if the result fits in u128 after right shift.
    pub fn fits_u128_after_shr64(&self) -> bool {
        self.0[1] >> 64 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mul_small() {
        let result = U256::mul_u128(100, 200);
        assert_eq!(result.0[0], 20_000);
        assert_eq!(result.0[1], 0);
    }

    #[test]
    fn test_shr_64() {
        let result = U256::mul_u128(1u128 << 64, 1u128 << 64);
        assert_eq!(result.shr_64(), 1u128 << 64);
    }

    #[test]
    fn test_from_u128() {
        let v = U256::from_u128(42);
        assert_eq!(v.0[0], 42);
        assert_eq!(v.0[1], 0);
    }
}
