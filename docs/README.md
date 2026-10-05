# Rusty Engine documentation

This is the small, current documentation set for Rusty Engine's packaged C#
downstream lane. CoreCLR is the normal development loader and NativeAOT is an
explicit fidelity/release path. The pages describe what the source does now,
not a promise that every Rust API is exposed to C#. History stays in Git and
Den, not here.

The root [README](../README.md) is the repository landing page and
[AGENTS.md](../AGENTS.md) is the compact task-time guidance.

## Architecture

- [Architecture overview](architecture.md): the Rust, generated C#, C# product
  and TypeScript layers, source owners, runtime-rendered output, packaging.
- [Desktop shell](desktop-shell.md): the native window, `render-wgpu` on a
  window surface with the product UI composited through Chromium.

## C# SDK

- [C# SDK guide](csharp-sdk.md) is the downstream entry page: start, run,
  update and troubleshoot a product with the `rusty` CLI. Its references:
  - [product project, build and staging](csharp-product-project.md);
  - [lifecycle and services](csharp-lifecycle.md);
  - [managed helpers](csharp-helpers.md) (`EntityStore` and Engine adapters,
    class components, `Stat`/`Track`, optional `InventoryEdit`, explicit
    save/debug);
  - [offline images and GLB export](csharp-offline-images.md);
  - [implicit surfaces and other runtime services](csharp-implicit-surfaces.md).
- [C# capability map](csharp-capabilities.md) inventories the generated
  service families, managed helpers and native runtime mechanisms. The
  generated contracts and their Rust ABI sources are authoritative over it.
- [C# SDK/runtime distribution](csharp-distribution.md): the Linux-x64 release
  pair, the desktop pack, the pin, the shared cache, release notes and
  migration notes.
- [C# product style](csharp-product-style.md): a recommended product-side
  organization that needs no hidden Engine framework.

## Engine services

- [World interaction and controller aim assistance](controller-interaction.md):
  containers/doors, sticky targeting, controller aiming, and agent testing
  without pixel hunting. Start with `interaction.inspect`.
- [World streaming and state contract](world-streaming-contract.md): movement
  support, sparse block state and Engine call affinity.
- [Voxel residency and edit costs](voxel-budgets.md): how voxel changes apply,
  limits and measured costs.
- [Smooth voxel surfaces and densities](smooth-voxel-surfaces.md): surface
  modes and characters per material, densities and brush edits, and collision
  that follows the drawn surface.
- [Lighting and skies](lighting-and-sky.md): retained torch lighting, voxel
  irradiance queries, persistence and product-clock panorama blending.
- [Portable assets](portable-assets.md): Engine-owned sprite/model semantics
  over loose files and bundles.
- [Recorded audio](recorded-audio.md): clip containers and device playback.
- [HTTP downloads](http-downloads.md): outbound HTTPS fetches, library
  downloads and update checks.
- [Rope physics](rope-physics.md): Dynamics tethers and character coupling.
- [Implicit topology on ambiguous faces](implicit-topology-diagnosis.md): how
  implicit meshing resolves ambiguous faces.

## Inspection, diagnostics and verification

- [Playtest inspection](playtest-inspection.md): held time, the observer
  camera, drawing mode and the product playtest modules.
- [Viewpoints and presentation observations](presentation-capture.md): frame
  identity and captures.
- [Performance diagnostics](performance.md) and
  [runtime profiling](runtime-profiling.md): regression lanes, renderer
  statistics, callback timing and native profiles.
- [CoreCLR diagnostics](coreclr-diagnostics.md): runtime process discovery,
  managed profiling, callback breakpoints over SSH, and dumps.
- [Verification notes](verification.md): CI lanes and the report-only
  playtest warning-delta capture.

## Working on the Engine

- [Parallel lanes](handoffs/README.md): the worktree and landing protocol for
  agents working at the same time.
- [Agent review](agent-review/README.md): reviewer lanes and packets.
