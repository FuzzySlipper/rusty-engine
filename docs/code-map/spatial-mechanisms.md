# Spatial mechanisms

## Purpose

Route work involving canonical voxel space and the collision, navigation, mesh,
motion, trigger, occlusion, picking, primitive, and edit mechanisms derived
from it.

## Owns

- `engine-spatial`: the cohesive provider-facing spatial service over one
  canonical voxel authority.
- `svc-volume`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, and
  `svc-mesh`: focused reusable algorithms and data structures.
- Atomic voxel edits and derived-index refresh within the spatial owner.

## Does not own

- Game-specific movement policy, AI goals, combat rules, or scheduling.
- A second authoritative world model in renderer or UI code.
- Stored voxel asset conversion, scene persistence, or product input handling.

## Primary paths

- [`engine-spatial/src/lib.rs`](../../rust/crates/engine-spatial/src/lib.rs)
- [`engine-spatial/src/entity_motion.rs`](../../rust/crates/engine-spatial/src/entity_motion.rs)
- [`engine-spatial/src/rigid_body.rs`](../../rust/crates/engine-spatial/src/rigid_body.rs)
- [`engine-spatial/src/voxel_edit.rs`](../../rust/crates/engine-spatial/src/voxel_edit.rs)
- [`engine-spatial/src/trigger.rs`](../../rust/crates/engine-spatial/src/trigger.rs)
- [`svc-volume`](../../rust/crates/svc-volume)
- [`svc-spatial`](../../rust/crates/svc-spatial)
- [`svc-collision`](../../rust/crates/svc-collision)
  - [`static_mesh.rs`](../../rust/crates/svc-collision/src/static_mesh.rs)
  - [`dynamics.rs`](../../rust/crates/svc-collision/src/dynamics.rs)
- [Rigid-body dynamics](../topics/rigid-body-dynamics.md)
- [`svc-pathfinding`](../../rust/crates/svc-pathfinding)
- [`svc-mesh`](../../rust/crates/svc-mesh)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Reconstructed voxel surfaces](../topics/voxel/reconstructed-surfaces.md)

## Public downstream surfaces

- Downstream Rust calls named services for queries, movement, edits, collision,
  navigation, and meshing.
- `engine-spatial` is the normal cohesive entry point when derived structures
  must stay synchronized with canonical voxel edits.
- The smaller service crates remain useful where a consumer needs only one
  focused mechanism.
- `VoxelCollisionScene::replace_static_mesh_colliders` projects immutable
  content plus caller-owned instance transforms at a separate exact revision.
  World queries combine those triangles with voxel colliders, while voxel
  edit picking deliberately retains its voxel-only result type. Prepared voxel
  edits and history reverts guard that independent revision before swapping a
  rebuilt scene.
- `svc-mesh::texture_mapping` owns the executable six-face tile basis and exact
  cell-space projection used by the default greedy mesher; it owns no image or GPU
  resource.
- `svc-mesh::SurfaceMode` selects optional derived reconstruction. The shared
  scalar field, ambiguity/QEF policy, quotas, and chunk-owner seam remain in
  Rust; canonical world services continue to use greedy output unless a caller
  explicitly selects another disposable presentation.
- The caller-driven rigid-body service consumes exact entity component slots
  plus this canonical voxel/static-triangle environment. Rapier caches are
  derived, bounded, and non-durable; complete accepted steps publish atomically
  through entity-state.

## Private or forbidden paths

- Do not add browser input, DOM events, HTTP, WebGL, or product-shell concerns.
- Do not infer gameplay meaning from collision layers, trigger facts, or paths.
- Do not bypass `engine-spatial` with independent mutable copies when atomic
  synchronization is required.

## Acceptance gates and fixtures

```bash
cargo test -p engine-spatial --locked
cargo test -p svc-volume -p svc-spatial -p svc-collision -p svc-pathfinding -p svc-mesh --locked
cargo clippy -p engine-spatial --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Provider evidence lives under
[`engine-spatial/tests`](../../rust/crates/engine-spatial/tests) and
[`fixtures/spatial-grid`](../../fixtures/spatial-grid).

## Common agent mistakes

- Adding game policy to a pathfinding or collision helper.
- Treating a derived mesh or collision index as independent authority.
- Updating voxel storage without preserving atomic derived-state consistency.
- Generalizing one consumer's tick loop into an Engine scheduler.

## Follow-up routing

- Stored voxel artifacts and offline conversion:
  [Voxel assets and conversion](voxel-assets-and-conversion.md).
- Entity transforms and components:
  [Entity state and state machines](entity-state-and-state-machines.md).
- Retained visual projection:
  [Rust render model and projection](rust-render-model-and-projection.md).
