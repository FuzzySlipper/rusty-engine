//! Deterministic tick and interval primitives for Rusty Engine simulation.
//!
//! [`Tick`] is a monotonic, zero-based simulation counter; [`TickDelta`] is a
//! non-negative difference between ticks; and [`TickInterval`] describes a
//! recurring simulation cadence. Arithmetic saturates instead of wrapping so
//! overflow cannot silently corrupt ordering.
//!
//! This crate deliberately owns time values only. Wall-clock time, real-time
//! measurement, scheduling policy, and gameplay rules belong to their owning
//! runtime services.

#![forbid(unsafe_code)]

/// A monotonic, zero-based simulation tick counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    /// The next tick. Saturates at `u64::MAX`.
    pub fn next(self) -> Tick {
        Tick(self.0.saturating_add(1))
    }

    /// This tick advanced by `delta`. Saturates at `u64::MAX`.
    pub fn advance(self, delta: TickDelta) -> Tick {
        Tick(self.0.saturating_add(delta.0))
    }

    /// Ticks elapsed since `earlier`. Saturates at zero if `earlier` is later.
    pub fn since(self, earlier: Tick) -> TickDelta {
        TickDelta(self.0.saturating_sub(earlier.0))
    }
}

/// A non-negative number of ticks between two [`Tick`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TickDelta(u64);

impl TickDelta {
    pub const ZERO: TickDelta = TickDelta(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// A recurring cadence: "fires every `period` ticks", at ticks
/// `0, period, 2·period, …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickInterval {
    period: u64,
}

impl TickInterval {
    /// An interval that fires every `period` ticks. `period` is clamped to at
    /// least 1 (a zero period would otherwise fire on every tick).
    pub const fn every(period: u64) -> Self {
        Self {
            period: if period == 0 { 1 } else { period },
        }
    }

    pub const fn period(self) -> u64 {
        self.period
    }

    /// Whether the interval fires on `tick` (tick 0 fires, then every `period`).
    pub fn fires_at(self, tick: Tick) -> bool {
        tick.raw().is_multiple_of(self.period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_next_and_advance() {
        assert_eq!(Tick::ZERO.next(), Tick::new(1));
        assert_eq!(Tick::new(5).advance(TickDelta::new(3)), Tick::new(8));
        // Saturating, never wrapping.
        assert_eq!(Tick::new(u64::MAX).next(), Tick::new(u64::MAX));
    }

    #[test]
    fn tick_since_saturates_at_zero() {
        assert_eq!(Tick::new(10).since(Tick::new(4)), TickDelta::new(6));
        assert_eq!(Tick::new(4).since(Tick::new(10)), TickDelta::ZERO);
    }

    #[test]
    fn ticks_order_naturally() {
        assert!(Tick::new(1) < Tick::new(2));
        let mut ticks = vec![Tick::new(3), Tick::new(1), Tick::new(2)];
        ticks.sort();
        assert_eq!(ticks, vec![Tick::new(1), Tick::new(2), Tick::new(3)]);
    }

    #[test]
    fn interval_fires_on_multiples_of_period() {
        let every3 = TickInterval::every(3);
        assert_eq!(every3.period(), 3);
        let firing: Vec<u64> = (0..10).filter(|&t| every3.fires_at(Tick::new(t))).collect();
        assert_eq!(firing, vec![0, 3, 6, 9]);
    }

    #[test]
    fn interval_period_is_clamped_to_one() {
        let every = TickInterval::every(0);
        assert_eq!(every.period(), 1);
        assert!(every.fires_at(Tick::new(7)));
    }
}
