# Rendering successor contract

Status: approved implementation contract for Den tasks #6156-#6163

Rusty Engine will preserve the complete proven Asha rendering feature set behind a new
successor-owned border. This is not a promise to retain Asha package names or dependency topology.
It is a promise that implemented rendering behavior will not disappear merely because the first
Rusty Engine demo used only a narrow subset.

## Pinned donor

- Repository: `git@github.com:FuzzySlipper/asha-engine.git`
- Commit: `6462a6de20d48ea1a3b7456826804bd9507860a5`
- Commit date: 2026-07-23
- Donor worktree at audit: clean and equal to `origin/main`
- Repository license file: none at the pinned tree
- Licensed binary fixture: Kenney Animated Characters Retro GLB with its adjacent CC0 text

The frozen inventory in [`../render/donor-inventory.txt`](../render/donor-inventory.txt) contains
134 committed source, test, package, fixture, golden, manifest, and license files. Its sorted-list
SHA-256 is `99b33ece319e614695bd60c26f723aa0f5bdd48c83488dbd6d6dc4151b67b001`.
Only committed files at the pinned tree are donors. Caches, build outputs, evidence dumps, and local
untracked files are not migration input.

## Ownership

```text
downstream game or Studio Rust authority
  state + typed facts + explicit appearance/resource descriptions
                         |
                         v
             Rust render-model / render-projection
                         |
              versioned retained frames
                         |
                         v
        isolated TypeScript retained projection / Three host
                         |
       GPU, audio, DOM overlays, editor viewport, readouts
```

- Downstream Rust owns gameplay meaning, current authoritative state, and the choice to emit a
  visual/audio/effect intent.
- `render-model` owns renderer-neutral values, validation, retained operations, and stable JSON.
- `render-projection` owns deterministic handle allocation, change detection, entity/spatial
  projection, voxel meshes, lighting/material adapters, debug projection, and diagnostics.
- `render-presentation` owns bounded, fail-atomic animation, audio, billboard, particle, and
  telemetry projection mechanisms. These mechanisms retain presentation state only.
- `render/` owns the independently installed TypeScript contract decoder, retained scene,
  Three/WebGL backend, resource hosts, browser surface, inspection, and editor viewport.
- A renderer pick is a hint. The downstream authority must revalidate it before acting.
- A renderer readout is observation. It cannot mutate or certify gameplay.

## Complete behavior families

The machine-readable lifecycle is in
[`../render/completeness.tsv`](../render/completeness.tsv). The required families are:

1. retained handles, hierarchy, primitives, transforms, layers, metadata, and minimal updates;
2. inline/shared mesh payloads, static and animated mesh resources and instances;
3. materials, instance parameters, textures, sprite atlases, sprites, and UV behavior;
4. ambient, directional, point, and spot lights with explicit shadow degradation;
5. entity, scene-like authored input, voxel chunks/materials, debug primitives, and diagnostics;
6. renderer-neutral grids, viewport channels, camera state/transitions, projection, and picking;
7. animation graphs, parameters, transitions, blend resolution, playback projection, and hosts;
8. retained and one-shot audio, billboards, bounded particles, and telemetry overlays;
9. retained TypeScript application, Three resource ownership/disposal, browser/WebGL mounting,
   inspection, deterministic snapshots, and failure behavior; and
10. cross-language fixtures and real-browser acceptance.

Every family must end in `ported`, `adapted`, or `equivalent` with concrete successor tests before
#6163 can close. `planned` is allowed while the campaign is in progress. `excluded` and `unmapped`
are forbidden dispositions.

## Required adaptation

The following Asha structures are not rendering behavior and must not cross the border:

- RuntimeSession, RuntimeBridge, native bridge operations, provider/global registries, or browser
  provider attachment;
- replay records, certification hashes, reaction frames, decision receipts, and mandatory
  origin/causation/correlation matching;
- `core-state`, `core-scene`, `core-catalog`, project-bundle, level-generation control records, or
  a global protocol umbrella;
- fixed Asha code-generation destinations, compatibility manifests, generated-tunnel product
  claims, and proof/evidence catalogs; and
- renderer callbacks, animation sampling, audio completion, particle simulation, picks, camera
  movement, or editor previews feeding back into gameplay mutation.

Replacement rules:

- Asha catalog validation becomes a narrow immutable resolved-render-asset view. It preserves
  kind and content-hash checks without importing catalog authority.
- Asha scene/state projection becomes adapters over `entity-state`, `engine-spatial`, and explicit
  downstream appearance/light descriptions.
- Replay-certified animation inputs become ordinary typed controller inputs and transition facts.
  Deterministic transition behavior remains; replay bookkeeping does not.
- Native mesh buffer handles become inline bounded payloads or an explicit renderer resource
  provider. There is no general runtime bridge.
- Cross-language safety comes from versioned serde JSON, checked fixtures, decoder tests, and
  golden behavior—not an Asha-wide generated contract system.

## Package and gate shape

```text
rust/crates/render-model
rust/crates/render-projection
rust/crates/render-presentation

render/
  package.json
  pnpm-lock.yaml
  packages/render-contracts
  packages/render-projection
  packages/renderer-three
  packages/renderer-host
```

The Rust crates are ordinary provider mechanisms and run in `./scripts/verify.sh`. The `render/`
workspace owns Node, pnpm, Three, browser, and host dependencies behind `verify:render`. Ordinary
Engine verification never installs that workspace. Studio uses the repository-local packages;
external consumers use an exact published or exact-commit artifact rather than a sibling link.

`rusty-engine-demo` is the first migration consumer. It must delete its private render contracts
and Three backend after switching to the shared packages. Studio is the second consumer and must
not implement another viewport renderer.

## Implemented TypeScript border

The isolated workspace now exposes three layers with one-way dependencies:

- `@rusty-engine/render-contracts` mirrors the complete Rust retained and presentation frames and
  strictly decodes unknown JSON before it reaches mutable renderer state;
- `@rusty-engine/render-projection` applies whole retained frames fail-atomically and exposes a
  backend-neutral scene/resource readout; and
- `@rusty-engine/renderer-three` owns Three objects, shared-buffer borrows, GLB resources, material
  realization, texture/atlas retention and UV projection, sprites, lights, picking, browser
  mounting, and editor viewport primitives.

Every `u64` that crosses the JSON/JavaScript border is constrained to the exact integer range
`0..=2^53-1`. Rust rejects unsafe handles, resource ids, source identities, seeds, revisions, and
ticks before encoding; the TypeScript decoder repeats that check for untrusted external payloads.
Shared mesh bytes arrive through a narrow borrow-copy-release provider and are copied immediately
into renderer-owned arrays. It is not a general bridge and cannot expose gameplay mutation.

The browser acceptance fixture fetches the repository-local CC0 animated GLB and exercises group
and primitive nodes, inline static meshes, named animation playback, retained materials and
texture/atlas descriptions, sprite UVs, lights, world projection, picking, and disposal through a
real WebGL context. Node tests retain the more exhaustive error, fallback, replacement, channel,
and resource lifecycle matrix.

## CI boundaries

- **Engine:** Rust formatting, metadata, unit/integration tests, Clippy, standalone audit, and
  render completeness format. No Node installation.
- **Render:** frozen pnpm install, boundary checks, typecheck, unit/golden tests, build, and real
  Chromium/WebGL coverage. Triggered by `render/**` and named Rust render contract/fixture paths.
- **Demo consumer:** exact-revision install plus full demo browser acceptance when shared render
  surfaces or demo integration change.
- **Studio:** remains separate and begins only after the rendering clean-clone closeout.

## Completion rule

Textual similarity is not the metric. The donor inventory, capability matrix, adapted tests,
cross-language fixtures, demo migration, and browser evidence together prove preservation. Any
representation change must name the replacement behavior. Nothing may be deferred merely because
no current demo screen exercises it.
