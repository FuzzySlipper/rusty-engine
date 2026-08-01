# Rusty Engine design

Status: current provider architecture

Rusty Engine is a standalone, host-neutral mechanism provider for object-centric games. Game policy,
orchestration, game-specific project schemas, product persistence aggregates, input meaning, and the
meaning of presentation intents belong to concrete downstream products. Reusable scene, entity,
asset, voxel-authoring, serialization, and persistence mechanisms may live here when they remain
independent of one game's vocabulary. Shared renderer-neutral projection and renderer host
mechanisms live here so the demo and Studio cannot drift into independent renderers. The first
reference consumer is
[`rusty-engine-demo`](https://github.com/FuzzySlipper/rusty-engine-demo), extracted after it had
proved the provider boundary in this repository.

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
- Add traits, registries, protocols, and compatibility layers only after multiple concrete consumers
  prove the same seam.
- Keep host-neutral mechanisms, renderer-neutral projection, backend realization, browser/webview
  lifecycle, and product-shell policy in visibly separate owners.
- Maintain a one-way dependency: consumers may depend on Engine; Engine never imports or checks out
  a consumer to verify itself.

Object-centric does not mean Unity-style component scripts or an ECS scheduler. It means entity
identity and typed component data remain easy to inspect while behavior is owned by explicit code.

## System at a glance

```text
downstream game
  entities / game components / services / scheduling / persistence / presentation intent
                     |
                     +--> entity-state
                     +--> gameplay-mechanics (optional)
                     +--> gameplay-rules (optional package support)
                     +--> engine-spatial -------------------+
                     +--> voxel-asset                       |
                     +--> render-model --> render-projection----+
                                      \-> render-presentation---|
                                                          v
foundation: core-assets / core-ids / core-math / core-space / core-time / core-voxel
services:   svc-volume / svc-spatial / svc-collision / svc-pathfinding / svc-rng / svc-mesh

offline only: GLB + request --> voxel-convert --> canonical voxel-asset JSON --> downstream admission

isolated rules workspace: downstream TS candidate --> canonical package JSON --> gameplay-rules (Rust)
isolated renderer workspace: retained JSON --> render-projection (TS) --> Three backend / host adapters
```

No Engine crate knows the downstream game's component families, event vocabulary, game-specific
stored-project schema, or browser API. The Rust render crates know only renderer-neutral values and
explicit read-only provider views; the isolated renderer workspace knows no gameplay authority.

## Entity component boundary

`entity-state::EntityState` owns reusable entity invariants:

- stable identity, name, lifecycle, labels, and explicit relationships;
- one instance-owned typed component store with stable authored type identities;
- built-in transform, bounds, collision, renderable, kinematic, controller, and asset-binding
  components;
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

`engine-inspector` is the read-only cross-owner leaf for the admitted result. Its mechanics report
projects the fixed seven-kind presence table, evaluated stat stages and attributed decisions,
resolved tracks, intrinsic/effect activations, joined indexed inventory, unique item identities,
equipment assignments, and bounded traversal evidence. Its damage-receipt projection preserves
the complete bounded stage/decision/change/fact detail. Inspection neither mutates components nor
becomes a renderer, product schema, save owner, event journal, or replay authority.

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

## Optional gameplay rules support

Rules-heavy consumers may opt into a `gameplay-rules` sibling crate without
making it a dependency of `entity-state` or `gameplay-mechanics`. It owns only
a strict bounded package envelope, exact package dependencies, canonical
encoding and fingerprints, source provenance, bounded diagnostics, and
deterministic package-set resolution. The payload is opaque JSON.

The downstream game owns the payload schema, every semantic definition and
compiler rule, mechanics bindings, orchestration, persistence, and execution.
TypeScript may produce immutable build-time candidates in an isolated
workspace, while Rust remains the semantic and runtime authority. Direct Rust
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

This is not a universal gameplay IR. Engine defines no formula, predicate,
operation, action, condition, effect, behavior, evaluator, registry, runtime
session, scheduler, or d20 vocabulary through this surface. A mechanics-only
game ignores it entirely. The exact schema-1 API, bounds, ordering, failure
identity, TypeScript isolation, and first-consumer proof are frozen in
[gameplay-rules-contract.md](gameplay-rules-contract.md).

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
rule.

## Foundation and service crates

The smaller `core-*` and `svc-*` crates are normal workspace packages, not an origin-oriented donor
layer. They provide narrow identities, coordinate types, time values, voxel storage, collision,
pathfinding, deterministic RNG, and meshing. Their exact donor sources and adaptations are recorded
in [donor-provenance.md](migration/donor-provenance.md).

These crates expose mechanism rather than policy. For example, `svc-pathfinding` can propose a path
but does not own AI intent; `svc-rng` creates a scoped deterministic stream but does not decide what
is random; `svc-mesh` emits geometry but owns no renderer.

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

`render-projection` owns deterministic, fail-atomic adapters over explicit read-only inputs. Entity
projection reads `EntityState`; voxel projection reads `VoxelCollisionScene`; authored projection
accepts one ordinary appearance/resource aggregate; debug projection accepts typed overlays. Each
projector computes against cloned retained state, validates the complete frame, and commits stable
handles only when construction succeeds. The TypeScript retained projection stages frames with
copy-on-write records: unchanged immutable definitions are structurally shared, while every
record named by a mutation is privately copied before validation or commit. Missing resources are
classified rather than resolved through an ambient registry.

`render-presentation` owns the other renderer-neutral family: typed audio sources and impulses,
world/entity billboards, bounded particle emitters and bursts, telemetry overlay requests, and
animation graph/controller/playback projection. Its controller is an explicitly invoked mechanism,
not an update loop or gameplay behavior graph. Fixed-point parameters, stable priority ordering,
transition timing, and blend resolution remain deterministic; persistence journals, certification
hashes, and provenance envelopes do not exist. Every projector can validate a complete domain
batch, while `PresentationProjectorSet` provides one fail-atomic mixed-domain frame boundary.
Resource checks use only immutable kind/content identity supplied by the caller.

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

The Three adapter retains that hierarchy in a separate scene. The browser surface owns one
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

External consumers pin all four render packages to one exact public Engine commit. Their package
preparation and peer graph are independently checked in a clean temporary consumer, so the shared
boundary is not a workspace-only convention. Operational commands, CI ownership, and explicit
limitations are recorded in [rendering-operations.md](rendering-operations.md).

## Host and platform boundary

Rusty Engine is not a web-game engine. The current first-party renderer and tool UI use web
technology because Three/WebGL, DOM, WebAudio, and Chromium provide a practical backend, editor
surface, and real integration environment. They are replaceable backend and host choices rather
than Engine authority or a requirement that products be delivered over HTTP.

The boundary is layered deliberately:

- ordinary Rust crates own host-neutral state, services, authoring, persistence, and
  renderer-neutral projection;
- `@rusty-engine/render-contracts` and `@rusty-engine/render-projection` own strict data decoding and
  retained projection without Three, DOM, browser lifecycle, or HTTP loading;
- `@rusty-engine/renderer-three` owns Three/WebGL realization. Its current browser-surface modules
  are explicit backend adapters, not a renderer-neutral or Engine-wide platform API;
- `@rusty-engine/renderer-host` owns the current browser/webview lifecycle, DOM overlays, WebAudio,
  input capture, inspection, and editor-host behavior; and
- Demo and Studio own their product shells, window/application policy, semantic input mapping,
  resource locations, and user-facing acceptance.

Assets and project data cross public borders through explicit identities, descriptors, bytes,
resolvers, and typed file operations. Core capability must not depend on public URLs, `fetch`,
same-origin behavior, browser storage, an HTTP control route, or Playwright-only mutation hooks.
Electron or Tauri may reuse the webview-facing adapters while supplying local window, filesystem,
resource, and lifecycle ports. A headless process or future backend can reuse the authoritative and
renderer-neutral layers without emulating a website.

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
The reviewed Loading Bay application proves the same identity matching, static admission, outlet
lifecycle, operation serialization, and canonical acceptance with its independently typed Weapon
contract and Rust owner. Its panel calls only `readLoadingBayWeapon` and
`replaceLoadingBayWeapon`; the core protocol sees neither value nor operation. The external panel
uses the narrow lease for settlement, while built-in Voxel operations already enter the same store
through named core methods. Exact Git package pins and an explicit cross-repository gate replace
ambient sibling access. Product-owned typed read/mutation contracts and game panels remain
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
authority. Animated-mesh import enters through `asset-import`. A GLB remains an exact single-file
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

Studio's Node/HTTP host serves the isolated application, forwards bounded JSON to the explicit
adapter binary, and owns one separate versioned host-user settings boundary. Preferences are keyed
by canonical project root, stored outside project bytes and browser storage, guarded by SHA-256
compare-and-swap and same-directory atomic replacement, and applied to renderer-host camera/input
configuration and Studio lighting presentation without creating gameplay authority. The host does
not interpret project content or
make HTTP/browser behavior an Engine prerequisite. Ordinary Rust verification remains independent
of Studio, Node, the browser, and any sibling checkout; the cross-repository demo and Chromium proof
is an explicit integration gate.

## Downstream ownership

A game built on Engine should own its complete behavioral story:

1. game-specific authored configuration, schema, and meaning layered on reusable Engine formats;
2. semantic admission into concrete game components;
3. named services and centrally invoked systems;
4. explicit scheduling and consequential typed facts/events;
5. runtime snapshot, project storage location, and product migration policy; and
6. input, readout, and typed presentation intents consumed by the shared renderer border.

The reference demo owns all of those surfaces, including its `ExtractionBeacon` addition. That
feature extended the downstream schema, service, snapshot, browser readout, and presentation without
changing the Engine revision or public vocabulary. Its second authored room composition then reused
the settled meanings with no Rust change. This is the intended dependency direction.

## Promotion rule

Do not move a game concept into Engine merely because it could be reusable. Promotion requires at
least a second concrete consumer and a smaller stable seam that can be stated without importing
either product's policy. When that evidence exists:

1. compare both real call sites and data lifecycles;
2. extract only the shared mechanism;
3. leave game names, orchestration, schemas, and presentation meaning downstream;
4. add focused provider tests independent of either product; and
5. keep Engine verification free of downstream checkouts.

Duplication across one early consumer is cheaper than a premature plugin API,
registry, or universal gameplay abstraction. A first consumer may still earn a
small representation-neutral support seam when the useful behavior can be
stated without importing that consumer's vocabulary, the direct alternative
would duplicate transport/admission infrastructure rather than product
meaning, and the boundary is reviewed before implementation. The optional
`gameplay-rules` envelope is this narrow exception; Rusty D20's actual rule
schema and compiler remain downstream.

This rule governs newly invented abstractions; it is not a deletion filter during an approved
successor migration. A proven donor capability required by a named first-party consumer may be
preserved or adapted before two new implementations exist. Demo and Studio already establish
concrete demand for shared rendering and authoring mechanisms. In a functionality-equivalence
campaign, preserve useful behavior, remove obsolete topology, name every replacement or exclusion,
and validate the successor-owned boundary rather than requiring the first consumer to rediscover the
missing mechanism privately.

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
- Node, browser, Three, WebAudio, DOM, Studio, or editor dependencies in ordinary Rust-provider
  work (Node-backed packages remain isolated under `rules/`, `render/`, or
  `studio/` with their own gates);
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
own frozen TypeScript/browser gate. A post-push gate installs the four render packages from the exact
public Engine commit into a clean temporary consumer; the external demo retains its own complete
product gate against an exact Engine revision.

Validation follows ownership: focused Rust/headless tests prove Rust mechanisms;
headless cross-language tests prove rules artifacts plus renderer-neutral
contracts and projection; focused Three/WebGL evidence
proves the backend; Chromium proves real DOM, WebAudio, input, canvas, and browser lifecycle; a
packaged Electron/Tauri host will own its packaging/lifecycle proof when one exists; and Demo or
Studio proves user-visible behavior in its supported host. Chromium success does not turn HTTP or
browser semantics into Engine requirements, while headless success does not replace real host tests
for genuinely host-owned behavior.

Source organization follows [rust-style.md](topics/development/rust-style.md): one primary behavior owner or cohesive
type family per file, thin crate roots, and no one-type-per-file rule. File size is a review signal,
not a CI policy.
