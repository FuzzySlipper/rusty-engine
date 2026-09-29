# Lane: voxel-render

**Task:** #8797. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

## What to do

Make voxel scene render projection cost follow the chunks that changed.

Today `render-projection/src/voxel.rs`
`VoxelRenderProjector::project_mapped_directional` (called from
`csharp-engine-services/src/voxel_scene_presentation.rs`
`project_scene[_directional]`) does this on every call:
- revalidates every instance and material;
- walks every chunk's groups (`validate_and_snapshot`) and compares every chunk
  content hash;
- clones the handle registry for rollback;
- re-validates the emitted frame;
- keeps the `StaleSourceRevision` / `StaleRebaseRevision` guards.

Remove the guards and the registry clone. Visit only dirty, added and removed
chunks, plus material or slot-mapping changes. Validate before mutating.

## Inputs from landed work

- Since #8739 (`64a9b164`) the scene keeps mesh chunks as shared
  `Arc<VoxelMeshChunk>` in a map, and `mesh_update().dirty_chunks` names what
  changed. See `docs/evidence/incremental-voxels-8739/README.md`.
- The main lane is fixing a #8739 review finding in `engine-spatial/src/lib.rs`
  (`mesh_neighbourhood_of_voxel`). Dual Contouring needs a wider dirty radius,
  so the set of dirty chunks grows. The projector should rely on whatever
  `dirty_chunks` says, not on its own neighbourhood rule. Rebase over that fix
  before landing.
- #8737 (`65ff4919`) did the same retained, validate-then-mutate conversion for
  appearance objects (`render-projection/src/runtime_appearance.rs`). Its
  randomized equivalence test, which drives a model renderer that refuses stale
  handles, is a good pattern to copy.

## Files

- **Owns:** `render-projection/src/voxel.rs`,
  `csharp-engine-services/src/voxel_scene_presentation.rs`.
- **Leave alone:**
  - `engine-spatial/src/lib.rs` and `voxel_edit.rs`: main lane;
  - `render-projection/src/runtime_appearance.rs` and `appearance.rs`: main
    lane;
  - `render-projection/src/retained.rs`: `StableHandleRegistry` is shared.

## Evidence

- Before/after cost for a one-chunk edit in a 256-chunk scene. The #8739
  probes in `engine-spatial/examples/` and the `voxel-exercise` product under
  `docs/evidence/incremental-voxels-8739/scripts` are reusable.
- A fresh renderer attachment gets the same baseline as before.
