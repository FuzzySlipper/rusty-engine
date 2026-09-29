# Lane: spatial

**Tasks:** #8754 (it now includes #8755). **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

## What to do

#8754 has two parts; land them together in one change:
- Stop building a throwaway `EntityState` in world-origin rebase
  (`csharp-engine-services/src/world_origin.rs` `call_entities()`) and trigger
  reconcile (`spatial.rs` `entity_state()`). Consume the product's rows through
  typed slices or a small read trait, like the existing `CharacterStepWorld`.
- Drop the call-local revision fields:
  - `NativeCharacterStepReceipt.revision_before/after`;
  - `NativeKinematicMotionLease.revision_before/after`;
  - the kinematic revision clause in
    `csharp/Rusty.Engine/Entities/EntityKinematicMotion.cs`
    `ValidateMotionReceipt`.

Also remove these two guards: the static-mesh `expected_geometry_hash` /
`StaleAsset` check and the trigger `expected_revision` check. They sit in your
files, and #8798 (main lane) has handed them to you. Both are P4-style guards
under the campaign rule:
- The static-mesh check lives in `svc-collision/src/static_mesh.rs`
  (`commit_projection`). `apply_residency` already fills it from the asset it
  checks against, so it cannot fail there. Its callers are in
  `csharp-engine-services/src/spatial.rs`.
- The trigger check is `TriggerVolumeSystem::require_revision` in
  `engine-spatial/src/trigger.rs`.

Note these points when you land:
- rusty-dagger reads the trigger receipt's `RevisionAfter`. Check what it means
  after your change, and update Dagger to the exact new pair if its build
  breaks.
- #8742 already removed the entity-count caps in `trigger.rs`,
  `world_origin.rs`, `kinematic.rs` and `motion.rs`. It left the paging bounds
  (`MAX_TRIGGER_READ_ITEMS`, trigger overlap page size) for #8744, so don't
  touch those.

## Files

- **Owns:**
  - `csharp-engine-services/src/spatial.rs`, `world_origin.rs`, `kinematic.rs`;
  - `engine-spatial/src/trigger.rs`, `world_origin.rs`;
  - `svc-collision/src/static_mesh.rs` guards;
  - the character and kinematic receipts in `csharp-engine-abi`;
  - `EntityKinematicMotion.cs`.
- **Leave alone:**
  - `engine-spatial/src/lib.rs` mesh neighbourhood and `voxel_edit.rs`: main
    lane, fixing #8739;
  - `voxel_scene_presentation.rs`: voxel-render lane.

## Evidence

Focused bridge tests: trigger enter/stay/exit facts, and rebase results before
and after. Run `scripts/test-csharp-sdk-package.sh --coreclr-smoke`, since the
ABI and C# change.
