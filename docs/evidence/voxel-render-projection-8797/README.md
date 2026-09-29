# Voxel render projection follows changed chunks (#8797)

`VoxelRenderProjector` used to redo work across the whole world on every call,
including a refresh with nothing to do. It now visits only the chunks the
scene says changed since the last call, and checks them before it changes any
retained state.

## Removed

- **Revision guards.** `StaleSourceRevision` and `StaleRebaseRevision`
  rejected a scene whose revision went backwards, or whose chunks changed at
  the same revision. The source is always the session's current scene, so
  nothing stale reaches the projector. The one real case the guard hit was a
  scene replaced at the same revision (voxel asset staging builds a new scene
  at revision 0). It turned that into an error; it is now projected in full.
- **Whole-world check on every call** (`validate_and_snapshot`). For every
  chunk of every instance, it built a complete mesh payload (copying every
  position, normal, UV and index), validated it, then threw it away. It also
  walked every group to find the material slots in use.
- **Registry clone for rollback** and the re-validation of the emitted frame
  (`RenderFrameDiff::try_from_published_ops`). Checks now run before any
  mutation, so there is nothing to roll back. The frame is built directly, as
  the appearance projector has done since #8737.
- **Checks on Engine-produced values:**
  - material descriptor validation and the `voxel-material/<slot>` id check.
    The descriptors come from Appearance's retained resources, which the
    renderer already accepted, and the presentation bridge sets the id;
  - mesh payload validation. The meshes come from `engine-spatial`;
  - empty or duplicate instance ids and transform validation. Ids are the
    caller's retained keys, and the only production caller generates them;
  - the publication revision overflow check.

  These are removed with their `VoxelProjectionError` variants.
- **Old test** `stale_or_same_revision_changed_scene_rejects_without_projector_mutation`.
  It is replaced by `a_replaced_scene_at_the_same_revision_is_projected_in_full`.

## Kept, and why

- **`MissingMaterial`.** An edit can paint a slot the presentation never bound.
  `RefreshScene` does not resolve materials again, so this is the only place
  that catches it before a mesh group points at an undefined material.
- **`TexturedReconstructedSurface`.** Reconstructed surfaces have no tile
  coordinates, so a textured material cannot render on them.
- Both checks run before anything is mutated. They cover the chunks being
  visited, or every chunk when the materials changed.
- **`Handle`.** Handle allocation still returns the registry's error. It needs
  2^40 handles, so it is not recoverable and is not checked in advance.
- **Publication stream and revision.** These are part of the renderer
  protocol and are unchanged.

## What replaces it

- **Scene side (`engine-spatial`).** This is a small addition to a file owned
  by the main lane:
  - `VoxelCollisionScene::mesh_lineage()`, a process-unique id. A build,
    including a world-origin rebase, gets a new one. Local changes and clones
    keep it.
  - `mesh_chunk(coord)`, a lookup by chunk coordinate.
- **Projector.** For each instance it keeps the lineage and source revision it
  last projected, then:

  | Scene compared with the last projection | Chunks visited |
  |---|---|
  | same lineage, same revision | none |
  | same lineage, next revision | `mesh_update().dirty_chunks` only |
  | new lineage, skipped revisions, a new or rebound instance, or a changed slot mapping | every chunk; each one's mesh hash and translation is compared with what was retained |

  The dirty list comes straight from the scene. The projector has no
  neighbourhood rule of its own, so the Dual Contouring fix in `0fb137ec`
  reaches it unchanged.
- **Material changes.** Changed descriptors emit `DefineMaterial` only. They
  trigger the material checks over every chunk, but no payloads.

## Fixed along the way

When a chunk's mesh hash and its translation both changed, the old projector
replaced the payload but never sent the new transform. Its code was
`if hash changed { replace } else if moved { update }`. A world-origin rebase
followed by an edit left chunks drawn at their old position. The randomized
test below found this at step 137. The projector now sends both operations.

## Evidence

### Correctness (`render-projection/src/voxel.rs` tests)

`incremental_projection_realizes_the_same_scene_as_a_fresh_attachment` copies
the model renderer pattern from #8737. The renderer refuses to create a handle
twice, to parent to a dead handle, and to update or destroy a handle it does
not hold. The test covers three instances: Greedy Cubes, Dual Contouring and
Marching Cubes, with 4³ chunks. Over 300 steps it applies:

- random edit batches;
- transform moves;
- slot-mapping toggles;
- removing and re-adding an instance;
- scene replacement (a new lineage at revision 0);
- world-origin rebases;
- skipped projections, so several revisions pass between calls.

After every projection, the incrementally realized scene must equal a fresh
projector's baseline in a fresh renderer. That comparison covers every node's
parent, label, transform, mesh payload and material. At least 200 of the
frames must touch only some of the chunks.

The existing tests still pass: stable handles, boundary edits, residency
admit/replace/evict, rebase without payload replacement, and material errors
without mutation. So do the `csharp-engine-services`, `csharp-product-runtime`
(including the fresh graphics and voxel baseline test) and `engine-spatial`
suites.

### Cost (`results/projection-cost.txt`)

`render-projection/examples/voxel_projection_cost.rs` projects a 256-chunk
terrain: 16 × 16 chunks of 8³, one chunk high. It then times only the
projection call, 200 samples each, for two cases:

- a refresh with no change;
- a refresh after a one-cell edit inside one chunk.

The before binary was built from `0fb137ec`; the after binary is this change.
Three runs alternated between the two builds on a shared, loaded machine.

| Scenario (256 chunks) | Before, median | After, median |
|---|---|---|
| refresh, nothing changed | 577–743 µs | 0.2–0.3 µs |
| refresh after a one-chunk edit (1 `ReplaceMeshPayload`) | 645–933 µs | 1.6–2.5 µs |

### Fresh attachment baseline

With `VOXEL_BASELINE_FRAMES` set, the probe writes two frames:

- the initial projection;
- a fresh projector's projection of the world after 200 edits.

Both builds produced byte-identical JSON (sha256 `dad76007…`). A fresh
renderer attachment gets the same baseline as before.

## Migration

- `VoxelProjectionError` now has only `MissingMaterial`,
  `TexturedReconstructedSurface` and `Handle`. The only production caller,
  the C# voxel scene presentation bridge, formats the error with `{:?}`.
  Nothing matched on the removed variants.
- The C# API does not change. `RefreshScene` has the same semantics and is
  cheaper.
