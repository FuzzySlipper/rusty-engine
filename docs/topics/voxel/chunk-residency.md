# Canonical voxel chunk residency

Status: implemented provider contract for task 6851.

## Decision

`VoxelChunkResidencyService` is the explicit mechanism for admitting,
replacing, and evicting complete chunks in `VoxelCollisionScene`. It is not a
world streamer: downstream games decide what to load, how far around a player
to retain it, where payload bytes come from, and when memory pressure warrants
eviction.

Each operation names one stable signed `VoxelChunkIdentity`. Admission requires
absence, replacement and eviction require the exact current
`VoxelChunkContentHash`, and every transaction requires the exact observed
scene source revision. Dense payload dimensions must match the scene chunk
size. Coordinates, material slots, operation count, aggregate payload cells,
resident chunk count, and resident solid cells are bounded before publication.
Empty chunks remain explicitly resident even though they produce no mesh.

## Coherent publication

```text
downstream residency decision + complete payloads + expected revision
                              |
                              v
validate all operations, hashes, bounds, limits, and active leases
                              |
                              v
build candidate canonical voxels + collision + navigation + mesh updates
                              |
                    failure --+--> discard candidate
                              |
                              v
recheck scene, static-collision, and lease generations
                              |
                              v
publish one source revision and one typed receipt
```

`prepare` performs validation and constructs a complete candidate away from
live authority. `commit` rejects if the source/residency authority, admitted
static collision, or lease registry changed in the meantime. `apply` combines
both steps. No operation in a rejected transaction becomes visible.

The accepted receipt separates admitted, replaced, evicted, and retained
chunks; reports the exact dirty set; and records resident counts, authority and
residency hashes, coherent derived revisions, and rebuilt/reused/removed mesh
counts. A whole-chunk lifecycle dirties the owner plus every resident chunk
whose derived seam can depend on it: face neighbors for greedy cubes and the
26-cell chunk halo for Marching Cubes and Dual Contouring. Eviction retains the
removed identity in the dirty set so retained rendering destroys its stable
handle.

## Leases and edit history

`VoxelChunkLeaseRegistry` is instance-owned pin evidence. A caller explicitly
acquires and releases leases for resident chunks; active evidence blocks
replacement and eviction with `ChunkPinned`. Lease acquisition does not imply
a player radius, asynchronous I/O, cache ownership, or hidden drop behavior.

Residency cannot silently prune or reinterpret the global hash-chained edit
history. `apply_with_history` requires one explicit policy:

- `RejectIfNonEmpty` leaves both authorities unchanged while history has
  entries.
- `ResetToPublishedAuthority` publishes the prepared residency candidate and
  rebases history to that exact authority, returning reset evidence.

History schema 3 persists base resident coordinates and mesh options. Undo
therefore cannot resurrect an evicted chunk or erase an explicitly resident
empty chunk. Ordinary voxel edits preserve residency when clearing the last
solid cell; eviction remains an explicit residency operation.

## Downstream integration

The complete `rusty-engine` facade re-exports the residency types. The
executable example shows caller-owned payload selection and lease lifetime:

```bash
cargo run -p rusty-engine --example voxel_chunk_residency --locked
```

Downstream code should retain the receipt's accepted source revision and each
resident chunk's content hash as the preconditions for its next decision. It
should not mutate `svc-volume` independently or treat a renderer mesh as loaded
world authority.

## Representative measurement

Run:

```bash
./scripts/measure-voxel-chunk-residency.sh
```

The release probe admits, replaces, and evicts one row of sparse but complete
dense chunks through the public facade. Times are single-run characterization
on the development host; payload and resident-cell bytes are deterministic.

| Chunk edge | Chunks | Admit us | Replace us | Evict us | Payload bytes | Resident bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 16 | 1 | 562 | 558 | 46 | 8,192 | 16,384 |
| 16 | 8 | 4,670 | 4,834 | 305 | 65,536 | 131,072 |
| 16 | 64 | 41,160 | 43,520 | 2,712 | 524,288 | 1,048,576 |
| 32 | 1 | 3,742 | 4,149 | 299 | 65,536 | 131,072 |
| 32 | 8 | 37,432 | 40,754 | 3,168 | 524,288 | 1,048,576 |
| 32 | 64 | 378,083 | 392,099 | 21,038 | 4,194,304 | 8,388,608 |
| 64 | 1 | 36,539 | 39,932 | 2,624 | 524,288 | 1,048,576 |
| 64 | 8 | 352,666 | 418,738 | 26,834 | 4,194,304 | 8,388,608 |
| 64 | 64 | 3,407,319 | 3,567,225 | 167,191 | 33,554,432 | 67,108,864 |

The probe deliberately exposes dense candidate materialization cost rather
than claiming a frame budget. The contract bounds one transaction at 64 chunks,
64-cube payloads, and 4,096 total residents. A game can choose smaller batches;
Engine does not add an ambient queue or streaming scheduler.

## Verification

```bash
cargo test -p engine-spatial -p render-projection --locked
cargo clippy -p engine-spatial -p render-projection -p rusty-engine --all-targets --locked -- -D warnings
cargo run -p rusty-engine --example voxel_chunk_residency --locked
./scripts/measure-voxel-chunk-residency.sh
./scripts/verify.sh
```
