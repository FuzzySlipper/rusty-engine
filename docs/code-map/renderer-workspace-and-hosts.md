# Renderer workspace and hosts

## Purpose

Route the isolated Engine-private TypeScript retained projection, Three backend,
compiled artifact, and Rust webview host/resource lifecycle.

## Owns

- `@rusty-engine/render-contracts`: strict decoding of the Rust retained-frame
  border and bounded renderer-neutral multi-view composition descriptors.
- `@rusty-engine/render-projection`: retained client-side projection.
- `@rusty-engine/renderer-three`: Three/WebGL resources and render passes.
- `@rusty-engine/renderer-host`: backend/host composition, presentation hosts,
  inspection surface, immutable submission/resource statistics, timing,
  camera, versioned default-light/shadow policy and readout, and editor viewport integration.
- `@rusty-engine/application-host`: one bundled downstream browser/wrapper
  composition root, trusted rich-DOM mount, typed frame/camera/interaction
  ports, transactional whole-frame replacement, combined static-mesh/texture/
  animated-GLB resource admission, and lifecycle cleanup.
- `render/artifacts/application-host`: reproducible public artifact with no
  renderer peer or runtime dependencies in the downstream lock.
- `render/browser`: real Chromium/WebGL/WebAudio/DOM acceptance.
- `render/product-playtest`: bounded public application-host product fixture for
  on-demand black-box playtesting; it owns no production or test authority.
- `render/private/webview`: fixed private bridge and thin composition root.
- `renderer-webview-host`: embedded artifact, one Wry child webview, bounded
  resource admission, named Rust operations, typed IPC observations, and disposal.

## Does not own

- Gameplay, entity, spatial, or asset authority.
- Product-specific shell policy.
- Ordinary Rust verification or a requirement that Engine be a web-game engine.

## Primary paths

- [`render/packages/render-contracts/src`](../../render/packages/render-contracts/src)
- [`render/packages/render-projection/src`](../../render/packages/render-projection/src)
- [`render/packages/renderer-three/src`](../../render/packages/renderer-three/src)
  - `animated-mesh.ts` owns asset-scoped geometry/material templates, independent
    per-instance skeleton/mixer state, exact normalized sampling, bounded
    deformation diagnostics, and their retained replacement/disposal lifecycle.
  - `voxel-surface-material.ts` owns the Three shader specialization for
    Euclidean whole-texture repeat and half-texel-safe atlas-region sampling;
    it observes renderer-neutral material and texture facts and never remeshes.
  - `lighting.ts` owns bounded Three light realization, requested-shadow status,
    and typed lighting-policy failures; `browser-surface.ts` owns the independent
    world/viewmodel neutral rigs and WebGL shadow-map switch.
  - `view-composition.ts` owns atomic Three camera/target/presentation realization,
    stale target revision guards, deterministic view order, physical-pixel viewport
    conversion, and target/presentation disposal. It is backend-private.
  - `three-renderer.ts` owns the frozen CPU visibility readout: effective retained
    visibility and camera-frustum membership per handle. It explicitly reports
    GPU occlusion as `notMeasured` rather than inventing a depth authority.
- [`render/packages/renderer-host/src`](../../render/packages/renderer-host/src)
  - `animated-mesh-capture.ts` owns deterministic PNG/contact-sheet encoding
    over an already-mounted `RendererSurface`; it does not load assets or own a
    second animation loop.
- [`render/packages/application-host/src`](../../render/packages/application-host/src)
- [`render/artifacts/application-host`](../../render/artifacts/application-host)
- [`render/browser`](../../render/browser)
- [`render/product-playtest`](../../render/product-playtest)
- [`render/private/webview`](../../render/private/webview)
- [`renderer-webview-host/src/lib.rs`](../../rust/crates/renderer-webview-host/src/lib.rs)
- [`renderer-webview-host/examples/webview_smoke.rs`](../../rust/crates/renderer-webview-host/examples/webview_smoke.rs)
- [`render/package.json`](../../render/package.json)
- [Rendering operations](../rendering-operations.md)
- [Downstream renderer and Studio boundary](../topics/development/downstream-renderer-and-studio.md)
- [Product playtesting and evidence authority](../topics/development/product-playtesting.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Textured voxel campaign closeout](../textured-voxel-campaign-closeout.md)
- [Structured world indicators](../topics/world-indicators.md)

## Public downstream surfaces

- Ordinary games use `rusty_engine::renderer_webview_host` and
  `rusty_engine::render_host_contracts`; TypeScript package topology is private.
- Browser/Tauri/Electron products that need rich DOM import only
  `@rusty-engine/application-host`; its bundled closure keeps renderer package
  names and backend types out of downstream manifests, locks, and source.
- Structured indicators are realized by the existing billboard host in one
  Engine-owned pointer-transparent overlay. Stable local meter identities allow
  value-only updates without rebuilding unrelated DOM; deterministic priority,
  safe-area, edge, overlap, and suppression policy remains backend-owned.
- The downstream owns the outer window/event loop and content/storage policy.
  The adapter owns one child webview and accepts pre-admitted content-hash-bound
  bytes without URLs or filesystem access.
- First-party Engine workspaces may use package-root TypeScript exports. No
  downstream game may deep-import them or reach the private bridge.
- `RendererSurface.configureViews` publishes a complete immutable composition;
  `viewCompositionReadout` exposes target freshness and resource counts without
  exposing WebGL, Three textures, or a CPU readback path.
- `RendererSurface.visibilityReadout` and the editor viewport counterpart expose
  deterministic per-handle CPU visibility facts for their camera passes. They
  are optional work-gating observations, not gameplay or renderer authority.
- Generic static meshes bind the renderer-neutral `uv` attribute directly to
  the resolved material texture. Whole-texture and atlas-region voxel sampling
  remain an explicit voxel-surface specialization; ordinary textured meshes do
  not enter that shader path.
- Textured voxel replacement reuses retained geometry/object handles and the
  reference-counted texture owner. The exact public consumer's lifecycle and
  provider disposal evidence are reconciled in the
  [textured voxel campaign closeout](../textured-voxel-campaign-closeout.md).

## Private or forbidden paths

- Do not deep-import package `src/` files from downstream consumers.
- Do not expose generic JavaScript invocation, eval, module imports, browser
  objects, or a second command/observation transport through the Rust adapter.
- Do not make DOM state, Three objects, or renderer buffers authoritative.
- Do not add historical Asha runtime/bridge dependencies.
- Do not require Node or browser installation for the ordinary Rust gate.

## Acceptance gates and fixtures

```bash
./scripts/verify-render.sh
./scripts/verify-renderer-webview-host.sh
./scripts/verify-rust-sdk-consumer.sh
```

The render gate includes isolation, exact behavior accounting, package tests,
and the real browser path. Retained inputs live under
[`fixtures/render`](../../fixtures/render). Its deterministic boundary check
also keeps the public-host playtest fixture on the sole application-host path;
the model-driven session itself remains on-demand and outside ordinary CI.

## Common agent mistakes

- Proving only headless projection when the changed behavior is browser-owned.
- Leaking backend-specific types into `render-contracts`.
- Forgetting disposal, replacement, stale-frame, or partial-failure behavior.
- Substituting authored entity/resource counts for renderer-owned submission
  statistics, or treating an unavailable counter as zero.
- Treating an exact-revision consumer install as the ordinary downstream
  contract; local sibling-facade consumption is the supported path.

## Follow-up routing

- Rust frame changes:
  [Rust render model and projection](rust-render-model-and-projection.md).
- First-party authoring viewport: [Studio](studio.md).
- Downstream loading-bay product behavior belongs in `rusty-engine-demo`.
