# Rusty Engine documentation

This is the small, current documentation set for Rusty Engine's packaged C#
downstream lane. CoreCLR is the normal development loader and NativeAOT is an
explicit fidelity/release path. The documentation describes demonstrated
owners and source paths, not a promise that every proposal or Rust API is
already exposed to C#.

**Current direction: campaign #8723 (architecture reset).** The campaign section
of [AGENTS.md](../AGENTS.md) and the Den charter
`rusty-engine/architecture-reset-2026-09` override preservation language in any
page here, in older task contracts, comments and tests. Existing mechanisms
such as validation layers, policy caps, receipts, leases, replay rules and
revision guards are candidates for removal, not requirements. Where a page
describes one, it describes what the code does today, not a reason to keep it.

- [Architecture overview](architecture.md) explains the Rust, generated C#,
  C# product, and TypeScript lanes.
- [Rope physics contract](rope-physics.md) records campaign #6992's bounded
  solver design and probes; the proposed services are not yet SDK capabilities.
- [C# SDK guide](csharp-sdk.md) is the downstream entry page: start, run,
  update and troubleshoot a product with the `rusty` CLI, then its capability
  references: [product project, build and staging](csharp-product-project.md),
  [lifecycle and services](csharp-lifecycle.md),
  [managed helpers](csharp-helpers.md) (`EntityStore` and Engine adapters,
  class components, `Stat`/`Track`, optional `InventoryEdit`, explicit
  save/debug), [offline images](csharp-offline-images.md) and
  [runtime implicit surfaces](csharp-implicit-surfaces.md).
- [World interaction and controller aim assistance](controller-interaction.md)
  is the green path for containers/doors, sticky targeting, controller aiming,
  and agent testing without repeated pixel hunting. Start with `interaction.inspect`.
- [C# SDK/runtime distribution](csharp-distribution.md) explains the exact
  Linux-x64 release pair, the pin, the shared cache and release information.
- [C# product style](csharp-product-style.md) gives a recommended, product-side
  organization that does not require a hidden Engine framework.
- [C# capability map](csharp-capabilities.md) inventories the current generated
  service families, managed helpers, and retained native runtime mechanisms.
  Managed-helper rows name the owning SDK guide sections; the generated
  contracts and their Rust ABI sources remain authoritative over this summary.
- [CoreCLR diagnostics](coreclr-diagnostics.md) covers runtime process discovery, standard
  managed profiling, callback breakpoints over SSH, and dumps.
- [World streaming and state contract](world-streaming-contract.md) covers movement
  support, sparse block state, Engine call affinity and voxel budget measurements.
- [Runtime profiling](runtime-profiling.md) explains callback timing, runtime
  correlation, and optimized Rust CPU captures.
- [Desktop shell](desktop-shell.md) is the native window: `render-wgpu` on a
  window surface with the product UI composited through Chromium.
- [Verification notes](verification.md) describe the report-only Playwright
  warning-delta capture and compatible-baseline comparison.

The root [README](../README.md) is the repository landing page and
[AGENTS.md](../AGENTS.md) is the compact task-time guidance. Historical
documentation remains in Git history as donor material only; do not restore it
wholesale or use it to reintroduce superseded authoring or downstream-language
assumptions. The pre-reset validation and limit audit ledgers are in
[history](history/README.md), for provenance only.

- [Lighting and skies](lighting-and-sky.md) covers retained torch lighting, voxel irradiance queries, persistence and product-clock panorama blending.

See [portable asset descriptors](portable-assets.md) for Engine-owned sprite/model semantics over loose files and bundles.
