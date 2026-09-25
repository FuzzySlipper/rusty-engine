# Rusty Engine documentation

This is the small, current documentation set for Rusty Engine's packaged C#
downstream lane. CoreCLR is the normal development loader and NativeAOT is an
explicit fidelity/release path. The documentation describes demonstrated
owners and source paths, not a promise that every proposal or Rust API is
already exposed to C#.

- [Architecture overview](architecture.md) explains the Rust, generated C#,
  C# product, and TypeScript lanes.
- [Rope physics contract](rope-physics.md) records campaign #6992's bounded
  solver design and probes; the proposed services are not yet SDK capabilities.
- [C# SDK guide](csharp-sdk.md) explains the current product bootstrap,
  lifecycle, services, generated output, leases, and optional managed helper
  packages. Ordinary gameplay lives in its entity/mechanics sections:
  `EntityStore` and Engine adapters, class components, `Stat`/`Track` and live
  mechanics components, optional `InventoryEdit`, and explicit save/debug.
- [World interaction and controller aim assistance](controller-interaction.md)
  is the green path for containers/doors, sticky targeting, controller aiming,
  and agent testing without repeated pixel hunting. Start with `interaction.inspect`.
- [C# SDK/runtime distribution](csharp-distribution.md) explains the exact,
  verified Linux-x64 release pair used by clean downstream CI.
- [C# product style](csharp-product-style.md) gives a recommended, product-side
  organization that does not require a hidden Engine framework.
- [C# capability map](csharp-capabilities.md) inventories the current generated
  service families, managed helpers, and retained native runtime mechanisms.
  Managed-helper rows name the owning SDK guide sections; the generated
  contracts and their Rust ABI sources remain authoritative over this summary.
- [CoreCLR diagnostics](coreclr-diagnostics.md) covers worker discovery, standard
  managed profiling, callback breakpoints over SSH, and dumps.
- [World streaming and state contract](world-streaming-contract.md) covers movement
  support, sparse block state, Engine call affinity and voxel budget measurements.
- [Runtime profiling](runtime-profiling.md) explains worker timing, runtime
  correlation, and optimized Rust CPU captures.
- [Validation inventory](validation-inventory.md) explains the searchable validation/limit
  candidate survey and the per-check review questions.
- [Verification notes](verification.md) describe the report-only Playwright
  warning-delta capture and compatible-baseline comparison.

The root [README](../README.md) is the repository landing page and
[AGENTS.md](../AGENTS.md) is the compact task-time guidance. Historical
documentation remains in Git history as donor material only; do not restore it
wholesale or use it to reintroduce superseded authoring or downstream-language
assumptions.
