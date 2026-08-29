# Rusty Engine documentation

This is the small, current documentation set for Rusty Engine's NativeAOT C#
downstream lane. It describes demonstrated owners and source paths, not a
promise that every proposal or Rust API is already exposed to C#.

- [Architecture overview](architecture.md) explains the Rust, generated C#,
  C# product, and TypeScript lanes.
- [C# SDK guide](csharp-sdk.md) explains the current product bootstrap,
  lifecycle, services, generated output, leases, and optional managed helper
  packages.
- [C# product style](csharp-product-style.md) gives a recommended, product-side
  organization that does not require a hidden Engine framework.
- [Managed capability migration](csharp-managed-migration.md) is the living
  capability ledger and decision gate for retiring or adapting the legacy
  gameplay-facing Rust and TypeScript lanes.

The root [README](../README.md) is the repository landing page and
[AGENTS.md](../AGENTS.md) is the compact task-time guidance. Historical
documentation remains in Git history as donor material only; do not restore it
wholesale or use it to reintroduce old Product Model, downstream-Rust, or
compiled-TypeScript assumptions.
