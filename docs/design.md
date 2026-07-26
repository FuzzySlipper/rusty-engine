# Rusty Engine design

Status: current provider architecture

Rusty Engine is a standalone mechanism provider for object-centric games. Game policy,
orchestration, project schemas, persistence aggregates, input, and the meaning of presentation
intents belong to concrete downstream products. Shared renderer-neutral projection and renderer
host mechanisms live here so the demo and Studio cannot drift into independent renderers. The first
reference consumer is
[`rusty-engine-demo`](https://github.com/FuzzySlipper/rusty-engine-demo), extracted after it had
proved the provider boundary in this repository.

Historical product experiments and measurements live in
[experiment-results.md](experiment-results.md). Completed migration and extraction decisions live
in [migration-cluster-ledger.md](migration-cluster-ledger.md).

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
- Maintain a one-way dependency: consumers may depend on Engine; Engine never imports or checks out
  a consumer to verify itself.

Object-centric does not mean Unity-style component scripts or a dynamic ECS world. It means entity
identity and typed capability data remain easy to inspect while behavior is owned by explicit code.

## System at a glance

```text
downstream game
  entities / game components / services / scheduling / persistence / presentation intent
                     |
                     +--> entity-state
                     +--> engine-spatial -------------------+
                     +--> voxel-asset                       |
                     +--> render-model --> render-projection----+
                                      \-> render-presentation---|
                                                          v
foundation: core-assets / core-ids / core-math / core-space / core-time / core-voxel
services:   svc-volume / svc-spatial / svc-collision / svc-pathfinding / svc-rng / svc-mesh

offline only: GLB + request --> voxel-convert --> canonical voxel-asset JSON --> downstream admission

isolated renderer workspace: retained JSON --> render-projection (TS) --> Three/host surfaces
```

No Engine crate knows the downstream game's component families, event vocabulary, stored-project
schema, or browser API. The Rust render crates know only renderer-neutral values and explicit
read-only provider views; the isolated renderer workspace knows no gameplay authority.

## Entity capability boundary

`entity-state::EntityState` owns reusable entity invariants:

- stable identity, name, and lifecycle;
- transform;
- collision flags;
- renderable identity and visibility;
- kinematic extent and velocity;
- read-only views and projection nodes; and
- snapshot encoding plus one atomic `EntityCommandBatch` mutation boundary.

It does not expose arbitrary ECS queries, component matching, implicit scheduling, or game-specific
state. A downstream service uses the batch only when several reusable capability changes must commit
together. Ordinary game component mutation remains with the downstream feature owner.

## Spatial authority and derived mechanisms

`engine-spatial::VoxelCollisionScene` holds canonical material voxels alongside projections derived
from them:

- `svc-spatial` and `svc-volume` store deterministic voxel state;
- `svc-collision` builds Parry-backed collision queries;
- `svc-pathfinding` derives bounded navigation projections;
- `svc-mesh` derives deterministic visible-face meshes; and
- `VoxelEditService` validates an expected-revision transaction and replaces the complete coherent
  result only after every affected projection succeeds.

`KinematicMotionSystem` is a centrally invoked mechanism over explicit body views. It supplies no
game loop, actor policy, or component-local update callback. A downstream runtime decides when to
invoke it and what accepted facts mean.

`TriggerVolumeSystem` is the sole overlap authority for registered trigger volumes. Each
`KinematicTriggerDefinition` names one trigger entity and a typed `TriggerGeometrySource` that
selects where the volume's live AABB comes from during reconciliation:

- `ActiveCollision` (the default and historical behavior) requires the trigger entity to be active
  with an enabled collision capability in addition to bounds and a composed world transform. Such a
  trigger entity is also a solid motion obstacle, because kinematic motion treats every active
  collision body as solid.
- `EntityBounds` derives the same AABB from the canonical entity lifecycle, bounds, and composed
  world transform without consulting the collision capability at all. The trigger entity therefore
  senses subjects without ever becoming a solid obstacle; its collision state, when present, is
  irrelevant to sensing and produces no diagnostics.

The boundary stays one-directional: motion solidity is owned exclusively by the collision
capability (`EntityMotionService` obstacles are active-collision entities only), while trigger
sensing is owned exclusively by `TriggerVolumeSystem` over `EntityState`. Subject eligibility is
unchanged by the geometry source — a subject always requires the canonical active-collision and
entity-state rules — and reconciliation stays bounded, deterministic, typed, snapshot-capable, and
fail-closed with actionable stale/missing bounds or transform diagnostics. Downstream consumers
must not recompute overlap, special-case motion, or maintain a second spatial authority; geometry
source is a per-definition Engine vocabulary with no gameplay semantics, tag exceptions, or
ambient callbacks. Trigger snapshots remain schema 1: definitions written before the geometry seam
decode as `ActiveCollision`, and new definitions round-trip their geometry source exactly.

Collision, navigation, and meshes must not become independent world authorities. Optimization may
make rebuilding more incremental, but it must retain the same source revision and atomic coherence
rule.

## Foundation and service crates

The smaller `core-*` and `svc-*` crates are normal workspace packages, not an origin-oriented donor
layer. They provide narrow identities, coordinate types, time values, voxel storage, collision,
pathfinding, deterministic RNG, and meshing. Their exact donor sources and adaptations are recorded
in [donor-provenance.md](donor-provenance.md).

These crates expose mechanism rather than policy. For example, `svc-pathfinding` can propose a path
but does not own AI intent; `svc-rng` creates a scoped deterministic stream but does not decide what
is random; `svc-mesh` emits geometry but owns no renderer.

## Shared rendering boundary

`render-model` owns the complete versioned retained-frame vocabulary: stable handles, hierarchy,
primitive and mesh geometry, materials, textures, sprites, static and animated assets, lighting,
editor-grid descriptions, picks, validation, and canonical JSON. It contains no state store,
catalog, filesystem, renderer object, runtime facade, or replay requirement.

`render-projection` owns deterministic, fail-atomic adapters over explicit read-only inputs. Entity
projection reads `EntityState`; voxel projection reads `VoxelCollisionScene`; authored projection
accepts one ordinary appearance/resource aggregate; debug projection accepts typed overlays. Each
projector computes against cloned retained state, validates the complete frame, and commits stable
handles only when construction succeeds. Missing resources are classified rather than resolved
through an ambient registry.

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
[voxel-model-conversion.md](voxel-model-conversion.md).

`voxel-convert` is an offline authoring/build tool. It accepts one explicit request and bounded GLB
source, completes conversion and validation before touching its target, then atomically installs the
canonical artifact. Runtime consumers depend on `voxel-asset`, not the converter or GLB parser.

The checked request, licensed source, and canonical output remain here because they are provider
verification fixtures, not demo content. See [voxel-asset-format.md](voxel-asset-format.md).

## Studio authoring boundary

The isolated `studio/` workspace is a first-party product over a closed project-owned Rust adapter;
it is not an authority layer inside the Engine crates. The external adapter owns the selected
project schema, trusted root, compatibility, and publication policy while composing reusable
`asset-catalog`, `authored-scene`, `entity-state`, `engine-spatial`, `voxel-asset`,
`voxel-annotation`, `voxel-convert`, `content-store`, `engine-inspector`, and render-projection
mechanisms.

Protocol 6 executes one named request at a time. Reads rebuild canonical owner views. Mutations carry
the accepted project hash plus narrower asset/revision/layer/plan guards, stage a complete candidate,
rerun downstream admission and renderer projection, atomically publish the project file, and return
a canonical reread. Voxel history and annotation documents are durable project data; conversion
plans and prepared history reverts remain private process state, are limited to one retained
candidate of each kind, and only matching identities can be applied. Project/scene/entity lifecycle,
full transforms, typed lights and capabilities, and general mesh import/reimport each use named
operations rather than a universal editor mutation. Asset import candidates retain exact source,
settings, generated-ID, project, and plan identity and replace only their own prior generated assets.
Trusted host voxel/mesh/GLB/license paths are explicit, bounded, symlink-checked selections;
replacement is compare-and-swap guarded. Primitive/template generation, annotation semantics,
conversion material policy, and deterministic environment generation remain in their Rust owners.
Rejected or stale operations publish no bytes.

Static and animated mesh appearance follows the same authority path. The downstream Rust adapter
validates the selected asset and animation clip, then projects typed resource descriptors,
`defineAnimatedMesh`, instance creation, and named playback. Studio's trusted Node host only resolves
bounded project-relative GLB bytes, rejects symbolic links, and verifies the admitted SHA-256 before
the shared renderer consumes them; the browser has no filesystem authority. Angular owns the
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

1. authored configuration and its schema;
2. semantic admission into concrete game components;
3. named services and centrally invoked systems;
4. explicit scheduling and consequential typed facts/events;
5. runtime snapshot and project persistence policy; and
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

Duplication across one early consumer is cheaper than a premature plugin API, registry, or universal
gameplay abstraction.

## Architectural exclusions

The current provider deliberately excludes:

- a complete game runtime, session facade, or universal scheduler;
- a strict ECS query/update framework or implicit component mutation rights;
- game-specific component families, events, rules, and persistence schemas;
- a service locator, dependency-injection container, or plugin registry;
- a universal command/event union, behavior graph, or authored gameplay language;
- replay or certification as a prerequisite for ordinary execution;
- Node, browser, Three, WebAudio, DOM, Studio, or editor dependencies in ordinary Rust-provider
  work (they remain isolated under `render/` with their own gate);
- Asha's former Gameplay Fabric, runtime facade, provider/bundle lifecycle, and bridge topology; and
- an operational dependency on `rusty-engine-demo` or any sibling checkout.

These are evidence-based boundaries, not permanent bans on future capabilities. A named consumer
may justify a new mechanism, but it should extend the direct authority path rather than recreate a
second structural center.

## Verification and evolution

`./scripts/verify.sh` is the ordinary Rust-provider gate. It requires no Node installation or demo
checkout and covers locked metadata, standalone path auditing, documentation links, formatting,
workspace/provider fixtures (including renderer-neutral model/projection), Clippy, and
byte-reproducible conversion. The separately installed `render/` workspace has its own frozen
TypeScript/browser gate. A third post-push gate installs the four render packages from the exact
public Engine commit into a clean temporary consumer; the external demo retains its own complete
product gate against an exact Engine revision.

Source organization follows [rust-style.md](rust-style.md): one primary behavior owner or cohesive
type family per file, thin crate roots, and no one-type-per-file rule. File size is a review signal,
not a CI policy.
