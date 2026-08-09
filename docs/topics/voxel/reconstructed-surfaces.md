# Reconstructed voxel surface modes

Status: implemented experimental presentation decision for task 6599.

## Decision

`svc-mesh` exposes three explicit derived surface modes:

- `greedyCubes` is the default and retains the existing exact visible-cube,
  greedy-rectangle, tile-coordinate, and packed-V2 behavior;
- `marchingCubes` is an optional table-free isosurface path with deterministic
  face ambiguity and interior-loop rules; and
- `dualContouring` is an independent optional Hermite/QEF path with one vertex
  per active scalar cell.

Omitting the option still selects `greedyCubes`. A surface mode is disposable
presentation configuration: it does not change canonical sparse voxel-object
bytes, content hashes, collision, navigation, editing, annotations, animation
frame selection, or gameplay queries. The first implementation is intentionally
experimental and does not change the default.

## Scalar field

The reconstructed modes share one deliberately explicit binary field:

1. authored voxel coordinate `[x,y,z]` is sampled at physical point
   `[x+0.5,y+0.5,z+0.5]`;
2. occupied samples have scalar `+0.5`, empty samples have `-0.5`, and the
   isovalue is zero;
3. the occupied coordinate bounds receive one empty sample of padding on every
   side;
4. crossings use linear interpolation, so every binary edge crosses at its
   midpoint, on the canonical voxel face; and
5. material identity does not alter or smooth the scalar field.

There is no smoothing pass. The rounded/chamfered visual character comes from
reconstructing between voxel-center samples and interpolating normals, not from
mutating or blurring occupancy. Sparse inputs with a very large empty bounding
box can therefore hit the sampled-cell or temporary-field quota even when they
contain few occupied voxels.

## Marching Cubes policy

The implementation follows the cell traversal, edge interpolation, and scalar
gradient principles of Lorensen and Cline's
[Marching Cubes paper](https://doi.org/10.1145/37402.37422), but it does not copy
their lookup table. Each active cell builds a contour graph over its 12 edges
and six faces.

A face with two crossings connects them directly. A four-crossing face uses the
bilinear determinant from the
[asymptotic decider](https://doi.org/10.1109/VISUAL.1991.175782). Binary diagonal
saddles are exact determinant ties, so the implementation breaks that tie from
the global face coordinate and axis. Adjacent cells and independently meshed
chunks therefore make the same choice. The interior rule is also explicit:
each resulting closed edge loop stays independent and is triangulated to its
own centroid. Loops are never joined merely because they occur in one cell.

Normals are the normalized negative trilinear gradient, with a finite outward
geometric fallback. Polygon order is reversed when its geometric normal
disagrees with the averaged vertex normal. The implementation covers all 256
binary configurations in tests and assigns chunk output by a deterministic
occupied-corner owner, using a one-chunk resident halo at world seams.

This is an Engine-specific binary-field policy, not a claim of topology
equivalence for arbitrary sampled continuous fields. A future non-binary field
or stronger interior-topology guarantee requires a separately reviewed rule and
fixtures.

## Dual Contouring policy

The independent path follows the Hermite sample, dual vertex, and quadratic
error function principles in Ju, Losasso, Schaefer, and Warren's
[Dual Contouring paper](https://doi.org/10.1145/566570.566586) and the authors'
[QEF implementation note](https://www.cs.rice.edu/~jwarren/papers/techreport02408.pdf).
No source code or QEF implementation was copied.

Every sign-changing scalar edge supplies its midpoint and normalized trilinear
gradient. One active cell vertex minimizes the planes defined by those Hermite
samples. Engine solves the symmetric `A^T A` system with a fixed 16-sweep
Jacobi eigendecomposition and a relative `1e-8`/absolute `1e-12` rank threshold.
The solve is translated around the sample mass point for numerical stability.
Zero-rank or non-finite results use that mass point; every result is clamped to
the active cell. Readout counts rank-deficient solves and mass-point fallbacks.

Each crossing scalar edge connects its four incident dual vertices. Winding is
checked against their averaged normal, and the shorter diagonal wins with a
stable equality choice. Tests cover flat, singular, coincident, tiny, and poorly
conditioned samples; all outcomes are finite, bounded, and repeated-run equal.

## Materials and textures

Reconstruction preserves material slots but not hard voxel-face material
boundaries:

- Marching Cubes assigns each cell loop to the most frequent occupied corner
  slot, with the lower slot winning a tie.
- Dual Contouring assigns each crossing-edge quad to its occupied endpoint's
  slot.
- Output groups remain ordered by ascending material slot.

These rules keep identity deterministic and visible, but a boundary can move to
the reconstructed surface and small islands can lose visual area. They are not
multi-material interface reconstruction.

Greedy cubes retain the exact cell-space tile coordinates used by repeating and
atlas-region materials. Reconstructed modes emit no UV stream. Projection
rejects a textured reconstructed material before projector state changes with
`TexturedSurfaceUnsupported`; it never silently removes the texture or falls
back to color. The real comparison binds the checked 16x8 directional atlas to
greedy output through the normal content-addressed texture resource path and
shows explicit unsupported cards for the other two modes.

## Bounds and lifecycle

`SurfaceMeshLimits` checks source faces, sampled cells, generated vertices,
generated indices, temporary scalar/QEF storage, and material partitions before
the next excessive growth. `VoxelObjectRuntimeLimits` applies the corresponding
aggregate limits across unique animation meshes. `render-model` additionally
rejects any packed resource above 64 MiB and any returned set above 256 MiB
before output resource allocation.

`VoxelObjectAdmissionOptions` selects the mode. An admitted object's canonical
asset ID and content hash remain unchanged, while its readout names the derived
mode. `VoxelObjectRenderProjector` includes that mode in resource-cache and
instance snapshots, so changing mode at the same canonical identity defines
the replacement geometry and selects its frame. Three backend tests prove
replacement, release, reopen, and final disposal return exact geometry and
material resource counts to zero.

## Real corpus comparison

The checked comparison used `rusty-engine-voxels` commit
`6f3a5491b18d91ff33372ef3b112e759dde85bef` and this command:

```bash
./scripts/verify-voxel-surface-comparison.sh \
  /home/dev/rusty-engine-voxels \
  /home/dev/rusty-engine/target/task-6599-surface-evidence
```

The command emits `metrics.json`, `browser-metrics.json`, 24 real Chromium
cards, and a contact sheet. The final contact sheet SHA-256 was
`5554d03c73ec8f7b4775d2fec411f016a9c97ca7f6209c152cd7b54f67028dc0`;
`metrics.json` was
`e786e01e302cd3bb5e0373258827b63d90f297b803df6f330fa0f6990575de1b`.
The 22 supported cards replaced their complete frame in 5.1–58.0 ms (22.9 ms
mean) in that Chromium run and retained the expected one mesh resource, plus
one texture resource for the atlas card. Times below are observational from
one unoptimized local run, not gates.

| Checked default frame | Mode | Voxels | Vertices | Triangles | Material groups | Packed bytes | Build ms |
|---|---|---:|---:|---:|---:|---:|---:|
| normal character | greedy | 592 | 988 | 494 | 1 | 780,688 | 80 |
| normal character | marching | 592 | 5,196 | 4,152 | 1 | 2,529,376 | 189 |
| normal character | dual | 592 | 1,044 | 2,076 | 1 | 723,184 | 223 |
| high fidelity | greedy | 10,439 | 21,436 | 10,718 | 1 | 14,836,280 | 1,825 |
| high fidelity | marching | 10,439 | 102,092 | 81,672 | 1 | 48,358,624 | 4,800 |
| high fidelity | dual | 10,439 | 20,416 | 40,836 | 1 | 13,813,240 | 6,406 |
| knight walk | greedy | 48,089 | 123,524 | 61,762 | 6 | 17,663,328 | 2,459 |
| knight walk | marching | 48,089 | 352,774 | 282,328 | 6 | 46,276,384 | 6,096 |
| knight walk | dual | 48,089 | 69,762 | 141,164 | 6 | 13,156,168 | 8,206 |
| hard surface | greedy | 51,877 | 8,896 | 4,448 | 3 | 338,064 | 520 |
| hard surface | marching | 51,877 | 78,462 | 62,768 | 3 | 2,636,320 | 681 |
| hard surface | dual | 51,877 | 15,693 | 31,384 | 3 | 753,256 | 756 |
| atlas sentinel | greedy | 23,832 | 1,908 | 954 | 3 | 476,232 | 1,274 |

The complete animated comparison contains the default plus three walk frames.
All three modes preserve the knight, staff/bow, thin legs, and six material
groups across those frames. Marching and dual soften the per-voxel stairsteps
without erasing the pose changes. Dual's bounds move inward by a few
thousandths on some animated axes because its QEF vertex may sit inside the
cell; canonical frame facts do not change.

The normal and high-fidelity characters retain their overall silhouette in all
three modes. Greedy best preserves intentional block corners. Marching creates
the most visibly rounded joints and end caps but also the largest stream. Dual
has a similar softened silhouette with substantially fewer vertices than
Marching in this corpus, though still more triangles than greedy.

The assembled hard-surface model makes the aesthetic cost clearest: greedy
keeps crisp plate steps, while both reconstructions chamfer the outside contour.
All three material regions remain recognizable, but none of the numbers alone
establish better art. The atlas card proves the supported greedy mapping and
resource lifecycle; reconstructed modes are rejected because stable repeat and
atlas UV projection has not been defined.

Rank-deficient QEF cells are common on this binary field (756 normal, 14,006
high fidelity, 34,356 knight default, and 14,141 hard surface), but no corpus
cell required the mass-point fallback. That is an expected observation of flat
and edge-like Hermite sets, not evidence of failure. Focused adversarial tests
exercise the fallback path directly.

## Provenance and licensing

The implementation was written in this repository from the cited algorithmic
principles. It contains no borrowed Marching Cubes case table, Dual Contouring
source, QEF source, or framework. The cited papers remain under their
publishers' terms; links and mathematical ideas are referenced, not copied.
The comparison atlas and voxel models remain in the checked downstream corpus
with their existing provenance.

## Verification

```bash
cargo test -p svc-mesh -p voxel-object-runtime -p render-projection --locked
cargo clippy -p svc-mesh -p voxel-object-runtime -p render-projection --all-targets --locked -- -D warnings
pnpm --dir render --filter @rusty-engine/renderer-three test
./scripts/verify-render.sh
./scripts/verify-studio.sh
./scripts/verify-voxel-surface-comparison.sh /home/dev/rusty-engine-voxels
```

Synthetic tests prove mechanisms and failure paths. The explicit external gate
is the evidence for the current real corpus and browser-owned rendering only;
it does not make the consumer checkout or Chromium an ordinary Engine
dependency.
