# Rendering successor contract

Status: implemented and in closeout review under Den tasks #6156-#6163

Rusty Engine will preserve the complete proven Asha rendering feature set behind a new
successor-owned border. This is not a promise to retain Asha package names or dependency topology.
It is a promise that implemented rendering behavior will not disappear merely because the first
Rusty Engine demo used only a narrow subset.

## Historical donor

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

[`../render/donor-disposition.tsv`](../render/donor-disposition.tsv) gives every one of those 134
paths exactly one final `adapted` or `equivalent` disposition, capability owner, local successor
evidence, and representation note. The strict completeness gate proves the inventory and
disposition path sets are identical and every named capability/evidence path exists.

## Ownership

```text
downstream game or Studio runtime owners
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

- Downstream product owners own gameplay meaning, current state, and the choice to emit a
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

## Platform posture

This contract is host neutral even though its current complete realization uses Three/WebGL and a
browser/webview. The layers have different portability obligations:

- Rust render values and projection are independent of JavaScript, DOM, WebGL, and HTTP.
- `render-contracts` and TypeScript `render-projection` are backend neutral and can run headlessly
  without Three or DOM.
- `renderer-three` owns the Three/WebGL backend. Its browser-surface and editor-viewport modules are
  explicit current-host adapters, not requirements imposed on renderer-neutral consumers.
- `renderer-host` owns current DOM, WebAudio, browser input/lifecycle, overlay, inspection, and tool
  composition.

Resources enter through explicit frame descriptions and caller-owned byte/resolver capabilities;
arbitrary URL fetching, same-origin behavior, HTTP routing, and browser storage are not part of the
render contract. Chromium is the current real-host acceptance substrate. It proves this adapter,
including behavior that cannot be established headlessly, but does not make Rusty Engine a
network-delivered web-game platform. Rich-DOM browser, Tauri, and Electron products use the single
bundled application host, while Rust-only products without a product DOM use the fixed Rust webview
adapter. Process topology and transport remain downstream shell choices, and a future backend may
reuse the renderer-neutral border without preserving Three internals. The current
in-process-versus-sidecar default is in the
[downstream renderer and Studio boundary](topics/development/downstream-renderer-and-studio.md).

The repository-wide placement and validation rules are in Den ADR
`rusty-engine/host-platform-and-browser-validation-boundary`.

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

Every family is now `adapted` or `equivalent` with concrete successor evidence. `planned`,
`excluded`, and `unmapped` are forbidden by the strict closeout gate.

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
downstream projects do not install, import, build, or configure this workspace. They submit Rust
retained facts through the Engine-owned renderer/webview or application-host boundary.

When a selected downstream checkout needs focused proof, use the explicit integration gate and keep
the exact Engine and consumer heads in Den task/review evidence. That evidence does not become a
package pin or a freshness contract, and ordinary Engine CI does not launch every downstream suite.

The dependency-free root `package.json` and `pnpm-lock.yaml` are package-manager selection metadata
for that Git preparation path. They do not include the render workspace in ordinary Engine work,
install browser dependencies, or change the Rust-first root verification gate. All actual Node
dependencies, scripts, source, and lock state remain under `render/`.

`rusty-engine-demo` is the first migration consumer. The exact commits in
the migration record below are historical certification evidence for its
renderer-boundary transition. Studio is an Engine-owned host, and downstream
projects must not implement another viewport renderer.

## Implemented TypeScript border

The isolated workspace now exposes four layers with one-way dependencies:

- `@rusty-engine/render-contracts` mirrors the complete Rust retained and presentation frames and
  strictly decodes unknown JSON before it reaches mutable renderer state;
- `@rusty-engine/render-projection` applies whole retained frames fail-atomically with copy-on-write
  staging for changed records and structurally shared immutable definitions, and exposes a
  backend-neutral scene/resource readout; and
- `@rusty-engine/renderer-three` owns Three objects, shared-buffer borrows, GLB resources, material
  realization, texture/atlas retention and UV projection, sprites, lights, picking, browser
  mounting, and editor viewport primitives; and
- `@rusty-engine/renderer-host` owns the shared game/tool browser surface, explicit presentation
  host set, animated resource admission, WebAudio, DOM billboards, bounded particles, telemetry,
  stored/interactive cameras, editor channels, and inspection surfaces.

Every `u64` that crosses the JSON/JavaScript border is constrained to the exact integer range
`0..=2^53-1`. Rust rejects unsafe handles, resource ids, source identities, seeds, revisions, and
ticks before encoding; the TypeScript decoder repeats that check for untrusted external payloads.
Shared mesh bytes arrive through a narrow borrow-copy-release provider and are copied immediately
into renderer-owned arrays. It is not a general bridge and cannot expose gameplay mutation.

The browser acceptance fixture fetches the repository-local CC0 animated GLB and exercises group
and primitive nodes, inline static meshes, named controller-driven animation, retained materials
and texture/atlas descriptions, sprite UVs, lights, world projection, picking, WebAudio decode and
resume, billboards, particles, telemetry, an inspection grid, and disposal through real DOM and
WebGL contexts. Node tests retain the more exhaustive error, fallback, replacement, channel,
resource, camera, and lifecycle matrix.

The retained layer set also includes `viewmodel`, a renderer-neutral camera-relative presentation
channel. It reuses the same retained hierarchy and primitive/static/animated/voxel/sprite resource
operations. It is not a gameplay, weapon, input, camera, or generic UI model. The neutral projector
admits at most 128 live channel nodes and 16 distinct referenced assets, bounds local asset
coordinates and translation components to `+/-16`, rotation components to `+/-1`, and local scale
components to `+/-64`.
Oversized frames, transforms, assets, and retained channel lights reject before either neutral or
Three state commits.

The browser backend owns the realization: a separate retained Three scene, a fixed camera sharing
the world projection/aspect, one animation advance, world render, depth clear, then viewmodel
render. The ordinary world camera remains caller-owned. The viewmodel scene is not traversed by
picking, and both scenes are released by the same surface disposal. Editor viewport channel
policies continue to admit only their documented scene/debug layers; they do not silently turn
product viewmodels into authored editor content.

That repository-local fetch is fixture delivery inside the current browser adapter, not the public
resource model. Consumers provide explicit resource resolvers or bytes; an Engine capability must
not require an HTTP server or same-origin asset path.

## CI boundaries

- **Engine:** Rust formatting, metadata, unit/integration tests, Clippy, standalone audit, and
  render completeness format. No Node installation.
- **Render:** frozen pnpm install, boundary checks, typecheck, unit/golden tests, build, and real
  Chromium/WebGL coverage for the current backend and browser/webview host. Triggered by `render/**`
  and named Rust render contract/fixture paths.
- **Demo consumer:** exact-revision install plus full demo browser acceptance when shared render
  surfaces or demo integration change.
- **Public package consumer:** post-push clean temporary install of all four Git subdirectories at
  the exact public Engine SHA, followed by coherent retained-frame execution.
- **Studio:** remains separate and begins only after the rendering clean-clone closeout.

## Completion rule

Textual similarity is not the metric. The donor inventory, capability matrix, adapted tests,
cross-language fixtures, demo migration, and browser evidence together prove preservation. Any
representation change must name the replacement behavior. Nothing may be deferred merely because
no current demo screen exercises it. Operational commands and intentional limitations are in
[rendering-operations.md](rendering-operations.md).

Closeout also requires platform dependencies to remain in their owning layers: renderer-neutral
packages must not acquire Three/DOM/HTTP assumptions, and browser convenience must not create a new
gameplay or product authority path or synthetic product API.
