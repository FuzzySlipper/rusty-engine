# Rusty Engine design

Status: current architecture

Rusty Engine is a standalone, object-centric gameplay runtime. Rust owns live gameplay authority;
TypeScript composes project content before admission and hosts presentation after accepted state is
available. Asha is a provenance source and a body of design evidence, not a runtime dependency or
compatibility target.

This document describes the architecture implemented on `main`. Historical experiments,
measurements, and rejected alternatives live in [experiment-results.md](experiment-results.md).
Completed migration decisions live in
[migration-cluster-ledger.md](migration-cluster-ledger.md).

## Design priorities

The runtime is optimized for straightforward gameplay work rather than maximal abstraction.

- An entity's definition and components should lead directly to the service or system that owns its
  behavior.
- Rust has one live authoritative state. Presentation, collision indexes, navigation projections,
  and meshes are derived views of that state.
- Components are mostly data. They do not poll, subscribe, locate services, perform I/O, or own
  hidden update callbacks.
- Request-shaped behavior belongs to named services. Recurring bounded work belongs to named
  systems invoked from an explicit host phase.
- Ordinary behavior uses direct calls and typed return values. Typed events are reserved for
  accepted outcomes with real cross-domain consequences.
- Stored project content and runtime snapshots are different durable concepts. Neither is an event
  history.
- Language and process contracts exist at actual borders, not around every gameplay decision.
- New frameworks, registries, protocols, and generic languages must be earned by multiple concrete
  consumers.

Object-centric does not mean classical object-oriented objects or Unity-style component scripts.
An entity is an identity whose typed data is held by the session. Behavior remains outside its
components, with mutation owned by visible Rust services and systems.

## System at a glance

```text
optional TypeScript composition
             |
             v
strict stored project JSON -----> decode / migrate / semantic admission
                                             |
                                             v
resolved browser actions ---> GameRuntime ---> GameSession + EntityState
                                  |              |
                                  |              +--> feature components
                                  |              +--> canonical voxel state
                                  |
                                  +--> named Services and Systems
                                  +--> stable Scheduler intents
                                  +--> bounded typed GameEvent drain
                                             |
                                             v
                              accepted readout + response facts
                                             |
                                             v
                       browser projection and feedback adapters
                                      |              |
                                      v              v
                           typed render diffs    DOM / Web Audio
                                      |
                                      v
                              retained Three/WebGL
```

The authority path is intentionally short. Diagnostics and the runtime journal observe committed
outcomes; they do not decide how gameplay executes.

## Rust authority

### `GameRuntime`: orchestration and lifecycle

`game-host::GameRuntime` is the composition root for one running game. It owns:

- the active `GameSession`;
- authoritative `Tick` time;
- the durable-intent `Scheduler`;
- a bounded queue of consequential `GameEvent` values;
- a diagnostic journal of processed events; and
- the current `VoxelCollisionScene` when the project has spatial content.

Public runtime methods express semantic operations such as player movement, attacks, interaction,
voxel edits, navigation phases, motion phases, and time advancement. They call the responsible
service or system directly. `GameRuntime` is not passed wholesale into feature code, and it is not
a service locator.

The cross-domain event route is kept in one visible `drain_events` method. Its current meaningful
chains are deliberately small:

```text
SwitchActivated -> DoorService -> DoorOpened
EnemyDefeated -> EncounterService -> EncounterCleared -> DoorService -> DoorOpened
```

The queue has a fixed wave limit. Adding a new event requires a real committed occurrence and an
explicit downstream owner; it is not a substitute for a direct service call or ordinary return
value.

### `GameSession`: concrete game state

`game-host::GameSession` owns the feature state for one admitted session. Its concrete typed maps
currently cover doors, switches, control relationships, enemies, health, encounters, navigation,
player controllers, and weapons. This explicit structure makes domain ownership and snapshot
reconstruction visible.

`GameSession` intentionally is not a dynamic ECS world. It does not expose arbitrary queries or
grant broad mutation based on component matching. Feature modules receive the exact state needed by
their operation and keep mutating fields private to the smallest practical module.

### `EntityState`: reusable entity capabilities

`entity-state::EntityState` owns reusable entity invariants below game-specific behavior:

- identity, name, and lifecycle;
- transform;
- collision flags;
- renderable identity and visibility; and
- kinematic extent and velocity.

It supplies read-only `EntityView` values, projection nodes, snapshots, and one atomic
`EntityCommandBatch` boundary. The batch is used when a capability change must preserve several
entity invariants together, such as a door changing transform and collision or a motion phase
committing many resolved bodies. Ordinary domain state does not need to be translated into generic
commands.

### Feature-owned behavior

The `game-host` feature modules keep each local behavioral story together:

| Module | Data and behavior owner |
|---|---|
| `interaction` | Switch state and `InteractionService` |
| `door` | Door configuration/state and `DoorService` |
| `encounter` | Encounter membership/state and `EncounterService` |
| `player` | Input bindings, controller state, and `PlayerControllerService` |
| `navigation` | Navigation configuration/state and `EnemyNavigationSystem` |
| `combat` | Enemy, health, weapon data and `CombatService` |
| `scheduler` | Stable serializable delayed intents |
| `snapshot` | Aggregate reconstruction and cross-domain invariants |

Services own request-shaped operations. Systems own centrally invoked phases over a selected
population. The distinction is descriptive rather than framework-driven.

Source organization follows [rust-style.md](rust-style.md): one primary behavior owner or cohesive
type family per file, thin crate roots, and no one-type-per-file rule.

## Spatial authority and derived state

`engine-spatial::VoxelCollisionScene` keeps canonical material voxels together with their derived
query and presentation structures:

- the voxel world and material cells are authoritative spatial content;
- the Parry collision projection accelerates collision and ray queries;
- the navigation projection is rebuilt from the same voxel content;
- visible mesh chunks are rebuilt from that same content; and
- generated-room provenance is retained only when it remains meaningful authored truth.

Collision, navigation, and mesh data never become independent world models. `VoxelEditService`
validates an expected-revision transaction, builds all affected projections, and swaps the finished
scene only after the complete change is valid. The current bounded product uses a coherent full
rebuild; optimization should preserve this one-authority invariant.

`KinematicMotionSystem` and `EnemyNavigationSystem` are explicit phases. Player movement and combat
ray resolution use the same spatial authority through direct service calls.

Low-level foundation, voxel, collision, pathfinding, RNG, and mesh crates are ordinary workspace
members under `rust/crates`, alongside the higher-level crates that compose them. Their exact
sources and adaptations are recorded in [donor-provenance.md](donor-provenance.md); origin is not an
architectural layer. They do not bring Asha's former runtime facade, event control plane, replay
machinery, or lifecycle into the architecture.

## Content and admission

### TypeScript is code-as-content

`ts/packages/project-content` may use normal TypeScript functions, loops, composition, and tests to
author a project candidate. Its output is strict stored data. The checked JSON artifacts can be
loaded without executing TypeScript, and TypeScript owns no live gameplay state or behavior
instance.

This avoids both undesirable extremes:

- game content does not need to be handwritten as raw JSON; and
- Rust does not need to expose a universal behavior AST, branching language, or live script host.

If substantial game logic becomes difficult in Rust, the response is to improve feature locality
and service ownership first. A second runtime authority is not the default escape hatch.

### Rust admission creates sessions

`game-host::stored_project`, `project_codec`, and `project_admission` own the durable project border.
Decoding is strict and versioned. Migration is explicit. Semantic admission resolves assets,
relationships, component requirements, limits, and spatial content before constructing a
`GameSession` and its spatial scene.

Admission is the point where authored data becomes trusted runtime state. Runtime services do not
repeatedly validate project structure or depend on the TypeScript authoring package.

## Persistence

Rusty Engine preserves two distinct representations:

| Representation | Meaning |
|---|---|
| Stored project | Versioned authored input that can be admitted into a new session. |
| Runtime snapshot | Concrete live state used to reopen one already admitted session. |

`ProjectStore` canonically encodes an admitted project and installs it through a recoverable atomic
write. Project saves do not capture health loss, current cooldowns, open doors, runtime tick, or
transient events unless those values are deliberately materialized as new authored content.

Runtime snapshots preserve the concrete entity and feature state, voxel authority, tick, and
scheduled intents required to continue a session. Derived collision, navigation, and mesh
structures are reconstructed and checked. The diagnostic event journal and disposable
presentation cues are not replay authority and are not required to restore gameplay.

Scheduled work is a typed stable intent such as `CloseDoor { door }`, never a callback or closure.

## Presentation boundary

Presentation is downstream of accepted Rust state and facts.

`ts/packages/browser-shell` owns browser device input, action serialization, DOM state, Web Audio,
camera derivation, transient feedback, and the whole-state-to-render-diff adapter. It sends resolved
semantic actions to the Rust host; device events and render objects never enter gameplay authority.

`RuntimeProjectionAdapter` converts accepted projection nodes and voxel meshes into a small local
render vocabulary. `@rusty-engine/render-contracts` contains only the four operations the product
uses: create, update, destroy, and inline mesh replacement. `@rusty-engine/renderer-three` owns the
retained Three/WebGL objects and their resource lifecycle.

Current posture is rebuildable from a readout. Animation pulses, particles, billboards, and audio
one-shots are response-local effects: they may be dropped or fail without changing gameplay, and
they are not replayed after reset or page reload.

The camera is likewise a presentation projection of accepted player pose plus presentation-only
offsets. It is not a second transform authority.

## Offline voxel conversion

Static GLB conversion is an authoring/build operation:

```text
GLB + explicit request -> voxel-convert -> canonical voxel asset JSON -> project admission
```

`voxel-convert` parses and converts bounded source geometry. `voxel-asset` owns the strict durable
format and validation. `game-host` consumes only the resulting asset through normal project
admission; it does not link the GLB parser or execute conversion at runtime.

The complete boundary is documented in [voxel-asset-format.md](voxel-asset-format.md).

## Dependency direction and repository map

```text
internalized low-level donors
          |
          +--> entity-state
          +--> engine-spatial ----+
                                  |
                            game-host
                                  |
                    accepted HTTP/readout border
                                  |
      render-contracts --> browser-shell --> renderer-three / DOM / Web Audio

project-content --> stored JSON --> game-host admission
voxel-convert --> voxel-asset JSON --> game-host admission
```

| Path | Responsibility |
|---|---|
| `rust/crates/entity-state` | Reusable entity capabilities and atomic invariant changes |
| `rust/crates/engine-spatial` | Canonical voxel scene, derived collision/navigation/mesh, motion and edits |
| `rust/crates/game-host` | Game components, services, systems, orchestration, admission, persistence |
| `rust/crates/voxel-asset` | Durable voxel asset schema and canonical codec |
| `rust/crates/voxel-convert` | Offline bounded GLB conversion |
| `rust/crates/core-*` | Small shared identity, math, time, space, and voxel value crates |
| `rust/crates/svc-*` | Low-level volume, spatial, collision, navigation, RNG, and mesh services |
| `ts/packages/project-content` | Optional TypeScript project composition |
| `ts/packages/render-contracts` | Small typed render-diff vocabulary |
| `ts/packages/renderer-three` | Retained Three/WebGL backend |
| `ts/packages/browser-shell` | Browser input and disposable presentation |
| `content` | Checked projects, conversion requests, and generated artifacts |
| `fixtures` | Repository-local licensed test/product inputs |

Crates and packages should stay coarse and independently meaningful. Possible downstream reuse is
a locality test, not a reason to add plugin APIs, registries, or compatibility facades before a
real consumer exists.

## How to add a gameplay capability

A typical feature should follow this path:

1. Add authored fields to the stored project only when they are genuine configuration.
2. Validate and translate them during Rust admission.
3. Store live values in a cohesive component family owned by the responsible feature module.
4. Implement the operation in one named service or centrally scheduled system.
5. Return typed facts or a receipt; add a `GameEvent` only for a consequential cross-domain route.
6. Snapshot only the state and stable intents required to continue gameplay.
7. Add readout/projection data only when the browser or another real consumer needs it.
8. Prove the path with focused headless tests and, when product-visible, the real browser gate.

Content-only variation should normally stop at authoring data and expectations. A behavior change
should remain within its feature owner plus the persistence/presentation seams that truly consume
the changed meaning.

## Architectural exclusions

The current design intentionally excludes:

- a strict ECS, general query scheduler, or implicit system mutation rights;
- component-local `update` methods or ambient event subscriptions;
- a service locator, dependency-injection container, or plugin registry;
- a universal gameplay event bus, command union, reaction graph, or authored behavior language;
- live TypeScript gameplay authority or a per-decision language bridge;
- replay, hashes, or certification as prerequisites for ordinary execution;
- Asha's former runtime facade, Gameplay Fabric, provider/bundle lifecycle, and bridge topology;
- editor or Studio authority paths distinct from ordinary project admission and runtime commands.

These are not permanent bans on capabilities such as tools, replay diagnostics, networking, or
dynamic physics. Each needs a named consumer and a design that observes or extends the ordinary
authority path instead of replacing it.

## Evolution rules and known pressure points

- `GameRuntime` should remain the short visible cross-domain sequence even if feature-local helpers
  move out of it.
- `GameSession` and snapshot reconstruction contain deliberate concrete repetition. Introduce a
  helper only after another settled feature demonstrates the same safe shape; do not use reflection
  or a generic component registry merely to reduce lines.
- Spatial edits currently rebuild bounded derived state coherently. Optimize from measured product
  pressure, not by allowing collision, navigation, and mesh to drift into separate authorities.
- The browser readout is an actual language/process border and should remain typed and bounded. Do
  not create additional wire contracts inside the Rust gameplay call path.
- The runtime journal is useful diagnostic evidence, not a promise of replayable reconstruction.
- Donor code remains eligible only when a concrete consumer and complete dependency audit show that
  it fits below or above the current spine. Absence is the default.

Current measurements and limitations are maintained in
[experiment-results.md](experiment-results.md), not duplicated here.

## Historical decision record

Rusty Engine began as a falsification spike because repeated repairs to Asha's architectural center
were not restoring straightforward gameplay implementation. The experiment compared direct Rust
services with a trusted executable TypeScript runtime host. The TypeScript behavior itself was
concise, but the second authority lifecycle recreated bridge DTOs, opaque state ownership,
persistence translation, and scheduling complexity. That branch is preserved at Git tag
`external-ts-runtime-spike`.

The direct Rust path then proved navigation, input, generation, combat, presentation feedback,
stored projects, persistence, voxel editing, asset conversion, and standalone repository operation.
The result is no longer an aspirational sibling experiment: Rusty Engine is the canonical project.

See [migration-cluster-ledger.md](migration-cluster-ledger.md) for the durable migration record,
[donor-provenance.md](donor-provenance.md) for exact transferred sources, and
[m9-extraction-contract.md](m9-extraction-contract.md) for the final standalone extraction boundary.
