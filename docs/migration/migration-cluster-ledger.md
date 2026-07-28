# Asha-to-Rusty Engine migration cluster ledger

Status: M0-M11S and GM0-GM6 implemented; M11 Studio closeout and optional gameplay-rules implementation are in progress

Evidence baselines:

- original Rusty Engine walking-spike commit: `65c528975328b2d92384dea91adf1d21c1779bf4`;
- pinned Asha donor commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`; and
- reviewed standalone Rusty Engine head: `a2e55f9660e46751d4c78bcdd23b9a321b0dc961`; and
- published M10 demo extension head: `643e9c02e386cbefa0b6450fefde24162f100823`;
- first complete public renderer package baseline: `8cb49db6cfe9471faa23ab0661656a2366a83d8c`; and
- demo shared-renderer migration head: `42f428b0ee3f47de94d4372f512978f587d729f7`.

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

Absence by default rejects speculative topology; it is not a quota or deletion rule for useful,
implemented behavior in an owner-approved equivalence campaign. A known first-party consumer such
as Demo or Studio is concrete demand. Preserve the behavior it needs behind a successor-owned
boundary, while removing only named obsolete authority, dependency, and proof structures.

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
7. **Product proof:** a product-facing cluster reaches its real owning host path as well as focused
   tests; Chromium/Three is the current reference where browser/WebGL behavior is actually owned.
8. **Bounded amplification:** closeout identifies the surfaces changed by a content variation and a
   behavior variation.
9. **Standalone operation:** no sibling checkout or host-global package link becomes operational.
10. **Old-spine audit:** the change does not silently restore Asha's structural center.
11. **Host neutrality:** browser validation remains in browser/backend owners and does not introduce
    HTTP, DOM, URL, or Playwright requirements into host-neutral Engine mechanisms.

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
| M9 | Standalone repository | #6132-#6136 | The selected Rust donors, narrow local render edge, licensed fixture, verification, and CI became repository-local; the operational Asha dependency audit closed empty at the reviewed head above. |
| M10 | External reference consumer | #6137, #6141-#6145 | The walking product moved to public `rusty-engine-demo`, depends one-way on an exact Engine revision, added downstream-only `ExtractionBeacon` semantics, and proved a second TypeScript-authored composition without another Rust change. At extraction time Engine retained only provider crates and converter evidence. |
| M11R | Complete shared rendering | #6156-#6163 | The complete pinned Asha render investment was adapted into renderer-neutral Rust crates plus an isolated four-package TypeScript/Three/host workspace. Every one of 134 donor files has a final disposition; the demo deleted its private renderer and consumes the public exact-revision packages. |
| GM0-GM5 | Initial gameplay-mechanics provider | #6284-#6289 | Seven ordinary durable component families, one immutable catalog, named stats/tracks/effects/damage/inventory/equipment services, strict reconstruction, read-only inspection, runnable compositions, bounded costs, and literal 152-item `asha-rpg` disposition landed without a session, scheduler, IR, or shadow state store. |

## Accepted successor work

These accepted boundaries are not completed migration claims; Den owns their
live task and review state.

| ID | Tasks | Accepted boundary |
|---|---|---|
| GR0 | #6311 | Freeze a semantic-neutral optional rules package envelope, exact dependency resolution, provenance, diagnostics, bounds, and isolated TypeScript ownership against Rusty D20 without an Engine rules language. |
| GR1 | #6312 | Implement the host-neutral Rust `gameplay-rules` crate with direct-Rust and strict artifact paths. |
| GR2 | #6313 | Implement the isolated `rules/` TypeScript contracts and authoring support while keeping ordinary provider verification Node-free. |

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
- A narrow local Three renderer first fit above the spine, then exposed the risk of leaving reusable
  rendering with one product. The complete successor renderer now lives behind an isolated Engine
  workspace without restoring Asha's browser/runtime package graph.
- The external demo builds and proves real product behavior from an exact public Engine revision,
  while Engine verifies independently as a provider.
- Common stats, mutable tracks, attributed sources, explicit effects, damage/restoration,
  inventory, unique-item containment, and equipment fit one optional component-backed provider
  without turning the donor RPG runtime into Engine architecture.

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

M9 first replaced the former TypeScript presentation dependencies with bounded successor packages.
M10 then moved those narrow packages with their only real consumer. M11R deliberately brought the
complete reusable renderer back as an isolated `render/` workspace after the demo and planned
Studio established two concrete consumers. Ordinary Engine work still has no Node install or
browser dependency; TypeScript/Three/host work has its own lockfile and gates. Exact donor treatment
is recorded in [donor-provenance.md](donor-provenance.md).

The Kenney GLB and exact CC0 license are local under `fixtures/voxel-conversion`. CI checks out only
Rusty Engine. The operational dependency audit moved from 55 references before extraction to zero.

The exact M9 contract and closeout evidence are in
[m9-extraction-contract.md](m9-extraction-contract.md).

## External consumers and tooling

### M10: external demo consumer

Den task #6137 moved the loading-bay walking product into public
[`rusty-engine-demo`](https://github.com/FuzzySlipper/rusty-engine-demo). The demo depends one-way on
an exact Rusty Engine revision and owns game-specific composition, persistence, browser acceptance,
and presentation meaning. It now maps those meanings into the shared retained/presentation
contracts and owns no private renderer. Its `ExtractionBeacon` extended downstream Rust and
TypeScript meanings without adding Engine gameplay vocabulary, and `relay-annex.project.json` then
reused those meanings as a content-only change.

### M11R: complete shared renderer

Den campaign #6156-#6163 uses the rendering-specific donor pin
`6462a6de20d48ea1a3b7456826804bd9507860a5`. It preserves every implemented render family in
`render-model`, `render-projection`, `render-presentation`, and the isolated `render/` workspace,
while replacing sessions, bridges, replay/certification, catalogs/bundles, registries, generated
tunnels, and arbitrary fetch with explicit successor values and resource resolvers.

The complete capability matrix is [`render/completeness.tsv`](../../render/completeness.tsv); the
literal 134-file accounting is
[`render/donor-disposition.tsv`](../../render/donor-disposition.tsv). Engine Rust, the isolated
renderer, public package preparation, and the external demo each have separate proportionate gates.
See [rendering-operations.md](../rendering-operations.md) for commands and known limitations.

### M11S: successor functionality equivalence

Den task #6164 runs before Studio implementation. It audits proven Asha entity, scene, catalog,
asset, voxel, authoring, environment, level-generation, mesh-import, physics, serialization,
annotation, edit-history, trigger, and related rule behavior against the thinner successor. Useful
functionality is ported into an existing successor owner, consolidated behind a better-named owner,
or replaced by equivalent successor behavior. Historical crate count and package names are not
acceptance criteria.

The campaign keeps the old RuntimeSession/bridge/replay/provider/codegen/proof topology excluded.
It also prevents Studio from privately rebuilding a thin browser-shaped substitute merely because a
shared Engine mechanism was missing. Every donor family receives an explicit preserve/adapt/replace/
exclude disposition, focused owner-level evidence, and a host-neutrality check before M11S closes.

### M11: isolated first-party Studio

Den task #6138 supersedes the old unscheduled M8 placeholder. The isolated `studio/` workspace now
uses the shared renderer and the M11S owners through a closed project-owned Rust adapter. It covers
project/scene/entity/light/capability authoring, catalog and general asset import/reimport, the full
voxel/annotation/history/conversion/environment family, and versioned host-user camera/input
settings. Rust retains semantic validation, project persistence, mutation, and execution authority;
the explicit browser host owns only product UI, bounded adapter forwarding, and host-user
preferences. Angular/Nx and Playwright remain outside ordinary Engine installation and CI.

Den remains authoritative for exact review state and final M11 acceptance.

### GM0-GM5: gameplay-mechanics provider

Tasks #6284-#6289 use `asha-rpg` revision
`e4d6d1afb5b8387de4ff805d73b2041df29ee590` as bounded semantic and proof evidence. The successor
is one optional Rust crate over the canonical `entity-state` component store, not a crate-topology
port. Stats and tracks remain distinct; unique items are ordinary entities under canonical
containment; named services own mutations; operation receipts are bounded explanations rather
than events or replay records.

The checked
[`gameplay-mechanics-donor/disposition.tsv`](../../migration/gameplay-mechanics-donor/disposition.tsv)
accounts for all 152 donor-tree items as 3 adopted evidence rows, 12 adapted/rewritten rows, and
137 exclusions. The local checker proves the pinned path set is complete without requiring a donor
checkout. Provider closeout includes strict seven-component reconstruction, immutable mechanics
inspection, direct shooter/infrastructure/d20-shaped examples, and local-cost measurements.
Reference-product migration and three-composition product reconciliation remain the explicit
consumer stages after the reviewed provider revision; they are not hidden GM5 acceptance work.

## Reopened and absent clusters

### M7C: voxel annotations and edit history, reopened through M11S

The early live-edit and conversion slices did not yet justify a universal edit protocol. Studio is
now a named consumer for author annotations and bounded edit history, so #6164 reopens the useful
behavior before Studio implementation. Preserve annotation and undo/redo semantics where they serve
that consumer; do not restore Asha's universal protocol, replay, collaboration, or proof topology.

### Other absent families

- The initial `gameplay-mechanics` provider now owns bounded inventory/equipment plus common numeric
  source mechanisms. Product-specific item behavior, attacks, checks, turns, conditions, and
  relationship policy remain downstream.
- A universal rules language, condition/formula AST, scheduler, behavior graph, and RPG authority
  session remain absent. Future smaller mechanisms still require concrete consumer evidence.
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

For newly proposed capability, the concrete-consumer rule prevents speculative framework growth.
For an owner-approved equivalence campaign such as M11R or M11S, the named predecessor and
first-party consumers already establish the demand; the work must account for useful behavior rather
than requiring each consumer to reimplement it before promotion.

Do not create speculative tasks merely because the portability report labels something reusable or
because an Asha feature once existed.

## Repository and documentation ownership

Rusty Engine's Den project is the planning authority for future work. This repository's
[design.md](../design.md) is the current architectural description. This ledger changes only when a
cluster is accepted, reopened, combined, dropped, or newly justified by a concrete consumer.

Historical inputs include the Asha Den documents `expressive-typescript-gameplay-composition`,
`gameplay-implementation-fundamentals-proposal`, `architecture-novelty-budget-critique`,
`old-projects-retrospective-mapping`, and `external-object-owned-gameplay-runtime-spike`; the global
`ess-architecture-guide`; RuleWeaver; and the old RPG under `/home/stash/research/old-rpg`. They are
evidence, not inherited specifications.
