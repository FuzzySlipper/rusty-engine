# Asha-to-Rusty Engine migration cluster ledger

Status: M0-M7B accepted; M9 standalone implementation is under review; M10 and M11 are planned in
Den; M7C remains deliberately unscheduled

Evidence baselines:

- original Rusty Engine walking-spike commit: `65c528975328b2d92384dea91adf1d21c1779bf4`;
- pinned Asha donor commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`; and
- standalone Rusty Engine implementation head: `6fde873921ed4308a7e3949b2da8fc28810e0ff9`.

## Purpose

This ledger records how feature families moved from Asha evidence into complete Rusty Engine
capabilities. It is not a crate-copy checklist, an active task tracker, or a presumption that the
rest of Asha should be ported.

Rusty Engine is the canonical repository. New work begins from its current design and Den project.
Asha remains a source locator and historical comparison. A port back into Asha is not part of the
migration schedule and would require a new explicit owner decision.

Den owns current task, dependency, and review state. This file owns durable cluster boundaries,
dispositions, ordering lessons, and deferred decisions.

## Inheritance policy: absence by default

- Asha code enters Rusty Engine only for a named successor consumer.
- The complete dependency closure, source revision, license status, fixtures, and local adaptations
  are audited before transfer.
- A useful implementation may be copied, narrowed, or rewritten behind a successor-owned boundary;
  its original API and architecture are not automatically preserved.
- Asha documents and tasks are research evidence, not inherited specifications or backlog.
- Runtime facade, Gameplay Fabric, replay/certification, lifecycle, provider, bundle, protocol, and
  bridge structures remain excluded unless a future decision explicitly reopens them.
- Anything not selected remains absent. Recovering a useful donor later is safer than deleting
  accidental structural inheritance.

The complete crate inventory is linked from [donor-provenance.md](donor-provenance.md). Its
portability classifications shortlist evidence; they do not waive a fresh consumer and dependency
audit.

## Unit of migration

A migration cluster is a complete player-, author-, or runtime-visible capability:

```text
authored configuration
  -> strict stored data
  -> Rust semantic admission
  -> entity components and explicit relationships
  -> one named Service or System
  -> authoritative mutation and typed outcomes
  -> derived spatial/presentation state
  -> snapshot or project persistence where meaningful
  -> headless and real-product acceptance
```

Compiling a donor crate or reproducing an isolated algorithm is not a completed cluster.
Cross-cutting helpers are harvested from working features rather than scheduled as speculative
frameworks.

## Closure gates

Every future cluster must preserve these properties:

1. **Locality:** entity data leads directly to the responsible Rust behavior owner.
2. **Single authority:** Rust owns live gameplay; TypeScript authors content and hosts presentation.
3. **Explicit time:** recurring work is a named host phase and delayed work is a stable intent.
4. **Typed consequences:** cross-domain events are meaningful and explicitly routed; ordinary work
   stays as direct calls and typed returns.
5. **Derived projections:** collision, navigation, mesh, rendering, audio, and UI do not become
   parallel gameplay truth.
6. **Persistence clarity:** project content and runtime snapshots remain separate; neither requires
   event replay.
7. **Product proof:** a product-facing cluster reaches the real browser/Three path as well as
   focused tests.
8. **Bounded amplification:** closeout identifies the surfaces changed by a content variation and a
   behavior variation.
9. **Standalone operation:** no sibling checkout or host-global package link becomes operational.
10. **Old-spine audit:** the change does not silently restore Asha's structural center.

## Completed progression

| ID | Outcome | Den tasks | Durable result |
|---|---|---|---|
| M0 | Object-centric baseline | Pre-ledger | `EntityState`, direct services, typed door/encounter consequences, scheduling, snapshots, collision-aware motion, and a retained browser renderer established the replacement spine. |
| M1 | Navigation and autonomous locomotion | #6103 | `EnemyNavigationSystem` derives routes from canonical voxel state, resolves collision-aware movement, persists durable intent, and reports typed arrival/block/unreachable outcomes. |
| M2A | Player input, controller, and camera | #6104 | Browser devices resolve to typed actions; Rust owns accepted player pose and collision; the follow camera is presentation-only derived state. |
| M2B | Generated voxel environment and mesh | #6105 | One seeded voxel authority feeds collision, navigation, and visible mesh; generation parameters are content and the entity door remains the canonical aperture gate. |
| M3 | Combat, health, and weapons | #6106 | `CombatService` resolves authored attack intent against live transforms and voxel occlusion, mutates typed health/weapon state, and emits defeat as a real encounter consequence. |
| M4 | Presentation feedback | #6111-#6114 | Accepted facts produce rebuildable posture and disposable animation/audio/particle/billboard cues without gameplay writes or replay requirements. |
| M5 | Stored project admission | #6117-#6120 | Strict schema-v7 projects, assets, scenes, entities, relationships, and voxel sources pass one Rust semantic admission path before a session exists. |
| M6 | Project persistence and migration | #6121-#6124 | Canonical atomic project storage and explicit schema migration remain separate from schema-v9 live runtime snapshots. |
| M7A | Live voxel edits | #6125-#6128 | One expected-revision `VoxelEditService` transaction coherently replaces authoritative voxels plus collision, navigation, and mesh projections; snapshot and authored-save meanings are distinct. |
| M7B | Offline voxel asset conversion | #6129-#6131 | A bounded Rust converter produces a canonical durable voxel asset that enters through ordinary project admission and then behaves like authored voxels. |
| M9 | Standalone repository | #6132-#6136 | The selected Rust donors, narrow local render edge, licensed fixture, verification, and CI are repository-local; the operational Asha dependency audit is empty. Implementation is complete at the head above and awaits independent review closeout. |

Detailed measurements, implementation commits, browser behavior, and limitations are retained in
[experiment-results.md](experiment-results.md). Exact donor sources and adaptations are in
[donor-provenance.md](donor-provenance.md).

## What the migration established

The completed clusters answered the original architectural questions:

- Direct Rust services and concrete component families keep substantial gameplay changes local.
- Typed events can carry meaningful consequences without becoming the universal execution route.
- TypeScript remains useful as code-as-content without owning a second live state lifecycle.
- A canonical voxel authority can serve generation, collision, navigation, meshing, editing, and
  imported content.
- Presentation can be rich, disposable, and restartable above accepted state and facts.
- Stored projects and runtime snapshots can evolve independently without replay certification.
- Useful low-level donor code fits below the new spine after a bounded closure audit.
- A narrow local Three renderer fits above the spine without restoring Asha's browser package graph.
- The repository builds and proves its real product behavior without an Asha checkout.

The migration therefore ended as a successor adoption, not a patch queue intended to flow back into
Asha.

## Internalized low-level crate boundary

M9 copied only the accepted normal Rust closure. These are now ordinary workspace members under
`rust/crates`, with no origin-based directory boundary:

- foundations: `core-assets`, `core-ids`, `core-math`, `core-space`, and `core-time`;
- voxel state: `core-voxel`; and
- services: `svc-volume`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, `svc-rng`, and
  `svc-mesh`.

Their pinned source and bounded adaptations remain recorded in
[donor-provenance.md](donor-provenance.md); provenance is documentation rather than runtime
topology.

The former TypeScript presentation dependencies were replaced by bounded successor packages:

- `@rusty-engine/render-contracts` owns only the render operations the product emits; and
- `@rusty-engine/renderer-three` owns the retained Three/WebGL implementation used by the browser.

The Kenney GLB and exact CC0 license are local under `fixtures/voxel-conversion`. CI checks out only
Rusty Engine. The operational dependency audit moved from 55 references before extraction to zero.

The exact M9 contract and closeout evidence are in
[m9-extraction-contract.md](m9-extraction-contract.md).

## Planned successor work

### M10: external demo consumer

Den task #6137 will move the loading-bay walking product into `rusty-engine-demo` after M9 closes.
The demo will depend one-way on an exact Rusty Engine revision, own game-specific composition and
browser acceptance, and prove that downstream Rust and TypeScript meanings can grow without adding
generic Engine vocabulary prematurely. Reusable mechanisms stay here only after that real consumer
earns their boundary.

### M11: isolated first-party Studio

Den task #6138 supersedes the old unscheduled M8 placeholder. After M10 establishes the external
project boundary, it will port the valuable Asha Studio workflows into an isolated `studio/`
workspace in this repository. Studio may author artifacts and propose typed operations, while Rust
retains validation, persistence, mutation, and execution authority. Its Angular/Nx dependency and
verification domain remain outside ordinary Engine installation and CI.

These entries record owner-approved direction and dependency order, not completed implementation;
Den remains authoritative for their current status and acceptance criteria.

## Deliberately unscheduled and absent clusters

### M7C: voxel annotations and edit history

Live edits and offline conversion did not produce a concrete consumer for undo, collaborative
history, author annotations, or a universal edit protocol. Schedule this only when a named tool,
provenance, collaboration, or diagnostic workflow establishes ownership and persistence semantics.

### Other absent families

- Inventory/equipment should be added successor-native when a gameplay loop needs it; no compelling
  donor closure has been identified.
- Generic rules, modifiers, conditions, and relationship evaluation should emerge inside concrete
  gameplay features before generalization.
- Dynamic physics should wait for behavior the current kinematic/collision path cannot express.
- Networking, adversarial scripting, collaborative editing, and universal mod APIs are outside the
  current design decision.
- Replay may return for a named debugging or product need as an observer of ordinary execution, not
  its prerequisite.

## Selecting future donor work

For any future Asha-derived capability:

1. Start with a concrete Rusty Engine consumer and acceptance behavior.
2. Inspect the portability report, then re-audit the exact current source and transitive closure.
3. Classify each relevant item as copy unchanged, narrowly adapt, rewrite from behavioral evidence,
   provenance-only, or exclude.
4. Define the successor owner before copying implementation.
5. Slice work by complete capability outcomes, not crate, protocol, or historical task names.
6. Close with focused tests, real product evidence when visible, persistence behavior, provenance,
   and the standalone/old-spine audits.

Do not create speculative tasks merely because the portability report labels something reusable or
because an Asha feature once existed.

## Repository and documentation ownership

Rusty Engine's Den project is the planning authority for future work. This repository's
[design.md](design.md) is the current architectural description. This ledger changes only when a
cluster is accepted, reopened, combined, dropped, or newly justified by a concrete consumer.

Historical inputs include the Asha Den documents `expressive-typescript-gameplay-composition`,
`gameplay-implementation-fundamentals-proposal`, `architecture-novelty-budget-critique`,
`old-projects-retrospective-mapping`, and `external-object-owned-gameplay-runtime-spike`; the global
`ess-architecture-guide`; RuleWeaver; and the old RPG under `/home/stash/research/old-rpg`. They are
evidence, not inherited specifications.
