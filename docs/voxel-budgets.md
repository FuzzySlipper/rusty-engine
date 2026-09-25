# Voxel residency and remesh budgets

Measured for task #8612 on 2026-09-25 against Engine implementation
`2d3b97f7b9687afe003423efbdecf26595a17899`, using the added
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
same chunk seven times through the residency service. Payload construction is
outside the replacement timer. Each timed call includes synchronous candidate
construction, collision/navigation rebuilding, mesh updates and commit.

## Observed costs

Times are milliseconds. Edit and replace show median / maximum of seven calls.
Mesh memory is the initial mesh vector **capacity bytes**, including positions,
normals, tile coordinates, indices and material groups; it excludes enclosing
objects, allocator overhead and all other state. RSS is Linux process peak
across construction and both transaction series, including simultaneous old
and candidate scenes, not steady-state bytes per chunk.

| Shape / chunks | Solid cells | Build ms | Cell edit ms | Chunk replace ms | Mesh capacity bytes | Peak RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid / 1 | 4,096 | 3.54 | 5.71 / 5.90 | 2.31 / 2.58 | 1,008 | 10,212 |
| Solid / 16 | 65,536 | 56.04 | 71.22 / 90.66 | 25.60 / 26.78 | 16,128 | 81,696 |
| Solid / 64 | 262,144 | 237.75 | 279.50 / 363.96 | 96.40 / 101.25 | 64,512 | 314,648 |
| Checker / 1 | 2,048 | 9.53 | 10.67 / 15.62 | 6.16 / 7.99 | 1,867,872 | 12,948 |
| Checker / 16 | 32,768 | 112.47 | 112.43 / 148.83 | 22.65 / 25.53 | 29,885,952 | 133,616 |
| Sparse / 256 | 256 | 220.09 | 368.02 / 428.50 | 188.10 / 196.96 | 258,048 | 136,828 |

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

For initial sizing, use **one transaction at most per admitted update**, a
**one-chunk mutation batch**, and a separately measured **8 ms CPU admission
allowance**. These are recommended starting knobs, not new encoded limits.
Only the one-solid-chunk case fits that allowance for both measured mutation
paths here; the one-checker replacement nearly consumes it, and its cell edit
exceeds it. Even the 16-solid-chunk case exceeds a 16.67 ms frame on replacement.
Do not advertise 60 Hz streaming at that working set based on the 4,096-chunk
admission ceiling. A wall-clock budget cannot interrupt an already-started
synchronous call. Reduce the active set, admit during loading, accept explicit
hitches, or request an Engine incremental/overlapped projection capability.

Reserve memory for the **old and candidate scene together**, plus transport
and renderer realizations. For these measured scenarios, a **512 MiB CPU
probe-process envelope** covers the largest observed peak (307.3 MiB), but it
is not a 512 MiB guarantee for 64 arbitrary chunks or a total-game budget.
For visible fragmented geometry, separately budget at least the measured
mesh capacities, then measure actual retained renderer and GPU memory. No
universal mesh-memory cap is enforced here. Set a product working-set cap from
its actual content and hardware measurements; do not extrapolate linearly
from smooth solid cubes or allocate until the hard limit rejects.

What degrades first: synchronous update latency and input responsiveness for
large resident sets; mesh generation, transfer/GPU upload and memory for
fragmented surfaces; memory peaks during replacement. Eventually explicit
admission limits reject a transaction. Before claiming a product view radius,
measure its whole admission-to-visible path and track `ResidentChunkCount`,
`SolidVoxelCount`, `DirtyChunkCount`, `RebuiltMeshChunks`, `ReusedMeshChunks`,
`RemovedMeshChunks`, residency receipts and `ColliderChunkCount`. A 100 km²
world on disk says nothing about how much can be resident or remeshed per tick.
