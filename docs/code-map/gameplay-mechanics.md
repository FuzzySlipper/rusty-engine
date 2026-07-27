# Gameplay mechanics

## Purpose

Route work involving reusable component-backed stats, tracks, attributed
sources, explicit effects, inventory data, unique items, equipment, damage,
restoration, and their bounded receipts.

## Owns

- `gameplay-mechanics`: stable typed mechanics identities, checked scalar and
  exact-ratio arithmetic, immutable catalog admission, independently registered
  durable components, direct named services, strict reconstruction, and typed
  operation evidence.
- Deterministic source ordering and applied/suppressed/inapplicable decisions.
- Exact-slot mutation and failure non-mutation inside each advertised operation.

## Does not own

- Attacks, hit tests, targets, turns, reactions, ticks, AI, effect scheduling,
  game-specific item behavior, complete saves, input, or presentation.
- Entity lifecycle or containment authority; those remain in `entity-state`.
- A mechanics world/store, ECS scheduler, gameplay AST, callback registry,
  ambient event bus, universal transaction, or replay/session layer.

## Primary paths

- [`gameplay-mechanics/src/lib.rs`](../../rust/crates/gameplay-mechanics/src/lib.rs)
- [`gameplay-mechanics/src/catalog.rs`](../../rust/crates/gameplay-mechanics/src/catalog.rs)
- [`gameplay-mechanics/src/component.rs`](../../rust/crates/gameplay-mechanics/src/component.rs)
- [`gameplay-mechanics/src/source.rs`](../../rust/crates/gameplay-mechanics/src/source.rs)
- [`gameplay-mechanics/src/stat.rs`](../../rust/crates/gameplay-mechanics/src/stat.rs)
- [`gameplay-mechanics/src/track.rs`](../../rust/crates/gameplay-mechanics/src/track.rs)
- [`gameplay-mechanics/src/damage.rs`](../../rust/crates/gameplay-mechanics/src/damage.rs)
- [`gameplay-mechanics/src/item.rs`](../../rust/crates/gameplay-mechanics/src/item.rs)
- [`gameplay-mechanics/src/snapshot.rs`](../../rust/crates/gameplay-mechanics/src/snapshot.rs)
- [`gameplay-mechanics/tests/contract.rs`](../../rust/crates/gameplay-mechanics/tests/contract.rs)
- [Canonical design](../design.md)

## Public composition

Create and admit one `MechanicsCatalog`, add the gameplay registrations to the
same `ComponentRegistry` used to construct `EntityState`, attach only the
components an entity needs, and invoke named services directly:

```rust
let registry = gameplay_component_registry()?;
let mut state = EntityState::from_definitions_with_registry(registry, definitions)?;

let preview = DamageService::preview(&state, &catalog, &request)?;
let receipt = DamageService::apply(&mut state, &catalog, request)?;
```

The catalog version is downstream compatibility policy. Its SHA-256 fingerprint
is diagnostic/cache identity, not an automatic balance-change lock. Component
snapshots persist referenced IDs and the version; `decode_snapshot_with_catalog`
rejects unresolved references before returning state. Consumers with additional
registered component families pass their complete registry to
`decode_snapshot_with_catalog_and_registry`.

`SourceDefinitionId` identifies authored behavior. `SourceInstanceIdentity`
identifies one live intrinsic, effect, equipped-item, or request activation.
Priority and typed instance identity provide canonical ordering. Receipts expose
every Engine decision and the exact component revisions read.

## Mutation and atomicity

- Stat evaluation and damage preview are read-only.
- Track mutation and complete multipart damage each publish one exact
  `TracksComponent` slot. All protection and target tracks are co-located there,
  so a rejected late part publishes nothing.
- Equipment changes publish one exact `EquipmentComponent` slot.
- Unique ownership is canonical containment. Transfer rejects an equipped item;
  callers explicitly unequip, then transfer. Both intermediate states are
  valid.
- Before removing an effect/source that lowers a derived maximum, reconcile the
  track to the prospective maximum, then publish the effect/source change. A
  failed second step leaves a lower but valid current value.

Do not route these operations through `EntityCommandBatch`, clone complete
`EntityState`, expose mutable component references, or add a private entity map.
If a future invariant cannot be split into valid states, route a narrowly
specified atomic seam to the `entity-state` owner.

## Frozen bounds and ordering

- Scalar magnitude: `1_000_000_000_000`.
- Ratio numerator/denominator: at most `1_000_000`, denominator nonzero,
  normalized at construction and decode.
- Damage parts: 8.
- Request-local sources: 32.
- Receipt response decisions: 256.
- Source bindings/effects: 64 each; equipment assignments: 32; inventory
  stacks: 128.
- Damage order: prevention, flat reduction, combined exact scale and one
  toward-zero rounding, ordered absorption, target application.

The base path does exact component-slot checks but visits zero entries for
absent intrinsic, effect, inventory, equipment, or item state. Operations do
not scan the entity population.

## Acceptance gates

```bash
cargo test -p gameplay-mechanics --locked
cargo test -p entity-state --locked
cargo clippy -p gameplay-mechanics -p entity-state --all-targets --locked -- -D warnings
./scripts/check-doc-links.sh
./scripts/check-asha-equivalence.sh --final
./scripts/verify.sh
```

## Common agent mistakes

- Storing final derived stats instead of admitted base values plus sources.
- Treating tracks as negative stats or calling damage a restoration operation.
- Reusing a preview after a reaction instead of submitting a fresh apply.
- Adding an item-instance namespace or unique-item list beside `EntityId` and
  containment.
- Hiding effect timing or callbacks inside a component.
- Turning detailed receipts into an ambient journal, event bus, or replay
  requirement.

## Follow-up routing

- Entity component registration, lifecycle, exact revisions, containment, and
  snapshot substrate:
  [Entity state and state machines](entity-state-and-state-machines.md).
- Spatial hit/overlap/occlusion mechanisms:
  [Spatial mechanisms](spatial-mechanisms.md).
- Product-specific mechanics meaning and orchestration: downstream consumer.
