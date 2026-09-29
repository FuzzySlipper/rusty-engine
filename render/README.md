# Rusty Engine renderer workspace

This isolated pnpm workspace owns strict renderer-contract decoding, retained
projection, Three/WebGL realization, browser and webview hosts, checked
reproducible artifacts, and browser evidence. It is an Engine-private backend,
not a package graph for ordinary downstream games.

Run its complete gate from the repository root:

```bash
./scripts/verify-render.sh
```

Capability and ownership decisions remain in repository documentation rather
than generated bundles. The current public consumption and renderer ownership
boundary is summarized in the
[Engine architecture overview](../docs/architecture.md). Historical renderer
notes remain available in Git history as implementation donor material.

`render/artifacts/` and `rust/crates/renderer-webview-host/artifacts/` are
ignored build outputs. `pnpm --dir render run build` writes the browser bundles
(the runtime pack builder runs it), and `pnpm --dir render run
build:webview-artifact` writes the webview bundle that `renderer-webview-host`
embeds.
