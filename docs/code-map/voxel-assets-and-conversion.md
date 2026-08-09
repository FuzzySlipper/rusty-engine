# Voxel assets and conversion

## Purpose

Route work involving durable voxel volumes and objects, annotations, bounded
GLB conversion, animation sampling, runtime admission, playback, and collision
selection.

## Owns

- `voxel-asset`: canonical versioned voxel-volume, voxel-frame, and
  voxel-object formats plus strict codecs and limits, including bounded
  per-frame local anchors and coarse collision facts.
- `voxel-annotation`: versioned annotation layers, queries, and atomic edits.
- `voxel-convert`: bounded offline GLB import, geometric voxelization,
  animation sampling, conversion planning, and atomic installation.
- `voxel-object-runtime`: admitted runtime frames, immutable frame-fact
  readout, clips, playback state, mesh realization, and explicit collision
  policy.

## Does not own

- Game-specific animation decisions, combat collision meaning, or asset storage
  policy.
- A live browser importer or URL/fetch seam.
- Renderer resources or Three.js animation playback.

## Primary paths

- [`voxel-asset/src/lib.rs`](../../rust/crates/voxel-asset/src/lib.rs)
- [`voxel-annotation/src/lib.rs`](../../rust/crates/voxel-annotation/src/lib.rs)
- [`voxel-convert/src/lib.rs`](../../rust/crates/voxel-convert/src/lib.rs)
- [`voxel-convert/src/object_conversion`](../../rust/crates/voxel-convert/src/object_conversion)
- [`voxel-object-runtime/src/lib.rs`](../../rust/crates/voxel-object-runtime/src/lib.rs)
- [Voxel asset format](../topics/voxel/voxel-asset-format.md)
- [Voxel model conversion](../topics/voxel/voxel-model-conversion.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Reconstructed voxel surfaces](../topics/voxel/reconstructed-surfaces.md)
- [Textured voxel campaign closeout](../textured-voxel-campaign-closeout.md)

## Public downstream surfaces

- Strict JSON formats and codecs provide the durable border.
- Runtime surface texture mapping is deliberately separate from conversion-time
  source texture sampling; use the runtime texture decision before changing
  either seam.
- `voxel-convert` binaries and library APIs are offline producers with bounded
  diagnostics and resource ceilings. Its narrow `evaluate_clip_node_poses`
  seam returns canonical scale-preserving node transforms at one explicit
  clip time without deforming or materializing a mesh.
- `voxel-object-runtime` admits validated artifacts and exposes direct playback
  and realization mechanisms for downstream composition. Runtime frame anchor
  lookup and collision metadata are read-only; the caller owns world
  application and game meaning. Capsule facts use the schema's right-handed
  local-Y axis and exclude caps from `halfHeight`; use the public endpoint and
  bounds helpers instead of importing a host API's capsule convention.
- `VoxelObjectAdmissionOptions` can select a derived surface mode without
  changing object bytes, hashes, frame facts, anchors, or collision. Runtime
  aggregate mesh/scalar/material limits still apply across unique frames.

## Private or forbidden paths

- Do not loosen byte, count, work, animation, or snapshot bounds for a fixture.
- Do not make independently bounded items accumulate without an aggregate
  resource ceiling.
- Do not infer game meaning from animation clips or collision modes.
- Do not duplicate clip interpolation or silently drop node scale in a caller;
  consume the converter's node-pose seam and choose its explicit rigid-scale
  admission policy when rigid placement is required.
- Do not bypass canonical encoding, hashes, provenance, or atomic installation.

## Acceptance gates and fixtures

```bash
cargo test -p voxel-asset -p voxel-annotation -p voxel-convert -p voxel-object-runtime --locked
cargo clippy -p voxel-asset -p voxel-annotation -p voxel-convert -p voxel-object-runtime --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Important fixtures live under
[`fixtures/voxel-conversion`](../../fixtures/voxel-conversion),
[`fixtures/voxel-mesh`](../../fixtures/voxel-mesh), and
[`content`](../../content).

## Common agent mistakes

- Calling a per-clip or per-frame bound an aggregate bound.
- Treating an offline converter as a runtime service.
- Reusing a visible voxel frame as authoritative gameplay state without an
  owning consumer decision.
- Adding browser-only APIs to simplify conversion tests.

## Follow-up routing

- Canonical live voxel space and edits: [Spatial mechanisms](spatial-mechanisms.md).
- Catalogs, manifests, and publication:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Visual realization: [Rust render model and projection](rust-render-model-and-projection.md).
