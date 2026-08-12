# Chunk-granular voxel mesh updates

Status: implemented provider contract for task 6848.

## Decision

`VoxelCollisionScene` retains one stable signed `[x, y, z]` identity per
resident mesh chunk. A prepared edit derives the exact candidate set whose
visible surface may change, meshes only those resident candidates, reuses every
other immutable `VoxelMeshChunk`, and publishes the complete candidate scene
only after collision, navigation, and all candidate meshes succeed.

The dirty set is derived in canonical voxel coordinates with Euclidean chunk
addressing. An interior edit dirties its owner. An edit on a face, edge, or
corner also dirties every resident old/new chunk touching that boundary. This
superset is intentional for reconstructed surfaces: Marching Cubes and Dual
Contouring sample the resident halo and retain their existing deterministic
primitive-owner rules. Removed coordinates remain in the update receipt so a
retained handle can be destroyed.

`VoxelMeshChunk::content_hash` fingerprints the complete derived payload,
including surface mode, positions, normals, optional tile coordinates, indices,
groups, and bounds. It is deliberately distinct from `source_chunk_hash`.
Neighbor occupancy can change a chunk's seam geometry without changing that
chunk's own voxels, so the canonical chunk hash is provenance rather than a
safe render replacement key.

## Retained publication

`VoxelRenderProjector` keeps stable handles for `(instance, signed chunk)`.
At each accepted projection it emits only:

- create plus payload for a newly visible chunk;
- one whole-chunk `replaceMeshPayload` when the derived payload hash changes;
- destroy for a removed chunk; and
- no operation for an unchanged chunk.

Whole-chunk replacement is the selected granularity. The representative
measurements below do not justify buffer-subrange patching.

Voxel frames carry an optional renderer-neutral publication stamp with a stable
stream, exact base and next projector revisions, and operation count. The retained
TypeScript projection and Three backend stage the whole frame, reject clipped
operation counts, dropped revisions, and stale/duplicate revisions, then commit once. Replaced and
destroyed Three geometry follows the existing exact disposal path. Unstamped
legacy/general render frames remain compatible and retain their prior ordering
contract.

A renderer projection should consume one `VoxelRenderProjector`. Distinct
projectors currently allocate from the same voxel handle namespace, so a stable
publication stream does not make their handles composable. One caller-owned
projector may project multiple voxel instances; a future caller-unique handle
namespace would be required before independent projectors can share a retained
renderer. Publication ordering never authorizes voxel edits or becomes world
authority.

## Atomicity and ownership

```text
edit intent + expected source revision
                |
                v
validate and derive signed dirty chunks
                |
                v
build candidate canonical world + collision + navigation
                |
                v
rebuild dirty resident meshes; reuse unrelated chunks
                |
      failure --+--> discard candidate; live revision unchanged
                |
                v
commit one coherent spatial revision
                |
                v
project stable create/replace/destroy handles
                |
                v
stage stamped retained frame --> reject clipped/stale --> apply + dispose once
```

Canonical voxels, collision, navigation, mesh chunks, source revision, and
mesh-update receipt are built off to the side. Any validation, meshing,
collision, or navigation failure leaves the live scene and retained projector
on their prior coherent revisions. Renderer application is a separate
fail-atomic projection transaction and cannot feed changes back into spatial
authority.

Engine owns chunk identity, dirty-neighbor derivation, candidate meshing,
stable handles, frame ordering, replacement, and disposal. Downstream owns
world meaning, edit scheduling, materials, and residency policy. No browser,
transport, generation, persistence, or streaming policy enters the Rust
spatial contract.

Explicit admit/replace/evict transactions, leases, and edit-history interaction
are defined separately in [canonical voxel chunk residency](chunk-residency.md).

## Representative measurement

Run:

```bash
./scripts/measure-voxel-chunk-updates.sh
```

The release probe uses a deterministic 64 by 64 varied-height terrain split
into sixteen 16-cube chunks. Both paths build the same post-edit authority:
`elapsed_us` covers whole post-edit scene rebuild plus projection for the
baseline and edit apply plus projection for the incremental path.
`encoded_bytes` and operation counts measure the renderer-neutral JSON
control/payload frame. One run on the development host produced:

| Mode | Edit | Whole us / bytes / replacements | Incremental us / bytes / replacements | Rebuilt / reused |
|---|---|---:|---:|---:|
| greedyCubes | one cell | 25,522 / 1,483,643 / 16 | 21,250 / 275,187 / 3 | 3 / 13 |
| greedyCubes | 4x4x4 | 20,897 / 1,481,588 / 16 | 16,010 / 273,132 / 3 | 3 / 13 |
| marchingCubes | one cell | 51,451 / 5,618,487 / 16 | 31,082 / 661,606 / 2 | 4 / 12 |
| marchingCubes | 4x4x4 | 51,082 / 5,615,530 / 16 | 26,940 / 988,465 / 3 | 4 / 12 |
| dualContouring | one cell | 111,562 / 7,082,607 / 16 | 55,487 / 2,437,070 / 4 | 4 / 12 |
| dualContouring | 4x4x4 | 103,031 / 7,079,820 / 16 | 42,852 / 2,435,987 / 4 | 4 / 12 |

The same script decodes and applies each update through the public TypeScript
retained projection and `ThreeRenderer`. It reports the median of seven fresh
applications after each base frame is already resident:

| Mode | Edit | Retained whole / incremental us | Three whole / incremental us |
|---|---|---:|---:|
| greedyCubes | one cell | 18,135 / 3,055 | 27,263 / 5,050 |
| greedyCubes | 4x4x4 | 19,511 / 3,084 | 29,633 / 4,966 |
| marchingCubes | one cell | 113,709 / 12,984 | 176,681 / 19,994 |
| marchingCubes | 4x4x4 | 107,637 / 19,413 | 171,549 / 29,314 |
| dualContouring | one cell | 121,799 / 39,427 | 195,226 / 64,246 |
| dualContouring | 4x4x4 | 146,524 / 41,673 | 216,477 / 67,934 |

Timings are characterization rather than a hardware-independent budget. The
stable result is the bounded application work: unrelated chunks preserve
object and geometry identity, while encoded update size falls by roughly 66%
to 88% on this deliberately corner-touching fixture.

## Verification

```bash
cargo test -p engine-spatial -p render-model -p render-projection --locked
cargo clippy -p engine-spatial -p render-model -p render-projection --all-targets --locked -- -D warnings
pnpm --dir render --filter @rusty-engine/render-projection test
pnpm --dir render --filter @rusty-engine/renderer-three test
./scripts/measure-voxel-chunk-updates.sh
./scripts/verify-voxel-chunk-consumer.sh /absolute/path/to/rusty-craftsurvive
./scripts/verify-render.sh
```

The explicit consumer gate validates the adjacent complete facade and invokes
CraftSurvive's own non-synthetic load/edit performance probe. The consumer is
selected by the operator and is never an Engine dependency.
