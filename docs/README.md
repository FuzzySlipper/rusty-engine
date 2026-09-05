# Rusty Engine documentation

This is the small, current documentation set for Rusty Engine's packaged C#
downstream lane. CoreCLR is the normal development loader and NativeAOT is an
explicit fidelity/release path. The documentation describes demonstrated
owners and source paths, not a promise that every proposal or Rust API is
already exposed to C#.

- [Architecture overview](architecture.md) explains the Rust, generated C#,
  C# product, and TypeScript lanes.
- [C# SDK guide](csharp-sdk.md) explains the current product bootstrap,
  lifecycle, services, generated output, leases, and optional managed helper
  packages.
- [C# SDK/runtime distribution](csharp-distribution.md) explains the exact,
  verified Linux-x64 release pair used by clean downstream CI.
- [C# product style](csharp-product-style.md) gives a recommended, product-side
  organization that does not require a hidden Engine framework.
- [C# capability map](csharp-capabilities.md) inventories the current generated
  service families, managed helpers, and retained native runtime mechanisms.
- [CoreCLR diagnostics](coreclr-diagnostics.md) covers worker discovery, standard
  managed profiling, callback breakpoints over SSH, and dumps.
- [Verification notes](verification.md) describe the report-only Playwright
  warning-delta capture and compatible-baseline comparison.

The root [README](../README.md) is the repository landing page and
[AGENTS.md](../AGENTS.md) is the compact task-time guidance. Historical
documentation remains in Git history as donor material only; do not restore it
wholesale or use it to reintroduce superseded authoring or downstream-language
assumptions.
