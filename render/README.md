# Rusty Engine renderer workspace

This isolated pnpm workspace owns strict renderer-contract decoding, retained
projection, Three/WebGL realization, browser and webview hosts, checked
reproducible artifacts, and browser evidence. It is an Engine-private backend,
not a package graph for ordinary downstream games.

Run its complete gate from the repository root:

```bash
./scripts/verify-render.sh
```

Capability decisions remain in repository documentation rather than in
generated bundles. In particular:

- [Three scene particles](../docs/topics/three-scene-particles.md) covers
  pooled billboard/cube particles and approximate local collision;
- [lit sprite shader comparison](../docs/topics/lit-sprite-shaders.md) covers
  bounded lit-sprite modes, linear normal/depth maps, alpha/shadow policy, and
  the checked moving-camera/light comparison; and
- [downstream renderer and Studio boundary](../docs/topics/development/downstream-renderer-and-studio.md)
  defines the public consumption path and ownership boundary.

Do not edit `render/artifacts/application-host/index.js` or the webview artifact
by hand. Change typed source, rebuild through the workspace scripts, and verify
artifact freshness through the complete gate.
