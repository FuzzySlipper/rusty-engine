# Voxel residency and remesh budgets

Measured for task #8612 on 2026-09-25 against the per-cell-state and background
preparation implementation accompanying this report. Raw evidence records source
file hashes and the prior base revision. Measurements use the
[`voxel_budget` probe](../rust/crates/engine-spatial/examples/voxel_budget.rs).
These are CPU scene costs, not a browser/GPU or whole-product certification.

## Existing admission limits

| Boundary | Limit | Owner |
| --- | ---: | --- |
| Residency transaction | 64 operations | `engine-spatial/src/voxel_residency.rs` |
| Resident chunks after a residency transaction | 4,096 | same |
| Residency chunk leases | 4,096 | same |
| Payload slots in one residency transaction | 16,777,216 | same (64 × 64³) |
| Solid cells in a scene | 1,000,000 | `engine-spatial/src/lib.rs` |
| Cell edits per transaction | 4,096 | `engine-spatial/src/voxel_edit.rs` |
| Chunk edge | 1–64 cells | `engine-spatial/src/lib.rs` |
| Solid material slot | 1–4,095 | `engine-spatial/src/voxel_edit.rs` |

These limits reject oversized operations; they are not throughput targets,
mesh byte budgets or a scheduler. In particular, 4,096 resident chunks does
not mean 4,096 **filled** chunks: at edge 16 the solid-cell limit permits only
244 completely filled chunks (999,424 cells), plus a partial chunk.
The resident-count check belongs to the residency transaction; do not infer
that every alternative raw constructor has the same count admission gate.

There is **no global default chunk edge**: `SpatialSessionConfig` requires
`CollisionChunkSize`, and the storage owner explicitly supports different
chunk dimensions. The measurements below choose the common **16³** fixture
size, voxel size 1 and **GreedyCubes** (the default extraction mode).
At this edge, one dense C# `uint` payload is 16,384 bytes during the call;
that is not total retained Engine memory. Chunk storage, canonical cell
lists, collision/navigation projections and meshes are additional.

## Reproduction and conditions

```sh
cargo build --release -p engine-spatial --example voxel_budget
# Run each case as its own process so Linux VmHWM is independent.
target/release/examples/voxel_budget solid 1
target/release/examples/voxel_budget solid 16
target/release/examples/voxel_budget solid 64
target/release/examples/voxel_budget checker 1
target/release/examples/voxel_budget checker 16
target/release/examples/voxel_budget sparse 256
target/release/examples/voxel_budget stateful 1
```

Host: AMD Ryzen 7 8845HS, 8 cores/16 logical CPUs, Linux 7.2.2-arch1-1,
x86-64, rustc 1.98.0, Cargo release profile (optimized with debug info).
One process at a time, ordinary shared development host, no CPU pinning or
frequency control. No C# host, transport serialization, retained render
projection, GPU, shadows, product chunk cache, or gameplay workload is included.
This is a bounded sample, not a statistically established latency percentile.
[Raw observations](evidence/voxel-budgets/linux-x64-2026-09-25.json) are retained.

Chunks are separated by one empty chunk along X, so all six surfaces remain
exposed and neighbours cannot merge them. `solid` fills each chunk with one
material; `checker` fills cells with even x+y+z (isolated alternating solids);
`sparse` places one cell per chunk. Each process constructs a scene, toggles
one corner cell seven times through the cell-edit service, then replaces that
same chunk seven times through synchronous residency, then seven times through
background preparation and guarded commit. `stateful` fills a chunk with four
orientations and four variants, retaining that pattern during replacements.
Payload construction is
outside the replacement timer. Synchronous calls include candidate construction, collision/navigation rebuilding,
mesh updates and commit. Background start copies/moves already-owned Rust inputs
and starts a worker; preparation time includes that start and result polling.
Commit includes guards, swapping the scene Arc, releasing the old scene and
joining the finished worker. The callback can read the old scene while preparing.
The generated C# start additionally copies its input spans; that copy is not in
this Rust-only start timer.

## Observed costs

Times are milliseconds. Edit and replace show median / maximum of seven calls.
Mesh memory is the initial mesh vector **capacity bytes**, including positions,
normals, tile coordinates, indices and material groups; it excludes enclosing
objects, allocator overhead and all other state. RSS is Linux process peak
across construction and all mutation series, including simultaneous old
and candidate scenes, not steady-state bytes per chunk.

| Shape / chunks | Solid cells | Build ms | Cell edit ms | Chunk replace ms | Mesh capacity bytes | Peak RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid / 1 | 4,096 | 3.52 | 4.15 / 5.31 | 2.02 / 2.19 | 1,040 | 12,912 |
| Solid / 16 | 65,536 | 48.76 | 55.30 / 70.46 | 20.97 / 23.19 | 16,640 | 111,500 |
| Solid / 64 | 262,144 | 189.62 | 234.18 / 304.01 | 84.74 / 90.30 | 66,560 | 423,580 |
| Checker / 1 | 2,048 | 6.35 | 9.33 / 11.72 | 4.69 / 4.98 | 1,867,904 | 17,076 |
| Checker / 16 | 32,768 | 93.80 | 96.24 / 128.37 | 20.58 / 21.00 | 29,886,464 | 219,020 |
| Sparse / 256 | 256 | 209.11 | 312.98 / 360.86 | 165.71 / 169.23 | 266,240 | 250,952 |
| Stateful / 1 | 4,096 | 3.41 | 4.29 / 5.76 | 2.14 / 2.19 | 235,520 | 13,364 |

| Shape / chunks | Worker start median ms | Preparation median ms | Commit median / max ms |
| --- | ---: | ---: | ---: |
| Solid / 1 | 0.024 | 2.04 | 0.07 / 0.19 |
| Solid / 16 | 0.041 | 25.48 | 1.07 / 1.15 |
| Solid / 64 | 0.048 | 80.85 | 4.69 / 5.28 |
| Checker / 1 | 0.015 | 5.81 | 0.06 / 0.17 |
| Checker / 16 | 0.034 | 20.73 | 0.83 / 2.80 |
| Sparse / 256 | 0.042 | 157.15 | 7.28 / 7.66 |
| Stateful / 1 | 0.014 | 2.89 | 0.07 / 0.19 |

The solid chunk produces six quads; a checker chunk produces 12,288.
A 16³ checker chunk consequently has about 1.87 MB of mesh vector capacity
before any renderer copies. Solidity count alone does not predict mesh cost.
Sparse scenes still carry dense chunk storage and resident-volume traversal.
The measured RSS is Engine probe process memory; it does not include a
product's own chunk payload/cache and must not be used as their estimate.

## Practical scheduling envelope

There is no implemented per-tick remesh quota or partial asynchronous commit.
`RebuiltMeshChunks` counts changed mesh extraction, not total work: the scene
owner still scans canonical residency and rebuilds collision/navigation, and
reused mesh payloads also have costs. One changed mesh in a large working set
can exceed a whole frame. See `build_from_voxel_world_at_revision` in
[the scene owner](../rust/crates/engine-spatial/src/lib.rs).

For initial sizing, keep **one in-flight preparation per session** (also the
implemented ownership limit), prepare **one chunk per request**, and reserve a
separately measured **8 ms callback publication allowance**. These are starting
knobs, not a universal 60 Hz guarantee. Synchronous whole-scene reconstruction
still exceeds a 16.67 ms frame for larger working sets. Background preparation
moves that work off the callback; the observed commit column measures what
remains on it. Hash checks and releasing old allocations still scale with the
resident set. Presentation reconciliation, transport and GPU upload remain
additional and require product measurements.

State has a concrete storage cost: `size_of::<VoxelValue>()` is **6 bytes** on this
build (previous material-only layout: 4); `MaterialVoxel` remains **32 bytes**
with alignment. Stable encoded cells remain four bytes. A 16³ dense value array
therefore holds 24,576 bytes before its container, versus 16,384 previously.
Optional C# state input adds 16,384 bytes per 16³ chunk during admission.
The stateful solid sample produces 1,536 quads rather than six: orientation and
variant diversity can substantially increase mesh memory even without holes.

Reserve the **old and candidate scene together**, copied request data and renderer
realizations. The measured RSS peaks include both scenes and worker allocation;
they are not steady-state bytes per chunk. Use a measured CPU working-set cap
with headroom, then measure actual retained renderer and GPU memory separately.
No universal mesh-memory cap is imposed here. Do not extrapolate from smooth
solid cubes or allocate until the hard admission limit rejects a request.

What degrades first: synchronous callback/input latency without preparation;
preparation latency and memory with large working sets; mesh generation,
transfer/upload and memory with fragmented or diverse-state surfaces. Eventually
explicit admission limits reject the transaction. Track `ResidentChunkCount`,
`SolidVoxelCount`, `DirtyChunkCount`, `RebuiltMeshChunks`, `ReusedMeshChunks`,
`RemovedMeshChunks`, residency receipts and `ColliderChunkCount`, plus the separate
start/prepare/commit/presentation timings. A 100 km² world on disk does not specify
how much can be resident or remeshed per tick.
