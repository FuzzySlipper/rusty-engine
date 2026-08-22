//! Deterministic scoped random streams.
//!
//! # Lane
//!
//! `rust-service` — the single reusable substrate for authoritative randomness.
//! Callers must provide an explicit seed and scope; there is no wall-clock,
//! ambient entropy, global state, or platform RNG.
//!
//! `KeyedRngV1` is a separate stateless surface for decisions identified by a
//! caller-provided key. It does not advance, fork, or otherwise affect a
//! `ScopedRng` stream.

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

/// Maximum UTF-8 bytes accepted for a keyed-draw scope.
pub const MAX_KEYED_RNG_SCOPE_BYTES: usize = 256;

/// Maximum bytes accepted for a keyed-draw key.
pub const MAX_KEYED_RNG_KEY_BYTES: usize = 4 * 1024;

/// Maximum deterministic rejection-sampling attempts for one keyed draw.
pub const KEYED_RNG_V1_MAX_ATTEMPTS: u32 = 128;

/// Input validation or deterministic sampling failure for [`KeyedRngV1`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyedRngError {
    /// A keyed draw needs a non-empty scope to keep caller domains explicit.
    EmptyScope,
    /// The scope exceeds the bounded keyed-draw input quota.
    ScopeQuotaExceeded { actual: usize, maximum: usize },
    /// A keyed draw needs a non-empty caller-owned key.
    EmptyKey,
    /// The key exceeds the bounded keyed-draw input quota.
    KeyQuotaExceeded { actual: usize, maximum: usize },
    /// The inclusive range has its bounds in reverse order.
    InvalidRange { minimum: i64, maximum: i64 },
    /// Deterministic rejection sampling did not accept a sample within its quota.
    RejectionQuotaExceeded { attempts: u32 },
}

/// Version 1 of the stateless keyed inclusive-integer draw.
///
/// Versioning is part of the type name because the exact output sequence is a
/// durable caller contract. V1 hashes a length-framed seed/scope/key tuple under
/// fixed domain tags with FNV-1a, derives attempts with SplitMix64, and maps a
/// sample with multiply-high rejection sampling. The mapping rejects the small
/// uneven tail before taking the high half of the 128-bit product, so accepted
/// values are unbiased across the requested inclusive range.
///
/// This is deterministic service machinery, not a cryptographic primitive.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyedRngV1;

impl KeyedRngV1 {
    /// Draw an unbiased deterministic integer from `minimum..=maximum`.
    ///
    /// `scope` names a caller-owned decision domain and `key` identifies one
    /// decision within it. Both are complete, length-framed byte sequences;
    /// changing any byte, including one after the first four, changes the V1
    /// input frame. The result depends only on this argument tuple, so retrying
    /// an identical tuple reproduces the same value or bounded-rejection error.
    pub fn draw_i64_inclusive(
        seed: RngSeed,
        scope: &str,
        key: &[u8],
        minimum: i64,
        maximum: i64,
    ) -> Result<i64, KeyedRngError> {
        validate_keyed_input(scope, key, minimum, maximum)?;

        let input_hash = hash_keyed_v1_input(seed, scope.as_bytes(), key);
        let span = inclusive_span(minimum, maximum);

        if span == (1u128 << 64) {
            return Ok(minimum.wrapping_add(sample_keyed_v1(input_hash, 0) as i64));
        }

        let span = u64::try_from(span).expect("non-full i64 spans fit in u64");
        let offset = map_unbiased_v1(input_hash, span)?;
        let value = i128::from(minimum)
            .checked_add(i128::from(offset))
            .expect("a valid inclusive i64 span cannot overflow i128");
        Ok(i64::try_from(value).expect("a valid inclusive i64 span stays within i64"))
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

fn validate_keyed_input(
    scope: &str,
    key: &[u8],
    minimum: i64,
    maximum: i64,
) -> Result<(), KeyedRngError> {
    if scope.is_empty() {
        return Err(KeyedRngError::EmptyScope);
    }
    if scope.len() > MAX_KEYED_RNG_SCOPE_BYTES {
        return Err(KeyedRngError::ScopeQuotaExceeded {
            actual: scope.len(),
            maximum: MAX_KEYED_RNG_SCOPE_BYTES,
        });
    }
    if key.is_empty() {
        return Err(KeyedRngError::EmptyKey);
    }
    if key.len() > MAX_KEYED_RNG_KEY_BYTES {
        return Err(KeyedRngError::KeyQuotaExceeded {
            actual: key.len(),
            maximum: MAX_KEYED_RNG_KEY_BYTES,
        });
    }
    if minimum > maximum {
        return Err(KeyedRngError::InvalidRange { minimum, maximum });
    }
    Ok(())
}

fn inclusive_span(minimum: i64, maximum: i64) -> u128 {
    u128::try_from(
        i128::from(maximum)
            .checked_sub(i128::from(minimum))
            .and_then(|difference| difference.checked_add(1))
            .expect("an i64 inclusive span fits in i128"),
    )
    .expect("a valid inclusive i64 span is positive")
}

fn hash_keyed_v1_input(seed: RngSeed, scope: &[u8], key: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    feed_tagged_bytes(&mut hash, b"rusty-engine/svc-rng/keyed-i64/v1", b"input");
    feed_u64(&mut hash, seed.raw());
    feed_tagged_bytes(&mut hash, b"scope", scope);
    feed_tagged_bytes(&mut hash, b"key", key);
    hash
}

fn feed_tagged_bytes(hash: &mut u64, tag: &[u8], value: &[u8]) {
    feed_u64(hash, tag.len() as u64);
    feed_bytes(hash, tag);
    feed_u64(hash, value.len() as u64);
    feed_bytes(hash, value);
}

fn feed_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn sample_keyed_v1(input_hash: u64, attempt: u32) -> u64 {
    let mut hash = input_hash;
    feed_tagged_bytes(&mut hash, b"rusty-engine/svc-rng/keyed-i64/v1", b"attempt");
    feed_u64(&mut hash, u64::from(attempt));
    splitmix64(hash)
}

fn map_unbiased_v1(input_hash: u64, span: u64) -> Result<u64, KeyedRngError> {
    debug_assert_ne!(span, 0);
    for attempt in 0..KEYED_RNG_V1_MAX_ATTEMPTS {
        let sample = sample_keyed_v1(input_hash, attempt);
        if let Some(offset) = map_sample_unbiased_v1(span, sample) {
            return Ok(offset);
        }
    }
    Err(KeyedRngError::RejectionQuotaExceeded {
        attempts: KEYED_RNG_V1_MAX_ATTEMPTS,
    })
}

fn map_sample_unbiased_v1(span: u64, sample: u64) -> Option<u64> {
    debug_assert_ne!(span, 0);
    let product = u128::from(sample) * u128::from(span);
    let low = product as u64;
    let threshold = span.wrapping_neg() % span;
    (low >= threshold).then_some((product >> 64) as u64)
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

    #[test]
    fn scoped_rng_sequence_is_preserved() {
        let mut rng = ScopedRng::new(RngSeed::new(42), "level/tunnel");
        let values: Vec<u64> = (0..6).map(|_| rng.next_u64()).collect();
        assert_eq!(
            values,
            vec![
                13_308_457_129_731_648_163,
                2_232_188_147_979_576_733,
                7_550_675_432_941_175_364,
                6_414_595_564_799_098_339,
                6_771_639_045_484_950_311,
                14_345_072_847_349_944_858,
            ]
        );
    }

    #[test]
    fn keyed_v1_vectors_are_stable() {
        let vectors = [
            (
                RngSeed::new(7),
                "world/event",
                b"alpha".as_slice(),
                -9,
                9,
                1,
            ),
            (
                RngSeed::new(7),
                "world/event",
                b"alpha\0tail".as_slice(),
                -9,
                9,
                -9,
            ),
            (
                RngSeed::new(0x0123_4567_89ab_cdef),
                "procedural/placement",
                b"\x10\x20\x30\x40\x50",
                -4_000_000_000,
                4_000_000_000,
                1_625_475_134,
            ),
            (
                RngSeed::new(99),
                "full-range",
                b"all-bits",
                i64::MIN,
                i64::MAX,
                6_514_469_275_967_966_983,
            ),
        ];

        for (seed, scope, key, minimum, maximum, expected) in vectors {
            let value = KeyedRngV1::draw_i64_inclusive(seed, scope, key, minimum, maximum)
                .expect("golden vector input is valid");
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn keyed_v1_is_stateless_and_hashes_complete_keys() {
        let seed = RngSeed::new(23);
        let scope = "procedural/placement";
        let first = KeyedRngV1::draw_i64_inclusive(seed, scope, b"\x00\x01\x02\x03/a", 0, i64::MAX)
            .unwrap();
        let repeat =
            KeyedRngV1::draw_i64_inclusive(seed, scope, b"\x00\x01\x02\x03/a", 0, i64::MAX)
                .unwrap();
        let changed_tail =
            KeyedRngV1::draw_i64_inclusive(seed, scope, b"\x00\x01\x02\x03/b", 0, i64::MAX)
                .unwrap();

        let mut stream = ScopedRng::new(seed, "unrelated");
        let _ = (0..16).map(|_| stream.next_u64()).collect::<Vec<_>>();
        let after_stream_advance =
            KeyedRngV1::draw_i64_inclusive(seed, scope, b"\x00\x01\x02\x03/a", 0, i64::MAX)
                .unwrap();

        assert_eq!(first, repeat);
        assert_eq!(first, after_stream_advance);
        assert_ne!(first, changed_tail);
        assert_ne!(
            hash_keyed_v1_input(seed, b"scope", b"key"),
            hash_keyed_v1_input(seed, b"key", b"scope")
        );
    }

    #[test]
    fn keyed_v1_supports_inclusive_and_full_i64_ranges() {
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(RngSeed::new(1), "fixed", b"key", -7, -7),
            Ok(-7)
        );

        let cross_zero =
            KeyedRngV1::draw_i64_inclusive(RngSeed::new(2), "cross-zero", b"key", -3, 3).unwrap();
        assert!((-3..=3).contains(&cross_zero));

        let full = KeyedRngV1::draw_i64_inclusive(
            RngSeed::new(3),
            "full-range",
            b"key",
            i64::MIN,
            i64::MAX,
        )
        .unwrap();
        assert!((i64::MIN..=i64::MAX).contains(&full));
    }

    #[test]
    fn keyed_v1_rejects_the_uneven_mapping_tail() {
        // 2^64 leaves one extra source value when divided into three buckets.
        assert_eq!(map_sample_unbiased_v1(3, 0), None);
        assert_eq!(map_sample_unbiased_v1(3, 1), Some(0));
        assert_eq!(map_sample_unbiased_v1(3, u64::MAX), Some(2));
    }

    #[test]
    fn keyed_v1_reports_typed_input_errors() {
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(RngSeed::new(1), "", b"key", 0, 1),
            Err(KeyedRngError::EmptyScope)
        );
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(RngSeed::new(1), "scope", b"", 0, 1),
            Err(KeyedRngError::EmptyKey)
        );
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(RngSeed::new(1), "scope", b"key", 2, 1),
            Err(KeyedRngError::InvalidRange {
                minimum: 2,
                maximum: 1,
            })
        );
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(
                RngSeed::new(1),
                &"s".repeat(MAX_KEYED_RNG_SCOPE_BYTES + 1),
                b"key",
                0,
                1,
            ),
            Err(KeyedRngError::ScopeQuotaExceeded {
                actual: MAX_KEYED_RNG_SCOPE_BYTES + 1,
                maximum: MAX_KEYED_RNG_SCOPE_BYTES,
            })
        );
        assert_eq!(
            KeyedRngV1::draw_i64_inclusive(
                RngSeed::new(1),
                "scope",
                &vec![0; MAX_KEYED_RNG_KEY_BYTES + 1],
                0,
                1,
            ),
            Err(KeyedRngError::KeyQuotaExceeded {
                actual: MAX_KEYED_RNG_KEY_BYTES + 1,
                maximum: MAX_KEYED_RNG_KEY_BYTES,
            })
        );
    }
}
