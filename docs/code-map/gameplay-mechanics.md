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
- Studio classifies this crate as `non-studio`; workspace-owner enumeration is
  governance evidence only and introduces no Studio API or dependency.

## Primary paths

- [`gameplay-mechanics/src/lib.rs`](../../rust/crates/gameplay-mechanics/src/lib.rs)
- [`gameplay-mechanics/src/catalog.rs`](../../rust/crates/gameplay-mechanics/src/catalog.rs)
- [`gameplay-mechanics/src/component.rs`](../../rust/crates/gameplay-mechanics/src/component.rs)
- [`gameplay-mechanics/src/effect.rs`](../../rust/crates/gameplay-mechanics/src/effect.rs)
- [`gameplay-mechanics/src/source.rs`](../../rust/crates/gameplay-mechanics/src/source.rs)
- [`gameplay-mechanics/src/stat.rs`](../../rust/crates/gameplay-mechanics/src/stat.rs)
- [`gameplay-mechanics/src/track.rs`](../../rust/crates/gameplay-mechanics/src/track.rs)
- [`gameplay-mechanics/src/view.rs`](../../rust/crates/gameplay-mechanics/src/view.rs)
- [`gameplay-continuous-mechanics`](../../rust/crates/gameplay-continuous-mechanics/src/lib.rs)
- [`gameplay-mechanics/src/damage.rs`](../../rust/crates/gameplay-mechanics/src/damage.rs)
- [`gameplay-mechanics/src/item.rs`](../../rust/crates/gameplay-mechanics/src/item.rs)
- [`gameplay-mechanics/src/snapshot.rs`](../../rust/crates/gameplay-mechanics/src/snapshot.rs)
- [`gameplay-mechanics/tests/contract.rs`](../../rust/crates/gameplay-mechanics/tests/contract.rs)
- [`gameplay-mechanics/tests/gm1.rs`](../../rust/crates/gameplay-mechanics/tests/gm1.rs)
- [`gameplay-mechanics/tests/gm2.rs`](../../rust/crates/gameplay-mechanics/tests/gm2.rs)
- [`gameplay-mechanics/tests/gm3.rs`](../../rust/crates/gameplay-mechanics/tests/gm3.rs)
- [`gameplay-mechanics/tests/gm4.rs`](../../rust/crates/gameplay-mechanics/tests/gm4.rs)
- [`gameplay-mechanics/tests/gm5.rs`](../../rust/crates/gameplay-mechanics/tests/gm5.rs)
- [`gameplay-mechanics/tests/gm7_builder.rs`](../../rust/crates/gameplay-mechanics/tests/gm7_builder.rs)
- [`gameplay-mechanics/examples/compositions.rs`](../../rust/crates/gameplay-mechanics/examples/compositions.rs)
- [`builder cost evidence`](../../fixtures/gameplay-mechanics/builder-evidence-v1.json)
- [Completed campaign evidence](../gameplay-mechanics-campaign-closeout.md)
- [`engine-inspector/src/mechanics.rs`](../../rust/crates/engine-inspector/src/mechanics.rs)
- [`gameplay-mechanics donor disposition`](../../migration/gameplay-mechanics-donor/disposition.tsv)
- [Canonical design](../design.md)
- [Upstream promotion and authoring DSL](../topics/development/upstream-promotion-and-authoring-dsl.md)

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

The GM1 stat/track path is equally direct:

```rust
let evaluated = StatService::evaluate(
    &state,
    &catalog,
    player,
    &StatId::parse("max_health")?,
    &OperationId::parse("inspect_health")?,
    &[],
)?;
let mechanics = MechanicsEntityView::read(&state, player)?;
let health = mechanics
    .tracks()
    .and_then(|view| {
        view.values()
            .iter()
            .find(|value| value.track().as_str() == "health")
    });
```

## Continuous companion

[`gameplay-continuous-mechanics`](../../rust/crates/gameplay-continuous-mechanics/src/lib.rs)
is an optional, separately typed companion above `gameplay-standard` and the exact mechanics
crate. It owns finite-normalized-binary64 stat/track/source/effect facts, its independent catalog
and fingerprint, bit codecs, direct services, receipts, and opt-in combined registry/snapshot
helpers. It does not mutate the seven-kind exact component table, exact catalog, existing codecs,
or exact service behavior. Its rate, residual, cadence, interval, cap ordering, clock, and unit
semantics remain caller-owned as established by the continuous cadence experiment.

Use `combined_gameplay_component_registry` only when one `EntityState` deliberately carries both
families; each catalog validates independently. `engine-inspector` reports its continuous fields
under a separate `continuous-binary64` numeric family with normalized bits, never approximate
numeric identity.

Effect ownership is also a direct call boundary. A downstream realtime tick,
city phase, or tabletop turn owner decides when an effect expires and submits
the exact instance operation; Engine stores no timing state:

```rust
let applied = EffectService::apply(&mut state, &catalog, apply_request)?;
let expired = EffectService::expire(&mut state, &catalog, expire_request)?;
```

`EffectDefinition` admits one stacking group and policy, a maximum stack count,
and a non-empty bounded source list. `IndependentByProvenance` permits bounded
coexistence with unique typed provenance; `Refresh` updates one exact existing
instance; `Replace` removes the occupied policy group and inserts one exact new
instance. Each stack expands into a separately attributed source activation.
The regular contribution/response stacking policy—not the effect lifecycle
service—then decides which numeric or damage candidates apply.

Inventory and equipment remain direct named services as well:

```rust
let inventory = InventoryService::view(&state, &catalog, owner)?;
let transfer = InventoryService::transfer(&mut state, &catalog, stack_transfer)?;
let equipped = EquipmentService::equip(&mut state, &catalog, equip_request)?;
```

Fungible quantities have one canonical stack per item definition; grant,
consume, and transfer do not mint stack identities. Unique items use
`EntityId` plus canonical containment. `InventoryView` joins the owner's
component with the maintained direct-child containment index and reports
capacity/traversal evidence without scanning unrelated entities. An item is an
ordinary entity and may carry tracks, effects, or downstream components.

When a product has already selected an exact entity definition/ID, admitted
unique definition, and owner, `ItemService::materialize_unique` stages entity
admission, catalog-derived `ItemComponent` attachment, and that explicit
containment on a cloned candidate. It publishes all three or none and reports
the actual intermediate revisions. It deliberately does not allocate IDs,
choose names, enforce inventory policy, equip, or become a spawn/loadout
factory; those remain downstream choices.

Equipment assignments are slot references, not ownership. A multi-slot item
appears once per required slot, but its sources activate once with
`EquippedItem` provenance. Classification, exact slot count, containment,
exclusivity, source quota, and target capacity failures are checked before
publication. Transfer of a unique item deliberately rejects while equipped;
the caller performs an exact unequip, then submits the containment transfer.

The catalog version is downstream compatibility policy. Its SHA-256 fingerprint
is computed from canonical admitted definitions without the version and is
diagnostic/cache identity, not an automatic balance-change lock. Component
snapshots persist referenced IDs and the version; `decode_snapshot_with_catalog`
rejects unresolved references before returning state. Consumers with additional
registered component families pass their complete registry to
`decode_snapshot_with_catalog_and_registry`.

`MechanicsComponentKind::ALL` is the stable seven-kind inspection order. Each
kind exposes its durable type identity, codec identity, and codec version.
Current versions are:

| Component | Type identity | Codec | Version |
| --- | --- | --- | ---: |
| Stats | `rusty.mechanics.stats` | `rusty.mechanics.stats-json` | 1 |
| Tracks | `rusty.mechanics.tracks` | `rusty.mechanics.tracks-json` | 1 |
| Intrinsic sources | `rusty.mechanics.intrinsic-sources` | `rusty.mechanics.intrinsic-sources-json` | 1 |
| Active effects | `rusty.mechanics.active-effects` | `rusty.mechanics.active-effects-json` | 2 |
| Inventory | `rusty.mechanics.inventory` | `rusty.mechanics.inventory-json` | 2 |
| Unique item | `rusty.mechanics.item` | `rusty.mechanics.item-json` | 1 |
| Equipment | `rusty.mechanics.equipment` | `rusty.mechanics.equipment-json` | 1 |

Registration alone never serializes an arbitrary Rust type. Runtime-only
registrations are omitted; durable extension values require the exact explicit
codec above. Component revisions are instance-local and are reacquired after
restore rather than persisted.

`SourceDefinitionId` identifies authored behavior. `SourceInstanceIdentity`
identifies one live intrinsic, effect, equipped-item, or request activation.
Priority and typed instance identity provide canonical ordering. Receipts expose
every Engine decision and the exact component revisions read.

## Immutable inspection

`MechanicsEntityView` exposes borrowed raw component views. The
`engine-inspector` leaf adds `inspect_mechanics_entity_structural`, which reads stored facts
without evaluating, activating sources, or joining inventory. A product that already owns those
derived readouts may pass them to `inspect_mechanics_entity_from_evidence` to assemble the
compatible enriched shape; inspection never invokes the owner services itself. This preserves
the evidence boundary while keeping the old diagnostic fields available from explicit receipts.
Snapshot callers choose `inspect_mechanics_snapshot_structural_json_v2` for the explicit
`storedStats`/`storedTracks` report or `inspect_mechanics_snapshot_json_v1_from_evidence` for
the former enriched wire shape with caller-supplied evidence.
The companion
`inspect_damage_receipt` projects every bounded part, decision, track change,
fact, and source-cost field from the operation-local receipt.

For a strict stored artifact, the CLI requires both authorities explicitly:

```bash
cargo run -p engine-inspector --bin rusty-inspect -- \
  mechanics entity-state.json mechanics-catalog.json 42
```

The command admits the supplied catalog, reconstructs all required component
extensions through the canonical registry, validates every catalog reference,
then projects one entity. It does not infer a catalog, mutate the state, load a
product schema, or render anything.

## Mutation and atomicity

- Stat evaluation and damage preview are read-only.
- Stat evaluation selects contributions canonically, applies additions, combines
  exact scales with one toward-zero rounding step, then resolves minimum and
  maximum constraints. Its bounded decisions explain every Engine choice.
- `StatService::set_base` publishes one exact `StatsComponent` replacement and
  rejects a prospective value that would leave a stat-bounded track invalid.
- Track mutation and complete multipart damage each publish one exact
  `TracksComponent` slot. All protection and target tracks are co-located there,
  so a rejected late part publishes nothing.
- Track setting makes `RejectOutOfBounds` versus `ClampToBounds` explicit.
  Maximum reconciliation makes `PreserveCurrent` versus `ClampToMaximum`
  explicit; ratio preservation remains intentionally absent.
- Equipment changes validate every present stat-bounded track against the
  prospective equipment source set, then publish one exact
  `EquipmentComponent` slot. A lowering change that would strand a track
  reports `EquipmentWouldInvalidateTrack` without mutation; accepted receipts
  expose the bounded track/source validation work.
- Fungible grant/consume publish one exact `InventoryComponent`. Transfer
  validates both candidates, then uses the bounded homogeneous `entity-state`
  replacement seam to publish both inventories in one global revision.
- Unique-item transfer changes only canonical containment after checking both
  inventory capacity projections and explicit inventory revision guards.
- Item destruction rejects equipped references; accepted item destruction and
  owner destruction use `entity-state` lifecycle cleanup so containment and
  equipment cannot dangle.
- Effect apply, refresh, replace, remove, and expire each validate a complete
  prospective `ActiveEffectsComponent` and publish that one exact slot. Receipts
  identify removed/current instances, activated sources, revisions, and bounded
  traversal cost.
- Unique ownership is canonical containment. Transfer rejects an equipped item;
  callers explicitly unequip, then transfer. Both intermediate states are
  valid.
- Before removing an effect or equipped source that lowers a derived maximum,
  reconcile the track to the prospective maximum, then publish the effect or
  equipment change. A rejected operation reports that bound without mutation;
  a failed retry leaves the explicitly lowered but valid current value.

The active-effects durable codec is version 2. It stores only catalog version,
effect instance/definition identity, typed provenance, and stacks. Timers,
timestamps, durations, turns, callbacks, and scheduler state belong to the
downstream owner and reconnect through the stable effect instance identity.

The inventory durable codec is version 2. It stores catalog version, canonical
fungible stacks, and caller-authored capacity limits. Item and equipment
components store only stable definition/slot/entity references. Strict
reconstruction validates item definitions before joined inventory and
equipment invariants.

## Damage and restoration

`DamageService` owns one fixed, auditable pipeline: prevention, selected flat
reduction, combined exact scaling with one toward-zero rounding step, canonical
protection-track absorption, and target-track application. Each part receipt
records every numeric stage, the rounding policy, absorbed/applied/unapplied
amounts, and the applied/suppressed/inapplicable decision for every collected
source response. Protection and target depletion are typed facts for the
downstream owner to interpret.

Preview stages the complete multipart result without publishing it. Apply runs
a fresh preview against current state and replaces the exact `TracksComponent`
slot once; it never commits a retained preview. Flat reductions aggregate in a
checked wider intermediate before clamping at zero. Absorption stops once no
damage remains and treats both depleted protection tracks and later absorbers
as inapplicable. Any stale revision, missing track, invalid current bound,
receipt quota, or arithmetic failure leaves the slot and global revisions
unchanged.

Healing and repair use `TrackService::restore`, returning a separate
`TrackMutationReceipt` under the same derived bounds. A realtime or turn-based
owner that permits a reaction between inspection and resolution explicitly
changes the relevant source/effect state and then submits a fresh damage apply;
Engine stores no pending attack or reaction session.

Do not route these operations through `EntityCommandBatch`, clone complete
`EntityState`, expose mutable component references, or add a private entity map.
If a future invariant cannot be split into valid states, route a narrowly
specified atomic seam to the `entity-state` owner.

## Frozen bounds and ordering

- Scalar magnitude: `1_000_000_000_000`.
- Ratio numerator/denominator: at most `1_000_000`, denominator nonzero,
  normalized at construction and decode.
- Stat additions aggregate through a checked wider intermediate and must fit the
  admitted scalar range before the checked exact scale stage.
- Damage parts: 8.
- Request-local sources: 32.
- Receipt response decisions: 256.
- Sources per effect: 32; stacks per instance: 32; active instances: 64;
  independent instances per stacking group: 64; expanded active-effect sources
  per entity: 256.
- Stats/tracks per entity: 128 each.
- Active-source collection and per-source decision expansion stop at that
  receipt ceiling before cloning an over-limit entry.
- Intrinsic source bindings: 64; equipment assignments: 32; inventory stacks:
  128.
- Homogeneous component replacements per publication: 32; capacity metrics per
  catalog and limits per inventory: 32 each; item capacity costs: 32;
  classifications per item/slot: 16; required slots per item: 8.
- Contained entities visited per inventory projection: 256; equipped item
  source activations: 256; sources per item: 32.
- Item capacity cost units: `1_000_000`; inventory capacity limit units:
  `1_000_000_000_000_000_000`; fungible quantities use the catalog's positive
  per-definition maximum up to the `1_000_000_000` component stack bound.
- Damage order: prevention, flat reduction, combined exact scale and one
  toward-zero rounding, ordered absorption, target application.
- Damage source-cost evidence includes source collection performed while
  resolving stat-derived protection and target bounds.
- Track-room comparisons use a wider intermediate capped by the admitted
  operation amount, so both signed scalar endpoints remain valid track bounds.

The base path does exact component-slot checks but visits zero entries for
absent intrinsic, effect, inventory, equipment, or item state. Operations do
not scan the entity population. Catalog definitions and nested source records
are canonicalized at admission, so reordered authored input produces the same
lookup order and diagnostic fingerprint.

### Public variable-collection limits

Every public variable collection is admitted or produced behind a fixed cap.
Receipt arrays are not caller-managed journals; their limits derive from the
same request/component/catalog caps and are checked before publication.

| Surface | Collection cap and enforcement |
| --- | --- |
| Catalog families | stats 128, tracks 128, sources 256, damage kinds 64, effects 128, items 256, equipment slots 64, capacity metrics 32; `MechanicsCatalog::admit` rejects over-limit input |
| Source/effect definitions | stat contributions 32 and damage responses 32 per source; sources 32 per effect; stacks 32; active instances 64; expanded effect activations 256 |
| Item/slot definitions | sources 32, classifications 16, and capacity costs 32 per item; allowed classifications 16 per slot; required slots 8 |
| Entity components | stats 128, tracks 128, intrinsic bindings 64, effects 64, fungible stacks 128, capacity limits 32, equipment assignments 32 |
| Requests | damage parts 8, request-local sources 32, requested equipment slots 32; duplicate source/slot identities reject before mutation |
| Stat/damage evidence | stat decisions 256; damage response decisions 256, parts 8, facts 128, and distinct touched tracks at most the 128 admitted catalog tracks |
| Effect/equipment evidence | removed effects 64, activated effect sources 256, equipment changes and observed item revisions 32, equipped source activations 256 |
| Inventory evidence | contained direct children 256, unique item projection 256, capacity arrays 32, and stack arrays 128; traversal counts report the bounded work |
| Revision evidence | at most 32 equipped item slots plus the fixed owner component kinds; entries are sorted and deduplicated before return |
| Inspector projection | exactly 7 presence rows; other arrays retain the component/evaluation/inventory/receipt caps above |

The GM5 measurement adds 2,048 unrelated entities around a one-stat/one-track
target and still reports zero intrinsic, effect, equipment, item, and
request-source visits for both stat evaluation and one-part damage. Services
clone only the component candidate they promise to replace (or the two
homogeneous inventory candidates for transfer), never complete `EntityState`.

### Infrastructure falsification fixture

`gm7_builder` composes the public APIs into one deliberately small
infrastructure-shaped probe. Test-local authored identities give one entity a
production stat and stat-bounded durability track. Contained module entities
become attributed sources only through canonical equipment, a temporary
improvement becomes an attributed effect source, and impact/repair use
`DamageService` and `TrackService`. A test-local day/phase value decides when
to call `EffectService::expire`; neither time nor product vocabulary enters an
Engine type.

The fixture covers stale track and relationship revisions, uncontained and
non-equippable items, track and damage-part bounds, a late multipart damage
failure, a rejected negative restore, effect/equipment prospective bound
rejection, explicit reconciliation, snapshot/reopen, identical semantic
evaluation, and continued mutation. Whole strict snapshots plus every
registered mechanics slot revision and the global relationship revision prove
that every rejection leaves components and relationships unchanged.

The checked evidence compares one installed module with eight. Production
evaluation grows from two to nine decisions, equipment visits and item reads
grow from one to eight, and one effect entry/source activation remains one.
The canonical snapshots are 3,484 and 8,595 bytes. Public APIs expose bounded
visits and canonical bytes, not allocator or component-clone counters, so the
fixture records no invented allocation/clone number.

This is a headless regression and falsification fixture. It is not an external
consumer, live product proof, a rules-support consumer, or a promotion vote for
`gameplay-rules`.

## Acceptance gates

```bash
cargo test -p gameplay-mechanics --locked
cargo test -p gameplay-mechanics --test gm7_builder --locked
cargo test -p entity-state --locked
cargo test -p engine-inspector --locked
cargo run -p gameplay-mechanics --example compositions
cargo clippy -p gameplay-mechanics -p entity-state -p engine-inspector --all-targets --locked -- -D warnings
./scripts/check-doc-links.sh
./scripts/check-asha-equivalence.sh --final
./scripts/check-gameplay-mechanics-donor-disposition.sh
./scripts/verify.sh
```

Ordinary mechanics-only changes use the Rust provider gate; the isolated
Studio gate is additional evidence only when a Studio-owned boundary changes.

## Common agent mistakes

- Reimplementing local vitality, stat, inventory, equipment, damage, or
  restoration stores because a small game uses only part of the established
  mechanics surface.
- Storing final derived stats instead of admitted base values plus sources.
- Treating tracks as negative stats or calling damage a restoration operation.
- Reusing a preview after a reaction instead of submitting a fresh apply.
- Treating repair/healing as signed damage instead of a bounded track restore.
- Adding an item-instance namespace or unique-item list beside `EntityId` and
  containment.
- Treating an equipment assignment as ownership, activating a multi-slot
  item's sources once per slot, or transferring it without explicit unequip.
- Defining Engine meanings for mass/power/volume instead of using typed
  caller-named capacity metrics and costs.
- Scanning every entity to build an inventory instead of using the maintained
  direct containment index.
- Adding item behavior callbacks, implicit split/merge lifecycle, or
  stack-instance identities.
- Hiding effect timing or callbacks inside a component.
- Treating stack count as a free-form intensity formula, or making apply
  silently choose refresh/replace behavior for the caller.
- Turning detailed receipts into an ambient journal, event bus, or replay
  requirement.

## Follow-up routing

- Entity component registration, lifecycle, exact revisions, containment, and
  snapshot substrate:
  [Entity state and state machines](entity-state-and-state-machines.md).
- Spatial hit/overlap/occlusion mechanisms:
  [Spatial mechanisms](spatial-mechanisms.md).
- Product-specific mechanics meaning and orchestration: downstream consumer.
- Neutral authoring helpers or missing reusable mechanisms:
  [Upstream promotion and authoring DSL](../topics/development/upstream-promotion-and-authoring-dsl.md).
