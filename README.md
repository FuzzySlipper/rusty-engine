# Rusty Engine

Rusty Engine is a standalone provider for object-centric games. It owns reusable entity, spatial,
collision, navigation, voxel, mesh, asset, offline-conversion, and shared rendering mechanisms. It
does not own a game runtime, project schema, browser shell, or game-specific presentation policy.

The loading-bay walking product that established these boundaries now lives in
[`FuzzySlipper/rusty-engine-demo`](https://github.com/FuzzySlipper/rusty-engine-demo). That project
depends one-way on an exact public Engine revision; Engine has no build, runtime, or checkout
dependency on the demo.

## Provider boundary

```text
downstream game policy and orchestration
             |
             +--> entity-state
             +--> engine-spatial --> core-* / svc-*
             +--> voxel-asset
             +--> render-model --> render-projection ----+--> retained JSON
                              \-> render-presentation ----+

offline authoring input --> voxel-convert --> canonical voxel-asset JSON

isolated render workspace --> retained TS projection --> Three/host/editor surfaces
```

The important ownership rule is mechanism here, game meaning downstream. `entity-state` provides
typed entity capabilities and an atomic mutation boundary. `engine-spatial` composes one canonical
voxel authority with derived collision, navigation, mesh, motion, and edit operations. The smaller
`core-*` and `svc-*` crates remain independently useful implementation layers. `voxel-asset` and
`voxel-convert` define a strict durable artifact and a bounded offline producer. `render-model` and
`render-projection` provide the complete retained-scene border and fail-atomic adapters.
`render-presentation` provides bounded animation, audio, billboard, particle, and telemetry
mechanisms with no gameplay or renderer authority. The separately gated `render/` workspace is
shared by downstream demo and Studio consumers.

The implemented ownership and promotion rules are in [docs/design.md](docs/design.md).

## Repository map

| Path | Responsibility |
|---|---|
| `rust/crates/entity-state` | Reusable entity capabilities, views, snapshots, and atomic invariant changes |
| `rust/crates/engine-spatial` | Canonical voxel scene and derived collision, navigation, mesh, motion, and edit services |
| `rust/crates/voxel-asset` | Strict canonical voxel-volume asset and conversion-input vocabulary |
| `rust/crates/voxel-convert` | Bounded offline GLB conversion and atomic artifact installation |
| `rust/crates/render-model` | Complete versioned retained-frame vocabulary and validation |
| `rust/crates/render-projection` | Entity, authored, voxel, lighting/material, and debug projection |
| `rust/crates/render-presentation` | Animation controllers and disposable audio, billboard, particle, telemetry projection |
| `render` | Isolated TypeScript retained projection, Three backend, and renderer hosts |
| `studio` | Isolated Angular/Nx authoring product over a project-owned Rust adapter and the shared renderer |
| `rust/crates/core-*` | Small identity, math, time, space, voxel, and asset-reference foundations |
| `rust/crates/svc-*` | Focused volume, spatial, collision, pathfinding, RNG, and mesh mechanisms |
| `content/conversion` | Checked generic conversion request |
| `content/assets` | Reproducible canonical voxel artifact |
| `fixtures` | Repository-local provider fixtures, including the licensed Kenney source |

## Verify

Only a Rust toolchain and ordinary shell utilities are required for the provider gate:

```bash
./scripts/verify.sh
```

The gate checks formatting, locked Cargo metadata, standalone paths, documentation links, all
workspace and provider-fixture tests, Clippy with warnings denied, and the converter's byte-for-byte
reproducibility test. Install and verify the isolated renderer workspace separately with:

```bash
./scripts/verify-render.sh
```

That gate checks package boundaries, strict Rust-to-TypeScript decoding, retained projection,
Three resource lifecycle, renderer hosts/editor/inspection, deterministic snapshots, and the real
Chromium/WebGL/WebAudio/DOM/GLB path. Ordinary Engine verification deliberately does not install
Node dependencies. Exact public Git-package consumption has its own clean temporary-consumer proof:

```bash
./scripts/verify-render-consumer.sh <40-character-public-sha>
```

Studio has its own install and gate, so ordinary provider work still resolves no Angular/Nx or
Playwright dependencies:

```bash
pnpm --dir studio install --frozen-lockfile
./scripts/verify-studio.sh
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

The integration gate selects the external consumer explicitly and CI checks out the exact revision
recorded in `studio/demo-consumer-source.json`; Engine never inspects a sibling demo implicitly.

Run the focused Rust checks directly with:

```bash
./scripts/audit-standalone.sh
./scripts/check-doc-links.sh
cargo test --workspace --locked
```

## Offline voxel conversion

Regenerate the checked generic artifact with:

```bash
cargo run -q -p voxel-convert --bin voxel-convert -- \
  --request content/conversion/kenney-wall-a.request.json \
  --source fixtures/voxel-conversion/kenney-wall-a.glb \
  --output content/assets/kenney-wall-a.voxel.json
```

The format, limits, provenance, and failure behavior are documented in
[docs/voxel-asset-format.md](docs/voxel-asset-format.md).

## Documentation

- [Current design](docs/design.md) — provider ownership, dependency direction, and promotion rules.
- [Rendering successor contract](docs/rendering-successor-contract.md) — complete rendering scope,
  ownership, adaptation, and closeout rule.
- [Shared rendering operations](docs/rendering-operations.md) — verification, exact-revision
  consumption, CI topology, resource rules, and known limitations.
- [Studio migration contract](docs/studio-migration-contract.md) — first-party authoring scope,
  owner adoption, isolation, parity accounting, and deliberate topology exclusions.
- [Studio adapter protocol](docs/studio-adapter-protocol.md) — closed project-owned Rust operations,
  optimistic guards, and cross-repository acceptance.
- [Rust source organization](docs/rust-style.md) — lightweight module and behavior-owner style.
- [Voxel asset format](docs/voxel-asset-format.md) — current durable format and converter boundary.
- [Migration cluster ledger](docs/migration-cluster-ledger.md) — durable successor and extraction decisions.
- [Experiment results](docs/experiment-results.md) — historical walking-product evidence and measurements.
- [Donor provenance](docs/donor-provenance.md) — exact source revisions, adaptations, exclusions, and licenses.
- [M9 extraction contract](docs/m9-extraction-contract.md) — historical standalone-repository closure.

Rusty Engine is canonical for these provider crates. Asha remains historical evidence and a source
locator, not a compatibility, build, runtime, or planning authority.
