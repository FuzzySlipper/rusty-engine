# Incremental voxel edits and residency (#8739)

Voxel edits and residency changes now write into the scene's chunks and rebuild
only what they touch:
- the changed chunks' meshes, plus neighbours whose surfaces meet the change;
- the changed chunks' colliders;
- the navigation cells around the changed voxels.

Nothing clones, hashes or rebuilds the whole scene. Engine voxel history, chunk
leases, background residency preparation and the revision/hash guards are gone.
About 6,350 lines were deleted and 1,700 added, including rewritten tests.

## Removed

- **Whole-scene rebuilds.** Every edit and residency change used to build a
  complete candidate scene off to the side, then swap it in:
  - a new voxel world from a sorted copy of every voxel;
  - every collider and navigation cell;
  - the mesh list, with unchanged chunk meshes deep-copied;
  - a whole-scene hash.

  With noncollidable materials configured, it cloned the world and rebuilt
  collision and navigation a second time. The bridge also cloned the session's
  scene before each change.
- **Flat voxel copies.** The scene stored sorted `solid_voxels` and
  `material_voxels` vectors of every voxel. `material_voxels()` now computes
  them on request.
- **Prepare/commit guards:**
  - `VoxelEditService::preview`/`commit` and `PreparedVoxelEdit`;
  - `VoxelChunkResidencyService::prepare`/`commit`/`finish_prepared`;
  - the source-revision, authority-hash, residency-hash, static-collision,
    world-origin and lease-generation checks between them.
- **Product-supplied guards:**
  - the edit transaction's expected revision and the `StaleRevision` outcome;
  - the residency transaction's expected revision;
  - the per-operation expected content hash.
- **Engine voxel history.** Survey #8761 found no production caller; CraftSurvive's
  build undo is product-owned. Removed:
  - `VoxelEditHistory`, its codec and history policy;
  - Undo/Redo, the history cursor, entry and delta reads, and export/restore;
  - the SDK's `VoxelHistoryPersistenceStore`.
- **Chunk leases.** Product-owned pins against the product's own eviction:
  `AcquireChunkLease`/`ReadChunkLease`, the registry and the pinned-chunk
  refusal.
- **Background residency preparation.** Start/Poll/Commit/Cancel and the worker
  thread existed to move whole-scene rebuilds off the callback. A local
  residency change now costs about as much as building its chunk (0.5–1.6 ms
  in the probe, 0.7 ms through the product path).
- **Caps and refusals (the voxel part of #8742):**
  - 4,096 edits per transaction, 64 chunks per residency transaction, the
    payload-slot and resident-chunk caps;
  - the duplicate-address and duplicate-chunk refusals (a later entry wins);
  - "chunk already resident" and "chunk not resident" (admitting identical
    content or evicting an absent chunk is a no-op);
  - the "configure material collision before any edit" rule. Setting it later
    now updates collision for the whole scene.
- **`Voxel.ReadAt(index)`,** which indexed the flat voxel copy.
- **Old tests** of preview/commit, history, leases, preparation, stale guards
  and caps.

## Kept, and why

- **Coordinate, material-slot and state validation,** checked before any write.
  A nonzero state is refused in smooth surface modes: the mesh cannot
  represent it, and the edit would otherwise fail halfway.
- **Reverting written voxels when a chunk mesh fails to build.** A mesh limit is
  the one failure that can happen after a write. The existing mesh-limit test
  exercises it.
- **World-origin rebases** still rebuild the scene. Every chunk shape moves, so
  nothing could be kept.
- **Tombstoned handles** and the scene's diagnostic accessors are unchanged.

## What replaces it

- `VoxelCollisionScene` keeps mesh chunks as shared `Arc`s in a map. Its
  counts and authority hash are maintained per voxel, as an order-independent
  sum.
- `publish_local_change` installs the rebuilt meshes, then calls
  `CollisionProjection::reconcile_chunks`, which reuses unchanged chunk shapes.
- Navigation uses `NavProjection::refresh_cells`. Its hash is now an
  order-independent sum, so goldens changed values but not paths.
- `VoxelEditService::apply(scene, edits)` and
  `VoxelChunkResidencyService::apply(scene, operations)` validate, write, build
  the touched meshes and publish.
- **In the bridge:** `RuntimeSpatialBridge::edit_scene` drops the collision
  source's copy of the scene `Arc` and mutates the session's copy through
  `Arc::make_mut`. With nothing else holding it, nothing is cloned.
- **Dynamics** remembers the Spatial publication identity it bound instead of
  holding the scene; an edit would otherwise force a whole-scene copy.
- **Receipts:** a residency batch that changes nothing returns an ordinary
  receipt with zero changes instead of an error. An edit that changes nothing
  still returns `NoChanges`.

## Evidence

### Correctness (`engine-spatial/tests/voxel_local_changes.rs`)

- **Equivalence.** The test runs 150 random edit batches over a 4 × 4-chunk
  terrain, then residency changes (two evictions, an admission, a replacement),
  then 50 more edits. The result matches a scene whose chunks were all admitted
  in one pass:
  - authority hash and solid count, also against a scene built from the
    voxels;
  - every chunk mesh's content hash and the collider count;
  - navigation count and hash;
  - a grid of raycasts.

  Writing this test found two bugs, both fixed:
  - edits dirtied every neighbour chunk, not only neighbours at the boundary;
  - chunks created by an edit left their empty cells out of navigation.
- **Noncollidable materials.** Materials set before and after edits both match
  a full build.
- **A one-cell edit in a 256-chunk world** rebuilds its own chunk only. A
  corner cell also rebuilds its three touching neighbours; 255 meshes are
  reused.
- **A distant resting body.** A Dynamics body resting on a far chunk stays
  asleep through an edit elsewhere. The rebind keeps 63 of 64 floor colliders
  and replaces one.

### Cost (`results/voxel-edit-scaling.txt`, `results/voxel-budget.txt`)

Release build; before is `c3100825`.

| | Before | After |
|---|---|---|
| Clear 1–123 cells, 16-chunk world | 12.6–13.1 ms | 0.35–0.39 ms |
| Clear 1–123 cells, 64-chunk world | 50.6–52.2 ms | 0.38–0.42 ms |
| Cell edit, 64 solid chunks (median) | 104.0 ms | 1.18 ms |
| Chunk replace, 64 solid chunks | 77.1 ms | 1.60 ms |
| Cell edit, 256 sparse chunks | 145.7 ms | 0.02 ms |
| Chunk replace, 256 sparse chunks | 156.3 ms | 0.56 ms |
| Peak RSS, 64 solid chunks | 413 MiB | 98 MiB |

A single-chunk scene costs the same as before (about 2 ms).
[docs/voxel-budgets.md](../../voxel-budgets.md) has the full table.

### Through a product (`scripts/voxel-exercise`, `results/exercise-*.json`)

An SDK product streams a 16 × 16-chunk floor (256 chunks). For 600 fixed
updates it flips one far-away voxel with `ApplyEdits` and steps a character
standing on the far corner. It also:
- unloads and reloads a chunk at update 300;
- attempts an invalid edit at update 400;
- places a noncollidable voxel at update 450.

The same source builds against the `c3100825` SDK/runtime pack with
`OLD_VOXEL_API`, which adds revisions and hashes. `scripts/run-exercise.py` runs
it with a headless browser. Two runs each:

| | Before | After |
|---|---|---|
| `ApplyEdits`, one voxel, p50 | 45.5–45.6 ms | 86 µs |
| `ApplyEdits`, one voxel, p95 | 49.2–49.7 ms | 133–136 µs |
| Unload + reload one chunk | 91–93 ms | 0.65–0.76 ms |

Both versions report `VOXEL_EXERCISE_PASSED` with identical facts:
- 256 resident chunks, 16,385 solids, 114,688 navigation cells;
- the character grounded on all 540 checked updates;
- the invalid edit refused, with the scene's revision and hash unchanged;
- the ray passing through the noncollidable voxel to the floor at y = 1.0.

### Checks

- **Workspace:** 1,178 tests pass (excluding `renderer-webview-host`, which
  needs GTK), including the rewritten spatial/voxel suites. Clippy is clean
  apart from the pre-existing #8757 lints.
- **C#:**
  - the SDK and examples build;
  - `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes;
  - `scripts/test-runtime-pack.sh` runs the NativeAOT fixture's voxel edit,
    no-change, refusal and residency checks.
- **Standalone fixtures** (world streaming, lighting, voxel capacity, voxel
  atlases) build against the new package.

## Migration

Products pin their SDK version, so nothing breaks until they move.

**Edits**
- `new VoxelEditTransaction(session, revision, edits)` becomes
  `new VoxelEditTransaction(session, edits)`.
- Drop `VoxelEditStatus.StaleRevision` handling and `CurrentRevision`; read
  `AcceptedRevision`.

**Residency**
- `new VoxelResidencyTransaction(session, revision, policy, operations, slots)`
  becomes `new VoxelResidencyTransaction(session, operations, slots)`.
- `VoxelResidencyOperation` loses `ExpectedContentHash`.
- Replace background preparation with `ApplyResidency`.

**History and leases**
- Undo: apply the inverse edits.
- Saves: persist the product's own voxel data (CraftSurvive already keeps an
  overlay).
- Drop `AcquireChunkLease`/`ReadChunkLease` and the lease `Dispose`.

**rusty-craftsurvive**
- **`TerrainWorld`:**
  - `ApplyRequest` drops the edit revision and its `StaleRevision` case;
  - `Synchronize` drops the residency revision, policy and hashes;
  - its `leases` map and `AcquireChunkLease` call go.
- **`LiveSubstrateProof`:** its background-preparation and history-policy proof
  steps use `ApplyResidency` instead.

## Review fix: reconstructed surfaces

With Dual Contouring, clearing a voxel two cells inside a chunk changed the
neighbouring chunk's mesh, but only the owning chunk was rebuilt. The cause
was in the mesher, not the dirty-chunk rule:
- each chunk samples its whole one-chunk halo;
- `svc-mesh` `dual_contouring` emitted a vertex for every active cell in that
  halo, including cells only neighbouring chunks' quads use.

A chunk's payload therefore changed whenever anything near it changed.

Owned chunk meshing now keeps only the vertices its own quads reference, in
cell order. A quad a chunk owns depends only on cells within one voxel of its
edge, so the existing boundary-cell rule is exact for reconstructed surfaces
too. Standalone and explicit scalar meshing are unchanged. Dual Contouring
chunk payloads no longer carry unused halo vertices, so their content hashes
change.

In `tests/voxel_local_changes.rs`:
- `reconstructed_surface_edits_two_cells_from_a_boundary_match_a_fresh_build`
  runs the review case and its neighbours for both reconstructed modes. Before
  the fix it reproduces the reviewer's hash (13465877434649881627).
- `random_reconstructed_surface_edits_match_a_fresh_build` compares every chunk
  mesh after each of 60 random edits, again for both modes.

## Found, not fixed here

- **Renderer projection still revisits every chunk.** The voxel render
  projection re-walks and re-validates every chunk mesh on each projection
  pass, even when one chunk changed. That is retained graphics work (#8737).
- **Builder and primitive limits.** The fresh-scene solid-voxel cap and the
  primitive expansion limit remain for #8742.
