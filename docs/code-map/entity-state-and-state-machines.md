# Entity state and state machines

## Purpose

Route work involving reusable entity identity, capabilities, transforms,
relationships, activation, snapshots, and explicit state-machine instances.

## Owns

- `entity-state`: entity definitions and instances, typed capability values,
  relationship facts, transforms, activation, views, snapshots, and atomic
  capability mutation.
- `state-machine`: named state-machine definitions and instance transitions
  over entity identifiers.
- Validation and typed errors at those mutation boundaries.

## Does not own

- Ordinary service-owned spatial, collision, navigation, asset, or presentation
  state.
- A universal command bus, gameplay behavior graph, scheduler, or replay
  runtime.
- Game-specific rules or orchestration.

## Primary paths

- [`entity-state/src/lib.rs`](../../rust/crates/entity-state/src/lib.rs)
- [`entity-state/src/command.rs`](../../rust/crates/entity-state/src/command.rs)
- [`entity-state/src/capability.rs`](../../rust/crates/entity-state/src/capability.rs)
- [`entity-state/src/relationship.rs`](../../rust/crates/entity-state/src/relationship.rs)
- [`entity-state/src/snapshot.rs`](../../rust/crates/entity-state/src/snapshot.rs)
- [`state-machine/src/lib.rs`](../../rust/crates/state-machine/src/lib.rs)
- [Canonical design](../design.md)

## Public downstream surfaces

- The crate roots re-export the supported data, mutation, snapshot, and
  transition vocabulary.
- Downstream Rust composes these mechanisms directly with named game services.
- Serialized entity snapshots are durable facts only where the owning consumer
  chooses to persist them.

## Private or forbidden paths

- Do not route every service mutation through `entity-state` command batches.
- Do not add callbacks, subscriptions, renderer handles, I/O, or service
  location to components or capability data.
- Do not persist closures or arbitrary executable behavior in state-machine
  definitions.

## Acceptance gates and fixtures

```bash
cargo test -p entity-state -p state-machine --locked
cargo clippy -p entity-state -p state-machine --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Focused regressions live under
[`entity-state/tests`](../../rust/crates/entity-state/tests) and
[`state-machine/tests`](../../rust/crates/state-machine/tests).

## Common agent mistakes

- Treating a capability as an implicit system callback.
- Adding a global registry when direct composition is sufficient.
- Confusing an entity snapshot with a universal game save or replay record.
- Moving service-owned state into the entity command batch for convenience.

## Follow-up routing

- Spatial facts and movement: [Spatial mechanisms](spatial-mechanisms.md).
- Durable scenes and entity persistence:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Presentation-only state:
  [Rust render model and projection](rust-render-model-and-projection.md).

