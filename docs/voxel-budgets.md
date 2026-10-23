# Voxel residency and edit costs

How voxel changes apply, their limits, and how to measure their CPU cost
with the
[`voxel_budget`](../rust/crates/engine-spatial/examples/voxel_budget.rs) and
[`voxel_edit_scaling`](../rust/crates/engine-spatial/examples/voxel_edit_scaling.rs)
probes. GPU and whole-product costs are measured separately.

## How a change is applied

`Voxel.ApplyEdits` and `Voxel.ApplyResidency` write into the scene's chunks in
place. They then rebuild only:
- the meshes of the changed chunks, plus resident neighbours whose surfaces
  touch the change;
- the colliders of the changed chunks, and of those neighbours when their
  materials are reconstructed surfaces, whose colliders are what they draw.

Each chunk's mesh and collider are built together, on worker threads when a
change rebuilds several chunks, and installed in order before the call
returns. A deferred residency transaction builds them on a worker thread
instead, from copies of the chunks they read, and installs them at the start
of a call once built ([residency and edits](world-streaming-contract.md#residency-and-edits-apply-in-place)). A cube chunk's collider is its collidable voxels merged into boxes.
Unchanged chunks keep their meshes and collider shapes; a bound Dynamics world
keeps those colliders too. Nothing copies, hashes or rebuilds the whole scene.
The authority hash is an order-independent sum maintained per voxel.

A failed edit changes nothing:
- invalid coordinates, material slots or states are refused before any write;
- if a touched chunk's mesh cannot be built, the written voxels are reverted.

## Limits

| Boundary | Limit | Owner |
| --- | ---: | --- |
| Voxel coordinate | ±(2²⁴ − 1) per axis | `core-space` `MAX_VOXEL_COORDINATE_ABS` |
| Solid material slot | 1–65,535 (0 is empty) | `engine-spatial/src/voxel_edit.rs` |
| Chunk edge | 1–64 cells | `engine-spatial/src/lib.rs` |

The coordinate bound is where voxel surface tile coordinates, carried in `f32`,
stop being exact: the far corner of the outermost voxel is 2²⁴. Scenes, edits,
residency, voxel assets and voxel object frames all refuse a coordinate past it
when it enters, so meshing never meets one.

Edit, residency, scene-building and primitive calls have no count caps.

## Measuring

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

Each prints the median and maximum milliseconds of the seven calls and the
process's peak RSS. Compare runs on the same machine and build. A call's cost
follows the chunk that changed, not the resident world or the number of cells
changed inside it: a solid 16³ chunk's mesh and collider, or a checker chunk's
12,288-quad mesh.

`voxel_edit_scaling` clears 1–123 cells inside one chunk of a 16- or 64-chunk
world.

## Memory

`size_of::<VoxelValue>()` is 6 bytes: material, solid tag and 15 state bits.
A 16³ dense value array is therefore 24,576 bytes before its container.
Optional C# state input adds 16,384 bytes per 16³ chunk during admission.

Mesh cost depends on the surface, not the solid count. A solid 16³ chunk makes
six quads; a checker chunk makes 12,288 (about 1.87 MB of vertex capacity). The
stateful sample makes 1,536 quads, because orientation and variant diversity
stops greedy merging.

The probes' peak RSS is Engine probe memory only. It excludes the product's own chunk
data, renderer realizations and GPU memory; measure those separately.
