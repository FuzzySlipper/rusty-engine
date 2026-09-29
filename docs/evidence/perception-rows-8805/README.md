# Perception occluder rows (#8805)

## What changed

- `SpatialPerceptionQuery` takes `occluders: &[SpatialOcclusionCollider]`
  (entity and world-space AABB) instead of `entities: &EntityState`. The
  perception bridge maps the product's enabled `NativeSpatialEntityCollider`
  rows into that slice. Disabled rows are skipped, as before, when the throwaway
  store only admitted enabled collision. The ABI does not change.
- `spatial.rs` `entity_state()` is deleted. It was the last per-call
  `EntityState` adapter in the C# bridge.
- `SpatialOcclusionService` has one entry point, `cast_ray(scene, query,
  colliders)`, which is the former `cast_ray_against_colliders`. The
  `EntityState` forms `cast_ray` and `cast_ray_with_overrides`, and
  `SpatialOcclusionHitboxOverride`, are removed. After this change no Rust
  host called them. Products that want a hitbox pass that box as the
  collider, and the bridge's ray queries already do.

## Evidence

- `cargo test -p engine-spatial -p csharp-engine-services` passes.
  - New engine test
    `entity_occluders_block_sight_except_for_the_observer_and_target_boxes`.
    Every earlier perception test passed an empty entity store, so entity
    occlusion had no engine-level coverage.
  - New bridge test `native_visibility_is_blocked_only_by_enabled_occluder_rows`:
    the target's own box does not hide it, an enabled box between observer
    and target occludes, and a disabled one does not.
  - `engine-spatial/tests/occlusion.rs` is rewritten on supplied boxes:
    nearest order, entity-before-voxel ties, lowest-id ties, a large box set,
    and a typed invalid direction.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
- Clippy on the touched crates reports only the #8757 baseline lints.

## Migration

Rust only; the C# API is unchanged.
- `SpatialPerceptionQuery { entities, .. }` becomes `{ occluders, .. }`.
- `SpatialOcclusionService.cast_ray(&scene, &entities, query)` becomes
  `SpatialOcclusionService::cast_ray(&scene, query, colliders)`.
