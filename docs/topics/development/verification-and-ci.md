# Verification and CI ownership

Rusty Engine keeps focused checks close to the mechanism they prove and routes
CI by changed ownership surface. The goal is complete evidence once, not the
same build repeated through several wrappers.

## Owning gates

| Gate | Owns | Does not substitute for |
|---|---|---|
| `./scripts/verify.sh` | Complete Rust facade, provider mechanisms, structural boundaries, clean SDK consumer | Three/WebGL or browser behavior |
| `./scripts/verify-render.sh` | Renderer-neutral packages, Three backend, checked artifacts, Rust webview adapter, real Chromium | Downstream product acceptance |
| `./scripts/verify-studio.sh` | Engine-hosted Studio and its consumed renderer contracts/backend | Generic provider or downstream-project behavior |
| `./scripts/verify-rules.sh` | Isolated rules authoring plus Rust package compatibility | Downstream rules meaning or execution |
| `./scripts/verify-docs.sh` | Documentation links, architecture checker probes, code-map advisory, CI routing | Compiled implementation behavior |
| `./scripts/verify-all.sh` | Deliberate repository-wide aggregate | The default inner development loop |

Renderer dependency admission skips lifecycle scripts so workspace `prepare`
hooks do not compile packages during installation. Verification then snapshots
both checked generated artifacts, performs one package build and bundle pass,
compares both snapshots, and reuses the compiled output for package tests. The
Rust webview contracts/smoke and real Chromium remain independent evidence
because they exercise different hosts.

The public application-host publisher normalizes bundler-only dependency-region
comments. A valid install may resolve the same dependency through the render or
Studio workspace, but that physical symlink path is not public artifact content
and must not create source-identical artifact drift.

## Change routing

- `render/browser/**`, the bundled application host, and checked renderer
  artifacts run the renderer gate.
- Render contracts, projection, renderer-host, and renderer-three packages run
  both renderer and Studio compatibility gates because Studio consumes them.
- A generated `renderer-webview.js` change runs renderer verification but does
  not by itself run the complete Rust provider gate. Rust source changes to the
  webview host still run both because the complete facade re-exports that leaf.
- Ordinary Rust/provider source runs the provider gate. Named Rust owners used
  by Studio additionally run Studio compatibility.
- Gameplay-rules sources and their owning migration/contracts run the rules
  gate. Shared repository documentation does not run rules merely because it is
  documentation.
- Documentation and workflow changes run the documentation/routing gate;
  product-specific contract documents may additionally run their owning gate.

`scripts/check-ci-routing.py` makes these coarse ownership decisions and the
single-pass renderer contract deterministic. Its negative probes ensure broad
Studio routing, missing host routing, missing cancellation, and aggregate
renderer rebuilds fail instead of drifting silently.

## Ordinary task workflow

1. Run the narrowest package/crate or checker while iterating.
2. Run the one full gate that owns the changed surface on the coherent
   candidate. Run another product gate only when the ownership table routes the
   change there.
3. Commit and push the exact candidate, then use the surviving exact-SHA CI and
   review evidence. Do not rerun broad green gates solely as ceremony.
4. Use `verify-all.sh` for deliberate repository-wide changes, releases, or a
   specific integration question—not every local edit.

All workflows cancel an older in-progress run for the same workflow/ref when a
newer revision arrives. Rust-compiling workflows share a toolchain- and
lockfile-sensitive Cargo cache through `Swatinem/rust-cache`; Cargo fingerprints
remain the authority for whether restored output is reusable.

Retained duplication must represent an independent boundary. In particular,
provider and renderer gates both inspect some Rust render structures, and
renderer and Studio gates both compile Studio-consumed renderer packages. Those
overlaps prove different public consumers and must not be collapsed merely to
improve timings.
