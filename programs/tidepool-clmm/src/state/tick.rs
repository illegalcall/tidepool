use anchor_lang::prelude::*;

pub const TICK_ARRAY_SIZE: usize = 64;
pub const MIN_TICK: i32 = -443636;
pub const MAX_TICK: i32 = 443636;

#[account]
#[derive(InitSpace)]
pub struct TickArray {
    pub pool: Pubkey,
    pub start_tick_index: i32,
    #[max_len(64)]
    pub ticks: Vec<Tick>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, InitSpace)]
pub struct Tick {
    pub initialized: bool,
    /// Net liquidity change when this tick is crossed (positive = add, negative = remove)
    pub liquidity_net: i128,
    /// Total liquidity referencing this tick as either lower or upper bound
    pub liquidity_gross: u128,
    /// Fee growth on the opposite side of this tick for token A (Q64.64)
    pub fee_growth_outside_a: u128,
    /// Fee growth on the opposite side of this tick for token B (Q64.64)
    pub fee_growth_outside_b: u128,
}

impl TickArray {
    pub fn get_tick_offset(&self, tick_index: i32, tick_spacing: u16) -> Option<usize> {
        let spacing = tick_spacing as i32;
        if tick_index < self.start_tick_index {
            return None;
        }
        let offset = ((tick_index - self.start_tick_index) / spacing) as usize;
        if offset >= TICK_ARRAY_SIZE {
            None
        } else {
            Some(offset)
        }
    }

    pub fn get_tick(&self, tick_index: i32, tick_spacing: u16) -> Option<&Tick> {
        let offset = self.get_tick_offset(tick_index, tick_spacing)?;
        self.ticks.get(offset)
    }

    pub fn get_tick_mut(&mut self, tick_index: i32, tick_spacing: u16) -> Option<&mut Tick> {
        let offset = self.get_tick_offset(tick_index, tick_spacing)?;
        self.ticks.get_mut(offset)
    }

    pub fn end_tick_index(&self, tick_spacing: u16) -> i32 {
        self.start_tick_index + (TICK_ARRAY_SIZE as i32) * (tick_spacing as i32)
    }
}

pub fn check_tick_bounds(tick_index: i32) -> bool {
    tick_index >= MIN_TICK && tick_index <= MAX_TICK
}

pub fn check_tick_alignment(tick_index: i32, tick_spacing: u16) -> bool {
    tick_index % (tick_spacing as i32) == 0
}

pub fn get_tick_array_start_index(tick_index: i32, tick_spacing: u16) -> i32 {
    let ticks_in_array = TICK_ARRAY_SIZE as i32 * tick_spacing as i32;
    let mut start = tick_index / ticks_in_array * ticks_in_array;
    if tick_index < 0 && tick_index % ticks_in_array != 0 {
        start -= ticks_in_array;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_tick_bounds() {
        assert!(check_tick_bounds(0));
        assert!(check_tick_bounds(MIN_TICK));
        assert!(check_tick_bounds(MAX_TICK));
        assert!(!check_tick_bounds(MIN_TICK - 1));
        assert!(!check_tick_bounds(MAX_TICK + 1));
    }

    #[test]
    fn test_check_tick_alignment() {
        assert!(check_tick_alignment(0, 10));
        assert!(check_tick_alignment(100, 10));
        assert!(check_tick_alignment(-100, 10));
        assert!(!check_tick_alignment(5, 10));
        assert!(!check_tick_alignment(-15, 10));
        assert!(check_tick_alignment(60, 60));
    }

    #[test]
    fn test_get_tick_array_start_index() {
        assert_eq!(get_tick_array_start_index(0, 10), 0);
        assert_eq!(get_tick_array_start_index(639, 10), 0);
        assert_eq!(get_tick_array_start_index(640, 10), 640);
        assert_eq!(get_tick_array_start_index(-1, 10), -640);
        assert_eq!(get_tick_array_start_index(-640, 10), -640);
        assert_eq!(get_tick_array_start_index(-641, 10), -1280);
    }

    #[test]
    fn test_tick_array_get_offset() {
        let ta = TickArray {
            pool: anchor_lang::prelude::Pubkey::default(),
            start_tick_index: 0,
            ticks: vec![Tick::default(); TICK_ARRAY_SIZE],
            bump: 0,
        };
        assert_eq!(ta.get_tick_offset(0, 10), Some(0));
        assert_eq!(ta.get_tick_offset(10, 10), Some(1));
        assert_eq!(ta.get_tick_offset(630, 10), Some(63));
        assert_eq!(ta.get_tick_offset(640, 10), None); // out of range
        assert_eq!(ta.get_tick_offset(-10, 10), None); // before start
    }

    #[test]
    fn test_tick_array_end_index() {
        let ta = TickArray {
            pool: anchor_lang::prelude::Pubkey::default(),
            start_tick_index: 0,
            ticks: vec![Tick::default(); TICK_ARRAY_SIZE],
            bump: 0,
        };
        assert_eq!(ta.end_tick_index(10), 640);
        assert_eq!(ta.end_tick_index(1), 64);
        assert_eq!(ta.end_tick_index(60), 3840);
    }
}
