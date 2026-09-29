# Retained graphics changes (#8737)

Graphics objects are now Engine-retained records that a product changes one at
a time. `Graphics.PublishChanges` names the objects that changed, and only those
objects are validated, compared and turned into renderer operations. Moving 50
objects in a 4,000-object world costs the same as in an empty one.

`Graphics.PublishSnapshot` stays as a thin adapter over the same path, because
every current product uses it (Dagger, Space, CraftSurvive, the fixtures and
`EntityGraphicsProjection`). `PublishAttachedSnapshot` is gone: no product used
it, and joint attachments now travel with `PublishChanges`.

## Removed

Each snapshot used to do all of this before a single moved object reached the
renderer:
- **Bridge:**
  - copied every fact and cloned every appearance identity string;
  - rebuilt the object-to-appearance map;
  - re-validated every joint attachment against its rig;
  - pushed a `SetParentJoint` for every attached object, changed or not.
- **Attached snapshots** also cloned the whole staged call as a rollback copy.
  That clone shared the graphics state's `Arc`, so the next write deep-copied
  the entire graphics state.
- **Runtime projector:**
  - cloned each object's catalog `Appearance` into a scene node;
  - cloned the whole resource catalog;
  - kept a second copy of every fact to replay on the next light or resource
    change.
- **Scene projector:**
  - re-validated every resource, node transform, appearance and light;
  - walked every ancestor chain for cycles;
  - cloned the handle registry for rollback;
  - diffed every node and resource;
  - re-validated every emitted operation.

A light change or resource release re-ran that whole pipeline over the last
snapshot. Also removed:
- **Unused machinery:**
  - `SceneAppearanceProjector` and its `AppearanceScene`, `AppearanceNode` and
    `AppearanceLight` inputs;
  - the authored-preview mode and `ProjectionAvailability`. No caller outside
    the runtime used them.
- **Bridge bookkeeping:**
  - the bridge's `retained_appearances`, `joint_attachments`,
    `retained_object_count` and `retained_light_count` copies. The projector
    already knows these; `retained_light_count` was never read.
- **Scans:** the linear scan of every object on each appearance disposal.
- **Tests** of the removed scene projector's authored-preview/availability mode.
  The resource-redefinition, inspection and cycle tests were rewritten against
  the new projector.

## What replaces it

`RuntimeAppearanceProjector` (`render-projection/src/runtime_appearance.rs`)
keeps, per object, its last fact and the appearance value last realized. It
also keeps a parent-to-children index and, for each appearance, the objects
using it.

`apply(upserts, removals)` works in three steps:
1. **Collect.** It gathers:
   - the upserts that differ from the retained fact;
   - the removals;
   - the users of any appearance changed in place since the last call. This
     covers material overrides, sprite frames and viewports, and mesh
     inspection.
2. **Validate** only those objects, against the state after the whole batch:
   - appearance, transform and parent;
   - cycles, only for objects whose parent changed;
   - joint attachments;
   - that a removed parent's children are removed or moved.
3. **Emit** operations for those objects:
   - a structural change (parent, layer, geometry, mesh or overrides)
     recreates the object and its retained subtree;
   - anything else is an update.

A refused batch changes nothing.

The projector re-validates and re-diffs resources only after `resources_mut()`.
A redefined mesh recreates its instances. Found through a scan that runs only
when a mesh body changed under the same identity.

`project(facts)` is the snapshot adapter. It computes the omitted identities
and calls `apply`, so unchanged facts cost a lookup and a comparison each.

Other changes:
- **Lights** use `apply_lights` for the one changed light, instead of
  reprojecting the whole light set and object snapshot.
- **ABI:** `NativeAppearanceChangesRequest { upserts, removals, attachments }`
  and `publish_changes` in the graphics table. The generated C# surface is
  `Graphics.PublishChanges(AppearanceChangesRequest)`.
- **Joint attachments** belong to the child's fact. `PublishSnapshot` cannot
  name joints, so it keeps each object's attachment.

## Kept, and why

- **Validation of changed objects.** A bad fact must not reach the renderer:
  unknown appearance, missing parent, cycle, missing joint, invalid transform.
  It now runs per changed object instead of per object per frame.
- **Recreating a structurally changed object's subtree.** The renderer removes
  a destroyed node's children with it, so they must be created again.
- **Validate before mutate.** This replaces the registry clone. The only
  failure after validation, handle exhaustion, is checked up front.

## Evidence

### Correctness (`render-projection/src/runtime_appearance.rs` tests)

- **Equivalence.** `incremental_batches_realize_the_same_scene_as_one_snapshot`
  runs 300 random batches of adds, moves, visibility toggles, appearance
  switches, reparents and subtree removals, plus appearance material changes.
  It applies them to a model renderer that refuses stale or duplicate handles
  and orphaned parents. Every 25 batches its realized scene, keyed by product
  object, must equal a fresh projector's single snapshot of the same facts. To
  check the test itself, disabling subtree recreation makes it fail.
- **Focused tests:**
  - a one-object batch in a 1,000-object scene emits one `Update`;
  - snapshot semantics;
  - subtree recreation and reparenting;
  - removing a parent without its children is refused;
  - refused batches leave state unchanged;
  - lights;
  - appearance changes in place;
  - mesh redefinition order (destroy dependents, redefine, recreate);
  - inspection updates in place;
  - mesh release;
  - joint attachments, including a parent appearance change that would strand
    a joint.
- **Bridge tests** (`csharp-engine-services`, 180) pass. One covers a refused
  joint through `publish_changes` that changes nothing, then a snapshot that
  keeps the attachment.

### Cost, projector only (`scripts/projector-probe`, `results/projector-probe.jsonl`)

Release build, median per frame over 200 frames. The world is static mesh
instances plus 50 cubes that move every frame. Before is `64a9b164`.

| Objects | Before: snapshot | After: snapshot adapter | After: `apply` (50 moved) |
|---:|---:|---:|---:|
| 1,000 | 653 µs | 59 µs | 10 µs |
| 5,000 | 3,861 µs | 327 µs | 11 µs |
| 20,000 | 19,672 µs | 1,611 µs | 11 µs |

### Through a product (`scripts/graphics-exercise`, `results/exercise-*.json`)

An SDK product keeps:
- 4,000 unchanged mesh instances;
- 100 instances of a second mesh;
- an animated body;
- 50 cubes that move every update.

Over 400 fixed updates it:
- moves cube 2 under cube 1 at update 100, and back at 110;
- at update 150, has a missing joint refused, then attaches a cube to the
  body's `RightHand`;
- hides 10 instances at 200;
- removes 100 instances at 250;
- at 300, removes the second mesh's instances and disposes the appearance and
  mesh.

It is built and run three ways:
- **Changes:** `PublishChanges` with only the changed objects.
- **Snapshot:** `PublishSnapshot` of every object.
- **Before:** `PublishSnapshot`/`PublishAttachedSnapshot` against the
  `64a9b164` SDK and runtime pack, with `OLD_GRAPHICS_API`.

`scripts/run-exercise.py` runs each under a headless browser. At the end it
attaches a fresh renderer (a new `outputs/fresh` stream) and counts what that
renderer's baseline contains. Two runs each; publish call time, excluding the
event updates:

| | Before: snapshot | After: snapshot | After: changes |
|---|---:|---:|---:|
| Publish p50 | 7.00–7.03 ms | 1.20–1.22 ms | 77 µs |
| Publish p95 | 9.2–9.5 ms | 1.57–1.65 ms | 105–118 µs |
| Event updates (reparent, attach, hide, remove, release) | 5.8–13.6 ms | 0.9–1.4 ms | 0.07–0.24 ms, one 1.1 ms outlier |

The outlier is the attach publish in one changes run; the other run's took
0.13 ms.

All six runs report `GRAPHICS_EXERCISE_PASSED` and record no error
diagnostics. Each has:
- 3,952 retained objects;
- the missing joint refused with the scene unchanged;
- the released mesh closed.

The fresh renderer's baseline is identical in all six:
- 3,900 static mesh instances, 51 primitives and 1 animated instance;
- one `SetParentJoint` to `RightHand`;
- 10 hidden objects;
- only the kept mesh defined, so the released one is gone.

The snapshot adapter's remaining 1.2 ms is per-object work: C# converts every
fact, and Rust looks up and compares each one (0.33 ms of it at 5,000 objects
in the probe). A product that moves few objects should publish them with
`PublishChanges`.

### Checks

- **Workspace:** 1,180 tests pass, excluding `renderer-webview-host`, which
  needs GTK. Clippy is clean for the touched crates, apart from the
  pre-existing #8757 lints.
- **C#:**
  - the SDK and `Rusty.Engine.Entities.Example` build;
  - `fixtures/csharp-joint-attachments`, migrated to `PublishChanges`, builds
    against the new package.
  - `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes;
  - `scripts/test-runtime-pack.sh` passes (CoreCLR and NativeAOT fixture
    bundles, which publish graphics snapshots).

## Review fix: animation controllers follow a recreated target

The incremental projector recreates an object, with a new renderer handle,
when its parent, layer or structural appearance changes. It also recreates the
object's subtree. A projected animation controller kept its old target. The
next flush then sent an `Update` for an unchanged revision and failed with
`animation controller revision is not newer`.

Now:
- `detach_retargeted_controllers` runs before the graphics frame is published,
  both in `stage_changes` and in the call-end resource reconcile. Each
  projected controller whose object's handle changed gets its projection
  destroyed (`AnimationProjector::detach_entity`). That Destroy goes out
  before the frame that removes the old target, as the renderer requires. The
  controller keeps the projection's descriptor.
- The flush right after the frame recreates it on the new target with that
  descriptor (`AnimationProjector::create_from_descriptor`). Clip phases
  therefore continue rather than restart. If the state has moved on, a normal
  `Update` follows.
- The two mesh-release paths (disposing an inline-mesh appearance, disposing a
  mesh resource) now join the call-end resource reconcile instead of
  projecting immediately. Every retained-graphics frame then goes through the
  two publish points above. `RuntimeAppearanceProjector::release_static_mesh`
  is gone.

`animation_controller_follows_its_target_when_the_target_is_recreated` covers
two cases:
- **A direct layer change.** It asserts the output order: animation destroy,
  then the graphics frame, then animation create, with the new target.
- **Recreating the animated object's parent.**

Without the detach step, the test fails with the reviewer's error.

## Reproduction

```sh
# Projector probe; the before side depends on this repository at 64a9b164 (git rev).
(cd scripts/projector-probe/before && cargo run --release)
(cd scripts/projector-probe/after && cargo run --release)

# Product exercise: pack each side's SDK and runtime pack, then stage and run.
scripts/pack-csharp-sdk.sh <version> <feed>
scripts/build-runtime-pack.sh --output <pack>
RustyEngineExerciseContentRoot=$PWD/fixtures/csharp-joint-attachments/content \
RustyEngineExerciseSdkVersion=<version> RustyEngineExerciseOldApi=<true|false> \
  dotnet msbuild Exercise.csproj -restore -t:StageRustyEngineCoreClrProduct \
  -p:RustyEngineProductBindHost=127.0.0.1 -p:RustyEngineProductPort=40851
python3 scripts/run-exercise.py <pack>/bin/rusty-product-host \
  <exercise>/obj/Rusty.Engine/Product <changes|snapshot> out.json
```

## Migration

- **Moving objects:**
  - `PublishSnapshot(all)` still works;
  - to pay only for what moved, keep publishing the objects that changed:
    `Graphics.PublishChanges(new(changedFacts, removedIds, ReadOnlyMemory<MeshJointAttachment>.Empty))`.
- **Attachments:**
  - `PublishAttachedSnapshot(new(facts, attachments))` becomes
    `PublishChanges(new(facts, removals, attachments))`, listing the attached
    child among the upserts;
  - a later `PublishSnapshot` keeps the attachment;
  - upserting the child without an attachment clears it.
- **Test fakes:** implementations of `IGraphicsService` replace
  `PublishAttachedSnapshot` with `PublishChanges(AppearanceChangesRequest)`.
  Affected in downstream tests: Dagger's `SpriteWorkbenchProductTests` and
  `DaggerfallExteriorTerrainAppearanceTests`.
- **Behaviour changes:**
  - a batch may not name the same object twice, or both upsert and remove it;
  - a layer change now recreates the object; before, a primitive's layer change
    was silently ignored.

## Found, not fixed here

- **Voxel scene render projection walks every chunk per call.** It revalidates
  every instance, compares every chunk hash and clones its handle registry. It
  also keeps source/rebase revision guards. Filed as #8797.
