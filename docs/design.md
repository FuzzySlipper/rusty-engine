# Rusty Engine design

Status: current provider architecture

Rusty Engine is a standalone, host-neutral mechanism provider for object-centric games. Game policy,
orchestration, game-specific project schemas, product persistence aggregates, input meaning, and the
meaning of presentation intents belong to concrete downstream products. Reusable scene, entity,
asset, voxel-authoring, serialization, and persistence mechanisms may live here when they remain
independent of one game's vocabulary. Shared renderer-neutral projection and renderer host
mechanisms live here so downstream products and Studio cannot drift into
independent renderers. The provider never requires a particular downstream
product as an integration or verification dependency.

Historical product experiments and measurements live in
[experiment-results.md](migration/experiment-results.md). Completed migration and extraction decisions live
in [migration-cluster-ledger.md](migration/migration-cluster-ledger.md).

## Design priorities

- Keep reusable mechanisms concrete, typed, and independently testable.
- Keep game meaning and product policy in the downstream repository that owns them.
- Preserve one authoritative data source when collision, navigation, and meshes are derived from the
  same spatial content.
- Make mutation owners visible through direct services, systems, or atomic command boundaries.
- Keep components and value types mostly data; do not add hidden polling, subscriptions, I/O, or
  service location.
- Keep host-neutral mechanisms, renderer-neutral projection, backend realization, browser/webview
  lifecycle, and product-shell policy in visibly separate owners.
- Maintain a one-way dependency: consumers may depend on Engine; Engine never imports or checks out
  a consumer to verify itself.
- Promote neutral mechanisms through explicit architecture judgment, not a
  consumer-count threshold. One credible downstream proof or concrete need can
  justify Engine ownership when centralization removes parallel authority or
  correctness risk. Surveys and later adopters may improve the seam; they are
  not prerequisites. See
  [upstream promotion and authoring DSL](topics/development/upstream-promotion-and-authoring-dsl.md).

Object-centric does not mean Unity-style component scripts or an ECS scheduler. It means entity
identity and typed component data remain easy to inspect while behavior is owned by explicit code.

## System at a glance

```text
downstream game
  entities / game components / services / scheduling / persistence / presentation intent
                     |
                     +--> entity-state
                    +--> gameplay-mechanics (optional)
                    +--> gameplay-continuous-mechanics (optional)
                     +--> gameplay-rules (optional package support)
                     +--> engine-spatial -------------------+
                     +--> voxel-asset                       |
                     +--> render-model --> render-projection----+
                                      \-> render-presentation---|
                                                          v
foundation: core-assets / core-ids / core-math / core-space / core-time / core-voxel
services:   svc-volume / svc-spatial / svc-collision / svc-pathfinding / svc-rng / svc-mesh

offline only: GLB + request --> voxel-convert --> canonical voxel-asset JSON --> downstream admission

isolated rules workspace: optional downstream TS authoring DSL --> package/wire AST --> gameplay-rules (Rust)
isolated renderer workspace: retained JSON --> render-projection (TS) --> Three backend / host adapters
```

No Engine crate knows the downstream game's component families, event vocabulary, game-specific
stored-project schema, or browser API. The Rust render crates know only renderer-neutral values and
explicit read-only provider views; the isolated renderer workspace knows no gameplay authority.

## Entity component boundary

`entity-state::EntityState` owns reusable entity invariants:

- stable identity, name, lifecycle, labels, and explicit relationships;
- one instance-owned typed component store with stable authored type identities;
- built-in transform, bounds, collision, renderable, kinematic,
  character-motion, controller, rigid-body, and asset-binding components;
- typed component registration, attach/read/has/replace/remove, deterministic per-type iteration,
  bounded inspection, and destruction cleanup;
- read-only entity views, projection nodes, and an identity-ordered reverse containment index; and
- snapshot encoding plus one atomic `EntityCommandBatch` mutation boundary.

Renderable facts carry one explicit renderable-local transform in addition to the entity transform.
The entity transform remains the sole world, gameplay, and spatial authority. Render projection
composes `entity world * renderable local` only when constructing the retained visual instance;
collision, navigation, motion, triggers, gameplay services, and entity relationships never observe
that presentation offset. Identity is the compatibility default and is omitted from older durable
snapshot and authored-scene encodings. Authored scene schema 5 owns non-identity visual-local TRS.

`ComponentRegistry` is a bounded construction input for a particular `EntityState`, not a global
plugin registry. Registration maps one stable `ComponentTypeId` to one Rust type and rejects
duplicate identities, conflicting Rust types, and codec drift before changing the instance. Rust
`TypeId` and type names are internal downcast checks only; they never become durable identity.
Downstream crates may declare and explicitly register their own inert component types without
editing `entity-state` or a central component enum.

Mutation remains service-owned. `EntityAuthoringService` performs typed attach, replacement, and
removal without exposing `&mut T`; `TransformService`, activation, relationships, and
`EntityCommandBatch` retain their narrower invariants and typed receipts. Generic component writes
capture an instance-local `ComponentRevision` for one `(EntityId, ComponentTypeId)` slot. A change
to a different entity or component type therefore does not make that guard stale. Each accepted
slot change advances both its slot revision and the global `EntityState` revision; the global
revision remains the durable/coarse ordering value used by established entity operations and
snapshots. A staged built-in command batch advances the global revision once after all validation
succeeds. Rejection changes neither values, relationships, slot revisions, nor the global revision.
Slot revisions are process-local guards rather than durable facts and must be reacquired after
snapshot restoration.

A service-specific prepared operation captures the revisions of exactly the component slots and
relationship facts it reads, validates its complete candidate before publishing, and reports one
typed service receipt. Ordinary operations stage only the exact component values they own; they do
not clone the complete `EntityState`. A true cross-slot invariant must either use an explicit
valid-state precondition/operation split or first earn the smallest reusable atomic publication seam
at `entity-state`. It must not replace narrow guards with the global revision merely because every
component shares one store, nor turn the store into a generic callback transaction language.
Transform parenting, containment, and source ancestry remain explicit relationship maps rather than
components.

The one promoted cross-slot seam is
`EntityAuthoringService::replace_components<T>`. It accepts at most 32 replacements of one
registered component type, requires unique entity slots and each slot's exact revision, validates
every candidate before writing any of them, then advances the global revision once. It exists for
mechanisms such as a fungible transfer between two inventories; it is not a heterogeneous
transaction, command language, callback route, or permission for unrelated component types to
share an operation owner. Containment also maintains a reverse direct-child index so an inventory
projection can enumerate one owner's contents without scanning the entity population. The forward
relationship remains canonical, and reparenting, snapshot restoration, item destruction, and owner
destruction update both directions together.

The schema-3 snapshot keeps its established built-in JSON fields. A registered downstream type is
either runtime-only and omitted, or durable through an explicit stable codec identity and positive
version. Durable values occupy the omitted-when-empty `registeredComponents` section; restoration
requires the caller to supply the matching instance registry and rejects unknown required kinds,
duplicates, tombstones, bad values, and codec identity/version drift. This is a component
persistence seam, not a universal game save, replay format, or permission to persist callbacks.

The store does not expose archetypes, arbitrary ECS matching, implicit scheduling, component
callbacks, service location, or game-specific state. A downstream service uses the command batch
only when several reusable built-in component changes must commit together. Ordinary game
component mutation and all substantial gameplay behavior remain with the downstream feature owner.

## Gameplay mechanics boundary

The optional `gameplay-mechanics` crate supplies a small common vocabulary for component-backed
stats, mutable tracks, attributed sources, explicit effects, fungible stacks, unique items,
equipment, and damage. It is not a game runtime. Downstream still owns attacks, targeting, turns,
reactions, ticks, effect-expiration timing, complete saves, and the consequences of returned facts.

`MechanicsCatalog` is an immutable admitted set of typed definitions with a downstream compatibility
version and a deterministic diagnostic fingerprint. Live values use seven independently registered
durable components: `StatsComponent`, `TracksComponent`, `IntrinsicSourcesComponent`,
`ActiveEffectsComponent`, `InventoryComponent`, `ItemComponent`, and `EquipmentComponent`.
Definitions and request/receipt values are not components. Components contain inert data and strict
codecs; direct `StatService`, `TrackService`, `EffectService`, `DamageService`,
`InventoryService`, `EquipmentService`, and `ItemService` calls own validation and mutation.

GM0 freezes these mechanics contracts:

- signed stored scalars are checked `i64` values bounded to `+/-1_000_000_000_000`; scale values are
  normalized exact rationals with components at most `1_000_000`, combined exactly, then rounded
  once toward zero;
- a source definition is authored catalog data while `SourceInstanceIdentity` records whether one
  live activation is intrinsic, effect-owned, equipped-item-owned, or request-local. Source
  priority followed by that typed identity is the canonical order;
- source decisions are explicit `Applied`, `Suppressed`, or `Inapplicable` receipt entries;
- damage validates and stages every bounded part before publishing one `TracksComponent`
  replacement. Its fixed order is prevention, flat reduction, combined exact scaling and one
  rounding step, canonical protection-track absorption, then target-track application;
- preview is pure and reports exact component revisions. Apply recomputes from current component
  state instead of committing a retained preview;
- unique stateful items are ordinary `EntityId` values with `ItemComponent`; ownership is the
  canonical containment relationship. Equipment stores references but never a second ownership
  list; and
- lowering a source-derived track maximum uses a visible valid-state split:
  `TrackService::reconcile_to_maximum` first lowers the current value while the old bound remains
  valid, then the source/effect/equipment owner publishes its separate component change. Transfer
  similarly rejects an equipped item until an explicit exact-slot unequip succeeds, then changes
  containment.

GM1 promotes the catalog/source/stat/track slice into the production API. Catalog admission
canonicalizes both definition families and their bounded nested records; its diagnostic fingerprint
covers admitted Engine definitions but deliberately excludes the downstream compatibility version.
Stat evaluation applies selected additions, combines selected exact scales and rounds once toward
zero, then resolves selected minimum/maximum constraints against the admitted stat bounds. The
returned ledger records every applied, suppressed, and inapplicable contribution plus each numeric
stage. The additive aggregate uses a checked wider intermediate and must fit the admitted scalar
range before scaling; ratio products use checked wider intermediates as well.
`StatService::set_base`, track spend/restore, policy-governed set, and maximum reconciliation
stage one component replacement behind the exact slot revision. A base change that would strand a
stat-bounded track is rejected until the caller reconciles that track. Rejection leaves both slot
and global revisions unchanged.

GM2 promotes active effects into an explicit lifecycle without giving Engine a time owner. An
admitted `EffectDefinition` binds one stacking group and policy, a bounded stack count, and one or
more admitted source definitions. `ActiveEffectInstance` stores only stable instance and definition
identity, typed provenance, and current stacks. `IndependentByProvenance`, `Refresh`, and `Replace`
are catalog policy; apply, refresh, replace, remove, and expire remain distinct `EffectService`
calls chosen by the downstream owner. Expire is deliberately removal with a caller-supplied
operation identity—not a timer, turn hook, callback, or implicit update.

Each effect stack activates a separately attributed copy of every bound source, after which the
ordinary source stacking rules determine applied and suppressed stat or damage decisions. An effect
operation validates the complete prospective `ActiveEffectsComponent`, all bounded source
expansion, and every present stat-derived track before publishing one exact component slot. If
removing or replacing an effect would strand a track above its new maximum, the operation returns
`EffectWouldInvalidateTrack` without mutation. The downstream owner explicitly clamps that track
to the reported prospective maximum through `TrackService::reconcile_to_maximum`, then retries the
effect operation. This is the same valid-state split as GM0; no half-committed effect operation or
heterogeneous transaction is introduced.

The active-effects durable codec is version 2 and persists only those authoritative fields.
Downstream duration, timestamp, day/phase, turn, and scheduling state reconnect by stable effect
instance identity after load. Effect receipts expose the exact effect revision, observed component
revisions, source expansion cost, removed/current instances, and activated source identities; they
are operation evidence rather than an ambient event journal.

The optional `gameplay-standard` vocabulary adds no resolution kernel. Its
standard predicate is an exact expression comparison used by the existing
`Program::When`; its first operations are typed plans for track spend/restore,
damage submission, and effect apply/remove. A product passes explicit
capability-role-to-`EntityId` bindings and input values, while planning obtains
a conservative source snapshot of every mechanics slot on participating
entities (including absent slots and currently equipped item slots) and
produces the existing mechanics request unchanged. This intentionally avoids
executing mutations while planning, so sequential private-candidate programs
remain admissible. A downstream product transaction validates that snapshot and catalog before cloning a
private candidate; only then does it rebase the request guard to candidate
state, retain the typed mechanics receipt, and guard and swap its full
candidate exactly once. A
preview follows that same traversal and private staging path before aborting.
This remains product transaction policy: Engine does not construct a
heterogeneous world transaction or infer targets, attack meaning, timing,
consequences, effect expiry, or persistence.

GM3 promotes damage, healing, and repair through the existing direct service boundaries.
`DamageService::preview` is a pure full-pipeline evaluation; `DamageService::apply` recomputes that
evaluation against current components and atomically publishes the staged `TracksComponent`.
Each part ledger discloses the fixed stages and explicit toward-zero rounding policy. Selected flat
reductions aggregate in a checked wider intermediate before clamping at zero, while absorption
visits canonical protection tracks only until the remaining damage is exhausted and marks depleted
or later absorbers inapplicable. A late invalid part, bound, or arithmetic operation leaves every
track and revision untouched. Typed depletion facts and bounded source-traversal evidence are
returned to the downstream owner; receipts are not persisted or broadcast.

Healing and repair deliberately remain `TrackService::restore` operations. They share the same
stat-derived bounds and exact-slot mutation authority as damage without pretending restoration is
negative damage. Realtime reactions and tabletop interrupts are explicit downstream orchestration:
the owner may apply an effect or other state change after preview, then submits a fresh damage
apply. Engine retains no pending hit, reaction window, combat session, event queue, or scheduler.

GM4 promotes bounded inventory, unique-item ownership, equipment, and their source integration.
Catalog item definitions classify an item as fungible or unique, assign caller-named capacity
costs, and optionally declare an exact equipment slot count, classifications, one exclusivity
group, and equipped sources. Capacity metric names have no Engine-defined meaning: one inventory
may limit mass and another may limit power, volume, or a game-specific metric. Costs are positive
bounded integers, quantity/cost accumulation uses checked wider intermediates, and receipts expose
both the resolved usage and bounded traversal work.

`InventoryComponent` stores one canonical stack per fungible definition plus optional capacity
limits. Grant and consume replace one exact inventory slot. Fungible transfer prepares both
inventory candidates and publishes them through the bounded homogeneous component-replacement seam;
a stale revision, insufficient quantity, destination limit, or late candidate failure changes
neither inventory. There is no stack-instance identity, implicit split/merge lifecycle, item
behavior dispatch, or global inventory registry.

A unique item is an ordinary entity with `ItemComponent`. Canonical containment is its ownership
fact, while a joined `InventoryView` reads the owner's stack component and only the maintained
direct containment children. Non-item children do not become inventory entries, and unrelated
entities are never scanned. Because an item is an ordinary entity, it may also carry normal tracks,
active effects, or downstream components without creating a second item-state system.

`EquipmentComponent` stores canonical slot-to-item references, never ownership. Multi-slot
equipment repeats the same item entity across its required slots, while classification,
slot-count, containment, exclusivity, and aggregate source quotas are validated over the complete
candidate before one exact equipment-slot replacement. An equipped item's sources activate once
per item—not once per occupied slot—with `EquippedItem` provenance for both stat and damage
ledgers. Swap is one equipment replacement; transfer remains the GM0 valid-state split of explicit
unequip followed by a capacity-checked containment change. Destroying an equipped item is rejected.
Destroying an unequipped item clears its containment and components, while destroying an owner
removes its equipment component and releases its direct children without leaving references.

Equip, unequip, and swap also evaluate every present stat-bounded track against
the complete prospective equipment source set before publication. A lowering
mutation that would strand a track returns
`EquipmentWouldInvalidateTrack` with the prospective bounds and changes no
component, relationship, or revision. The caller reconciles the track to that
maximum, reacquires the global relationship guard, and retries the exact
equipment operation. Equipment receipts expose the bounded track/source
validation work; no heterogeneous transaction or complete-state clone is
introduced.

The inventory durable codec is version 2 and stores only catalog version, canonical stacks, and
capacity limits. Item and equipment codecs remain strict inert data. Reconstruction validates item
kinds and catalog references before joined inventory/equipment invariants, and immutable component
views plus the joined inventory projection expose exact revisions without providing mutation.

Those splits deliberately avoid an unrestricted heterogeneous transaction and complete-world
cloning. Each intermediate state is valid, and failure of the later step cannot expose an
out-of-bounds track or an equipped item owned elsewhere. A future operation that cannot satisfy that
rule must add a narrow generic seam at `entity-state`, not a mechanics store, command AST, or shadow
revision.

Entity-local evaluation performs exact component-slot lookups and iterates only present bounded
entries. The focused base damage path visits zero intrinsic sources, effects, equipment
assignments, item components, and request sources; it performs no global entity scan. Strict
reconstruction supplies the explicit gameplay registry to `entity-state`, then validates every
component's catalog version and referenced definition before returning the candidate state.
Immutable catalog and entity-mechanics views expose canonical values and exact component revisions
for analysis and later inspector projection without creating a mutation lane.

GM5 stabilizes that initial provider surface rather than adding another runtime layer. All seven
component type identities, codec identities, and positive codec versions are public metadata on
`MechanicsComponentKind`; arbitrary registered component types remain runtime-only unless their
own explicit durable codec says otherwise. Strict reconstruction rejects unknown required kinds,
duplicate type/value records, codec drift, malformed component values, catalog-version mismatch,
and unresolved definitions before returning a candidate. The downstream catalog version remains
the compatibility and migration authority. The canonical definition fingerprint remains
diagnostic/cache/receipt evidence and deliberately does not lock ordinary balance changes.

`engine-inspector` is the read-only cross-owner leaf for the admitted result. Its ordinary
mechanics report projects fixed seven-kind presence plus stored component facts and structural
catalog declarations; it does not re-evaluate stats, activate effects, or join inventory. A caller
may separately supply an owner-produced evaluation readout or receipt, which inspection copies
without recomputation. Its damage-receipt projection preserves the complete bounded
stage/decision/change/fact detail. Inspection neither mutates components nor becomes a renderer,
product schema, save owner, event journal, or replay authority.

The direct consumer example under `gameplay-mechanics/examples` covers realtime shooter,
infrastructure, and d20-shaped downstream composition with named services only. The GM5
reconstruction proof attaches all seven component families, omits an explicitly runtime-only
fixture from persistence, restores through the complete registry and catalog, then continues the
same operation on original and restored state with the same authoritative result. A separate
2,048-unrelated-entity measurement leaves the simple stat and one-part damage source costs at zero,
matching the code path's exact slot lookups and component-local cloning.

GM7 reconciles that provider with an exact reviewed realtime product, the bounded in-repository
infrastructure falsification fixture, and a real rules-heavy Rusty D20 product. The builder found
one reusable equipment/track-bound invariant, which was fixed at `EquipmentService`; Rusty D20's
schema, semantic compiler, orchestration, complete save, and UI remained downstream. The exact
revisions, browser/fresh-process evidence, release allocation observations, quotas, dependency
direction, and stopping point are recorded in
[the gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md).

See [Gameplay mechanics](code-map/gameplay-mechanics.md) for entry points, frozen quotas, and focused
gates.

## Optional continuous mechanics support

`gameplay-continuous-mechanics` is a separately named opt-in owner for persisted continuous
gameplay facts. It depends on `gameplay-standard` only for the accepted finite,
normalized-binary64 `ContinuousValue` semantics; `gameplay-standard` remains the value,
expression, and explicit conversion owner, not a live mechanics authority. The continuous crate
owns its own typed stat, track, source, effect, catalog-version, and operation identities; its own
catalog fingerprint; four durable component identities/codecs; and direct stat, track, and effect
services. The exact `MechanicsCatalog`, `MechanicsComponentKind::ALL`, seven exact codecs, exact
snapshots, and exact services remain unchanged.

Continuous component codecs persist normalized binary64 bits, never public raw `f64` state or
decimal spellings. Public definitions and component values store `ContinuousValue` directly; a
crate-owned strict serde adapter accepts only a 16-digit lowercase hexadecimal bit string.
Catalog admission and codec validation reject non-finite and negative-zero bit
patterns, preserve subnormal bits, sort identities and contributions canonically, and bound
definitions, source contributions, effects, and entity components. Continuous stat resolution has
explicit sum/highest/lowest/unique selection followed by explicit minimum/maximum constraints;
all comparisons use exact normalized-bit value identity, never approximate equality. Services
guard their exact component slot revision, prepare and validate their one-component candidate,
and publish only after success. Their receipts retain catalog provenance, values, and revisions.

The continuous registry is opt-in. `combined_gameplay_component_registry` composes the frozen
exact registrations and continuous registrations into one `EntityState`; it is not a second
entity authority. `decode_snapshot_with_catalogs` validates each catalog independently after one
strict reconstruction. Exact-only registry and snapshot entry points continue to decode the
historical exact family unchanged. `engine-inspector` exposes a separate family-labelled
continuous projection with normalized-bit values rather than widening the fixed exact report.

Continuous mechanics owns no residual carry, integration rate, cadence, interval, cap ordering,
scheduler, clock, unit ontology, damage reinterpretation, or persistence aggregate. A downstream
caller that integrates a rate retains the #7188 carry/cadence contract and explicitly submits a
resulting continuous value through the named service.

## Optional gameplay rules support

Rules-heavy consumers may opt into a `gameplay-rules` sibling crate without
making it a dependency of `entity-state` or `gameplay-mechanics`. It owns only
a strict bounded package envelope, exact package dependencies, canonical
encoding and fingerprints, source provenance, bounded diagnostics, and
deterministic package-set resolution. The payload is opaque JSON.

The downstream game owns the payload schema, every semantic definition and
compiler rule, mechanics bindings, orchestration, persistence, and execution.
An optional TypeScript authoring DSL may produce immutable build-time
candidates in an isolated workspace, while Rust remains the semantic and
runtime authority. Direct Rust
construction and checked artifact admission require no Node or TypeScript at
runtime. The implemented `rules/` workspace generates its shared envelope
types and bounds from a Rust-owned descriptor, emits byte-identical canonical
artifacts, and remains outside ordinary provider verification.

The Rust crate admits either an explicit `RulePackageCandidate` or strict
schema-1 bytes into the same immutable `AdmittedRulePackage`. It owns canonical
bytes and their content fingerprint, then resolves a complete caller-supplied
set into deterministic dependency order only after aggregate bounds and every
exact dependency have passed. It publishes nothing to a component store,
catalog, filesystem, or global registry. Downstream code owns semantic
compilation, publication, loading, and persistence of both its compiled
definitions and any package files. See
[Gameplay rules](code-map/gameplay-rules.md) for implementation paths and
gates.

Schema 1 preserves the original portable safe-integer payload contract.
Schema 2 is an explicit opt-in for finite IEEE-754 binary64 payload numbers;
it owns one Rust/TypeScript-identical ECMAScript numeric representation while
leaving runtime precision, units, formulas, and presentation downstream.
Existing schema-1 bytes are never silently upgraded.

This semantic-neutral envelope is not a universal gameplay IR. Engine defines no formula, predicate,
operation, action, condition, effect, behavior, evaluator, registry, runtime
session, scheduler, or d20 vocabulary through this surface. A mechanics-only
game ignores it entirely. The exact versioned API, bounds, ordering, failure
identity, TypeScript isolation, and historical implementation proof are frozen in
[gameplay-rules-contract.md](gameplay-rules-contract.md).

## Gameplay resolution boundary

The optional `gameplay-resolution` sibling standardizes one bounded lifecycle
for a downstream-owned gameplay attempt: admit an intent, gather facts, check
policy, structurally traverse a downstream program, run a frozen ordered set of
downstream interceptors, stage downstream effects, commit once, and return an
attributed receipt. Preview follows the same path but aborts the staged
transaction instead of committing it. Rejection, suspension, provider limits,
child-resolution failure, staging failure, and commit failure publish no
authoritative mutation under the transaction contract.

The crate owns only lifecycle structure, deterministic traversal, correlation
and causation, quotas, and generic receipt/trace containers. Its program nodes
are limited to sequence, conditional selection, and an opaque downstream
operation. Policy supplies every intent, fact, predicate, operation,
interceptor, effect, event, evidence, rejection,
fault, suspension, and trace-detail type. A downstream may use the standard
resolver or replace the complete resolver while retaining the public request,
transaction, and receipt borders.

`gameplay-resolution` has no dependency on `gameplay-rules`,
`gameplay-mechanics`, `entity-state`, rendering, TypeScript, Node, or a game.
Downstream code may compile an opaque `gameplay-rules` payload into its own
policy/program types, bind planned effects to named `gameplay-mechanics` or
other capability owners, and expose the returned receipt through its own
read-only diagnostics. Those relationships do not reverse into the kernel.

This is not a universal gameplay AST, verb catalog, scheduler, event bus,
plugin registry, replay runtime, save format, or script VM. Engine does not
define attacks, targets, stats, damage, spells, items, conditions, weapons,
pickups, turns, or other game meaning. Historical Dagger and realtime Doom
adoption did shrink this seam by removing selector assumptions; that is useful
overfitting evidence, not a current requirement to wait for another consumer.
The changes are recorded in the task-7032 report.

See [Gameplay resolution](code-map/gameplay-resolution.md) for the concrete
owners, dependency prohibitions, and focused gates.

## Incubating gameplay-standard capability layer

`gameplay-standard` is an optional, host-neutral metadata layer over four
already independent owners: `entity-state`, `gameplay-mechanics`,
`gameplay-resolution`, and `gameplay-rules`. It exposes one static readout in
each named module namespace (`modules::entity_state`, `modules::mechanics`,
`modules::resolution`, and `modules::rules`). A readout has a bounded stable
identity, positive version, and current maturity. All four are **Incubating**
at version 1: the adoption route is available now, while compatible additive
growth remains expected.

Each module re-exports the exact public API of its owning crate next to its
readout. It does not wrap or re-own entity facts, mechanics, attempt
resolution, rules packages, or their errors. The low-level crates remain
first-class direct dependencies and facade namespaces. A downstream product
may select any subset (for example, mechanics plus resolution) or opt out of
the layer entirely and construct owner services directly.

Availability is broader than a downstream product's chosen defaults. A product
may adopt a small preset or no preset while every exposed capability remains
available for explicit selection; this crate does not declare mandatory
modules, bootstrap a session, or discover capabilities at runtime. It has no
`StandardGameplay` aggregate, registry, trait-object collection, scheduler,
persistence, runtime facade, or global module list. A later neutral capability
is added as another named module and static readout, without changing an
aggregate world/session owner.

Product-specific extensions stay downstream as ordinary typed Rust values,
services, and composition. They may use standard capability nodes together
with downstream-specific nodes, but they are not JSON or dynamic extensions to
this crate. Beyond the explicitly named numeric families and standard mechanics
leaves below, the layer does not define gameplay vocabulary, product policy,
timing, orchestration, or persistence contracts.

See [Gameplay standard](code-map/gameplay-standard.md) for the exact readouts,
selection example, and focused verification.

Its borrowed projection helpers are inspection/adoption views, not a second evaluator: they
retain supplied definitions, requirements, gathered values, planning evidence, mechanics receipts,
generic resolution receipts/traces, and package provenance exactly as their owners produced them.
Small action-actor and destructible/resource presets emit only ordinary catalog fragments,
components, and named service inputs. These **Incubating** helpers are optional beside both typed
downstream extension composition and direct low-level owner APIs; none creates an aggregate
runtime or product session.

`gameplay-standard` also provides separate opt-in exact and continuous value
families. Exact expressions use the unchanged bounded `MechanicsScalar` and
`ExactRatio` owners; continuous expressions use finite normalized binary64
values. Neither family coerces into the other: mechanics widening and
continuous quantization are named boundaries with receipts. Expressions accept
only explicit immutable input bundles and typed declared roles; they perform no
world lookup, callback, scheduling, persistence, or product-unit work.
Their family-specific requirement artifacts expose sorted, deduplicated typed
inputs and canonical declared roles; an input cannot reference an undeclared
role. Downstream product leaves compile through a matching static family trait
to a closed Rust expression before admission, not through a runtime callback,
registry, or JSON extension path.

The bounded [continuous cadence and residual experiment](topics/gameplay/continuous-cadence-experiment.md)
compares caller-declared partitions without adding an Engine clock, scheduler,
continuous stat, or persistence aggregate. Its current evidence keeps residual
integration and persistence caller-owned until a concrete neutral resource
mechanism can own value, residual, cadence, cap, and migration semantics
together.

Family-specific definitions are admitted as versioned payloads through the
existing `gameplay-rules` package route. Definitions use schema 1 for exact
safe-integers and schema 2 for continuous binary64 values.
Rust owns the descriptor and runtime evaluator; it generates the strict
TypeScript contract surface. Checked exact and continuous fixtures are authored
through that TypeScript surface, then Rust decodes, re-admits, fingerprints,
and rehydrates them. This is deterministic build-time convergence, not a
TypeScript evaluator or a Node dependency of ordinary Rust verification.

Product extension data remains a separate, source-correlated, bounded
`gameplay-rules` artifact. It is admitted exchange data and is compiled by a
caller-selected downstream Rust compiler to a closed product enum; it is never
a standard expression node, a registry entry, runtime JSON dispatch, or Engine
evaluation authority. Its declared extension schema version is independent of
the outer gameplay-rules schema, so explicit authoring routes can emit either
schema-1 integer or schema-2 binary64 envelopes without changing the extension
identity or adding a runtime extension path.

## Developer-command boundary

`developer-command` is an optional host-neutral contract crate for a product's
explicit developer tooling seam. It owns versioned typed request/reply
envelopes, stable command/correlation/runtime/profile identities, semantic
lanes, bounded descriptors, discovery, provenance, and synchronous in-process
dispatch guards. A product constructs one instance-owned binding set, declares
the compiled command descriptors it recognizes, and either binds a retained
`Send + 'static` handler or explicitly exposes a command for a caller-borrowed
owner. The latter is borrowed only at invocation and is never stored by Engine.
The product refreshes observed runtime facts and invokes either path from its
existing queue at a product-selected safe point.

The product retains the command-family vocabulary, owner services, live state,
queue, safe-point timing, command-specific authorization, transport/client
adapters, persistence, and ordinary typed owner receipts. Lanes are descriptor
metadata, not caller-supplied authority. Profiles and explicit bindings select
availability: in-process discovery reports declared descriptors plus separate
stored-bound and borrowed-bound flags, so omitted commands are unknown while
declared but unbound commands are unavailable. Host discovery emits only
executable descriptors in the generated camelCase v1 shape. Envelope
validation rejects stale runtime/profile/revision or catalog facts,
cancellation, timeout, and correlation errors before an owner is called;
entered handlers retain their ordinary mutation semantics. Stored and borrowed
dispatch share the same preflight, correlation/provenance, and bounded-history
finalization authority. Rejected preflight never reserves a correlation,
advances sequence, records history, or enters an owner.

This crate is not a universal command/event bus, scheduler, service locator,
reflection or method-name bridge, world transaction, generic component write
API, network server, filesystem API, or UI shell. Reply/provenance/history
values serialize only as output for a chosen adapter; they are not admitted
wire input. The strict Rust host adapters map decimal-string revision/catalog
facts and product payloads into the typed request, and map typed replies/errors
into the generated six-field response while retaining provenance and
pre-dispatch versus owner error phase in a Rust-side metadata sidecar. See
[Developer commands](code-map/developer-command.md) for the public composition
and focused gates.

`developer-command-standard` is a separate optional tooling leaf for the
Engine-owned standard gameplay route. It leaves `developer-command` generic
and binds no world itself. Its inspect commands call the existing
`engine-inspector` projections, its guarded admin commands retain the exact
`gameplay-mechanics` request, receipt, and error types, and its preview/play
helpers select `ResolutionMode::Preview` or `ResolutionMode::Apply` on the
existing `StandardResolver`. A downstream product still owns the policy,
transaction, queue, safe point, command authorization, and all product-typed
attempt payloads. The preview helper never creates a publication path; the
product transaction is the sole apply owner.

For a browser, Node, or agent-tool client, the DOM-free public
`@rusty-engine/developer-command-client` package decodes the generated generic
wire contract and optional standard host DTO schemas. It owns no transport or
authority. `@rusty-engine/application-host` may mount an optional pull-down
console over that injected client; the console is presentation only, and its
open state enters ordinary interface input mode. Standard admin host DTOs use
decimal wire identities, reacquire opaque live component revisions immediately
before the named owner service, and never serialize those revision guards.

## Spatial authority and derived mechanisms

`engine-spatial::VoxelCollisionScene` holds canonical material voxels alongside projections derived
from them:

- `svc-spatial` and `svc-volume` store deterministic voxel state;
- `svc-collision` builds Parry-backed collision queries;
- `svc-pathfinding` derives bounded navigation projections;
- `svc-mesh` derives deterministic visible-surface meshes, greedily merging
  only coplanar adjacent faces with identical material and normal; and
- `VoxelEditService` validates an expected-revision transaction and replaces the complete coherent
  result only after every affected projection succeeds.

Static mesh collision enters that same query owner without becoming voxel or
renderer authority. `MeshCollisionPolicy::Trimesh` retains the exact validated
mesh payload selected by offline import. An explicit filesystem/content adapter
resolves renderer-resource bytes when needed, then supplies bounded positions,
triangle indices, immutable geometry identity, and caller-owned instance
transforms to `svc-collision`. The service validates and hash-binds each asset,
builds Parry triangle meshes, and atomically replaces one complete instance
projection at an exact independent revision. World ray and AABB/sweep queries
consider voxel and triangle colliders together; voxel-edit raycasts remain
voxel-only so an external mesh cannot forge a voxel edit anchor. Voxel rebuilds
preserve the derived static projection. Prepared voxel edits and history reverts
capture its independent revision and reject if it changes before commit, so a
voxel swap cannot roll back a newer collider replacement. Persistence/reopen
reconstructs the projection from downstream content and entity facts rather
than serializing Parry state.
Static triangle meshes are non-dynamic colliders. Caller-driven non-kinematic
rigid-body response is a separate named mechanism: durable transform and
schema-1 rigid-body component facts remain in `entity-state`, while
`svc-collision` uses a contained single-threaded Rapier/Parry backend as derived
state and `engine-spatial` owns bounded fixed-step preparation plus atomic exact-
slot publication. Downstream owns step timing, gameplay-selected impulses,
consequences, complete saves, and presentation. Kinematic controller motion is
not redefined as a dynamic body. The backend choice, numeric guarantees, CCD
limits, and forbidden scheduler/session shapes are specified in
[Rigid-body dynamics](topics/rigid-body-dynamics.md).

Kinematic FPS character motion is another explicit named mechanism, not an
ordinary rigid body and not an expansion of the legacy axis-separated motion
paths. `engine-spatial::CharacterControllerService` combines caller-supplied
fixed-step commands with bounded local-+Y capsule casts/overlaps over canonical
voxel, admitted static-mesh, and active entity obstacles. It prepares
slide/step/snap, stance, jump-timing, support carry, recovery, controlled and
external velocity, and typed contact/ground/block/platform/impulse-proposal
facts. It owns no scheduler, device input, camera, gameplay ability, material
meaning, dynamic-body mutation, or presentation consequence.

The durable schema-1 `entity-state::CharacterMotionComponent` contains only
inert continuation facts: controlled/external velocity, stance and grounding,
jump timers, support anchor/pose/point velocity, fall heights, accepted command
sequence, and collision-world identity. A prepared controller step captures
the exact Transform and character-motion slot revisions plus the collision
environment identity. Commit rechecks all three and uses
`replace_character_motion_state` to publish Transform and motion together or
change neither. Character motion is mutually exclusive with the legacy
kinematic and rigid-body components, parented transforms, and non-unit scale.

`engine-spatial::FirstPersonLookService` is a separate optional pure service
for bounded yaw/pitch integration and forward/right/up basis construction. It
shares the controller's yaw-zero-is--Z and positive-yaw-toward-+X convention,
but owns no renderer camera, position/orbit, device normalization, recoil, bob,
or product smoothing. The adopted design and donor rationale are recorded in
[FPS character controller design](topics/fps-character-controller-proposal.md)
and its [survey](topics/fps-character-controller-survey.md).

Large-coordinate products use the explicit
`engine-spatial::WorldOriginRebaseService` contract documented in
[world-origin rebasing](topics/world-origin-rebasing.md). Exact signed global
positions and voxel/chunk identities remain canonical while entity motion,
collision/navigation projections, voxel chunk transforms, and retained frames
use one bounded local coordinate frame. Downstream owns threshold and cadence;
Engine owns guarded candidate preparation and atomic publication. Renderer and
projection code only observe the accepted local frame and cannot select an
origin or become global authority.

`KinematicMotionSystem` is a centrally invoked mechanism over explicit body views. It supplies no
game loop, actor policy, or component-local update callback. A downstream runtime decides when to
invoke it and what accepted facts mean.

`TriggerVolumeSystem` is the sole overlap authority for registered trigger volumes. Each
`KinematicTriggerDefinition` names one trigger entity and a typed `TriggerGeometrySource` that
selects where the volume's live AABB comes from during reconciliation:

- `ActiveCollision` (the default and historical behavior) requires the trigger entity to be active
  with an enabled collision component in addition to bounds and a composed world transform. Such a
  trigger entity is also a solid motion obstacle, because kinematic motion treats every active
  collision body as solid.
- `EntityBounds` derives the same AABB from the canonical entity lifecycle, bounds, and composed
  world transform without consulting the collision component at all. The trigger entity therefore
  senses subjects without ever becoming a solid obstacle; its collision state, when present, is
  irrelevant to sensing and produces no diagnostics.

The boundary stays one-directional: motion solidity is owned exclusively by the collision
component (`EntityMotionService` obstacles are active-collision entities only), while trigger
sensing is owned exclusively by `TriggerVolumeSystem` over `EntityState`. Subject eligibility is
unchanged by the geometry source — a subject always requires the canonical active-collision and
entity-state rules — and reconciliation stays bounded, deterministic, typed, snapshot-capable, and
fail-closed with actionable stale/missing bounds or transform diagnostics. Downstream consumers
must not recompute overlap, special-case motion, or maintain a second spatial authority; geometry
source is a per-definition Engine vocabulary with no gameplay semantics, tag exceptions, or
ambient callbacks. Trigger snapshots remain schema 1: definitions written before the geometry seam
decode as `ActiveCollision`, and new definitions round-trip their geometry source exactly.

`SpatialOcclusionService` is the corresponding read-only ray owner when a caller needs voxel and
retained-entity occlusion in one result. It queries `VoxelCollisionScene` together with
`EntityState`, deriving entity AABBs through the same active-collision selection and world-translation
geometry used by `EntityMotionService`; lifecycle, enabled collision, bounds, and transform decide
participation, while render visibility does not. One call inspects at most 4,096 entity records and
accepts at most eight ignored source/endpoint identities, with typed rejection before any scan when
those quotas or the ray contract are invalid. Strictly nearer hits win; an exact entity/voxel
distance tie preserves the entity hit, and an exact entity tie preserves the lower stable entity
ID. The service borrows both authorities immutably, so rejected and successful queries cannot
mutate either one. Downstream callers may exclude their source and intended target, but they do not
maintain a collider index or reproduce the combined ordering.

Collision, navigation, and meshes must not become independent world authorities. Optimization may
make rebuilding more incremental, but it must retain the same source revision and atomic coherence
rule. Live voxel edits derive a deterministic signed dirty-chunk set, rebuild only resident mesh
candidates that can own changed seam geometry, and reuse every unrelated immutable mesh payload.
The retained voxel projector keys replacement by the complete derived payload rather than the
canonical chunk hash, because neighbor occupancy can change a seam without changing that chunk's
own voxels. Stable chunk handles cross an optionally stamped monotonic render frame; clipped or
stale publications reject before retained/backend mutation. The detailed lifecycle and measured
stopping point are fixed in
[chunk-granular voxel mesh updates](topics/voxel/chunk-granular-updates.md).

Canonical chunk residency is also caller-driven. `VoxelChunkResidencyService`
accepts bounded batches of complete dense chunks with stable signed identities,
exact scene and content-hash preconditions, and explicit admit, replace, or
evict intent. It prepares canonical voxels and every collision, navigation, and
mesh derivative away from live state, then rechecks source, static-collision,
and lease generations before publishing one coherent revision. Instance-owned
leases provide explicit pin evidence without defining a player radius or I/O
policy. A nonempty global edit history must either reject residency change or
be explicitly rebased to the newly published authority; undo never resurrects
an evicted chunk. Empty resident chunks remain authority, while a last-solid
ordinary edit does not implicitly evict them. Downstream retains all sourcing,
streaming, scheduling, retention, and memory-pressure policy. See
[canonical voxel chunk residency](topics/voxel/chunk-residency.md).

## Foundation and service crates

The smaller `core-*` and `svc-*` crates are normal workspace packages, not an origin-oriented donor
layer. They provide narrow identities, coordinate types, time values, voxel storage, collision,
pathfinding, deterministic RNG, and meshing. Their exact donor sources and adaptations are recorded
in [donor-provenance.md](migration/donor-provenance.md).

These crates expose mechanism rather than policy. For example, `svc-pathfinding` can propose a path
but does not own AI intent; `svc-rng` creates a scoped deterministic stream but does not decide what
is random; `svc-mesh` emits geometry but owns no renderer.

Runtime voxel surface textures preserve that boundary and the default greedy mesher. The greedy
path projects canonical integer grid points into a deterministic outward-facing tile basis; material
definitions own color-only, whole-texture repeat, or atlas-region repeat; renderer-neutral frames
own bounded content-addressed encoded resources; and the isolated backend owns decode, GPU sampling,
and disposal. Atlas repetition happens inside the assigned region through a derived sampling
specialization, not by expanding a greedy rectangle or remeshing in TypeScript. World chunks use
absolute voxel origins for continuous phase, while voxel objects use object-local coordinates. The
selected formats, orientation, sampling, quotas, and failure behavior are fixed in
[the runtime voxel surface texture decision](topics/voxel/voxel-surface-textures.md). The exact
provider-to-consumer ownership chain, measured geometry/resource costs, and deliberate stopping point
are recorded in the [textured voxel campaign closeout](textured-voxel-campaign-closeout.md).

Optional `marchingCubes` and `dualContouring` modes are deterministic derived presentations over
the same canonical facts. They share one explicit binary center-sampled scalar field but retain
independent contour construction: Marching Cubes owns deterministic face/interior ambiguity rules,
while Dual Contouring owns Hermite samples and a bounded rank-aware QEF solve. Neither enters
collision, navigation, persistence, or gameplay queries. Reconstructed modes have no stable
tile/atlas UV projection and reject textured materials before mutation. Their experimental scope,
real-corpus measurements, provenance, and limitations are fixed in
[reconstructed voxel surfaces](topics/voxel/reconstructed-surfaces.md).

## Shared rendering boundary

`render-model` owns the complete versioned retained-frame vocabulary: stable handles, hierarchy,
primitive and mesh geometry, materials, textures, sprites, static and animated assets, lighting,
editor-grid descriptions, picks, validation, and canonical JSON. It contains no state store,
catalog, filesystem, renderer object, runtime facade, or replay requirement.

Large mesh streams may cross that border as versioned content-addressed resources instead of
expanded JSON arrays. `render-model` owns deterministic bounded packing and byte-layout validation;
the descriptor names identity, hash, length, encoding, and stream offsets but no path or URL.
`render-projection` can return those derived bytes beside a control frame, while its caller owns
publication and resolver policy. The durable format, measured tradeoffs, and migration are fixed in
[the voxel mesh data-plane decision](topics/voxel/voxel-mesh-data-plane.md).
Runtime image bytes and voxel tile-space/atlas mapping use the adjacent
[voxel surface texture decision](topics/voxel/voxel-surface-textures.md); they do not turn a path,
URL, browser image, or Three texture into Rust authority.

An authored sky uses that same retained texture owner through one nullable
`setSkyBackground` operation. The referenced payload must be a bounded 2:1,
sRGB, clamp-wrapped equirectangular panorama. Three realizes a renderer-owned
specialization behind the world that observes camera rotation but not
translation, depth, collision, picking, environment lighting, or reflections.
Replacement follows retained texture versions, clear returns to the host clear
color, and both replacement and disposal release the specialized GPU texture.
This is intentionally not a cubemap, lighting probe, post-processing graph, or
general renderer plugin seam.

`render-projection` owns deterministic, fail-atomic adapters over explicit read-only inputs. Entity
projection reads `EntityState`; voxel projection reads `VoxelCollisionScene`; authored projection
accepts one ordinary appearance/resource aggregate; debug projection accepts typed overlays. Each
projector computes against cloned retained state, validates the complete frame, and commits stable
handles only when construction succeeds. The TypeScript retained projection stages frames with
copy-on-write records: unchanged immutable definitions are structurally shared, while every
record named by a mutation is privately copied before validation or commit. Missing resources are
classified rather than resolved through an ambient registry.

An independently ordered projection may add a renderer-neutral publication stamp containing its
stable stream, monotonic revision, and exact operation count. This is transport/application
coherence, not gameplay authority: retained projection rejects a clipped or stale stamped frame
before commit, while unstamped general frames retain the original compatibility contract.

The optional catalog-material adapter is another explicit read-only projection: it validates one
complete `AssetCatalog` candidate, resolves exact version/hash-pinned voxel texture and atlas
dependencies, and emits immutable renderer-neutral provenance. It does not mutate catalog, voxel,
collision, navigation, or voxel-object state.

`render-presentation` owns the other renderer-neutral family: typed audio sources and impulses,
world/entity billboards, bounded particle emitters and bursts, telemetry overlay requests, and
animation graph/controller/playback projection. Its controller is an explicitly invoked mechanism,
not an update loop or gameplay behavior graph. Fixed-point parameters, stable priority ordering,
transition timing, and blend resolution remain deterministic; persistence journals, certification
hashes, and provenance envelopes do not exist. Every projector can validate a complete domain
batch, while `PresentationProjectorSet` provides one fail-atomic mixed-domain frame boundary.
Resource checks use only immutable kind/content identity supplied by the caller.

Particle visuals are a closed billboard-or-cube choice. Billboard sprites retain exact content
identity and flipbook facts; primitive cubes need no asset. Optional collision is a bounded set of
planes and AABBs captured in emitter-local coordinates at spawn and simulated only as disposable
presentation. It does not query `engine-spatial`, publish contacts, or feed results back to gameplay.
The shared host advances seeded particles and swept sphere proxies, while the Three sink pools
billboards into `Points` batches and cubes into `InstancedMesh` batches behind `RendererSurface`.
The legacy serialized `sprite` descriptor remains readable, but new Rust serialization emits the
discriminated `visual` form. See [Three scene particles](topics/three-scene-particles.md).

Ordinary retained sprites optionally carry one bounded material descriptor for unlit, authored
tangent-space normal, authored depth, color-gradient-derived, or synthetic curved lighting. Color
resources remain sRGB while normal and depth resources are explicitly linear; missing or wrongly
classified lighting textures fail before retained mutation. Alpha and shadow policy are explicit,
and omitted material facts preserve the unlit compatibility path. Three realizes these as
camera-facing mesh quads so the same retained pivot, atlas, hierarchy, picking, billboard, fog, and
disposal behavior remains available. Asset Pipeline owns source-map authoring and any selection
policy; Engine owns only admitted runtime facts and realization. See
[lit sprite shader comparison](topics/lit-sprite-shaders.md).
Sprite atlas rectangles use normalized decoded-image coordinates with the origin at the upright
PNG's top-left, U increasing right, and V increasing down. Importers retain ordinary upright PNG
rows; the sprite backend maps that image-space rectangle onto its quad without changing the shared
texture orientation used by generic meshes or voxel materials. This is the sole retained-sprite
orientation contract: bottom-up PNG payloads and importer-side row reversal are not supported.
Legacy frames with no material block retain their prior texture-aware transparency and depth-write
semantics; current Rust writers emit the block so an explicit blend policy remains distinguishable.

The low-volume authored `Appearance` candidate keeps the complete sprite descriptor inline. This
preserves its direct value-composition API and avoids imposing a heap allocation on every default
unlit authored sprite merely because optional lit-material facts enlarge that variant.
The isolated workspace entry points and evidence links are indexed in
[the renderer workspace README](../render/README.md).

World indicators remain in that billboard owner rather than creating a second
UI protocol. A structured billboard can compose bounded localized label/icon,
neutral ranged meters, and status cues under one stable handle. Exact finite
ranges, stable local identities, explicit pixel/distance sizing, priority,
safe-area, edge, and overlap facts cross the neutral border; deterministic DOM
layout, semantic accessibility, hash-verified resources, and disposal remain
host realization. Ordinary indicators are pointer-transparent. Games retain
health, faction, targeting, interaction, visibility, and camera meaning, and no
renderer readout mutates those facts. World-sized DOM and interactive diegetic
panels are deliberately outside the current contract. See
[structured world indicators](topics/world-indicators.md).

The isolated `render/` workspace realizes the same contract for Three, WebGL, WebAudio, DOM
overlays, inspection, and editor viewport use. It is installed and verified separately from the
ordinary Rust provider gate. Renderer picks and host readouts are hints/observations that downstream
authority must revalidate; animation sampling, audio completion, particles, cameras, and editor
previews never mutate gameplay.

Camera-relative retained presentation is one explicit `viewmodel` render layer, not a downstream
Three scene or weapon API. A caller creates a retained root on that layer and parents existing
primitive, static-mesh, animated-mesh, voxel-object, or sprite instances below it. The neutral
projection inherits the layer through the hierarchy and enforces fixed node, distinct-asset,
asset-extent, translation, and scale limits before committing a frame. Retained lights are rejected
inside this channel; the backend supplies a small neutral light rig.

The Three adapter retains that hierarchy in a separate scene. Camera-dependent sprite realization
also remains in this backend: immediately before each world, viewmodel, editor-channel, composed-view,
or pick pass it restores each sprite's authored local rotation, then realizes `spherical` and
Y-up `cylindrical` modes against that pass's camera. `none` remains untouched. Parent-before-child
ordering, orthographic view direction, and deterministic authored-yaw fallback keep nested and
degenerate billboards stable without changing the renderer-neutral descriptor or retained authority.
The browser surface owns one
animation-frame scheduler and coalesces accepted retained mutations, camera changes, and resizes
into its next backend submission. Active controls, animation, or particles retain continuous
display-clock advancement; an unchanged static scene does not continuously resubmit work to the
backend. The single callback registers its sole successor before camera, presentation, and WebGL
work so current submission load cannot delay registration beyond the browser's next display
scheduling window. On WebGL2, software, unknown, and timing-fallback paths keep at most one automatic GPU
command stream in flight. Positively identified accelerated WebGL2 with working timer queries
may use a fixed eight-slot renderer-owned completion-query ring; a sync-fence ring adds an
independent bound when that WebGL mechanism is available. This remains bounded
completion safety while avoiding a false 50–100 ms cadence cap on real accelerated drivers whose
query results become observable long after their measured 4–7 ms GPU work. When the browser
exposes asynchronous WebGL2 timer queries, the backend also leaves a completion-derived,
bounded progressive headroom interval after measured GPU execution before admitting another
automatic submission. Software renderers can under-report timer duration while asynchronous completion still
occupies browser CPU. The Three backend classifies the concrete WebGL renderer without exposing its
raw identity: positively identified software rendering uses the complete observed completion wall
latency as effective work. A valid timer result on positively identified accelerated hardware is
authoritative for execution duration, so delayed query-result polling does not inflate GPU work;
unknown renderers and timing fallback paths retain one ordinary polling interval before wall
latency adds pressure. The accelerated fast path retains 120 Hz for four-millisecond work and 60 Hz
for eight-millisecond work. Its prospective deadline is expressed in the accepted animation-frame
submission clock, rather than the later callback-observation wall clock: callback jitter cannot
turn a display-rate source into skipped presentation intervals, and work already enclosed by the
timer is not charged a second time after CPU submission. Software, unknown, and timing-fallback
deadlines remain completion-wall-based. Every nonzero measured submission
receives at least equal completion headroom, so exceptionally slow work cannot silently exceed
fifty-percent automatic duty; up to 100 ms of additional progressive headroom lowers ordinary slow
work toward twenty percent without adding an unbounded extra delay. Software rendering therefore
yields materially more browser and host CPU time. This remains workload-derived rather than a
fixed frame-rate cap. Positively identified software rasterizers also cap the WebGL backing-buffer
ratio at 0.25—one-sixteenth of the CSS pixel count for requests at or above one device pixel per
CSS pixel—while preserving CSS layout, projection, picking, content, and caller-requested ratios
below that ceiling. Accelerated and unknown renderers retain their requested ratio. Unsupported or
disjoint
timer timing retains completion-wall pacing and immediately restores strict single-slot admission,
and the public renderer surface exposes a frozen
read-only sample of the renderer class, applied allowance, latest decision, admission deadline,
actual automatic admission observation, selected capacity, timer-query occupancy, and sync-fence
occupancy without adding a polling or submission path. The host adds one fixed-size immutable
admission history to that same readout. It records each recent RAF source time, the exact
request/resize/controls/presentation/retained-animation demand reasons, whether the attempt had no
demand, was rejected by backend readiness, or was admitted, and the pre-submission capacity and
occupancy state. Each attempt also records immutable wall-clock phase boundaries from callback
entry and successor registration through demand, backend readiness, controls, camera,
presentation, backend submission, and callback exit; unreached phases are null. Lifetime outcome
counters remain exact after the history rolls over. Reading the history never polls WebGL,
schedules work, or mutates demand, so it can distinguish callback work, post-callback browser
delay, host-demand, and backend-admission failures without becoming a second renderer observer.
An explicit `renderOnce`
remains unconditional. Each submitted frame advances the shared renderer exactly once,
renders the world through the caller-owned camera, clears depth, and renders the camera-relative
scene through a fixed host-owned camera with the same projection and aspect.
World camera motion therefore cannot move the local presentation, and world depth cannot clip it.
The channel is excluded from picking and has no input or camera authority. Stop, resize, reset,
frame rejection, resource failure, and disposal remain operations of the existing single surface
lifecycle; no second render scheduler or renderer owner exists. Fence and measured-duty readiness
advance independently when the single RAF owner evaluates the next automatic submission, while
automatic admission still requires every available completion owner. The host does not run
inter-frame timer bursts to poll WebGL readiness: once the accelerated completion ring exists, those
non-rendering driver calls cannot create a presentation opportunity and only compete with browser
delivery of the next RAF.

`@rusty-engine/renderer-host` is the shared browser and tool-facing entry point. It composes the
retained Three surface, explicit caller-owned camera controls, animated resources, editor viewport,
inspection surface, WebAudio, billboard, particle, telemetry, and DOM overlay mechanisms. Its small
`RendererPresentationHostSet` fans one strictly decoded presentation frame out to named optional
hosts and reports every unavailable domain; it accepts no scene state, gameplay command, session,
or persistence object. Animation hosts may be attached after surface creation because they consume
the surface-owned animation projection, but the surface does not discover services or install
ambient callbacks. Demo and Studio consumers provide typed frames and resource resolvers instead
of constructing Three scenes of their own.

`@rusty-engine/application-host` is the sole downstream web-application entry point above that
internal host. Engine publishes it as one reproducibly built artifact with its renderer closure
bundled and no renderer peer dependencies. It creates the canvas and one downstream DOM root,
owns startup/failure, resize, cadence, pointer/focus modes, whole-content replacement, and disposal,
and exposes only bounded content, frame, camera, interaction, and lifecycle ports. One public
content aggregate pairs a complete Rust-projected frame with exact content-addressed mesh/texture
bytes. Engine snapshots and validates those bytes, derives the private manifests/resolvers, and
prepares the candidate surface and resources before atomic publication; failed initial mount or
replacement cleans up the candidate without exposing partial renderer state. A trusted downstream UI
framework mounts into that root but never receives the canvas, backend, private bridge, or package
topology. Browser, Tauri, and Electron products reuse the exact composition; only their typed Rust
transport differs. A development browser adapter may use bounded HTTP/WebSocket transport. A
packaged Tauri product without an independently justified isolation requirement ordinarily keeps
one named Rust product service in process with one WebView and typed IPC. A loopback sidecar is a
downstream product choice only when isolation, an independent lifecycle, or another measured need
justifies it; neither topology changes the application-host or gameplay-authority boundary. Trusted
downstream source is not treated as hostile plugin content, so this boundary adds no sanitizer or
sandbox framework.

The surface lighting contract is versioned and observational. Omitted settings retain the compatible
two-light neutral rig independently in the world and viewmodel scenes; a consumer may disable either
rig and supply ordinary retained lights instead. The host validates a bounded shadow policy before
mounting, while `renderer-three` preflights active requested-shadow count before changing retained
state and reports each request as active, disabled, or unsupported. Color, intensity, retained-light,
and active-shadow ceilings are provider facts. Games still own artistic values and when their Rust
authority emits or replaces light descriptors.

The same single surface optionally owns one bounded renderer-neutral view composition. Stable
camera descriptors select finite poses and perspective or orthographic projections; named render
targets select exact dimensions, RGBA8 sRGB color, optional depth, sampling, and a caller-owned
monotonic revision. Ordered views render the already-retained world scene either into the primary
surface or a named target. Ordered presentations can sample a named target into a normalized
primary-surface viewport without CPU readback or exposing a Three texture. Configuration is cloned,
deeply frozen, fully validated, and GPU resources are prepared before one atomic publication.
Duplicate producers, stale target revisions, feedback destinations, allocation failure, and quota
failure leave the prior composition current. Target replacement and surface disposal release the
backend resources. Readout distinguishes targets that were never rendered, are current, or became
stale after an accepted retained-scene or composition change; only a subsequent caller-owned
submission makes stale target content current again. An omitted composition preserves the original
single-camera path and submission loop. Cameras, viewports, targets, and their readout are
presentation facts only; they do not own visibility, discovery, navigation, input, or another
retained scene.

The retained Three backend also exposes a frozen visibility readout through browser and editor
surfaces. For each retained handle and camera pass it reports effective retained visibility and
CPU geometry-bound frustum membership, with a deterministic `frustumVisible`, `outsideFrustum`,
`hidden`, or `notDrawable` state. This is a readout and optional work-gating hint, not renderer or gameplay
authority; it describes the current retained state and camera, not a historical last-rendered bit.
GPU depth/occlusion is intentionally reported as `notMeasured`; asynchronous,
backend-specific occlusion queries remain outside this contract.

Historical Rusty Roguelike certification was the first exact public consumer
of that policy at
`e88856aca2b07212e79ca8a9a8cdc904cb49bd61`, pinning Engine
`b1f0415af6266783246371d227a2272de7d9f0d6`. Its Rust projection owns authored torch facts; the
browser selects disabled world defaults, retains the neutral viewmodel default, and proves that the
retained-light count exactly matches Rust projection while a real framebuffer shows localized warm
falloff. This is consumer evidence for the generic mechanism, not Engine ownership of torch policy.

Historical Rusty Roguelike revision `098b6d6c468711b4c149583996ac5147c9f58941` is the first public
multi-view consumer record and used Engine `8673aaa6d0b811195b3904f34d7729c0d6e92530`. It uses the
renderer-neutral contract to GPU-present an orthographic picture-in-picture over the exact
Rust-admitted retained local scene. Its desktop/mobile Chromium proof inspects distinct primary and
inset pixels, replaces the target across compact resize, and confirms that session revision and the
complete Rust minimap projection do not change. The accessible detailed minimap remains a separate
consumer of Rust-owned discovery and visibility facts.

Those exact TypeScript-package consumers remain historical certification evidence. Ordinary new
games depend on the complete `rusty-engine` Rust facade. Native/Rust-only products without a rich
product DOM reach rendering through the Rust-owned webview adapter; browser, Tauri, or Electron
products that need rich DOM use the single bundled application host described above. Engine
compiles or bundles the private TypeScript/Three closure behind either public boundary; downstream
neither selects renderer packages nor knows their topology. Operational commands, CI ownership,
and explicit limitations are recorded in
[rendering-operations.md](rendering-operations.md). The current downstream integration and
Engine-hosted Studio posture is centralized in
[downstream renderer and Studio boundary](topics/development/downstream-renderer-and-studio.md).

## Host and platform boundary

Rusty Engine is not a web-game engine. The current first-party renderer and tool UI use web
technology because Three/WebGL, DOM, WebAudio, and Chromium provide a practical backend, editor
surface, and real integration environment. They are replaceable backend and host choices rather
than Engine authority or a requirement that products be delivered over HTTP.

The boundary is layered deliberately:

- ordinary Rust crates own host-neutral state, services, authoring, persistence, and
  renderer-neutral projection;
- `render-host-contracts` owns typed camera, view, pick, physical-input, readout, diagnostic, and
  lifecycle facts without browser types or game meaning;
- `@rusty-engine/render-contracts` and `@rusty-engine/render-projection` own strict data decoding and
  retained projection plus the bounded multi-view descriptor without Three, DOM, browser lifecycle,
  or HTTP loading;
- `@rusty-engine/renderer-three` owns Three/WebGL realization. Its current browser-surface modules
  are explicit backend adapters, not a renderer-neutral or Engine-wide platform API;
- `@rusty-engine/renderer-host` owns the current browser/webview lifecycle, DOM overlays, WebAudio,
  input capture, inspection, and editor-host behavior;
- `@rusty-engine/application-host` bundles those private implementation owners behind one public
  web-product mount, rich-DOM root, typed runtime ports, and transactional surface lifecycle;
- `renderer-webview-host` is a platform leaf that embeds the reproducible Engine-private renderer
  artifact, owns exactly one Wry child webview, admits bounded content-addressed resource bytes,
  and exposes only named Rust operations plus typed observations; and
- downstream games and Studio own their product shells, outer window/event-loop policy, semantic input mapping,
  resource locations, and user-facing acceptance.

Assets and project data cross public borders through explicit identities, descriptors, bytes,
resolvers, and typed file operations. Core capability must not depend on public URLs, `fetch`,
same-origin behavior, browser storage, an HTTP control route, or Playwright-only mutation hooks.
The current concrete adapter supports Wry's platform webview; its downstream supplies the outer
window and event loop. Commands use the webview's private fixed method table and observations use
typed IPC. There is no public eval, generic dispatch, module import, URL resource loader, or second
control transport. A headless process or future backend can reuse the authoritative and
renderer-neutral layers without emulating a website.

The `rusty-engine` facade is deliberately a complete leaf over every public Rust library. It has no
features: downstream imports all namespaces, including the platform adapter, so a newly promoted
Engine mechanism cannot be silently omitted and reimplemented downstream. Internal libraries stay
independently meaningful and never depend back on the facade. The mechanically checked namespace
set is [the Rust SDK capability index](rust-sdk-capabilities.md).

The durable decision and placement litmus tests are in Den ADR
`rusty-engine/host-platform-and-browser-validation-boundary`.

## Durable voxel assets and offline conversion

`voxel-asset` owns a strict schema, semantic validation, canonical encoding, content identity, and
bounded conversion-input values. It performs no filesystem access, mesh parsing, runtime mutation,
or project admission.

Voxel volumes and voxel objects are separate durable meanings. A `voxel-volume/...` asset is grid
content that a product may admit into authoritative environment state. A `voxel-object/...` asset is
a reusable local-space model with one default frame and optional named full-frame clips; its pivot
and animated frames are presentation data unless a caller explicitly selects a stable collision
frame or proxy. Shared `VoxelFrame` resolution keeps both formats canonical without making visible
frame swaps into world edits, collision/navigation rebuilds, or an Engine scheduler. The current
M12 design and ordered implementation slices are in
[voxel-model-conversion.md](topics/voxel/voxel-model-conversion.md).

An animation frame may additionally carry bounded named local-space anchors and coarse authored
collision facts. Those facts preserve rig-free attachment points and optional capsule/box body or
hit-region geometry; they do not derive collision from visible voxels, assign combat meaning, or
mutate a collision world. `voxel-asset` validates, canonicalizes, and content-hash binds them.
`voxel-object-runtime` preserves them as immutable frame readout while continuing to deduplicate
meshes solely by voxel geometry. Positions and dimensions use local voxel-cell units under the
object grid's right-handed Y-up convention. Capsules are local-Y aligned; `halfHeight` measures
half the cylindrical axis segment excluding the hemispherical radius-sized caps, so total Y extent
is `2 * (halfHeight + radius)`. Downstream chooses whether and how to apply these facts.

`voxel-convert` is an offline authoring/build tool. It accepts one explicit request and bounded GLB
source, completes conversion and validation before touching its target, then atomically installs the
canonical artifact. Runtime consumers depend on `voxel-asset`, not the converter or GLB parser.

The converter also exposes one narrow transforms-only authoring seam for imported animation clips.
`evaluate_clip_node_poses` evaluates an explicit integer-microsecond time through the same channel
and hierarchy owner used by mesh sampling, returning scale-preserving affine local and world
transforms without materializing geometry. Callers that need rigid part placement must explicitly
admit unit or uniform scale; non-uniform scale, shear, singular transforms, and reflections are
typed rejections rather than values silently discarded by the Engine.

The checked request, licensed source, and canonical output remain here because they are provider
verification fixtures, not demo content. See [voxel-asset-format.md](topics/voxel/voxel-asset-format.md).

## Studio authoring boundary

The isolated `studio/` workspace is a first-party product over a closed project-owned Rust adapter;
it is not an authority layer inside the Engine crates. The external adapter owns the selected
project schema, trusted root, compatibility, and publication policy while composing reusable
`asset-catalog`, `authored-scene`, `entity-state`, `engine-spatial`, `voxel-asset`,
`voxel-annotation`, `voxel-convert`, `content-store`, `engine-inspector`, and render-projection
mechanisms.

Protocol 12 executes one named request at a time. Reads rebuild canonical owner views. Mutations carry
the accepted project hash plus narrower asset/revision/layer/plan guards, stage a complete candidate,
rerun downstream admission and renderer projection, atomically publish the project file, and return
a canonical reread. Voxel history and annotation documents are durable project data; conversion
plans and prepared history reverts remain private process state, are limited to one retained
candidate of each kind, and only matching identities can be applied. Project/scene/entity lifecycle,
full transforms, typed lights and entity components, and general mesh import/reimport each use named
operations rather than a universal editor mutation. Asset import candidates retain exact source,
settings, generated-ID, project, and plan identity and replace only their own prior generated assets.
Trusted host voxel/mesh/GLB/license paths are explicit, bounded, symlink-checked selections;
replacement is compare-and-swap guarded. Primitive/template generation, annotation semantics,
conversion material policy, and deterministic environment generation remain in their Rust owners.
Rejected or stale operations publish no bytes.

Protocol 11's `prepareVoxelObjectPlacement` is a narrow read-only exception to complete live-project
frames: it resolves the exact renderer resources for one canonical object that may have no live
instance yet. Its response is resource-only, bounded, content-matched, and incapable of creating or
updating a retained instance. Studio owns only the disposable ghost and one local candidate;
`attachVoxelObjectInstance` remains the focused one-instance downstream mutation. Protocol 12 adds
`attachVoxelObjectInstances` for 1–32 create-only placements when a composition would otherwise
repeat the entire project mutation and complete readout. The downstream adapter stages every
request-order owner allocation, typed component, complete admission, projection, and durable write
before one atomic publication; duplicate, colliding, stale, invalid, or over-quota entries publish
nothing. Success returns one ordered owner receipt and one canonical reread. Studio accepts that
readout once and clears its local single-placement undo candidate rather than pretending a member
represents the atomic batch. The resource candidate may survive a canonical reread solely to keep
the shared renderer mounted across placement.

Protocol 12 retains protocol 10's bounded identity for downstream entity components: canonical owner
entity, stable component type, and an optional exact inspector-contract identity advertised by the
adapter. It contains no component values, field schemas, mutation payloads, UI metadata, module
locations, or executable handles. Unknown components remain visible and read-only. The Engine shell
admits an immutable application-supplied contribution list, matches exact component/contract
identities, and gives panels only owner/project generations plus a single mutation-settlement
lease. Settlement serializes with core edits and accepts a downstream before/after hash receipt only
after a matching canonical `readProject`; late project, selection, or contract settlements are
discarded. The stock application explicitly composes the Engine-owned Voxel Object contribution.
Historical downstream-panel evidence validated the same identity matching,
static admission, outlet lifecycle, operation serialization, and canonical
acceptance without putting component values or operation names in the core
protocol. Product panels use the narrow lease for settlement, while built-in
Voxel operations already enter the same store through named core methods.
Named package carriers and explicit selected-consumer evidence replace ambient
sibling access. Ordinary downstream integration uses the adjacent complete Rust
facade and the checkout currently present on the machine. A selected downstream
compile, adapter, or browser proof is run only when that consumer is affected
or explicitly requested; its exact source commits are review evidence, not a
source dependency protocol.
Product-owned typed read/mutation contracts and game panels remain
downstream; Engine does not acquire game vocabulary, runtime plugin loading, a generic payload,
store/service-locator exposure, or a universal component bridge. The implemented contract and
limits are recorded in
[the downstream Entity inspector decision](studio-downstream-entity-inspector-extensions.md).

Applied voxel-object animation is a disposable presentation session, not another authoring model.
Studio selects a canonical instance and sends closed scrub/play/pause/sample/stop commands with an
explicit timestamp. The project-owned adapter retains `VoxelObjectPlayer`; admitted Rust clip
durations select the runtime frame, and render projection may return a retained
`setVoxelObjectFrame` patch after its complete project frame has been accepted. Studio applies that
patch to the existing authored renderer channel, waits for the renderer's successful generation
receipt, presents the authored frame duration, and only then advances a virtual explicit clock by
one pose. Slow adapter, transport, or renderer work therefore slows playback instead of skipping
canonical poses. The compact complete frame remains available for remounts and presentation changes,
and Studio periodically replaces from it so incremental history stays bounded. Retained frame
patches are bound to the exact accepted project-projection base generation, not to numeric render
handles alone. If a conversion candidate is currently displayed, Studio first compacts the applied
frame onto the saved canonical project base and performs a complete replacement; only subsequent
frames may use the retained patch path, and an unmatched patch fails closed. Each
readout names the saved initial frame separately from the transient posture and sampled frame.
Neither the player posture nor the browser sampling cadence enters project or object bytes, and
open, reread, close, and durable mutation discard the session. Conversion-candidate playback remains
a separate pre-apply inspection path. Pause and restore are user-priority controls: when one arrives
while a sample is in flight, Studio retains the latest control and dispatches it immediately after
that sample settles, with restore superseding an earlier queued pause. The adapter still receives
one ordered closed command at a time. The queued control is bound to the exact project and object
operation generations that admitted the sample; open, create, save-as, reread, close, or accepted
project replacement invalidates it before any old-scope settlement can dispatch.

Canonical voxel-object schema 1 remains sparse-run JSON and continues to anchor semantic hashes and
provenance. Dense retained meshes are deterministic derived presentation resources, not a binary
canonical sibling or second voxel authority. Complete definitions carry small resource descriptors;
packed little-endian bytes use explicit downstream publication and host resolution. Steady-state
frame swaps continue to carry no geometry. See
[the voxel mesh data-plane decision](topics/voxel/voxel-mesh-data-plane.md).

An applied voxel-object readout names its downstream-owned entity explicitly. The same identity is
present in the hierarchy, entity inspection, and renderer metadata, so the selected Entity
inspector can host one typed Voxel Object capability editor without heuristics. Its clip, loop,
scrub, play, pause, and restore controls are disposable presentation controls over the existing
Rust player. Conversion-candidate playback remains in the Voxel conversion workflow. This explicit
capability does not make Renderer Appearance a component registry and does not define a generic
downstream component AST or editor-operation tunnel.

Static and animated mesh appearance follows the same authority path. The downstream Rust adapter
validates the selected asset and animation clip, then projects typed resource descriptors,
`defineAnimatedMesh`, instance creation, and named playback. Studio's trusted Node host only resolves
bounded project-relative mesh sources and packed mesh-resource bytes, rejects symbolic links, and verifies
the admitted SHA-256 before the shared renderer consumes them; the browser has no filesystem
authority. A textual static-mesh source may omit texture coordinates or provide exactly one finite
`f32x2` UV per position vertex. `asset-import` validates that cardinality and retains the stream in
the renderer-neutral payload; deterministic packing uses V2 for UV-bearing meshes and V3 when an
optional normalized `f32x4` color stream is present, while meshes with neither retain the V1 shape.
The Three backend binds this ordinary geometry UV stream to
the exact resolved material texture without acquiring asset authority or entering the voxel-surface
shader specialization. Animated-mesh import enters through `asset-import`. A GLB remains an exact single-file
source. A JSON `.gltf` enters as an explicit immutable closure containing its root plus only the
bounded project-relative or data-URI buffers and PNG/JPEG images named by that root. The library
rejects network, absolute, traversal, ambiguous, missing, extra, unsupported, and over-quota
resources without resolving the filesystem; the CLI or downstream adapter owns symlink-checked
loading. It deterministically packs an admitted closure into the same self-contained GLB runtime
contract, then reuses the canonical scene/skin/clip parser and emits one bundle containing the GLB,
an `AnimatedMeshAsset` descriptor, catalog entry, and a manifest whose source hash binds the entire
closure. Reimport replaces that bundle as one structural transaction; the original host selection
is provenance, not a reopen dependency. `voxel-convert` remains the owner of
animation sampling and voxel materialization rather than becoming the Studio import operation.
The Three backend clones imported geometry and materials once into an asset-scoped retained
definition. Animated instances share those immutable render resources while `SkeletonUtils` gives
each instance its own hierarchy, skeleton, mixer, actions, and playback state. Destroying the last
instance retains the admitted definition for later recreation; definition replacement or renderer
disposal releases its geometry and materials exactly once without taking ownership of the caller's
resolved GLB source objects.

An animated mesh may also reference a bounded list of separately hash-addressed GLB clip packs.
Each pack is an immutable resource definition, so its decoded clips are shared while every target
instance retains independent actions and mixer state. A pack declares its producer/source/target/license
provenance and an exact target-rig signature: bounded decoded Three binding identities (not raw GLTF
names that contain reserved binding characters), parent hierarchy, explicit
local-matrix bind/rest convention and digest, one named root joint, and either in-place horizontal or
authored root-translation policy. The Three adapter derives the target and pack skeletons before
binding, rejects duplicate/missing joints, hierarchy or rest-pose drift, unsupported scale/non-root
translation channels, and external IDs that collide with embedded or earlier pack clips. In-place clips
may retain vertical root motion but must keep root horizontal translation constant. This is direct
target-rig playback, never a runtime retargeter or gameplay root-motion authority. Clip enumeration
reports an embedded or pack origin, while playback and exact normalized sampling use the existing
animated-mesh service surface.
The declared pack joint list may be the rooted subset relevant to its channels; the verified target
fingerprint nevertheless covers the complete coherent skinned target. Source local rest facts are
compared for that declared subset, and source inverse binds are compared when the clip pack carries
them. Source clip names bind exactly and case-sensitively; effective clip IDs remain the caller-facing
names. Rig-fingerprint inputs and effective clip readouts use explicit locale-independent code-unit
ordering so the same decoded identities produce the same digest and enumeration on every host.
The Three-local voxel-sprite experiment may additionally build a held-animation
frame bank from an already-admitted embedded or pack clip. A caller supplies
exact normalized samples or an explicit 8/12/24 Hz count expansion, capture
settings, and 1/4/8/16 actor-relative sectors; no Engine clock, gameplay clip
meaning, or durable cache format is introduced. The backend validates the
complete candidate against bounded frame, direction, resolution, pixel,
scene-wide resident-byte, ready-plus-candidate peak, and one-preparation limits before allocating
capture targets. Accounting conservatively includes each retained color/depth/
normal/coverage target, its persistent depth attachments, and one temporary
capture-depth attachment. It poses a disposable clone with a private mixer/skeleton,
captures each unique pose×direction once, and atomically replaces the prior
published bank only after every frame and its enhancement presentation is
ready. The candidate key/snapshot includes the animated handle, admitted asset
generation/hash, optional pack identity/hash, and the finite instance
position/quaternion/scale; any later candidate-source drift fails closed.
Published pixels are immutable snapshots: later live-source changes do not
invalidate them, and replacement remains explicit. Cancellation, stale-source
or capture failure, and replacement failure dispose only the candidate;
selection swaps resident textures through the
existing enhancement seam and never recaptures. Renderer-host and
application-host expose this as an explicit manually stepped experiment with
origin-qualified source/key, counters, CPU submission latency, estimated
per-bank and scene aggregate resident/peak bytes, switch cost, individual bank
destruction, and an explicit no-GPU-timing readout.
The mounted `RendererSurface` also owns deterministic animation inspection. A caller can stop its
automatic loop, pose one retained instance at a bounded normalized clip time through that same
mixer, submit the fixed frame, and encode individual PNGs, a contact sheet, and a revision-bound
JSON manifest. Sampled skinned-vertex bounds and bounded matrix/interpolation diagnostics are
renderer observations: they neither clamp the animation nor become project, asset, or gameplay
authority. Contact sheets may include the disposable origin, bounds, and contact-plane overlays,
but do not introduce a second loader, scene, skeleton, mixer, or render loop.
Angular owns the
asset/clip controls and renderer-neutral transform-manipulator intent, but does not construct a
private Three scene or replace materials inside an imported animated hierarchy merely to show
selection. The shared viewport projects the gizmo on its isolated debug overlay channel at the
selected object's world transform and maps renderer picks back to typed handles.

Angular owns forms, selection, transient brush state, and cancellation only. It structurally decodes
the closed protocol but does not recompute hashes, validate voxel semantics, replay history, cast an
authoritative ray, or forge conversion output. Renderer picks are untrusted hints transformed into
an authored-cell claim and re-cast by Rust. Entity selection and transform preview are disposable
derived frames applied through the shared renderer projection. During a gizmo drag, candidates stay
transient; pointer release and a change to another selection settle the candidate through the named
Rust transform operation. The selected tool remains disposable UI state across settlement and
selection, so the gizmo reanchors to the newly accepted or selected transform. Explicit Revert,
Escape, or pointer cancellation restores accepted or pointer-down state without publication. Voxel
brush and bounded conversion samples use the same disposable debug-layer frame path. Every
transform commit remains hash- and revision-guarded and is canonically reread from the Rust adapter.
Studio exposes entity/world transform and renderable-local transform as separate owners. Its origin
triad, admitted mesh bounds, contact plane, and numeric clearance are disposable observations of the
retained frame. “Align lower bound to contact plane” solves a new renderable-local offset and submits
the named revision-guarded scene mutation; it does not move entity or spatial authority.

Lighting preview is also disposable Studio presentation state. `Work Light` removes authored light
operations only from the frame being presented and adds one ambient and one directional editor light
with shadows disabled; `Authored Lights` presents the accepted Rust projection unchanged. The choice
is a backward-compatible host-user scene-view preference. Switching modes never edits project bytes,
changes the owner readout, or gives the browser authority over stored lights.

Studio's Node/HTTP host serves the isolated application, forwards bounded JSON to one selected
project-owned adapter, and owns one separate versioned host-user settings boundary. Generic Studio
starts at one stable address and discovers a trusted root-local `.rusty-studio.json` bootstrap only
when an operator opens a project. The bounded bootstrap names a command/argv and root-relative cwd;
it is not a registry, plugin marketplace, schema loader, or global service locator. Studio performs
the build/start/describe/open transaction and keeps the prior admitted session until the candidate
has passed all stages. The adapter remains the project schema and semantic authority. Preferences are keyed
by canonical project root, stored outside project bytes and browser storage, guarded by SHA-256
compare-and-swap and same-directory atomic replacement, and applied to renderer-host camera/input
configuration and Studio lighting presentation without creating gameplay authority. The host does
not interpret project content. Its managed serve path handshakes the selected adapter before
listening and publishes one operational identity containing the active root, selected source/build
identities, adapter binary hash, and negotiated protocol. A change to the selected session inputs
terminates the bounded host/adapter process group with an explicit restart-required receipt;
operational status remains observational and never becomes project authority. The host does not make
HTTP/browser behavior an Engine prerequisite. Ordinary Rust verification remains independent of
Studio, Node, the browser, and every sibling checkout. A downstream browser
proof is selected only by the affected downstream owner; it is not an ordinary
Studio gate or a cross-repository dependency.

The persistent generic service is the preferred interactive Studio entrypoint where installed, but
its one selected adapter and active project are process-wide. It is a single-active-authoring-session
service, not a multi-agent isolation boundary. Concurrent automation uses separate host processes,
ports, settings roots, and—when mutating—separate project copies until session isolation is explicitly
implemented and proved.

## Downstream ownership

A game built on Engine should own its complete behavioral story:

1. game-specific authored configuration, schema, and meaning layered on reusable Engine formats;
2. semantic admission into concrete game components;
3. named services and centrally invoked systems;
4. explicit scheduling and consequential typed facts/events;
5. runtime snapshot, project storage location, and product migration policy; and
6. input, readout, and typed presentation intents consumed by the shared renderer border.

Historical reference-demo evidence exercised those surfaces, including an
`ExtractionBeacon` addition, without changing Engine public vocabulary. That
completed example is not a current Studio consumer or proof gate; it records
the intended one-way dependency direction.

## Architectural exclusions

The current provider deliberately excludes:

- a complete game runtime, session facade, or universal scheduler;
- a strict ECS query/update framework or implicit component mutation rights;
- game-specific component families, events, rules, and persistence schemas;
- a service locator, dependency-injection container, or plugin registry;
- a universal command/event union, behavior graph, or Engine-owned authored
  gameplay language (the optional rules envelope carries an opaque
  downstream-owned payload);
- replay or certification as a prerequisite for ordinary execution;
- Node, browser, Three, WebAudio, DOM, Studio, or editor dependencies in ordinary host-neutral Rust
  mechanism crates (the complete facade may depend on the explicit `renderer-webview-host` platform
  leaf; Node-backed source remains isolated under `rules/`, `render/`, or `studio/`);
- HTTP serving, public URLs, browser storage, same-origin behavior, or a browser event loop as an
  Engine capability prerequisite;
- Asha's former Gameplay Fabric, runtime facade, provider/bundle lifecycle, and bridge topology; and
- an operational dependency on `rusty-engine-demo` or any sibling checkout.

These are evidence-based boundaries, not permanent bans on future capabilities. A named consumer
may justify a new mechanism, but it should extend the direct authority path rather than recreate a
second structural center.

## Verification and evolution

`./scripts/verify.sh` is the ordinary Rust-provider gate. It requires no Node installation or demo
checkout and covers locked metadata, standalone path auditing, documentation links, formatting,
workspace/provider fixtures (including renderer-neutral model/projection), Clippy, and
byte-reproducible conversion. The separately installed `rules/` workspace has
its own generated-contract and headless TypeScript gate, and `render/` has its
own frozen TypeScript/browser gate. The renderer gate also proves a byte-reproducible embedded
artifact and a real Wry/WebKit mount. The provider gate creates a clean Rust-only consumer with
exactly one dependency; its optional post-push form checks a public exact review revision.

Validation follows ownership: focused Rust/headless tests prove Rust mechanisms;
headless cross-language tests prove rules artifacts plus renderer-neutral
contracts and projection; focused Three/WebGL evidence
proves the backend; Chromium proves real DOM, WebAudio, input, canvas, and browser lifecycle; a
packaged Electron/Tauri host will own its packaging/lifecycle proof when one exists; and Demo or
Studio proves user-visible behavior in its supported host. Chromium success does not turn HTTP or
browser semantics into Engine requirements, while headless success does not replace real host tests
for genuinely host-owned behavior.

Black-box product playtesting is a third, separately recorded evidence layer
above focused Rust mechanisms and renderer/application-host contract tests. It
uses visible output and ordinary public input to judge a composed product; it
does not add Engine test authority, a hidden readback, or a second renderer or
control path. Browser and native playtests remain distinct host evidence.
Model-driven runs are completion, review, nightly, or release evidence rather
than an every-commit requirement. A confirmed defect should normally gain the
smallest deterministic regression at its owning layer. See
[Product playtesting and evidence authority](topics/development/product-playtesting.md).

Source organization follows [rust-style.md](topics/development/rust-style.md): one primary behavior owner or cohesive
type family per file, thin crate roots, and no one-type-per-file rule. File size is a review signal,
not a CI policy.
