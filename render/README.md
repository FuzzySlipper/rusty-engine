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

Do not edit `render/artifacts/application-host/index.js` or the webview artifact
by hand. Change typed source, rebuild through the workspace scripts, and verify
artifact freshness through the complete gate.
