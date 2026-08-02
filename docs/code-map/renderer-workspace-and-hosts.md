# Renderer workspace and hosts

## Purpose

Route the isolated TypeScript retained projection, Three backend, browser
surface, and renderer host/resource lifecycle.

## Owns

- `@rusty-engine/render-contracts`: strict decoding of the Rust retained-frame
  border.
- `@rusty-engine/render-projection`: retained client-side projection.
- `@rusty-engine/renderer-three`: Three/WebGL resources and render passes.
- `@rusty-engine/renderer-host`: backend/host composition, presentation hosts,
  inspection surface, immutable submission/resource statistics, timing,
  camera, and editor viewport integration.
- `render/browser`: real Chromium/WebGL/WebAudio/DOM acceptance.

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
- [`render/packages/renderer-host/src`](../../render/packages/renderer-host/src)
  - `animated-mesh-capture.ts` owns deterministic PNG/contact-sheet encoding
    over an already-mounted `RendererSurface`; it does not load assets or own a
    second animation loop.
- [`render/browser`](../../render/browser)
- [`render/package.json`](../../render/package.json)
- [Rendering operations](../rendering-operations.md)
- [Downstream Engine revision contract](../topics/development/downstream-engine-revisions.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Textured voxel campaign closeout](../textured-voxel-campaign-closeout.md)

## Public downstream surfaces

- Package-root exports declared in each package's `package.json`.
- Exact public Git revisions can be tested with
  [`verify-render-consumer.sh`](../../scripts/verify-render-consumer.sh).
- Browser, webview, and headless hosts compose over the same explicit retained
  border without owning game state.
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
- Do not make DOM state, Three objects, or renderer buffers authoritative.
- Do not add historical Asha runtime/bridge dependencies.
- Do not require Node or browser installation for the ordinary Rust gate.

## Acceptance gates and fixtures

```bash
./scripts/verify-render.sh
./scripts/verify-render-consumer.sh <40-character-public-sha>
```

The render gate includes isolation, exact behavior accounting, package tests,
and the real browser path. Retained inputs live under
[`fixtures/render`](../../fixtures/render).

## Common agent mistakes

- Proving only headless projection when the changed behavior is browser-owned.
- Leaking backend-specific types into `render-contracts`.
- Forgetting disposal, replacement, stale-frame, or partial-failure behavior.
- Substituting authored entity/resource counts for renderer-owned submission
  statistics, or treating an unavailable counter as zero.
- Treating an exact-revision consumer install as interchangeable with a local
  workspace link.

## Follow-up routing

- Rust frame changes:
  [Rust render model and projection](rust-render-model-and-projection.md).
- First-party authoring viewport: [Studio](studio.md).
- Downstream loading-bay product behavior belongs in `rusty-engine-demo`.
