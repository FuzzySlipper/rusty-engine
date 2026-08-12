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
- [`engine-spatial/src/character_controller.rs`](../../rust/crates/engine-spatial/src/character_controller.rs)
- [`engine-spatial/src/rigid_body.rs`](../../rust/crates/engine-spatial/src/rigid_body.rs)
- [`engine-spatial/src/voxel_edit.rs`](../../rust/crates/engine-spatial/src/voxel_edit.rs)
- [`engine-spatial/src/voxel_residency.rs`](../../rust/crates/engine-spatial/src/voxel_residency.rs)
- [`engine-spatial/src/trigger.rs`](../../rust/crates/engine-spatial/src/trigger.rs)
- [`svc-volume`](../../rust/crates/svc-volume)
- [`svc-spatial`](../../rust/crates/svc-spatial)
- [`svc-collision`](../../rust/crates/svc-collision)
  - [`static_mesh.rs`](../../rust/crates/svc-collision/src/static_mesh.rs)
  - [`dynamics.rs`](../../rust/crates/svc-collision/src/dynamics.rs)
- [Rigid-body dynamics](../topics/rigid-body-dynamics.md)
- [FPS character controller design](../topics/fps-character-controller-proposal.md)
- [FPS character controller survey](../topics/fps-character-controller-survey.md)
- [`character_controller` facade example](../../rust/crates/rusty-engine/examples/character_controller.rs)
- [`measure-character-controller.sh`](../../scripts/measure-character-controller.sh)
- [`verify-character-controller-consumer.sh`](../../scripts/verify-character-controller-consumer.sh)
- [`svc-pathfinding`](../../rust/crates/svc-pathfinding)
- [`svc-mesh`](../../rust/crates/svc-mesh)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Reconstructed voxel surfaces](../topics/voxel/reconstructed-surfaces.md)
- [Chunk-granular voxel mesh updates](../topics/voxel/chunk-granular-updates.md)
- [Canonical voxel chunk residency](../topics/voxel/chunk-residency.md)
- [`measure-voxel-chunk-updates.sh`](../../scripts/measure-voxel-chunk-updates.sh)
- [`measure-voxel-chunk-residency.sh`](../../scripts/measure-voxel-chunk-residency.sh)
- [`verify-voxel-chunk-consumer.sh`](../../scripts/verify-voxel-chunk-consumer.sh)

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
- `VoxelEditReceipt` exposes the exact signed dirty mesh set plus rebuilt,
  reused, and removed counts. `VoxelMeshChunk` separates canonical source hash
  provenance from its complete neighbor-sensitive derived payload hash.
- `VoxelChunkResidencyService::{prepare,commit,apply,apply_with_history}` owns
  bounded, explicit complete-chunk admission, replacement, and eviction. Exact
  source/content preconditions, instance-owned leases, and one selected history
  policy preserve fail-atomic canonical and derived publication; downstream
  retains sourcing, streaming, radius, scheduling, and memory-pressure policy.
- The caller-driven rigid-body service consumes exact entity component slots
  plus this canonical voxel/static-triangle environment. Rapier caches are
  derived, bounded, and non-durable; complete accepted steps publish atomically
  through entity-state.
- `CharacterControllerService::{step,prepare,commit}` is the host-neutral
  kinematic FPS path. It consumes one entity with Transform plus inert
  `CharacterMotionComponent`, a `CharacterControllerConfig`, and a sequenced
  fixed-step command. It solves against canonical voxel/static-mesh collision
  plus active entity obstacles, then returns accepted Transform/motion and
  bounded ground/contact/block/stance/step/platform/dynamic-impulse facts.
- `CharacterControllerConfig` and every public nested policy struct are
  `#[non_exhaustive]`, serde-defaulted, and constructible through `Default` or
  `responsive_fps()`. Sparse documents and downstream one-field overrides keep
  receiving defaults when compatible fields are added; `validate()` still
  rejects invalid values before queries or mutation.
- `FirstPersonLookService` is a separate pure yaw/pitch and basis service. It
  shares heading conventions with character movement but does not own a camera,
  device bindings, entity position, or collision.
- `svc-collision` exposes local-+Y `CharacterCapsule` cast/overlap facts for
  voxel chunks, admitted static meshes, and explicit active-entity obstacles
  without exposing backend handles.

## Private or forbidden paths

- Do not add browser input, DOM events, HTTP, WebGL, or product-shell concerns.
- Do not infer gameplay meaning from collision layers, trigger facts, or paths.
- Do not bypass `engine-spatial` with independent mutable copies when atomic
  synchronization is required.

## Acceptance gates and fixtures

```bash
cargo test -p engine-spatial --locked
cargo test -p engine-spatial --test character_controller --locked
cargo test -p entity-state character_motion --locked
cargo test -p svc-collision character_capsule --locked
cargo run -p rusty-engine --example character_controller --locked
./scripts/measure-character-controller.sh
./scripts/verify-character-controller-consumer.sh /absolute/path/to/rusty-craftsurvive
./scripts/measure-voxel-chunk-updates.sh
./scripts/measure-voxel-chunk-residency.sh
./scripts/verify-voxel-chunk-consumer.sh /absolute/path/to/rusty-craftsurvive
cargo test -p svc-volume -p svc-spatial -p svc-collision -p svc-pathfinding -p svc-mesh --locked
cargo clippy -p engine-spatial --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Provider evidence lives under
[`engine-spatial/tests`](../../rust/crates/engine-spatial/tests) and
[`fixtures/spatial-grid`](../../fixtures/spatial-grid).
The focused controller integration tests are in
[`engine-spatial/tests/character_controller.rs`](../../rust/crates/engine-spatial/tests/character_controller.rs).

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
