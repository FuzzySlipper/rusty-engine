# Asha-to-Rusty Engine Migration Cluster Ledger

Status: active scheduling baseline  
Ledger date: 2026-07-23  
Rusty Engine baseline: `65c528975328b2d92384dea91adf1d21c1779bf4`  
Asha evidence baseline: `a431974330589761c9e35fc4f8a55996a1b5ee48`

## Purpose

This ledger turns the remaining Asha feature surface into a sequence of complete capability
closures for Rusty Engine. It is not a crate-copy checklist and it does not presume that every Asha
feature, abstraction, document, or task should survive.

The working direction is to let Rusty Engine become the durable successor repository if the next
several feature families continue to validate its object-centric spine. A port back into Asha remains
possible, but it is the exceptional path. It would need to replace Asha's old structural center, not
place the successor behind another compatibility layer.

No Den project decision is made by this document. If a Rusty Engine project is created later, its
initial tasks should be generated from the then-current rows in this ledger rather than copied from
the Asha backlog.

Future milestone planning must also consult the full Asha crate inventory linked from
`docs/donor-provenance.md`. Use that report to shortlist donors and identify dangerous dependency
closures, then put only the selected crate or algorithm, intended treatment, rejected structural
dependencies, and concrete successor consumer into the milestone task. The report's `Feature later`
classification does not itself create backlog work, and `Reference unchanged` does not waive a
current dependency and semantic audit.

## Inheritance policy: absence by default

Asha is a donor and a body of evidence. It is not the default owner of Rusty Engine's shape.

- Asha code enters Rusty Engine only through a named feature-family decision in this ledger.
- Asha documents and Den documents are research inputs only. They are not migrated unless a durable
  Rusty Engine document has a current reader and a successor-native purpose.
- Asha task trees, protocol inventories, governance machinery, and compatibility promises do not
  transfer automatically.
- A crate name appearing in a donor column does not commit Rusty Engine to its API or dependency
  closure.
- Anything not named here remains absent. A future consumer may make a new case for it.
- Every accepted donor is recorded separately in `docs/donor-provenance.md` at an exact source
  revision.

This policy is intentionally asymmetric. Deleting an accidental inheritance later is harder than
recovering a useful implementation from the stopped Asha repository.

## What one migration cluster means

A cluster is a player- or author-visible capability closure:

```text
TypeScript-authored configuration
  -> strict stored data
  -> Rust admission into entity components and explicit relationships
  -> one named Service or System responsible for behavior
  -> authoritative state mutation
  -> typed committed facts for consequential outcomes
  -> derived collision/navigation/render/UI projections
  -> snapshot and reopen
  -> headless and browser-visible acceptance
```

A cluster is complete only when that path works as one feature. Compiling a donor crate, defining a
protocol, or reproducing an isolated algorithm is not migration success.

Cross-cutting abstractions are harvested from completed clusters. Rules, relationships, scheduling,
registries, codecs, and event vocabulary are not independent migration projects.

## Disposition vocabulary

| Disposition | Meaning |
|---|---|
| **Reference unchanged** | Use a bounded Asha crate/package whose dependency closure remains below the successor spine. |
| **Adapt or extract** | Preserve useful implementation or semantics behind a successor-owned API; do not inherit the donor's authority model. |
| **Behavioral evidence** | Use tests, examples, and observed behavior to define acceptance, but write the successor path around Rusty Engine's owners. |
| **Exclude** | Do not migrate. Reconsideration requires an explicit architecture decision, a concrete consumer, and a removal plan for any compatibility surface. |
| **Defer** | Potentially useful, but deliberately unscheduled until an earlier cluster creates a real consumer. |

## Non-negotiable closure gates

Every scheduled cluster must satisfy all of these gates:

1. **Locality:** an entity's definition and components lead directly to the responsible named Rust
   owner.
2. **Single authority:** Rust owns live gameplay truth; TypeScript composes admitted content and
   hosts presentation only.
3. **Explicit time:** recurring work appears in a short central phase schedule; delayed work is a
   stable serializable intent, never a callback.
4. **Typed consequences:** events carry meaningful accepted facts across domains, but no ambient
   event bus or universal reaction route decides ordinary behavior.
5. **Derived projections:** rendering, collision, navigation, audio, and UI are rebuilt or updated
   from authoritative state and never become parallel truth.
6. **Persistence:** durable component state and scheduled intents survive save/reopen without
   persisting callbacks or requiring replay certification.
7. **Product proof:** at least one real browser path demonstrates the feature; headless tests alone
   are insufficient for a product-facing cluster.
8. **Independent launch:** Asha and Rusty Engine remain separately runnable and never share live
   authority.
9. **Old-spine audit:** the cluster must not introduce `GameplayRuntimeHost`, Gameplay Fabric,
   `RuntimeSession`, universal proposal/reaction/replay contracts, or a per-decision language bridge.
10. **Bounded amplification:** closeout records which files and layers changed for one content
    variation and one behavior variation.

## Completed successor baseline

The following is already successor-owned or accepted below the new spine. It is the common base for
the schedule rather than work to repeat.

| Baseline capability | Current owner | Donor treatment | Proven closure |
|---|---|---|---|
| Entity identity, lifecycle, capabilities, atomic invariant changes | `entity-state` | `core-ids`, `core-math`, and `core-time` referenced unchanged | Entity/component inspection, atomic changes, projection, snapshot |
| Doors, switches, encounters, and enemy defeat consequences | `game-host` services and explicit event drain | Asha behavior not imported | Content admission through save/reopen and browser-visible door opening |
| Stable delayed intents | `game-host::scheduler` | `core-time` values only | Timed door close survives reopen |
| Voxel authority and spatial partition | `engine-spatial` | `core-space`, `core-voxel`, `svc-volume`, and `svc-spatial` referenced unchanged | One canonical voxel source rebuilds derived spatial state |
| Collision projection and kinematic motion | `engine-spatial::KinematicMotionSystem` | `svc-collision` referenced unchanged | Central bounded phase, typed moved/blocked facts, reopen, workload, browser proof |
| Content composition and admission | `ts/packages/project-content` plus `game-host::content` | Successor-owned | Normal TypeScript composition emits strict data; Rust admits once |
| Retained Three presentation | `ts/packages/browser-shell` | `@asha/contracts`, `@asha/render-projection`, and `@asha/renderer-three` referenced unchanged | Typed projection diffs reach the real renderer without the old runtime bridge |

## Current donor inventory boundary

At the pinned Asha snapshot, the main Rust workspace contains 96 crates, the TypeScript workspace
contains 24 packages, and `public-rust` contains six packages in addition to its workspace root.
Those counts explain why crate-by-crate migration would produce a misleading backlog: many crates
exist to support Asha's old structural center rather than a distinct product capability.

### Accepted unchanged donors

These are already in use and remain conditional on their dependency closures staying below the
successor spine:

- Rust foundations: `core-ids`, `core-math`, `core-time`, `core-space`, `core-voxel`.
- Rust services: `svc-volume`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, `svc-rng`, and
  `svc-mesh`.
- Presentation packages: `@asha/contracts`, `@asha/render-projection`,
  `@asha/renderer-three`.

If Rusty Engine becomes independently durable, sibling path dependencies must be replaced by pinned
Git dependencies, vendored sources with provenance, or a genuinely shared foundation repository
before Asha resumes development.

### Donor decisions by consumer

| Candidate | Current dependency signal | Initial treatment | Earliest consumer |
|---|---|---|---|
| `svc-pathfinding` | Production dependencies stop at `core-math`, `core-space`, and `svc-spatial` | Referenced unchanged behind `engine-spatial` | M1 navigation |
| `protocol-input`, `rule-input`, `protocol-view` | Carry useful vocabulary mixed with Asha lifecycle assumptions | Behavioral/type evidence only; no dependency | M2A player control/camera |
| `svc-rng` | Dependency-free deterministic scoped stream | Referenced unchanged | M2B generation |
| `svc-levelgen` | Useful generation logic, but emits through `core-events` | Small algorithm adapted; crate and control plane excluded | M2B generation, M7A editing |
| `svc-mesh` | Depends on `core-space`, `core-voxel`, `svc-volume`, and `svc-spatial` | Referenced unchanged behind a derived mesh projection | M2B generation |
| `svc-physics` | Small isolated service; collision-aware mode is intentionally incomplete/fail-closed | Behavioral evidence until a concrete dynamic-physics need exists | Unscheduled |
| `svc-combat` | Compact ray/collision logic, but owns its own combat state and replay hash | Adapt or extract algorithms into successor components/services | M3 combat |
| `render-animation`, `render-audio`, `render-billboard`, `render-particle` | Presentation-oriented but coupled to Asha render contracts to varying degrees | Inspect one output family at a time; adapt above typed accepted facts | M4 feedback |
| `core-assets`, catalog primitives | Potentially stable stored identity and lookup concepts | Selective reference or extraction after schemas settle | M5 project admission |
| `core-scene`, `svc-project-content`, `svc-serialization`, `rule-project-bundle` | Broad stored-project and lifecycle assumptions | Behavioral evidence first; do not import as a bundle | M5-M6 |
| voxel conversion/asset/edit/annotation families | Substantial completed work but many protocol and tooling borders | Split by concrete consumer; never migrate as one subsystem | M7A-M7C |

### Explicitly excluded structural inheritance

The following are not migration clusters and are not donors for the successor spine:

- `runtime-bridge-api`, `gameplay-runtime-host`, `runtime-session-composition`, and the TypeScript
  runtime-session/native-bridge path;
- `rule-gameplay-fabric`, `svc-gameplay-fabric`, Gameplay Fabric public SDK/conformance packages,
  and static gameplay-module composition;
- universal proposal, admission, reaction-frame, decision-receipt, ownership-routing, declared-read,
  and replay-certification paths;
- `sim-*` replay infrastructure by default;
- protocol crates merely because an Asha feature crossed a former crate/language/process border;
- broad governance/status/documentation frameworks from Asha;
- a Studio or editor shell before stored project and authoring semantics are earned by runtime
  consumers.

Diagnostics, deterministic tests, recordings, or a narrow replay tool may return later as ordinary
observers with a named use case. They do not regain authority over the execution path.

## Dependency-shaped migration schedule

```text
M0 completed walking successor baseline
 |
 +--> M1 navigation + autonomous enemy locomotion --------+
 |                                                        |
 +--> M2A player input + controller + camera ------------+--> M3 combat/health/weapon
 |                                                        |          |
 +--> M2B generated voxel environment + mesh ------------+          +--> M4 presentation feedback
 |                                                                   |
 +---------------- three or more settled component families --------+--> M5 stored scene/assets/project admission
                                                                         |
                                                                         +--> M6 durable project persistence

M2B + M1 --> M7A live voxel editing and derived-state invalidation
M7A ------> M7B voxel asset import/conversion
consumer --> M7C voxel annotations/edit history

M5 + M6 + proven authoring consumers --> M8 Studio/tooling
```

M1, M2A, and M2B can be developed as separate branches of the baseline, but only one cluster should
be active at a time until its authority path closes. M3 intentionally waits for navigation and
player intent so combat proves a real interaction rather than another test-only command.

## Cluster ledger

| ID | Status | Capability closure | Depends on | Asha use | Completion signal |
|---|---|---|---|---|---|
| M0 | Complete | Object-centric entity state, direct services, encounters/doors, scheduling, voxel collision, kinematic system, save/reopen, retained renderer | — | Bounded foundations, spatial/collision, renderer | Existing full `pnpm run verify` gate |
| M1 | Complete (#6103) | Navigation projection, authored navigation intent, autonomous enemy route following, replanning, blocked/unreachable outcomes | M0 | `svc-pathfinding` referenced unchanged; `rule-lifecycle/fps_movement.rs` used as evidence only | Visible sentry routes around authored collision; reopen is identical; typed arrival/blocked/unreachable facts and a 32-agent bounded phase are verified |
| M2A | Complete (#6104) | Resolved input, player controller, look/move intent, authoritative pose, derived camera | M0 | `protocol-input`, `rule-input`, and `protocol-view` used as evidence only; lifecycle/session routing excluded | One physical keydown drives bounded typed movement until keyup, visibly moves then blocks and stops one player; bindings are content; pose/controller reopen identically; camera is rebuilt presentation state |
| M2B | Complete (#6105) | Seeded environment generation, canonical voxel admission, collision/nav rebuild, derived mesh presentation | M0 | `svc-rng` and `svc-mesh` referenced unchanged; `svc-levelgen` algorithm adapted without `core-events` | Seed variation changes canonical/visible geometry without runtime code; the generated shell/pillar/aperture drive collision/navigation; the closed entity gate blocks and the opened gate permits real controller traversal; mesh and hash-verified regeneration agree after reopen |
| M3 | Complete (#6106) | Weapon configuration, attack intent, ray/target resolution, health, damage/defeat, encounter consequence | M1, M2A | Slab-ray/nearest-hit algorithm adapted from `svc-combat`; old FPS lifecycle used as behavioral and negative structural evidence | Player damages a moving enemy through authored primary fire, later defeats the encounter, health/weapon eligibility reopen identically, and typed defeat clears the existing door path |
| M4 | Complete (#6111-#6114) | Animation/audio/particle/billboard feedback derived from accepted movement, attack, damage, defeat, and door facts | M1-M3 facts | Presentation render families inspected as evidence only; no donor presentation crate/protocol imported | Typed response-local feedback is visible/audible, posture rebuilds from current state, dropped/restarted cues do not replay, and presentation failure never changes gameplay |
| M5 | Complete (#6117-#6120) | Stored scene, asset identities/catalog, entity definitions, project admission, diagnostics | At least M1, M2A, and M3 | `core-assets` referenced unchanged; catalog/scene/project/bundle families used only as bounded evidence | The checked-in schema-v7 loading-bay project drives the real browser through one strict Rust admission path with source-locatable diagnostics and no runtime facade |
| M6 | Ready to implement (#6121-#6124) | Durable project save/load and versioning, distinct from a live runtime snapshot | M5 | Selective serialization evidence only | Project content round-trips/version-migrates independently of session state; snapshots remain concrete runtime persistence |
| M7A | Deferred | Live voxel edit commands, authoritative voxel mutation, collision/navigation/mesh invalidation | M1, M2B | Adapt `rule-voxel-edit` behavior and narrow voxel services | One edit becomes visible and changes collision/navigation in the same accepted transaction; reopen preserves it |
| M7B | Deferred | Voxel asset import/conversion into the admitted project form | M7A, M5 | Adapt conversion and asset services/tools | A real external asset converts reproducibly, validates, loads, and behaves like authored voxels |
| M7C | Unscheduled | Voxel annotations and edit history | A named authoring/diagnostic consumer | Evidence from annotation/history protocols and services | Schedule only when undo, provenance, collaboration, or another concrete consumer exists |
| M8 | Unscheduled | Studio/editor workflows over established runtime and project APIs | M5-M7 plus repeated manual-authoring pain | Asha Studio is product evidence, not a shell to transplant | Tools manipulate the same admitted data and typed commands used without Studio; no editor-only authority path |

## M1: completed cluster definition

Navigation plus autonomous enemy locomotion is the strongest next slice because it combines a clean
donor, the existing collision/voxel authority, durable entity configuration, central real-time work,
and immediately visible product behavior.

Create one parent outcome with these dependency-ordered child outcomes:

1. **Donor boundary and provenance**
   - Inspect `svc-pathfinding` at the pinned Asha revision and its complete production dependency
     closure.
   - Decide unchanged reference versus a successor-owned narrow wrapper.
   - Record the decision and rejected Asha lifecycle dependencies in `docs/donor-provenance.md`.
2. **Navigation components and content admission**
   - Add explicit data for navigation capability, target/goal, movement limits, and current durable
     intent only where each value must survive.
   - Keep transient paths, query workspaces, and derived occupancy outside snapshots unless a test
     proves they are durable truth.
3. **One canonical navigation projection**
   - Derive walkability from the same voxel authority used by collision.
   - Define explicit rebuild/invalidation ownership; do not let pathfinding silently maintain a
     second map.
4. **Named system and typed outcomes**
   - Add an `EnemyNavigationSystem` (or equally explicit owner) to the central phase list.
   - Resolve target selection separately from route execution if both become nontrivial.
   - Commit typed arrived, blocked, and unreachable facts only when they are consequential.
5. **Collision-integrated movement**
   - Route translation through the existing kinematic/collision invariant path.
   - Define deterministic behavior when the path and collision projection disagree.
6. **Snapshot and reopen**
   - Preserve the enemy's durable goal and motion state.
   - Rebuild derived navigation data and demonstrate the same accepted outcome after reopening at an
     intermediate point.
7. **Browser and workload acceptance**
   - Show at least one enemy routing around an obstacle in the existing Three scene.
   - Include unreachable and dynamic-obstruction cases.
   - Add a bounded many-agent characterization without turning performance numbers into a premature
     framework.
8. **Closeout audit**
   - Run the complete verification gate and forbidden-old-runtime bundle audit.
   - Record change amplification for a content-only route variation and a behavior variation.
   - Update this row before scheduling M2A or M2B.

M1 must not introduce a generic AI behavior graph, component-local `update`, an ECS query scheduler,
or a universal navigation event protocol.

## Later-cluster slicing notes

### M2A: player input, controller, and camera

Treat input as a real border and movement as gameplay. TypeScript may capture browser devices and
resolve authored bindings into typed actions. Rust converts actions to player intent and owns the
controller/collision result. The camera is a derived presentation view of accepted player state plus
presentation-only offsets; it is not a second pose authority.

Do not migrate Asha's entire input or view protocol surface. Extract only types needed by the first
working controller and extend from concrete actions.

### M2B: generated environment and mesh

Generation produces authoritative voxel content once. Collision, navigation, and mesh are distinct
derived consumers of that content. The slice is incomplete if each consumer receives a separately
constructed approximation of the level.

Keep seed and authored generation parameters when they are project truth. Persist generated voxels
or prove deterministic regeneration deliberately; do not accidentally rely on replay to reconstruct
the level.

### M3: combat, health, and weapon

The existing `DefeatEnemy` test action is a proven cross-domain consequence, not the final combat
model. Replace it in the product path with player attack intent, weapon configuration, collision/ray
resolution, health mutation, and the same typed defeat fact. Keep health as a successor component;
do not import `svc-combat`'s independent state store or replay hash as a second authority.

M3 implements that boundary with `WeaponComponent` on the player, `HealthComponent` on damageable
enemies, and one direct `CombatService`. Rust derives the attack origin/direction from accepted
controller state, resolves live enemy hitboxes and canonical voxel occlusion, then emits typed
attack/miss/hit/damage/defeat facts. The old direct-defeat method remains only as a focused fixture
helper; the browser has no direct-defeat route or target-selecting attack payload.

### M4: presentation feedback

M4 implements feedback only after the facts it presents are stable:

```text
current accepted entity state ----------------> rebuildable animation posture
accepted movement/combat/door facts and events -> response-local semantic cue union
                                                   -> TypeScript feedback adapter
                                                        +-> CSS posture/pulses
                                                        +-> capped expiring particles
                                                        +-> anchored billboards
                                                        +-> fail-soft Web Audio one-shots
```

The host maps typed payloads directly; debug `lastEvents` strings are not a dispatch surface. The
semantic union is successor-owned and closed over the five outcomes this product currently shows.
It lives under the browser host rather than `GameRuntime`, `GameSession`, journals, or snapshots.
GET/reset responses therefore rebuild player/enemy/door posture and contain no transient cues.
Movement produced by a multi-step phase collapses to one response cue per entity, duplicate defeat
and door cues are bounded, the DOM caps active effects at 24, and all transients expire.

The real Chromium gate displays movement/block/attack/damage/defeat/door animation pulses, six
particle kinds, damage/status billboards, and schedules oscillator/gain Web Audio one-shots. It
resets while concrete pulse, DOM, and audio targets are active and proves they are cleared. It then
deliberately drops a movement response, fetches current state, proves gameplay fingerprints agree
while cues are absent, and repeats the concrete cleanup during an in-page sink restart before
rebuilding only current defeated/open posture. Focused TypeScript tests cover both reset cleanup and
an audio exception that cannot stop later visual operations.

Implementation commits are `bb16dbd5aa65878e9dadf36912d3478a06898f51` (typed projection),
`2146e94020787d798f37a2f0fd17e4c8259bc71a` (browser realization), and
`3ea43745208af284caa11680b221bb9c1131bd4a` (drop/restart/product proof). Verification is the full
`pnpm run verify` gate plus `git diff --check` and the production-bundle exclusion scan.
Review correction `59b4f4039fde0b63444d97fec2879b78195af5f1` adds concrete pulse/audio ownership and makes both
reset assertions exercise active effects rather than aggregate bookkeeping alone.

M4 change amplification is bounded:

| Change | Required ownership surfaces |
|---|---|
| Content-only damage/movement variation | Existing project-content options and regenerated strict JSON only; accepted fact payloads automatically change billboard values/anchors without presentation or Rust projection changes. |
| Presentation behavior variation for an existing cue | `presentation-feedback.ts`, CSS, and its focused fake-sink expectations; no gameplay service, content schema, snapshot, or Rust authority change. |
| New consequential feedback outcome | The producing typed Rust fact/event, the small browser-host semantic union mapping, the TypeScript adapter, and focused/product assertions; no generic protocol, registry, replay envelope, or second authority. |

The inspected Asha render families contributed only one-way/disposable/bounded projection evidence.
Their animation controller authority, catalog/hash closure, retained handle registries, broad
presentation/render protocols, origin/correlation metadata, and scene/runtime bridge remain absent.

### M5-M6: project admission and persistence

M5 waited until the settled player, navigation, generation, combat, encounter, door, and
presentation consumers revealed the concrete schema. It now admits one static schema-v7 project
with a typed asset catalog, entry scene, scene-local entity definitions, and a generated voxel
source. Rust resolves every renderable identity and validates component/relationship/spatial
invariants before returning a `GameSession`; no provider, extension, load-plan, or bundle owner was
introduced. The product host consumes the checked-in artifact directly, while the optional
TypeScript builder merely materializes an equivalent candidate.

M6 can now add a canonical codec, one explicit migration from the retained flat schema-v6 fixture,
and recoverable durable writes without changing runtime-snapshot ownership.

Keep two persistence concepts distinct:

- **Project content** is authored, validated, versioned input that can start sessions.
- **Runtime snapshots** are concrete durable state for reopening one admitted session.

Neither is an event history.

### M7: voxel authoring

Voxel work is three clusters, not one subsystem migration:

1. live authoritative edits and derived-state invalidation;
2. external asset import/conversion;
3. annotations/history only when a real tool or workflow consumes them.

This ordering preserves substantial Asha feature work without inheriting every protocol created by
its former process boundaries.

## Features that do not yet justify a migration cluster

- Inventory/equipment has no strong, self-contained Asha implementation family to preserve. Add it
  successor-native when a gameplay loop needs it.
- Generic rules, modifiers, relationships, and condition evaluation should emerge inside a concrete
  gameplay family before being generalized.
- Dynamic physics should wait for a behavior that cannot be expressed by the current kinematic and
  collision path.
- Networking, adversarial scripting, collaborative editing, and universal mod APIs are outside the
  current successor decision.
- Replay should return only for a named product/debugging need and should observe normal execution.

## Turning a ledger row into work

When work is scheduled—whether locally or in a future Rusty Engine Den project—use one parent task
per cluster and outcome-sized children in this order:

1. donor boundary, exact source revision, and rejected dependency paths;
2. entity components, relationships, and content admission;
3. canonical derived projection or index;
4. named service/system, authoritative mutation, and typed facts;
5. snapshot/reopen behavior;
6. headless tests, browser acceptance, and a bounded workload where relevant;
7. closeout: full verification, provenance, change amplification, and old-spine audit.

Do not create tasks per Asha crate, protocol, document, or historical feature name. A child that
cannot state a user/author/runtime outcome belongs inside another task or remains research.

Before creating tasks from a row, refresh the Asha source revision and dependency closure. The
pinned inventory is evidence from 2026-07-23, not permission to assume a moving donor is unchanged.

## Repository and Den decision rule

For planning purposes, Rusty Engine is now treated as the likely durable successor, not as a patch
queue expected to flow back into Asha. This gives new work a clean default and makes omitted legacy
material stay omitted.

The repository decision should be revisited after M1 and at least two other heterogeneous closures
(normally M2A and M3, with M2B allowed to substitute if it lands first). At that checkpoint, keep
Rusty Engine as the canonical repository if:

- the direct service/component path remains legible across navigation, player intent, and another
  substantial domain;
- useful Asha crates still fit below or above the spine without importing its old control plane;
- snapshot/project boundaries remain explicit without a universal replay layer;
- the product shell can grow through derived projection;
- change amplification stays materially lower than comparable Asha work; and
- there is a credible plan to remove sibling checkout dependence.

Porting back into Asha should be considered only if all of the following are true:

- the successor spine can replace, rather than coexist with, the old runtime center;
- there is an explicit deletion sequence for the superseded facade/fabric/replay paths;
- Rusty Engine's acceptance suite can run against the transplanted result; and
- old documents, tasks, and APIs do not regain authority merely because they remain in the repo.

If a new Den project is created, seed only active ledger clusters, their acceptance gates, and
required donor-independence work. Do not bulk-copy Asha tasks or planning documents. The Asha project
remains historical evidence and a source locator.

## Maintaining this ledger

Update this file only when one of these changes:

- a cluster becomes ready, active, complete, split, combined, or intentionally dropped;
- a dependency or completion signal changes;
- donor inspection changes a disposition;
- a new concrete consumer earns a deferred feature;
- the canonical-repository decision is made.

For every completed row, record the implementation commit, donor revision, verification command,
browser-visible acceptance, persistence result, and any architecture exception. Keep transient task
status in Den if a project is created; this file owns the durable capability ordering and migration
policy.

Historical inputs considered while forming this ledger include the Asha Den documents
`expressive-typescript-gameplay-composition`, `gameplay-implementation-fundamentals-proposal`,
`architecture-novelty-budget-critique`, `old-projects-retrospective-mapping`, and
`external-object-owned-gameplay-runtime-spike`; the global `ess-architecture-guide`; RuleWeaver; and
the old RPG under `/home/stash/research/old-rpg`. They remain evidence, not inherited specifications.
