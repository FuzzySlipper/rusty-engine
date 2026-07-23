# Experiment results

Status: walking falsification slices and the first four scheduled migration families implemented
on 2026-07-23 and registered for review in Den.

## Current decision

The first comparison overemphasized which language should execute gameplay. The more important
question is whether ordinary gameplay has a direct, legible structural path in any language.

Active `main` now uses this split:

```text
TypeScript content composition
        -> strict stored project definitions
        -> one Rust admission step
        -> concrete entity/component state
        -> direct Rust services and typed committed events
        -> derived projection
        -> retained Asha Three renderer plus successor DOM shell
```

Rust owns live session state, substantial gameplay logic, service composition, scheduling, and
persistence. TypeScript may use normal functions, loops, helpers, and type checking to generate
content, but it does not own runtime behavior instances, opaque gameplay state, callbacks, or a
second scheduler.

## What the language-host comparison established

The initial milestone implemented the same timed security door through direct Rust services and a
trusted executable TypeScript runtime over N-API. Its observed TypeScript scenario used five
gameplay bridge calls, 981 bytes into Rust, and 3,023 bytes out of Rust.

More importantly, enabling the short TypeScript behavior required:

| Removed runtime surface | Physical source footprint |
|---|---:|
| Generic Rust project-code host | 1,171 lines |
| N-API transport | 188 lines |
| Shared TypeScript boundary/runtime | 291 lines |
| TypeScript door behavior | 138 lines |

That route introduced opaque project state, invocation ownership, duplicated wire DTOs,
serialization, stable-message translation, bridge accounting, and another persistence lifecycle.
The first live test found casing drift inside a tagged Rust enum. These are structural obligations,
not shortcomings in TypeScript syntax.

The comparison was therefore useful negative evidence: moving logic across a language boundary
would relocate the same class of contract and lifecycle burden that the successor is meant to
remove. The complete implementation and its tests remain recoverable at Git tag
`external-ts-runtime-spike` (`9ed75581999291aa622713814d10832e597999d3`). Active `main` deletes
those runtime-host crates and packages.

## Active walking slices

### Security door

```text
Interact
  -> InteractionService
  -> SwitchActivated
  -> DoorService::open
  -> atomic transform/collision change
  -> DoorOpened
  -> optional stable CloseDoor schedule
```

The configured latched variation changes only `auto_close_after`. Save/reopen preserves a pending
close without persisting the diagnostic event journal.

### Encounter-gated exit

TypeScript composes explicit actor, encounter, exit, and enemy definitions. Rust admits those into
concrete `EnemyComponent`, `EncounterComponent`, `DoorComponent`, and relationship data before the
session starts.

```text
DefeatEnemy
  -> CombatService
  -> atomic collision/visibility change
  -> EnemyDefeated
  -> EncounterService
  -> EncounterCleared
  -> DoorService::open(exit)
  -> DoorOpened
```

There is no encounter polling, ambient subscription, dynamic topic, runtime script callback, or
generic rule resolution. `GameRuntime::drain_events` contains the finite route. After the first of
two enemies is defeated, the encounter remains active and the exit remains closed. The second
defeat produces exactly `EnemyDefeated`, `EncounterCleared`, and `DoorOpened` in order.

The one-enemy variation is a second generated project definition using the same TypeScript builder.
It changes no Rust runtime, service, event, persistence, or projection code and clears after the
first committed defeat fact.

Partial encounter progress survives save/reopen. Snapshots store concrete enemy and encounter
state, relationships, doors, and schedules; they do not store event history or replay frames.

The original direct `defeat_enemy` operation remains as a focused fixture/helper for the walking
encounter and navigation-blocker tests. M3 removes it from the browser product path: normal player
defeat now requires an accepted aimed attack, health mutation, and lethal combat fact.

### Asha spatial/collision transplant and new capability

`engine-spatial` references Asha's `core-space`, `core-voxel`, `svc-volume`, `svc-spatial`, and
`svc-collision` crates unchanged at the pinned donor commit. The dependency closure ends in
foundation/state/Parry code; it does not import Gameplay Fabric, `GameplayRuntimeHost`, replay
certification, or the runtime facade.

The successor adapter builds canonical `VoxelWorld` chunks and keeps Asha's
`CollisionProjection` explicitly derived. Its public scene exposes point, ray, AABB, and continuous
axis-swept AABB queries without leaking Parry mutation.

The genuinely new meaning is local and conventional:

```text
EntityDefinition.kinematic { half_extents, velocity }
  -> concrete KinematicCapability table
  -> GameRuntime::run_motion_phase
  -> KinematicMotionSystem::run (all bodies once, stable order)
  -> Asha axis-swept voxel query on X / Y / Z
  -> typed Moved / Blocked facts
  -> one atomic entity command batch
```

There is no component-local update method, ECS query scheduler, subscription, or per-body bridge
call. A blocked axis is stopped and its velocity component is zeroed while another axis may still
move. Motion and collision-scene definitions are snapshot-visible; restore rebuilds the derived
Parry projection and reaches the same final state as an uninterrupted run.

This is deliberately not a physics engine. It currently resolves kinematic bodies against canonical
static voxels (authored or generated), uses conservative axis separation, and has no dynamic-body
response, contact manifold, acceleration, or gravity.

### Named real-time workload

TypeScript composes `authored-voxel-wall-kinematic-lanes`: one independently positioned runner and
one wall voxel per lane. Rust admits the strict data once, then runs 180 bounded phases at a
simulated 60 Hz. The matrix truncates the checked-in 256-body definition deterministically for the
smaller cases; it does not change runtime code.

One warm release run on the development host produced:

| Bodies | Admission µs | Simulation µs / 180 phases | ns per body-phase | Projection µs / 180 passes | Facts committed | Snapshot bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 32 | 66 | 523 | 90.9 | 307 | 3,322 | 18,818 |
| 64 | 71 | 1,026 | 89.1 | 1,097 | 6,557 | 37,333 |
| 128 | 145 | 2,256 | 97.9 | 1,392 | 13,100 | 74,415 |
| 256 | 270 | 4,406 | 95.6 | 2,670 | 26,155 | 148,728 |

At 256 bodies the loop completed about 40,850 phases/second and all 256 bodies stopped without
tunneling. These are characterization numbers, not a stable benchmark or frame-budget guarantee;
the run has no renderer, input, AI, or operating-system workload mixed into it.

Allocation counts are not instrumented yet. State-copy pressure is bounded visibly instead: there
is no process serialization, the system snapshots 256 small `KinematicBodyView` values per phase,
and the receipt counts command/fact values (26,155 committed entity facts in the largest run). A
future vertical feature should add safe allocator telemetry before interpreting these timings as a
general engine capacity result.

### Navigation and autonomous enemy locomotion

The first migration family references Asha's `svc-pathfinding` unchanged. Its production dependency
closure stops at `core-math`, `core-space`, and `svc-spatial`; navigation policy, durable intent,
movement, facts, and persistence remain successor-owned.

```text
authored enemy navigation data
  -> NavigationComponent { goal, speed, query budget }
  -> voxel-derived read-only navigation projection
  -> EnemyNavigationSystem in the explicit phase schedule
  -> selected-body KinematicMotionSystem collision application
  -> Advanced / Arrived / Blocked / Unreachable facts
```

Routes are derived afresh and are not stored in snapshots. The spatial adapter finishes centering an
agent in a newly entered voxel before turning toward the next cell; this prevents a stateless query
from cutting across the corner of an adjacent solid, while continuous collision remains the
fail-closed authority for the actual body volume.

The loading-bay sentry visibly routes around a generated voxel pillar and reaches its configured
goal before the encounter proceeds. Save/reopen during the route rebuilds the navigation projection
from the same stored voxel authority and reaches the same final component/entity state. Focused
coverage also proves a solid goal becomes `Unreachable`, a body-width/projection mismatch becomes
`Blocked`, a dynamic kinematic blocker stops the agent and its removal permits fresh replanning, and
one bounded phase advances 32 configured agents through one named system.

The target and speed are content-only variations in the TypeScript builder. A behavior change is
localized to the navigation service/spatial adapter plus their focused tests; no protocol, bridge,
replay, or renderer contract changes are required.

### Player input, controller, and derived camera

The second migration family keeps the device boundary small without making TypeScript a live
gameplay host:

```text
authored player controller + DOM control bindings
  -> TypeScript keyboard/pointer resolution
  -> ResolvedPlayerAction { Move | Look }
  -> PlayerControllerService
  -> selected-body KinematicMotionSystem for movement
  -> durable entity translation + yaw/pitch controller state
  -> Moved / Blocked / LookChanged facts
  -> presentation-only follow-camera offset
```

The controller is ordinary data on the player entity. Admission requires transform, collision,
kinematic, and renderable capabilities and validates movement limits, look limits, and unique
keyboard controls. Rust never receives `KeyboardEvent`, key codes, mouse buttons, pointer-lock
state, or a device polling frame. TypeScript never receives mutable controller objects or writes a
pose directly.

Each move action sets one bounded velocity, runs the existing collision-aware selected-body path,
and clears velocity before returning. This makes the action visible and responsive without leaving
component-local update polling behind. Look actions update accepted yaw/pitch directly and clamp
pitch. Browser device capture holds only physical key codes: one keydown starts typed movement at
the admitted `moveStepSeconds` cadence, keyup/blur/hidden state stops it, simultaneous keys combine,
and an outstanding Rust request is never overlapped. The loading-bay proof uses one keydown over
time and one keyup, advances until collision, then verifies accepted position remains unchanged
after release; pointer movement changes accepted look state.

Player translation, controller configuration, bindings, yaw, and pitch survive snapshot/reopen.
The snapshot contains no camera data. The browser rebuilds the follow camera from accepted player
pose and a shell-owned height/distance offset on initial load and after every action.

Changing WASD to arrow-key bindings touches only project content/options and its composition test;
the Rust action and controller behavior are unchanged. A controller-algorithm change is localized
to `PlayerControllerService` and its focused runtime tests unless it introduces a genuinely new
durable configuration value. The initial vertical slice necessarily added one component/admission
shape, one snapshot record, the named service, and the browser border/readout; it did not add a
protocol crate, input context registry, replay codec, or lifecycle route.

### Generated voxel environment and mesh projection

The third migration family references Asha's dependency-free `svc-rng` and low-level `svc-mesh`
unchanged. The `svc-levelgen` crate was inspected but not linked because its small useful generation
loop is coupled to `core-events`, replay/hash records, collision summaries, runtime coordinate
frames, and render-chunk metadata.

Rusty Engine adapts only the bounded room-generation algorithm:

```text
authored seed + room dimensions
  -> successor GeneratedRoomConfig admission
  -> scoped deterministic RNG
  -> one ordered material-voxel result
  -> canonical VoxelWorld
       +-> CollisionProjection
       +-> NavProjection
       +-> svc-mesh visible-face payload
  -> retained Three replaceMeshPayload operation
```

The version-2 seed-4 room contains 366 solid material voxels. Its one derived mesh contains 3,024
vertices and 756 visible quads in three material groups, with output hash
`dc4b4a2265d8c686`. The seed chooses an interior pillar and a wall accent; the seed-4 pillar is
voxel `[4, 1, 6]`. A centered six-voxel aperture at `[3..6, 1..3, 11]` is absent from the canonical
voxel authority, so collision, navigation, and visible mesh all agree that it is open space. The
authored loading-bay door carries matching kinematic collider dimensions: closed collision blocks
the player at the aperture, while the existing atomic open transition disables collision and lets
the real controller cross it. The sentry's direct route crosses the generated pillar and the
navigation projection routes around it. The primitive floor and hand-authored visual obstacle
proxies were removed—the uploaded voxel mesh is the visible environment.

Snapshots store generator version, seed, dimensions, and expected output hash rather than hundreds
of generated voxels or mesh arrays. Reopen regenerates the material voxels, rejects a hash mismatch,
then rebuilds collision, navigation, and mesh. Focused coverage compares every material voxel and
mesh field across reopen before advancing the reopened and uninterrupted navigator to the same
result.

A seed variation changes only the project-content option/data. Rust entity definitions and runtime
behavior remain byte-identical while the canonical voxels, pillar, chunk hash, and visible mesh
change. A generator behavior change is localized to `engine-spatial` and its focused tests; the
unchanged RNG, mesher, collision, navigation, game service, and browser upload paths need no change
unless the stored configuration shape itself changes.

The default inline mesh JSON is about 96 KB and is currently included in every action response even
though the browser retains it by content hash. This is bounded and not a per-frame authority bridge,
but it is accepted temporary transport waste. The generator is also intentionally immutable and
single-chunk in this slice; neighbor-sensitive live remeshing belongs to M7A invalidation work.

### Weapon combat, health, and encounter consequence

The fourth migration family replaces the browser's direct-defeat controls with a real interaction
over the player controller and navigated enemies:

```text
authored primary-fire binding + WeaponComponent
  -> ResolvedAttackAction::Attack
  -> CombatService reads accepted player transform/yaw/pitch
  -> stable live-enemy target AABB query + canonical voxel raycast
  -> AttackFired / AttackHit or AttackMissed / DamageApplied facts
  -> HealthComponent mutation
  -> lethal EnemyDefeated fact and existing GameEvent
  -> EncounterCleared -> DoorOpened
```

Weapon damage/range/ammo/cooldown/muzzle offset live on the player entity. Maximum health, current
health, and an authored axis-aligned hitbox live on each damageable enemy. The TypeScript browser
never supplies a target entity or mutable ray origin: it resolves the admitted physical binding to
one attack intent, while Rust derives aim from accepted controller state and resolves current enemy
transforms. A moving sentry therefore changes the target query naturally without a combat-owned
position mirror.

The small slab-ray/nearest-hit algorithm is adapted from Asha `svc-combat`; the crate is not linked.
Its independent `CombatState`, copied fire-control command, health/replay hashes, readout golden,
and FPS runtime-session/role/event-adapter layers are absent. Static world occlusion comes from the
same canonical voxel projection already used by movement and navigation. On a lethal hit, the
existing atomic collision/visibility/velocity change runs before health/enemy state commits, then
the finite runtime drain applies encounter and door consequences exactly once.

Health, remaining ammo, and `ready_at_tick` survive snapshot schema 8. An intermediate snapshot
after nonlethal damage reopens with the same cooldown rejection, then both uninterrupted and
reopened sessions advance to identical damage/defeat facts and the same open exit. Nine focused
tests cover hit, no-target miss, voxel occlusion, cooldown and ammo eligibility, malformed
admission, repeated lethal idempotence, a live moving target, and save/reopen.

The real browser proof moves the first sentry, aims through accepted look actions, applies one
nonlethal hit while it is moving, lets navigation finish, and then defeats both enemies with the
authored 60-damage weapon. Enemy HP/ammo and recent typed facts remain visible in the DOM. Changing
enemy health or weapon damage is a project-content option/data change; changing target resolution
is localized to `CombatService` and its focused tests unless a new durable configuration value is
introduced.

M3 change amplification is explicit:

| Change | Required ownership surfaces |
|---|---|
| Content-only health/damage variation | `encounter-project.ts`, its composition test, and regenerated strict JSON; no Rust service, snapshot, or browser algorithm change. |
| Ray/target behavior variation with the settled component shape | `game-host::services` plus `combat_runtime` focused tests; no content schema, persistence codec, renderer donor, or language bridge. |
| Initial complete M3 vertical slice | Existing coarse `game-host` model/admission/service/runtime/snapshot/browser-host files, project-content schema/builder, browser input/readout, generated fixtures, focused tests, and documentation. No new crate/package/protocol or cross-language runtime owner. |

### Browser/Three/DOM product proof

The loading-bay browser shell links Asha's actual `@asha/renderer-three` and generated render
contracts. A small successor adapter turns whole Rust projection readouts into retained
`create`/`update`/`destroy` diffs. Three owns canvas objects and resource lifecycle; it never owns
gameplay state.

Six visible action/data paths run in the same product scene:

```text
DOM pointer/fire control -> authored binding -> ResolvedAttackAction -> CombatService
                         -> damage/defeat -> EncounterCleared -> DoorOpened
                         -> projection update -> retained Three door moves and enemies disappear

DOM spatial control -> one bounded KinematicMotionSystem phase sequence
                    -> Asha voxel sweep -> KinematicBlocked
                    -> projection update -> retained Three probe stops at the visible obstacle

DOM navigation control -> EnemyNavigationSystem -> Asha voxel-derived path query
                       -> selected collision-aware movement -> NavigationArrived
                       -> retained Three sentry visibly routes around the obstacle

DOM keyboard/pointer -> authored binding resolver -> ResolvedPlayerAction
                     -> PlayerControllerService -> collision-aware accepted pose
                     -> retained player projection + derived follow camera

opened encounter door -> disabled entity collider + canonical generated aperture
                      -> ordinary player controller traverses the visible exit

generated seed/dimensions -> canonical material voxels -> svc-mesh payload
                          -> retained Three chunk mesh shared with collision/navigation authority
```

The Asha renderer package has an optional encoded-frame convenience import from its old runtime
bridge. Rusty Engine never uses that path: Vite aliases it to a fail-closed local shim, and the smoke
rejects `RuntimeSession`, native bridge, Gameplay Fabric, or `GameplayRuntimeHost` markers in the
production bundle. The typed `applyFrame` path remains the unchanged donor implementation.

The automated product gate builds the bundle, starts the Rust host on an ephemeral port, launches
real headless Chromium with SwiftShader WebGL, dispatches keyboard/pointer input, resolves the
authored primary-fire binding, and exercises reset/navigation/combat/spatial actions. It requires
one-keydown/one-keyup moved-then-blocked-and-stopped player collision, changed look state, a
moved-and-damaged live sentry, the arrived sentry, combat hit/damage/defeat facts, open-door
transform, controller traversal through that exit, defeated entities, blocked probe, generated
seed/mesh readout, and retained-renderer evidence in the final DOM.

## Reproducible evidence

From a checkout with the public Asha donor beside this repository:

```bash
pnpm install
pnpm run verify
cargo run -q -p game-host --bin headless-door
cargo run -q -p game-host --bin headless-encounter
cargo run --release -q -p game-host --bin motion-workload -- --matrix
```

The current verification gate proves:

- Rust formatting, Clippy, and strict TypeScript compilation;
- generated project content is byte-for-byte current with its TypeScript composition;
- 11 TypeScript content-composition tests and six browser input-lifecycle/retained-projection tests;
- 52 Rust integration tests across entity state, donor collision/navigation/mesh queries, security door,
  content admission, encounter routing, kinematic/navigation motion, atomic rejection, projection,
  player control, combat/health/weapon behavior, generated-environment admission, and save/reopen;
- strict rejection of unknown stored-content and snapshot fields;
- a real Chromium/Three/WebGL product smoke, including a forbidden-old-runtime bundle audit.

## Active source footprint

These are physical line counts (`wc -l`), not complexity scores:

| Ownership surface | Production source footprint | Purpose |
|---|---:|---|
| Reusable Rust entity state | 4 files / 888 lines | Entity/capability storage, atomic entity mutation, snapshot, projection. |
| Successor spatial adapter/system | 1 file / 898 lines | Canonical donor scene construction, generated-room/aperture algorithm, collision/navigation/mesh derivation, bounded query facade, central kinematic phase. |
| Rust game host and runners | 11 files / 4,541 lines | Concrete components/services, combat/query ownership, routing, admission, scheduling, snapshots, headless/product/workload hosts. |
| TypeScript content composition | 5 files / 335 lines | Typed definitions, encounter/generation/combat and motion builders, reproducibility check. |
| TypeScript browser product shell | 6 files / 1,102 lines | Browser-owned held-input lifecycle, Rust-readout/mesh adapter, input/attack resolution and DOM readout, derived camera, Asha renderer mount, bridge exclusion shim, styling. |
| Generated project content | 3 files / 8,092 lines | Two encounter/generation/combat variations and pretty-printed 256-body workload data. |

The Rust object/component model is currently the largest single file, followed by generation and
snapshot code. They are explicit and easy to trace, but a later slice should test whether small
typed validation/codec helpers can reduce repetition without introducing reflection, registries, or
generic replay machinery.

## Findings

- The direct Rust service path solves the original discoverability problem without a language
  escape hatch.
- Typed events carry real cross-domain weight while remaining a short closed route.
- TypeScript still provides useful code-as-content ergonomics without participating in the live
  authority loop.
- The retained command batch has concrete consumers: door transform/collision, enemy
  collision/visibility, and a whole kinematic phase's translations/blocked velocities commit once.
- Batched entity-state reads and expected-revision machinery had no remaining in-process consumer and were
  deleted with the external host.
- The pivot removes substantially more runtime-host plumbing than the encounter slice adds.
- A substantial Asha service family can sit below the new object-centric center unchanged.
- The existing renderer can sit above it through typed projection without restoring the old runtime
  facade to the browser bundle.
- One canonical donor `VoxelWorld` can feed collision, navigation, and a real retained mesh without
  per-consumer approximations or importing the donor event/control plane.
- Asha's compact combat query algorithm can be harvested without importing its independent state,
  fire-control copy, replay hashes, FPS session, or event-adaptation route.
- The new capability's behavior has one owner; its expected amplification is model/command/snapshot
  binding plus admission and restore, not a cross-language protocol campaign.

## Decision boundary and remaining limits

The planned falsification work and first four migration families pass. That is strong evidence for
continuing Rusty Engine as the durable successor, but it is not evidence that every Asha feature
should move or that all current leaf-donor arrangements are durable infrastructure.

Before calling this durable infrastructure:

1. Review M3, then schedule M4 feedback strictly above its movement/attack/damage/defeat facts so
   presentation never becomes another gameplay authority.
2. Decide whether sibling donor references become pinned Git dependencies, vendored crates, or a
   shared foundation repository before Asha resumes development.
3. Add safe allocation telemetry and a longer mixed workload; the current matrix measures isolated
   CPU time and copy/fact proxies only.
4. Snapshot repetition has grown with M3 but remains direct and type-specific; revisit a small
   typed codec helper only if another settled component family repeats the same validation shape.
5. If the renderer remains a donor, extract a clean typed-frame subpath upstream so the local
   fail-closed alias is unnecessary.
