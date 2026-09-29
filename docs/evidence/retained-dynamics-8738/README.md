# One retained Rapier world per Dynamics world (#8738)

Each Dynamics world now keeps one live Rapier world, and every change applies to
it directly:
- body inserts, removals, teleports and property updates;
- rope joints;
- static environment colliders.

Contacts, sleeping and solver warm starts carry over between steps. The
EntityState copy of every body, the per-step world rebuild, prepare/commit
publication, the revision-checked anchor protocol and the Dynamics caps are
gone. About 5,000 lines were deleted and 1,700 added, including rewritten tests.

## Removed

- **Per-step reconstruction.** `simulate_dynamics*` rebuilt a Rapier world
  from EntityState on every call: every body, every static voxel/mesh collider,
  every rope joint.
- **The duplicate body store.** Dynamics worlds held an `EntityState`, with
  entity IDs mapped to body handles. Pose, velocity and sleep now live only in
  Rapier. The bridge keeps each body's authored shape, mass and material for
  readouts, and a body handle is its solver identity.
- **Prepare/commit publication:**
  - `RigidBodyService` and `PreparedRigidBodyStep`;
  - the environment, tether and body-set staleness checks;
  - `entity_state::replace_rigid_body_states`, with its component-revision
    guards;
  - the clone-then-swap `candidate` copies in every bridge mutation;
  - rope snapshot/restore.
- **The anchor revision protocol:**
  - `RefreshAnchor`;
  - the observation's world identity, entity revision and solver generation;
  - the reaction's source identity/generation, duplicate check and maximum
    impulse re-check.

  A reaction is now an ordinary impulse at its observed point. The #8760 survey
  found no stale-observation case at CraftSurvive, the only caller.
- **Rebase guards.** `RebaseWorldOrigin` no longer takes expected revisions,
  re-validates the Spatial receipt against before/after scenes, or returns a
  receipt. It translates bodies and fixed rope anchors by the origin delta and
  binds the rebased scene.
- **Caps and policy refusals (Dynamics part of #8742):**
  - body, action and contact quotas;
  - at most 8 ticks per step, and bounds on step seconds (now only > 0);
  - the per-step motion limit;
  - step/read list caps and the duplicate refusal;
  - 64 ropes/chains, 8 beads per chain, 576 tethers;
  - the 0.25 m/s reel cap;
  - the 1–8 / 1–16 rope solver bounds (now ≥ 1);
  - damping, friction, restitution and collision-group range checks;
  - the refusal of initial velocity on a locked axis, which is now masked.
- **Old tests.** The rope probes (they tested caches discarded every tick),
  `engine-spatial/tests/rigid_body.rs`, and the stale-candidate, quota,
  motion-limit and replay tests.

## Kept, and why

- **Finite/positive body and action values.** A NaN or non-positive mass in a
  retained world poisons it permanently, where before it only failed one call.
  Actions are checked before any is applied, so a refused call applies nothing.
- **Tether reach on a new attachment.** A joint created past its length pulls
  the bodies together in one step. Re-authoring the same endpoints keeps the
  current length, so a loaded rope cannot snap.
- **Tombstoned handles.** Destroy stays idempotent and a stale handle reports
  "destroyed" rather than "unknown".
- **The step/read lease.** Replacing leases with result buffers is #8744.

## What replaces it

- `svc-collision::DynamicsSolver` owns the Rapier world, a body-ID → handle
  map, the ropes and the static colliders.
- `bind_environment` diffs colliders by shared-shape identity. Unchanged chunk
  and mesh shapes stay in the world; removed ones wake the bodies touching them.
- `replace_body` (the `UpdateBody` path) rebuilds one body and re-attaches its
  ropes.
- `set_body_motion` teleports in place.
- An action's force lasts for its own `Step` call.

## Cost (`scripts/step-bench`, `results/step-bench.jsonl`)

The same scene runs in Rust through the old `RigidBodyService` (at `d3c179ca`)
and the new solver:
- a 32×32 voxel floor;
- four 6-crate towers;
- 16 balls and a roped ball;
- 600 steps at 60 Hz;
- at step 300, one teleport, one removal and one added crate;
- at step 400, a voxel edit digging out the floor under the first tower.

Release build, two runs each:

| | Before | After |
|---|---|---|
| Step p50 | 490–504 µs | 58 µs |
| Step p95 | 536–556 µs | 362–382 µs |
| Mean step, settled scene (steps 200–300) | 492–543 µs | 16–17 µs |
| Mean step, first 120 steps | 448–451 µs | 298–299 µs |
| 600 steps | 293–303 ms | 95–101 ms |
| Bodies asleep before the edit | 0 | 40 |

The rebuild path never lets a body sleep. A fresh world each step resets
Rapier's sleep counters, so settled stacks cost as much as moving ones.

Physical outcomes match: tower heights, the dug tower falling, the rope at 3.0 m
with one catch, and the teleported ball landing.

## Exercise through ordinary updates (`scripts/dynamics-exercise`)

The exercise is an SDK product that runs the same scene inside `Update` at a
fixed 60 Hz, with one addition. A character controller walks away from an 80 kg
sled on a 3 m tether for three seconds. It observes the sled's anchor each
update and passes the character's reaction to `StepWithReactions`. The product
also:
- teleports, destroys and adds bodies;
- digs the floor through `Voxel.ApplyEdits`, then rebinds collision.

`scripts/run-exercise.py` runs it with a headless browser on the runtime pack
and saves its facts line (`results/exercise-*-run-*.json`).

On this commit the product prints `DYNAMICS_EXERCISE_PASSED` in both runs, with
identical facts:
- 39 bodies asleep before the edit;
- the undug towers stand;
- the dug tower falls;
- the teleported ball and the added crate land;
- the rope stays at 3.0 m with one catch;
- 158 reactions drag the sled 14.2 m after the character's 13.1 m;
- the sled stays on the floor.

The same `Product.cs` also builds against the SDK and runtime pack from
`d3c179ca`. Every check passes except sleep, which fails with no body ever
asleep:

| Debug CoreCLR product | Before | After |
|---|---|---|
| `StepWithReactions` p50 | 724–750 µs | 174–200 µs |
| Mean, settled scene | 742–792 µs | 88–94 µs |
| Host update callback p50 | 803–822 µs | 235–246 µs |

## Checks

- **svc-collision:** 17 solver tests. They cover:
  - anchor response;
  - a hanging catch;
  - momentum exchange and determinism;
  - swing energy and off-centre ropes;
  - reeling and edit continuity;
  - reach refusal;
  - explicit mass, and invalid values refused;
  - locked axes;
  - per-call force;
  - a settling stack with its contacts;
  - environment rebind, touching only edited chunks;
  - rope survival across body replacement and removal;
  - translation;
  - sleep and wake.
- **The Dynamics bridge:** 15 tests, rewritten to the new contract. They cover:
  - chains;
  - terrain contact and repeatability;
  - reel controls and contact suppression;
  - anchor reactions (a repeated reaction applies twice);
  - character momentum exchange and light-anchor catch energy;
  - tether invalidation;
  - fast bodies without a motion cap;
  - step/read order;
  - body properties;
  - collision binding;
  - rebasing.
- **Workspace:** 1,205 tests pass (excluding `renderer-webview-host`, which
  needs GTK). Clippy is clean apart from the pre-existing #8757 lints.
- **C#:** the SDK, examples and the NativeAOT trial fixture build, and the
  CoreCLR SDK smoke passes.
- **`scripts/test-runtime-pack.sh`** passes: the CoreCLR and NativeAOT bundles
  both launch, and the NativeAOT fixture's Dynamics/tether/anchor checks pass.
  Its first runs exposed two fixture drifts from earlier campaign commits, both
  fixed here:
  - `fixtures/csharp-debug-execution-context` still called the `complete_call`
    export removed in #8736. It now checks that a successful callback's
    transition is visible immediately.
  - `fixtures/csharp-nativeaot-trial` still required character receipts to
    advance an entity revision. They stopped doing so in #8733, and #8755 owns
    those fields.

## Found, not fixed here

- **Voxel edits rebuild every collision chunk shape.** `VoxelEditService` builds
  a new scene, so after an edit the solver diff keeps no colliders: 16 of 16
  replaced in the bench, 45–54 µs, and every resting body on the floor wakes.
  The solver side is proven with `CollisionProjection::rebuild_chunk`. Keeping
  unchanged chunk shapes is incremental voxel editing, #8739.
- **Remaining character and entity caps.** `MAX_TETHER_REEL_SPEED` still caps
  character tethers. `entity_state::validate_rigid_body` bounds still decide
  whether a readout reports mass properties. Both are in #8742's remaining scope.

## Migration

Products pin their SDK version, so nothing breaks until they move.

**rusty-craftsurvive** (`RopePlayground.Rebase`)
- Before:

  ```csharp
  DynamicsWorldReadout state = ReadWorld(...);
  RebaseWorldOrigin(new(world, session, receipt, state.EntityRevision, state.Generation));
  ```

- After: `engine.Dynamics.RebaseWorldOrigin(new(world, session, receipt));`
  The `ReadWorld` call is no longer needed.
- Anchor reactions pass through unchanged.

**rusty-space** (test double `EngineTriples.cs`)
- `RebaseWorldOrigin` returns `void`.
- Drop `RefreshAnchor` if present.

**Any product**
- `RefreshAnchor`: call `ObserveAnchor` again.
- `DynamicsWorldReadout.EntityRevision`, the observation's
  `WorldIdentity`/`EntityRevision`/`SolverGeneration`, and the reaction's
  `SourceIdentity`/`SourceGeneration`/`MaximumImpulse` are gone.
- Code that expected `dynamics-tether-budget-exceeded`,
  `dynamics-motion-limit-exceeded` or a replayed-reaction refusal will no longer
  see them.
- `ReplaceBody` now invalidates ropes attached to the old body. Before, those
  ropes kept pointing at a removed body and failed every later step (found by
  reading the old code, not exercised).

## Reproduce

```sh
# Rust bench; the before side depends on this repository at d3c179ca (git rev).
(cd scripts/step-bench/before && cargo run --release)
(cd scripts/step-bench/after && cargo run --release)

# SDK exercise. Pack the SDK and runtime pack for the commit under test, then
# stage with a NuGet.Config pointing at the feed.
scripts/pack-csharp-sdk.sh <version> <feed>
scripts/build-runtime-pack.sh --output <pack>
RustyEngineExerciseSdkVersion=<version> dotnet msbuild Exercise.csproj -restore \
  -t:StageRustyEngineCoreClrProduct
python3 scripts/run-exercise.py <pack>/bin/rusty-product-host \
  obj/Rusty.Engine/Product out.json
```
