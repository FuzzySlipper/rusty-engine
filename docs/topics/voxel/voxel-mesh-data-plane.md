# Content-addressed voxel mesh data plane

Status: implemented format decision for renderer-neutral voxel-object geometry.

## Decision

Large voxel-object mesh streams do not travel inside Studio's JSONL control
responses. A Rust projector may replace the existing inline number arrays with
content-addressed `resource` sources. The control frame retains mesh layout,
groups, bounds, provenance, and byte offsets; deterministic packed bytes travel
through an explicit host resolver.

This is a presentation data plane, not a new voxel authority:

- canonical schema-1 voxel-object JSON remains the complete sparse-run source,
  content-hash anchor, and reload/migration format;
- packed mesh resources are deterministic derived artifacts and may be deleted
  and regenerated without changing the voxel object;
- resource identities name bytes, never paths, URLs, renderer handles, or
  gameplay objects; and
- downstream consumers own publication paths, cache lifetime, and admission
  timing. Engine projection returns bytes but performs no filesystem or HTTP
  work.

Inline payloads remain supported for bounded fixtures and consumers that do not
need a bulk-data channel. `sharedBuffer` remains a separate transient
borrow-handle seam; it is not a durable resource identity.

## Evidence and rejected candidates

The decision uses the checked `rusty-engine-voxels` baseline and 96x144x96
high-fidelity corpus at commit `6f12c100c362a462dbe39083b589191e9b786feb`.
Both contain 14 unique flipbook meshes.

| Candidate stream shape | Baseline | High fidelity | Decision |
|---|---:|---:|---|
| Expanded JSON arrays | 2.39 MB | 54.56 MB | Existing compatibility path only |
| Packed bytes in base64 JSON | 2.41 MB | 46.06 MB | Rejected: retains the control/parse cliff |
| Full little-endian binary | 1.81 MB | 34.54 MB | Selected behind resource references |
| Base plus whole-mesh binary deltas | 0.99 MB | 19.63 MB | Deferred: 64-67% of vertex values change |

Index topology changes by only 0.7-3.6%, so a later independently versioned
format may share index streams. Version 1 deliberately does not add that
complexity. It also does not use compression: this is a trusted local/LAN
workflow, and predictable bounded decode and main-thread work matter more than
internet transfer size.

## `packedStreamsLeV1`

Each resource is at most 64 MiB. `pack_mesh_resources` partitions an ordered
payload set deterministically before that ceiling, so a longer animation becomes
multiple independently bounded resources instead of one oversized response.
One mesh must fit in one resource. Resource sets are empty-safe; non-empty
resources have this 16-byte header:

| Bytes | Meaning |
|---|---|
| `0..8` | ASCII magic/version `RMSHLE01` |
| `8..12` | Complete resource byte length, little-endian `u32` |
| `12..16` | Mesh payload count, nonzero little-endian `u32` |

For each mesh, the packer then writes positions as little-endian `f32`, normals
as little-endian `f32`, and indices as little-endian `u32`. The retained source
descriptor carries the three aligned offsets, complete resource byte length,
and `packedStreamsLeV1` encoding. Layout counts determine each stream length;
streams may not overlap or exceed the resource.

The SHA-256 of the complete header and body is both the `contentHash`
(`sha256:<64 lowercase hex>`) and the suffix of the identity
(`mesh-resource/<same hex>`). Rust validation, the TypeScript decoder, the host
manifest, and Studio's byte service all reject identity, length, offset, header,
or hash drift. The renderer borrows the admitted bytes, copies the declared
streams into renderer-owned arrays, and releases the borrow on success and
failure.

The host manifest `rusty_renderer_mesh_resources.v1` admits at most 1,024
resources and 256 MiB in aggregate. These bounds price the current eager
preload/copy implementation honestly. If a real corpus exceeds them, the next
step is measured lazy chunk admission, not raising every allocation ceiling.

## Studio transport and versioning

The renderer contract's source variant and encoding name are the versioned data
seam. Studio protocol 9 keeps existing inline adapters valid through an optional
`meshResources` readout. An opting-in adapter supplies one mapping from resource
identity/hash/length to its owner-chosen project-relative path. The projection
contains no path.

Studio preloads the manifest before mounting the shared viewport. Its trusted
Node host serves hash-checked `.rmesh` bytes over the existing bounded
`/api/studio-render-resource` channel as `application/octet-stream`. No Rust
crate or renderer-neutral TypeScript package knows that endpoint, filesystem
layout, or HTTP exists.

An incompatible binary layout must use a new encoding and header version. An
incompatible manifest must use a new manifest `kind`. A future change to the
closed Studio operation family still requires a Studio protocol version bump;
this optional resource readout does not change that family.

## Migration and ownership

Consumers migrate one projection boundary at a time:

1. construct `VoxelObjectRenderProjector::with_packed_mesh_resources()`;
2. publish every returned `PackedMeshResource` atomically under downstream
   cache/storage policy;
3. return its resource/path mappings with the same projection; and
4. retain inline projection only where no bulk resolver exists.

Complete define/open and conversion-candidate projections use the same seam.
Steady-state animation remains one small `setVoxelObjectFrame` operation and
does not reread or resend mesh bytes. Deleting the downstream cache is safe: the
next admitted projection regenerates identical bytes from the canonical object.

This migration does not change voxel-object schema 1. A compact or binary
canonical sibling would create migration, hash, provenance, and editing costs
without fixing Studio's control-channel problem; it should be considered only
if canonical decode/storage measurements independently justify it.

## Verification

Provider and host behavior is covered by:

```bash
cargo test -p render-model -p render-projection --locked
./scripts/verify-render.sh
./scripts/verify-studio.sh
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

The external integration is the authority for real control bytes, binary bytes,
browser load cost, and visible high-fidelity Studio behavior.
