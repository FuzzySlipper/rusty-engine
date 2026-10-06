# Voxel residency and edit costs

These are CPU scene costs from the
[`voxel_budget`](../rust/crates/engine-spatial/examples/voxel_budget.rs) and
[`voxel_edit_scaling`](../rust/crates/engine-spatial/examples/voxel_edit_scaling.rs)
probes, measured on 2026-10-06. They exclude GPU
and whole-product costs.

## How a change is applied

`Voxel.ApplyEdits` and `Voxel.ApplyResidency` write into the scene's chunks in
place. They then rebuild only:
- the meshes of the changed chunks, plus resident neighbours whose surfaces
  touch the change;
- the colliders of the changed chunks, and of those neighbours when their
  materials are reconstructed surfaces, whose colliders are what they draw.

Each chunk's mesh and collider are built together, on worker threads when a
change rebuilds several chunks, and installed in order before the call
returns. A cube chunk's collider is its collidable voxels merged into boxes.
Unchanged chunks keep their meshes and collider shapes; a bound Dynamics world
keeps those colliders too. Nothing copies, hashes or rebuilds the whole scene.
The authority hash is an order-independent sum maintained per voxel.

A failed edit changes nothing:
- invalid coordinates, material slots or states are refused before any write;
- if a touched chunk's mesh cannot be built, the written voxels are reverted.

## Limits

| Boundary | Limit | Owner |
| --- | ---: | --- |
| Voxel coordinate | ±1,000,000 per axis | `engine-spatial/src/voxel_edit.rs` |
| Solid material slot | 1–4,095 | same |
| Chunk edge | 1–64 cells | `engine-spatial/src/lib.rs` |

Edit, residency, scene-building and primitive calls have no count caps.

## Reproduction

```sh
cargo build --release -p engine-spatial --example voxel_budget --example voxel_edit_scaling
target/release/examples/voxel_budget solid 64      # also: solid 16, checker 16, sparse 256, stateful 1
target/release/examples/voxel_edit_scaling 64      # also: 16
```

Voxel size 1, 16³ chunks, GreedyCubes. Chunks are separated by one empty chunk
along X so every surface stays exposed:
- `solid` fills each chunk;
- `checker` fills cells with even x+y+z;
- `sparse` places one cell per chunk;
- `stateful` fills one chunk with four orientations and four variants.

Each run builds the scene, then:
- toggles one corner cell seven times through `VoxelEditService`;
- replaces that chunk seven times through `VoxelChunkResidencyService`.

The host is an Intel Core i7-12700K shared with other sessions (load about 10),
running a release build, one process at a time, with no CPU pinning; each
figure is the median of three runs.

## Observed costs

Milliseconds, as median / maximum of seven calls.

| Shape / chunks | Cell edit | Chunk replace | Peak RSS |
| --- | --- | --- | --- |
| Solid / 16 | 0.24 / 0.30 | 0.40 / 0.43 | 8 MiB |
| Solid / 64 | 0.25 / 0.31 | 0.40 / 0.42 | 25 MiB |
| Checker / 16 | 2.56 / 3.58 | 2.24 / 2.45 | 62 MiB |
| Sparse / 256 | 0.05 / 0.10 | 0.10 / 0.13 | 11 MiB |
| Stateful / 1 | 0.47 / 0.73 | 0.64 / 0.67 | 4 MiB |

Cost follows the chunk that changed, not the resident world. What remains is
that chunk's work: a solid 16³ chunk's mesh and collider, or a checker chunk's
12,288-quad mesh.

`voxel_edit_scaling` clears 1–123 cells inside one chunk of a 16- or 64-chunk
world. Each call takes 0.12–0.15 ms with 16 resident chunks or with 64. The
number of cells changed inside one chunk barely matters.

## Memory

`size_of::<VoxelValue>()` is 6 bytes: material, solid tag and 15 state bits.
A 16³ dense value array is therefore 24,576 bytes before its container.
Optional C# state input adds 16,384 bytes per 16³ chunk during admission.

Mesh cost depends on the surface, not the solid count. A solid 16³ chunk makes
six quads; a checker chunk makes 12,288 (about 1.87 MB of vertex capacity). The
stateful sample makes 1,536 quads, because orientation and variant diversity
stops greedy merging.

Peak RSS above is Engine probe memory only. It excludes the product's own chunk
data, renderer realizations and GPU memory; measure those separately.
