# Continuous gameplay mechanics

## Purpose

Route opt-in persisted finite-binary64 gameplay stats, tracks, attributed sources, and explicit
effects. This is a separate numeric family; it neither changes nor reinterprets exact mechanics.

## Owns

- `gameplay-continuous-mechanics`: separately typed identities, independent catalog version and
  fingerprint, normalized-bit component codecs, component data, direct stat/track/effect services,
  receipts, strict catalog validation, and combined-registry snapshot composition.
- Deterministic ordering of definitions, source activations, contribution decisions, and bit-level
  value identity.
- Exact component-slot stale guards and one-component fail-atomic publication.

## Does not own

- `ContinuousValue`, continuous expressions, or exact/continuous conversion policy; those remain
  in `gameplay-standard`.
- Existing exact mechanics IDs, `MechanicsCatalog`, `MechanicsComponentKind::ALL`, codecs,
  fingerprints, snapshots, or services.
- Rate integration, residual carry, cadence, intervals, cap ordering, clocks, scheduling, units,
  attacks, damage semantics, persistence aggregates, or presentation.

## Primary paths

- [`gameplay-continuous-mechanics/src/lib.rs`](../../rust/crates/gameplay-continuous-mechanics/src/lib.rs)
- [`catalog.rs`](../../rust/crates/gameplay-continuous-mechanics/src/catalog.rs)
- [`component.rs`](../../rust/crates/gameplay-continuous-mechanics/src/component.rs)
- [`service.rs`](../../rust/crates/gameplay-continuous-mechanics/src/service.rs)
- [`snapshot.rs`](../../rust/crates/gameplay-continuous-mechanics/src/snapshot.rs)
- [`engine-inspector/src/continuous_mechanics.rs`](../../rust/crates/engine-inspector/src/continuous_mechanics.rs)
- [Continuous cadence and residual experiment](../topics/gameplay/continuous-cadence-experiment.md)

## Public composition

Use `continuous_mechanics_component_registry` for a continuous-only entity state. Use
`combined_gameplay_component_registry` only when one `EntityState` intentionally carries both
the frozen exact components and this continuous family. The catalogs are admitted, fingerprinted,
and validated independently; composition does not create a generic numeric store or a second
entity authority.

Continuous values enter public data only through `ContinuousValue`, which is finite and
normalizes negative zero. The crate-owned component and catalog serde adapter rejects persisted
negative zero, non-finite values, uppercase, and noncanonical widths; it emits a 16-digit lowercase
hexadecimal bit string, preserving every finite binary64 bit without unsafe JSON numbers. Inspection is separately
labelled `continuous-binary64` and exposes bits rather than approximate decimal identity.

`ContinuousTrackService::{spend,restore,set}` own direct persisted resource mutation. They do not
integrate a rate: a caller owns rate/residual/cadence policy and submits the resulting value through
one named operation. `ContinuousEffectService` owns explicit apply/remove only; downstream owns
effect timing and consequences.

## Focused verification

```bash
cargo test -p gameplay-continuous-mechanics --locked
cargo clippy -p gameplay-continuous-mechanics -p engine-inspector --all-targets --locked -- -D warnings
cargo test -p engine-inspector --locked
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```
