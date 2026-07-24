# Rusty Engine

Rusty Engine is a standalone, object-centric gameplay runtime. It is the canonical successor to
Asha's gameplay/runtime spine, built around direct Rust services and systems instead of universal
contracts, replay-driven execution, strict ECS indirection, or a second script authority.

The repository installs, builds, tests, and runs without an Asha checkout.

## Architecture

Rust owns live entities, components, gameplay services, scheduling, spatial authority, persistence,
and typed committed outcomes. Components remain mostly data, while named feature modules make the
responsible behavior owner easy to locate.

TypeScript has two bounded roles:

- optional code-as-content composition that emits strict project data admitted by Rust; and
- browser presentation over accepted readouts and facts through a small typed render-diff border.

```text
TypeScript content -> stored project -> Rust admission -> GameRuntime / GameSession
                                                        |
resolved input -> named Service or System -> accepted state and typed facts
                                                        |
                                                        v
                              browser adapters -> Three / DOM / Web Audio
```

The implemented architecture, ownership rules, data lifecycles, and extension pattern are in
[docs/design.md](docs/design.md).

## Implemented product surface

The current engine proves a connected gameplay and content path rather than isolated framework
pieces:

- object-centric entities with transform, collision, rendering, kinematic, and game-specific
  component families;
- switches, doors, timed intents, encounters, health, weapons, and consequential defeat routing;
- collision-aware player control, autonomous navigation, and centrally scheduled kinematic motion;
- one canonical material-voxel authority feeding collision, navigation, visible-face meshing, live
  expected-revision edits, snapshots, and authored saves;
- strict versioned project admission, explicit migration, canonical atomic storage, and runtime
  snapshots kept separate from authored content;
- deterministic offline GLB-to-voxel conversion through the ordinary project asset path; and
- a retained Three/WebGL browser product with derived camera, typed projection, rebuildable posture,
  and disposable animation, audio, particle, and billboard feedback.

The browser proof loads the authored loading bay, drives real player input and combat, routes
enemies around generated obstacles, clears the encounter and exit, edits voxel geometry, and checks
restart-safe presentation behavior. A second product path loads the converted Kenney wall, proves
visible collision and navigation behavior, edits it through the same voxel service, and reopens the
authored result without the converter.

## Repository map

| Path | Responsibility |
|---|---|
| `rust/crates/entity-state` | Reusable entity capabilities and atomic invariant changes |
| `rust/crates/engine-spatial` | Canonical voxel scene and derived collision, navigation, mesh, motion, and edits |
| `rust/crates/game-host` | Game components, services, systems, orchestration, admission, and persistence |
| `rust/crates/voxel-asset` | Strict canonical voxel asset format |
| `rust/crates/voxel-convert` | Offline bounded GLB conversion |
| `rust/crates/core-*` | Small shared identity, math, time, space, and voxel value crates |
| `rust/crates/svc-*` | Low-level volume, spatial, collision, navigation, RNG, and mesh services |
| `ts/packages/project-content` | Optional TypeScript project composition |
| `ts/packages/render-contracts` | Bounded typed render-diff vocabulary |
| `ts/packages/renderer-three` | Retained Three/WebGL implementation |
| `ts/packages/browser-shell` | Browser input and disposable presentation |
| `content` | Checked projects, generated content, conversion requests, and voxel artifacts |
| `fixtures` | Repository-local licensed product and test inputs |

## Install and verify

Install the pinned JavaScript dependencies and run the complete repository gate:

```bash
pnpm install --frozen-lockfile
pnpm run verify
```

The verification script runs the standalone dependency audit, TypeScript checks and tests, content
reproducibility, renderer and shell builds, the Rust workspace tests, clippy with warnings denied,
and the real Chromium product smoke.

Run the independence audit directly with:

```bash
pnpm run audit:standalone
```

## Run the product

Build the browser shell and start the Rust host:

```bash
pnpm run build:shell
cargo run -q -p game-host --bin browser-host
```

Then open `http://127.0.0.1:37881`.

The focused headless examples are:

```bash
cargo run -q -p game-host --bin headless-door
cargo run -q -p game-host --bin headless-encounter
```

## Authoring and workloads

Regenerate checked project content after changing its TypeScript composition:

```bash
pnpm run generate:content
```

Run the bounded release workload matrix and voxel measurements with:

```bash
cargo run --release -q -p game-host --bin motion-workload -- --matrix
cargo run --release -q -p game-host --bin voxel-edit-workload -- 256
cargo run --release -q -p voxel-convert --bin voxel-conversion-workload -- 256
```

The direct static-asset conversion command and format contract are in
[docs/voxel-asset-format.md](docs/voxel-asset-format.md).

## Documentation

- [Current design](docs/design.md) — authority, execution, persistence, presentation, and extension
  rules.
- [Experiment results](docs/experiment-results.md) — implementation evidence, measurements,
  rejected alternatives, and remaining limits.
- [Migration cluster ledger](docs/migration-cluster-ledger.md) — completed capability sequence,
  inheritance policy, and deliberately deferred work.
- [Donor provenance](docs/donor-provenance.md) — exact source revisions, paths, adaptations,
  exclusions, and licenses.
- [M9 extraction contract](docs/m9-extraction-contract.md) — the audited standalone-repository
  closure.
- [Rust source organization](docs/rust-style.md) — lightweight feature ownership and module style.
- [Voxel asset format](docs/voxel-asset-format.md) — durable format and offline conversion boundary.

## Provenance and history

The accepted low-level Rust closure, narrow TypeScript render edge, Kenney fixture, and exact CC0
license are local to this repository. Asha is historical implementation evidence and a source
locator, not build, runtime, package, or planning authority.

Rusty Engine began as an architecture falsification spike. An early comparison also implemented
trusted executable TypeScript gameplay through a batched native host. It demonstrated that changing
languages did not remove the lifecycle and state-ownership cost of a second runtime authority. That
implementation is preserved at Git tag `external-ts-runtime-spike`; it is intentionally absent from
active `main`.
