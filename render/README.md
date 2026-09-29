# Rusty Engine browser shell workspace

This pnpm workspace owns the TypeScript the Engine still ships to a browser:

- `application-host`: the Engine canvas, input capture and arbitration, the
  product UI mount and its UI projection, and the view of the world the runtime
  renders (streamed frames, or the desktop window showing through).
- `product-browser-host`: the local runtime transport, lifecycle and input
  recovery, and the runtime-pack shell page (`runtime-pack-shell/`).
- `live-debug-client` and `live-debug-panel`: the optional developer console
  over the product's debug catalog.

The world, audio and video are rendered in the runtime process by Rust
(`render-wgpu`, `render-audio`); see the
[architecture overview](../docs/architecture.md#runtime-rendered-output).
It is an Engine-private workspace, not a package graph for ordinary downstream
games.

Build and test it from the repository root:

```bash
pnpm --dir render install --frozen-lockfile
pnpm --dir render run verify
```

`render/artifacts/` is ignored build output. `pnpm --dir render run build`
writes the browser shell and live-debug bundles, which the runtime pack
builder installs.
