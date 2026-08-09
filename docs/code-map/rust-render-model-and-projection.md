# Rust render model and projection

## Purpose

Route renderer-neutral retained-frame vocabulary and Rust projection from
entity, authored-scene, voxel, and presentation facts.

## Owns

- `render-model`: the complete versioned retained-frame vocabulary and
  validation contract.
- `render-projection`: fail-atomic projection into retained entities, meshes,
  voxels, lights, materials, previews, and debug facts.
- `render-presentation`: bounded animation-controller, audio, billboard,
  particle, telemetry, and asset-view projection mechanisms.
- `render-host-contracts`: Rust-owned webview composition, camera, pick,
  physical-input, observation, diagnostic, and lifecycle facts.
- `rusty-engine`: complete unconditional namespace-preserving downstream facade.

## Does not own

- Gameplay authority or renderer resource handles.
- Three/WebGL, DOM/WebAudio, Chromium, product shell, or Studio UI.
- A universal event bus or gameplay runtime.

## Primary paths

- [`render-model/src/lib.rs`](../../rust/crates/render-model/src/lib.rs)
- [`render-projection/src/lib.rs`](../../rust/crates/render-projection/src/lib.rs)
- [`render-projection/src/material.rs`](../../rust/crates/render-projection/src/material.rs)
- [`render-presentation/src/lib.rs`](../../rust/crates/render-presentation/src/lib.rs)
- [`render-host-contracts/src/lib.rs`](../../rust/crates/render-host-contracts/src/lib.rs)
- [`rusty-engine/src/lib.rs`](../../rust/crates/rusty-engine/src/lib.rs)
- [Rust SDK capability index](../rust-sdk-capabilities.md)
- [`fixtures/render`](../../fixtures/render)
- [Rendering successor contract](../rendering-successor-contract.md)
- [Rendering operations](../rendering-operations.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Reconstructed voxel surfaces](../topics/voxel/reconstructed-surfaces.md)
- [Textured voxel campaign closeout](../textured-voxel-campaign-closeout.md)

## Public downstream surfaces

- `render-model` is the Rust-owned serialized border consumed by retained
  TypeScript contracts.
- Its packed-mesh helper creates deterministic bounded resource bytes; UV-free
  payloads retain packed V1 while UV-bearing payloads use packed V2 with an
  explicit stream offset. Callers own where those bytes are published.
- Projection crates translate owner facts into complete frames; they do not
  become a second source of gameplay truth.
- Voxel-object projection keys cached resources by canonical content hash and
  derived surface mode. A mode switch redefines geometry at the same canonical
  identity; textured reconstructed materials are rejected fail-atomically.
- `project_catalog_material` validates the complete catalog candidate and
  projects immutable texture/atlas/region provenance into the optional strict
  voxel-surface material descriptor before retained publication.
- Entity projection reads typed `EntityState` component views. Registering a
  new downstream component does not implicitly render it; its owning consumer
  must deliberately project any presentation meaning.
- Ordinary downstream games import these exact crate namespaces only through
  `rusty-engine`; the coverage check fails when any public library is omitted.
- Downstream hosts may consume the frame through any conforming backend. The
  supported Engine-owned concrete path is `renderer-webview-host`.

## Private or forbidden paths

- Do not add browser globals, HTTP, URL routes, DOM events, WebGL, or
  Playwright-only seams.
- Do not store live renderer handles or infer gameplay state from presentation.
- Do not add renderer behavior, resource loading, or projection callbacks to
  entity components or the component registry.
- Do not change the Rust frame without updating TypeScript decoding and golden
  evidence.
- Do not depend on historical Asha runtime or bridge concepts.

## Acceptance gates and fixtures

```bash
cargo test -p render-model -p render-projection -p render-presentation -p render-host-contracts -p rusty-engine --locked
cargo clippy -p render-model -p render-projection -p render-presentation -p render-host-contracts -p rusty-engine --all-targets --locked -- -D warnings
python3 ./scripts/check-rust-sdk-coverage.py
./scripts/verify-rust-sdk-consumer.sh
./scripts/verify-render.sh
```

Contract regressions live in the three crate test directories and
[`fixtures/render`](../../fixtures/render).

## Common agent mistakes

- Calling a render frame authoritative gameplay state.
- Adding a backend convenience field that leaks Three or browser concepts into
  the neutral contract.
- Updating only Rust or only TypeScript at the cross-language border.
- Treating telemetry or debug projection as an authority source.

## Follow-up routing

- TypeScript decoding and renderer resources:
  [Renderer workspace and hosts](renderer-workspace-and-hosts.md).
- Read-only operational views:
  [Inspection and diagnostics](inspection-and-diagnostics.md).
- Studio viewport integration: [Studio](studio.md).
