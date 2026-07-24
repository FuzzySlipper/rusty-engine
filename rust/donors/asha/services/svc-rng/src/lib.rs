//! Deterministic scoped random streams.
//!
//! # Lane
//!
//! `rust-service` — the single reusable substrate for authoritative randomness.
//! Callers must provide an explicit seed and scope; there is no wall-clock,
//! ambient entropy, global state, or platform RNG.

#![forbid(unsafe_code)]

/// Explicit authoritative seed for deterministic services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RngSeed(u64);

impl RngSeed {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// A deterministic random stream derived from a seed and textual scope.
///
/// The stream uses SplitMix64 after hashing the seed/scope pair. It is meant for
/// reproducible service decisions, not cryptography.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedRng {
    seed: RngSeed,
    scope_hash: u64,
    counter: u64,
}

impl ScopedRng {
    /// Create a deterministic stream from `seed` and `scope`.
    pub fn new(seed: RngSeed, scope: &str) -> Self {
        Self {
            seed,
            scope_hash: hash_seed_scope(seed, scope),
            counter: 0,
        }
    }

    pub const fn seed(&self) -> RngSeed {
        self.seed
    }

    pub const fn counter(&self) -> u64 {
        self.counter
    }

    /// Derive a child stream under an additional scope segment.
    pub fn fork(&self, scope: &str) -> Self {
        let mut child_seed = self.seed.raw();
        feed_u64(&mut child_seed, self.scope_hash);
        ScopedRng::new(RngSeed::new(child_seed), scope)
    }

    /// Advance and return the next deterministic `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let value = splitmix64(self.scope_hash.wrapping_add(self.counter));
        self.counter = self.counter.wrapping_add(1);
        value
    }

    /// Return a value in `0..upper`, or `None` when `upper == 0`.
    pub fn next_bounded_u32(&mut self, upper: u32) -> Option<u32> {
        if upper == 0 {
            return None;
        }
        Some((self.next_u64() % upper as u64) as u32)
    }

    /// Return a deterministic boolean.
    pub fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

fn hash_seed_scope(seed: RngSeed, scope: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    feed_u64(&mut h, seed.raw());
    for b in scope.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn feed_u64(h: &mut u64, value: u64) {
    for b in value.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_and_scope_replay_same_stream() {
        let mut a = ScopedRng::new(RngSeed::new(42), "level/tunnel");
        let mut b = ScopedRng::new(RngSeed::new(42), "level/tunnel");
        let left: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let right: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(left, right);
    }

    #[test]
    fn different_scopes_diverge() {
        let mut a = ScopedRng::new(RngSeed::new(42), "level/tunnel");
        let mut b = ScopedRng::new(RngSeed::new(42), "combat/spawn");
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn bounded_zero_is_rejected() {
        let mut rng = ScopedRng::new(RngSeed::new(1), "bounds");
        assert_eq!(rng.next_bounded_u32(0), None);
        assert!(rng.next_bounded_u32(3).is_some_and(|v| v < 3));
    }
}
