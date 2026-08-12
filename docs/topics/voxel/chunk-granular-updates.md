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
stream, monotonic projector revision, and exact operation count. The retained
TypeScript projection and Three backend stage the whole frame, reject clipped
operation counts and stale/duplicate revisions, then commit once. Replaced and
destroyed Three geometry follows the existing exact disposal path. Unstamped
legacy/general render frames remain compatible and retain their prior ordering
contract.

Callers that compose more than one voxel projector into one renderer should
construct each projector with a distinct stable publication stream. The stream
orders presentation only; it never authorizes voxel edits or becomes world
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

## Representative measurement

Run:

```bash
./scripts/measure-voxel-chunk-updates.sh
```

The release probe uses a deterministic 64 by 64 varied-height terrain split
into sixteen 16-cube chunks. `elapsed_us` covers full scene rebuild plus
projection for the baseline and edit apply plus projection for the incremental
path. `encoded_bytes` and operation counts measure the renderer-neutral JSON
control/payload frame. One run on the development host produced:

| Mode | Edit | Whole us / bytes / replacements | Incremental us / bytes / replacements | Rebuilt / reused |
|---|---|---:|---:|---:|
| greedyCubes | one cell | 23,451 / 1,482,582 / 16 | 19,149 / 275,170 / 3 | 3 / 13 |
| greedyCubes | 4x4x4 | 19,101 / 1,482,587 / 16 | 18,378 / 273,115 / 3 | 3 / 13 |
| marchingCubes | one cell | 51,229 / 5,616,559 / 16 | 29,610 / 661,589 / 2 | 4 / 12 |
| marchingCubes | 4x4x4 | 52,005 / 5,616,564 / 16 | 25,708 / 988,448 / 3 | 4 / 12 |
| dualContouring | one cell | 89,281 / 7,080,890 / 16 | 40,292 / 2,437,053 / 4 | 4 / 12 |
| dualContouring | 4x4x4 | 91,358 / 7,080,895 / 16 | 44,008 / 2,435,970 / 4 | 4 / 12 |

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
