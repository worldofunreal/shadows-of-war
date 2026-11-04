use wyrand::WyRand;

/// Stable, stateless sample for a combat tile. `salt` separates independent
/// combat effects without consuming an attack's mutable RNG stream.
#[inline]
pub(crate) fn stable_tile_sample(
    match_seed: u64,
    attacker_id: u16,
    target_owner: u16,
    tile_idx: u64,
    salt: u64,
) -> f64 {
    let key = match_seed
        .wrapping_add((attacker_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((target_owner as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add(tile_idx.wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add(salt);
    let mut rng = WyRand::new(key);
    (rng.rand() >> 11) as f64 / 9_007_199_254_740_992.0
}

pub trait NextIntExt {
    /// Generates a random integer between min (inclusive) and max (exclusive).
    fn next_int(&mut self, min: i32, max: i32) -> i32;

    /// Returns true with probability 1/odds.
    fn chance(&mut self, odds: u32) -> bool;
}

impl NextIntExt for WyRand {
    #[inline]
    fn next_int(&mut self, min: i32, max: i32) -> i32 {
        let range = (max - min).max(1) as u64;
        min + (self.rand() % range) as i32
    }

    #[inline]
    fn chance(&mut self, odds: u32) -> bool {
        if odds == 0 {
            false
        } else {
            self.rand().is_multiple_of(odds as u64)
        }
    }
}
