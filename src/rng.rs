//! A small, fast, seedable random number generator.
//!
//! We write our own instead of pulling in a crate: it is twenty lines,
//! compiles instantly, and guarantees the same seed produces the same
//! dungeon on every machine and every Rust version, forever. That
//! makes bugs reproducible: note the seed, replay the run.
//!
//! The algorithm is SplitMix64, which is simple and has good enough
//! statistical quality for games.

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        // `wrapping_*` operations deliberately allow overflow. In debug
        // builds plain `+` and `*` would panic on overflow instead.
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A random integer in `lo..hi` (`hi` excluded).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(lo < hi, "empty range {lo}..{hi}");
        let span = (hi as i64 - lo as i64) as u128;
        // Multiplying 64 random bits by the span and keeping the top
        // 64 bits maps them evenly onto 0..span, without the bias that
        // `% span` would introduce.
        let r = (self.next_u64() as u128 * span) >> 64;
        (lo as i64 + r as i64) as i32
    }

    /// True with the given percent chance, e.g. `chance(70)`.
    pub fn chance(&mut self, percent: i32) -> bool {
        self.range(0, 100) < percent
    }

    /// A uniformly chosen index into a slice of length `len`.
    pub fn index(&mut self, len: usize) -> usize {
        self.range(0, len as i32) as usize
    }
}

/// Combines two numbers into a new, well-scrambled seed. Used to give
/// each floor of a run its own seed: `mix(run_seed, depth)`.
pub fn mix(a: u64, b: u64) -> u64 {
    Rng::new(a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15)).next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_stays_in_bounds_and_hits_both_ends() {
        let mut rng = Rng::new(7);
        let mut seen = [false; 5];
        for _ in 0..1000 {
            let v = rng.range(-2, 3);
            assert!((-2..3).contains(&v));
            seen[(v + 2) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
