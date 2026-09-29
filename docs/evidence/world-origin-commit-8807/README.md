# World-origin commit rebases the live scene (#8807)

## What changed

- `NativeWorldOriginPrepareRequest` and `WorldOriginRebaseRequest` lose
  `expected_origin_revision`, `expected_voxel_source_revision` and
  `expected_static_mesh_revision`. `EntityOriginRebaser.Prepare` no longer
  reads the origin first.
- Prepare computes only each root's local transform in the target frame. That
  depends on the global position, the target and the session envelope, and
  not on the base origin or the scene. The prepared handle holds the target
  and those transforms: no scene candidate.
- Commit rebases the **live** collision scene into the target and moves the
  origin. `WorldOriginRebaseService::commit(origin, &scene, &prepared)` returns
  the rebased scene for the caller to install. The bridge installs it without
  first cloning the session scene.
- `StaleOrigin`, `StaleVoxelScene`, `StaleStaticMeshes` and
  `SceneOriginMismatch` are removed. `NativeWorldOriginPreparedReadout` loses
  `candidate_revision`, `candidate_voxel_source_revision` and
  `candidate_static_mesh_revision`, since no candidate exists before commit.

## Why not a revision fence

The task proposed recording base revisions at prepare and checking them at
commit. That keeps a cloned candidate scene alive between the calls and still
misses changes that bump no revision: `set_noncollidable_materials` rebuilds
the collision projection without changing the source revision, so a
candidate prepared before it would have reverted the filter. Rebasing the live
scene at commit removes the candidate, the fence and that gap together.
Committing two prepared rebases in any order is safe: each one's transforms
are correct for its own target, and the last commit decides the origin.

`SceneOriginMismatch` had no reachable path. Every session-scene replacement
clones the current scene, which keeps its origin and rebase revision. The one
exception is voxel-asset publish, which builds a fresh scene only for a
session whose origin is still zero, and its commit checks that the session
scene was not swapped since prepare.

## Evidence

- `cargo test -p engine-spatial -p render-projection -p csharp-engine-services`
  passes.
  - The bridge test
    `commit_rebases_the_live_scene_and_keeps_edits_made_after_prepare` has two
    parts. Two prepared rebases commit in either order, with revisions 1 → 2
    and the last target winning. Then a voxel edit made between prepare and
    commit is still present in the rebased scene (raycast hit) and in the
    receipt's source revision.
  - The engine test
    `failed_prepare_publishes_nothing_edits_after_prepare_survive_and_snapshots_are_typed`
    shows the same with a voxel clear.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
  Clippy reports only the #8757 baseline lints.
- Bindings regenerated. `scripts/test-csharp-sdk-package.sh --coreclr-smoke`
  passes, and `Rusty.Engine.Entities.Example` runs clean.

## Migration

- C#: `WorldOriginPrepareRequest(session, targetCellX, targetCellY,
  targetCellZ, entities)`. `WorldOriginPreparedReadout` has no candidate
  revisions. `EntityOriginRebaser` callers are unaffected.
- Rust: `prepare(&origin, request)` and
  `let (scene, receipt) = commit(&mut origin, &scene, &prepared)?`.
- rusty-dagger reads only `WorldOriginCommitReceipt`, which is unchanged, and
  does not build prepare requests.
