# Rusty Engine design

Status: current provider architecture

Rusty Engine is a standalone Rust mechanism provider for object-centric games. Game policy,
orchestration, project schemas, persistence aggregates, input, and presentation belong to concrete
downstream products. The first reference consumer is
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
  entities / game components / services / scheduling / persistence / presentation
                     |
                     +--> entity-state
                     +--> engine-spatial -------------------+
                     +--> voxel-asset                       |
                                                          v
foundation: core-assets / core-ids / core-math / core-space / core-time / core-voxel
services:   svc-volume / svc-spatial / svc-collision / svc-pathfinding / svc-rng / svc-mesh

offline only: GLB + request --> voxel-convert --> canonical voxel-asset JSON --> downstream admission
```

No Engine crate knows the downstream game's component families, event vocabulary, stored-project
schema, browser API, or renderer package.

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

## Durable voxel assets and offline conversion

`voxel-asset` owns a strict schema, semantic validation, canonical encoding, content identity, and
bounded conversion-input values. It performs no filesystem access, mesh parsing, runtime mutation,
or project admission.

`voxel-convert` is an offline authoring/build tool. It accepts one explicit request and bounded GLB
source, completes conversion and validation before touching its target, then atomically installs the
canonical artifact. Runtime consumers depend on `voxel-asset`, not the converter or GLB parser.

The checked request, licensed source, and canonical output remain here because they are provider
verification fixtures, not demo content. See [voxel-asset-format.md](voxel-asset-format.md).

## Downstream ownership

A game built on Engine should own its complete behavioral story:

1. authored configuration and its schema;
2. semantic admission into concrete game components;
3. named services and centrally invoked systems;
4. explicit scheduling and consequential typed facts/events;
5. runtime snapshot and project persistence policy; and
6. input, readout, and presentation borders.

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
3. leave game names, orchestration, schemas, and presentation downstream;
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
- browser, renderer, TypeScript, Node, Studio, or editor dependencies in ordinary Engine work;
- Asha's former Gameplay Fabric, runtime facade, provider/bundle lifecycle, and bridge topology; and
- an operational dependency on `rusty-engine-demo` or any sibling checkout.

These are evidence-based boundaries, not permanent bans on future capabilities. A named consumer
may justify a new mechanism, but it should extend the direct authority path rather than recreate a
second structural center.

## Verification and evolution

`./scripts/verify.sh` is the ordinary provider gate. It requires no Node installation or demo
checkout and covers locked metadata, standalone path auditing, documentation links, formatting,
workspace/provider fixtures, Clippy, and byte-reproducible conversion.

Source organization follows [rust-style.md](rust-style.md): one primary behavior owner or cohesive
type family per file, thin crate roots, and no one-type-per-file rule. File size is a review signal,
not a CI policy.
