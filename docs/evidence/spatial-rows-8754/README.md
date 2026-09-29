# Spatial bridge rows and receipt revisions (#8754, with #8755)

## What changed

- **Trigger reconcile and restore** take `TriggerCollider` rows (entity,
  world-space AABB, collision flag) instead of an `EntityState`. The bridge
  maps the product's `NativeSpatialEntityCollider` slice straight into the
  service iterator; it no longer builds a named, validated entity store per
  call. `trigger_geometry.rs` is gone, and so are the diagnostic codes that
  the rows cannot produce (missing collision, bounds or transform).
- **World-origin rebase** takes the product's rows directly. Each
  `WorldOriginEntity` carries its current local transform, and the service
  replaces only the translation. The `EntityState` prepare/commit/apply path,
  its entity-revision guard, root/parent/duplicate checks and the
  character-motion continuation rewrite are removed: no Rust host used them,
  and the C# bridge never exercised the continuation (its call-local store
  had no character motion). `commit_spatial` is now `commit`.
- **Receipts.** `NativeCharacterStepReceipt.revision_before/after` and
  `NativeKinematicMotionLease.revision_before/after` are removed from the
  ABI and the generated C# types. `ValidateMotionReceipt` was already
  deleted by #8741.
- **Guards handed over by #8798.** `TriggerVolumeSystem::require_revision`
  and the `expected_revision` inputs of `SetTriggerActive` and
  `RestoreTriggers` are removed. The static-mesh
  `expected_geometry_hash`/`StaleAsset` check is removed with the field and
  the two accessors that only filled it; `MissingAsset` still rejects an
  instance whose asset is absent.

## Kept, and why

- World-origin commit still checks the origin, voxel-source and static-mesh
  revisions. The prepared candidate holds a cloned, rebased collision scene;
  committing it after a voxel edit would silently discard the edit. The
  bridge test `stale_origin_or_voxel_scene_rejects_commit_without_publishing_candidate`
  and the engine test `failed_prepare_and_stale_commit_publish_nothing_and_snapshots_are_typed`
  cover it.
- The trigger system revision stays: overlap pages use it to fence
  continuations, and paging belongs to #8744.
- `entity_state()` in `spatial.rs` stays for perception occluders, which are
  outside this task.

## Evidence

- `cargo test -p engine-spatial -p svc-collision -p csharp-engine-services -p render-projection -p asset-import`:
  all pass. New bridge test
  `trigger_reconcile_reports_enter_stay_and_exit_from_collider_rows` shows
  enter → stay (continued, same revision) → exit, exit on a removed subject
  row, and a diagnostic for a trigger without a row. The rebase bridge test
  `prepared_world_origin_commits_scene_and_exact_copied_transforms` shows
  the prepared readout, the affected local transform (translation rebased,
  scale and rotation kept) and the commit receipt.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
- Clippy on the touched crates reports only the known #8757 baseline lints.
- `scripts/generate-csharp-native-bindings.sh` regenerated the bindings.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passed (it needs
  `DOTNET_ROOT` when dotnet lives in `~/.dotnet`, and a `TMPDIR` outside the
  repository path, because the leak check matches the repository root as a
  literal prefix).
- `Rusty.Engine.Entities.Example` runs clean, and the NativeAOT-trial fixture
  compiles.

## Migration

- `SpatialTriggerSetActiveRequest(session, trigger, active, tick)` and
  `SpatialTriggerRestoreRequest(session, activeTriggers, entities)` lose
  `ExpectedRevision`. Products that stored the trigger revision only to pass
  it back can drop it.
- `CharacterStepReceipt` and `KinematicMotionLeaseReceipt` lose
  `RevisionBefore`/`RevisionAfter`.
- Rust callers: `WorldOriginRebaseRequest` loses `expected_entity_revision`;
  `WorldOriginEntity` gains `transform`; `prepare(origin, scene, request)` and
  `commit(origin, scene, &prepared)` replace the `EntityState` path.
  `TriggerVolumeSystem::{reconcile, restore}` take `TriggerCollider` rows and
  `set_active`/`restore` lose `expected_revision`.
  `StaticMeshColliderInstance` loses `expected_geometry_hash`.
