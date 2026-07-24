# M9 extraction contract

This document freezes the only Asha Engine implementation closure that may enter Rusty Engine
during M9. It is an extraction contract, not permission to copy adjacent packages or recreate the
donor workspace. Rusty Engine remains the composition root and owns every gameplay, session,
content, persistence, and browser boundary.

## Source authority and audit method

- Donor repository: `git@github.com:FuzzySlipper/asha-engine.git`
- Pinned donor commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`
- Inspected local donor head: `6462a6de20d48ea1a3b7456826804bd9507860a5`
- Result: every accepted Rust crate, TypeScript package, donor-test fixture, Kenney fixture, and
  Kenney license named below is byte-unchanged between the pinned commit and the inspected head.

The audit covered tracked manifests and lockfiles, full Cargo metadata, pnpm's resolved local-link
readout, TypeScript source imports and package exports, fixture paths in code and data, executable
documentation commands, local scripts, and GitHub Actions. The machine-readable snapshot is
[`scripts/standalone-dependency-baseline.json`](../scripts/standalone-dependency-baseline.json),
checked by [`scripts/audit-standalone.mjs`](../scripts/audit-standalone.mjs).

At the start of M9 the audit records 55 distinct operational references. Historical references are
permitted only as exact normalized file-and-line records in the baseline; no whole document is
exempt. The baseline must only shrink as M9 proceeds; the final M9D operational baseline is empty.

```bash
pnpm run audit:standalone
pnpm run audit:standalone -- --print
```

The first command fails when the tracked source scan, Cargo local-package graph, or installed pnpm
local-link graph differs from the reviewed baseline. The second prints the observed operational set
for review; it does not rewrite the baseline.

## Pre-extraction operational inventory

| Surface | Current operational dependency | Why it is operational |
|---|---|---|
| Cargo | Twelve `../asha-engine/engine-rs/crates/...` workspace dependencies | `cargo metadata` resolves all twelve manifests outside this repository. |
| pnpm | Direct links to `@asha/contracts` and `@asha/renderer-three` | The installed package graph resolves both links into the sibling checkout. |
| TypeScript package closure | `@asha/render-projection`, `@asha/runtime-bridge`, `@asha/runtime-session`, and donor workspace build outputs | The linked renderer declares these packages; production source imports the projection and bridge, while the session import is test-only. CI installs/builds the donor workspace to make the links usable. |
| Fixtures | Kenney GLB/license paths in conversion code, tests, requests, artifacts, and project data | Conversion and deterministic regeneration read the sibling GLB; persisted provenance also records sibling-relative paths. |
| Donor crate tests | Three text/JSON fixtures plus one `svc-pathfinding` dev dependency on `svc-levelgen` | These are not in Rusty Engine's normal Cargo graph, but become relevant if donor crates become workspace members and their own tests run. |
| CI | Pinned Asha checkout, donor pnpm install, and donor package build | A fresh Rusty Engine checkout cannot currently verify by itself. |
| Executable docs | README sibling layout and the voxel-conversion source command | They instruct an operator to provide the sibling checkout. |

No local verification script directly invokes Asha. The operational dependency enters
`scripts/verify.sh` transitively through Cargo, pnpm/Vite resolution, and the conversion tests.

## Accepted Rust closure

Cargo's normal resolved local-package graph contains exactly these twelve donor packages. Package
names remain unchanged so existing successor crates do not acquire a compatibility facade merely
for extraction. M9 initially placed them in a visibly attributed family; after standalone
extraction was proven, they were normalized to ordinary `rust/crates/<package>` workspace members.
Successor crates continue to consume them through the root workspace dependency table.

| Package | Pinned source path | Normal local dependencies | Treatment | Required local test input |
|---|---|---|---|---|
| `core-assets` | `engine-rs/crates/foundation/core-assets` | none | Internalize unchanged | none |
| `core-ids` | `engine-rs/crates/foundation/core-ids` | none | Narrow to the consumed `EntityId`; exclude unused abstract/project/session/prefab IDs | none |
| `core-math` | `engine-rs/crates/foundation/core-math` | none | Internalize unchanged | none |
| `core-space` | `engine-rs/crates/foundation/core-space` | none | Internalize unchanged | `harness/fixtures/spatial-grid/conformance.json` |
| `core-time` | `engine-rs/crates/foundation/core-time` | none | Internalize unchanged | none |
| `core-voxel` | `engine-rs/crates/state/core-voxel` | none | Internalize unchanged | none |
| `svc-volume` | `engine-rs/crates/services/svc-volume` | `core-space`, `core-voxel` | Internalize unchanged | none |
| `svc-spatial` | `engine-rs/crates/services/svc-spatial` | `core-space`, `core-voxel`, `svc-volume` | Internalize unchanged | none |
| `svc-collision` | `engine-rs/crates/services/svc-collision` | `core-space`, `core-voxel`, `svc-spatial`, `svc-volume` | Internalize unchanged | none |
| `svc-pathfinding` | `engine-rs/crates/services/svc-pathfinding` | `core-math`, `core-space`, `svc-spatial` | Production source unchanged; narrowly adapt donor-only tests | `harness/fixtures/nav/generated-tunnel-path.snapshot.txt` |
| `svc-rng` | `engine-rs/crates/services/svc-rng` | none | Internalize unchanged | none |
| `svc-mesh` | `engine-rs/crates/services/svc-mesh` | `core-space`, `core-voxel`, `svc-spatial`, `svc-volume` | Internalize unchanged | `harness/fixtures/voxel-mesh/two-voxel-line.mesh.txt` |

The direct list is not treated as closure proof. The normal metadata graph confirms there is no
thirteenth local package. `svc-collision` additionally uses registry package `parry3d-f64 0.28.0`
(Apache-2.0). `core-space` donor tests use `serde` and `serde_json` (MIT OR Apache-2.0), which are
already normal Rusty Engine workspace dependencies.

`core-ids` is the only production value adaptation. Rusty Engine consumes only `EntityId`; the
donor's unused subject/process/mode/signal and project/session/prefab identity families would retain
old structural vocabulary without behavior. M9B keeps the exact established `EntityId` value,
formatting, ordering, and hash behavior and removes the unconsumed types.

`svc-pathfinding` has a donor-only dev dependency on `svc-levelgen`. That crate is outside the
accepted closure because it brings `core-events`, generation frames, replay/hash evidence, and
render summaries. M9B must replace only the affected generated-tunnel test setup with an equivalent
direct `VoxelWorld` fixture. It may retain the 204-byte path golden; it must not copy
`svc-levelgen`. This is the one approved source/test adaptation in the Rust family.

The three donor-test fixtures are internalized under the repository's ordinary `fixtures`
hierarchy, with their pinned source paths and hashes recorded in donor provenance:

| Fixture | SHA-256 |
|---|---|
| `spatial-grid/conformance.json` | `84667b10b625b1ac06e80c83140c39ab6e8c18ba0fdf34ffd541acc76f836dd2` |
| `nav/generated-tunnel-path.snapshot.txt` | `0ce28a96be1c5b48fa3ae5f184d268cafa534153da985b12761518648c92cf56` |
| `voxel-mesh/two-voxel-line.mesh.txt` | `c4a1cc5bfab948cc1aea56135b4f3f7dae43d593fbfc37eb7d6b614d974092cd` |

## Accepted TypeScript edge

The current package names overstate what the product consumes. Browser shell imports exactly:

| Current package/export | Runtime use |
|---|---|
| `@asha/renderer-three/backend` — `mountAshaRendererBrowserSurface` | Mount the retained Three/WebGL canvas surface and apply typed frames. |
| `@asha/contracts` — `entityId`, `renderHandle` | Construct branded render metadata and handles. |
| `@asha/contracts` — types `Geometry`, `Material`, `MeshPayloadDescriptor`, `RenderDiff`, `RenderFrameDiff`, `RenderHandle`, `RenderNode`, `Transform` | Type the successor-owned whole-state-to-diff adapter. |
| `@asha/contracts` — type `RenderFrameDiff` | Type the temporary runtime-bridge shim only. The shim is removed during extraction. |
| `@asha/render-projection` | No browser-shell export is consumed directly. |

The linked renderer's broad barrel also exposes tunnel, editor, static-room, animation, picking, and
encoded-frame conveniences. The mounted product path requires only the retained primitive/inline
mesh renderer and browser-surface behavior; lighting remains a small browser-owned scene rig rather
than a render-diff family.

M9C therefore performs a narrow successor fork rather than copying the donor package graph:

| Current package/surface | Classification | M9 destination and boundary |
|---|---|---|
| Generated `@asha/contracts` package | Replace | `@rusty-engine/render-contracts`, containing the render contract and only `EntityId`/`TagId` brands needed by it. Do not copy the generated protocol barrel, wire codecs, project/gameplay/replay contracts, compatibility metadata, or code-generation claims. |
| `@asha/renderer-three` retained renderer/browser family | Narrowly adapt | `@rusty-engine/renderer-three`; rename the public mounted surface, import the local render contract, and retain direct typed `applyFrame` plus real Three/WebGL behavior. |
| `@asha/render-projection` | Provenance-only | Browser shell consumes none of it. Its only route into the current backend is through unused generic/tunnel convenience exports; prune those exports rather than internalizing an unneeded package. Rusty Engine's existing `RuntimeProjectionAdapter` remains the explicit whole-state-to-diff owner. |
| `@asha/runtime-bridge` | Exclude | Remove encoded-frame decoding. Define any still-useful optional mesh-buffer view capability locally in the renderer package; typed direct frames remain the product border. |
| `@asha/runtime-session` and `game-workspace` | Exclude | They are donor test/package closure, not product behavior. Do not copy their tests, fixtures, declarations, or lifecycle vocabulary. |
| Renderer editor/static-room/tunnel helper modules and donor-wide goldens | Provenance-only | They are not imported by the product. Retain focused self-contained renderer tests and the real Rusty Engine Chromium proof instead of their project/runtime fixtures. |

Implementation tracing in M9C further confirmed that the donor animation, sprite, picking,
catalog/static-mesh, light-operation, and handle-buffer facilities have no Rusty Engine caller. The
successor fork therefore retains the algorithms behind the four operations actually emitted by
`RuntimeProjectionAdapter` (`create`, `update`, `destroy`, and inline `replaceMeshPayload`) rather
than preserving unused public surface area. This is a narrowing of the approved renderer family,
not an expansion of the donor closure.

The fork keeps `three 0.184.0` and `@types/three 0.184.1` as ordinary registry dependencies; both
declare MIT licensing. It does not vendor their implementation. The Vite runtime-bridge alias and
`renderer-runtime-shim.ts` become unnecessary and must be deleted.

## Product fixture and license

M9D internalizes only these two files from the voxel-conversion fixture family:

| File | Size | SHA-256 | Treatment |
|---|---:|---|---|
| `harness/fixtures/voxel-conversion/kenney-wall-a.glb` | 3,352 bytes | `6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00` | Copy unchanged to `fixtures/voxel-conversion/kenney-wall-a.glb`. |
| `harness/fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt` | 318 bytes | `3679c62e69e67da74fec17327635e67c92991ac82b0bdfcc203d8ecd473c016a` | Copy unchanged beside the GLB. |

The asset is Kenney Retro Urban Kit 2.0 under Creative Commons Zero (CC0). All request, artifact,
project, test, workload, and command paths move to the repository-local fixture and license. The
source hash, conversion settings, and produced voxel meaning remain unchanged. Source/license paths
are themselves canonical provenance, so the artifact content hash is expected to change when those
paths become local and must be regenerated and propagated exactly.

No other Asha voxel-conversion fixture, provider regression document, import service, protocol, or
tool is accepted.

## Attribution and licensing policy

The pinned Asha repository has no root `LICENSE`/`NOTICE`, the accepted Cargo manifests declare no
`license`, and the accepted TypeScript package manifests declare no `license`. M9 must not invent a
license assertion. This is a same-owner source transfer between the user's repositories; provenance
is preserved with donor commit, source path, copied/adapted classification, and local destination in
`docs/donor-provenance.md` and family READMEs.

Third-party registry dependencies retain their published licenses through Cargo/pnpm metadata and
lockfiles. The only copied third-party binary asset carries its exact adjacent CC0 license text.
If Rusty Engine later receives a repository-wide license, that is a separate owner decision and does
not replace these source and asset notices.

## Explicit exclusions

M9 may not copy or add a dependency on any Asha runtime facade, gameplay fabric, universal command
or event union, sim validator/applier/runner, replay/certification record, reaction/receipt frame,
project bundle/content provider, lifecycle/session host, protocol generator, native/runtime bridge,
registry, Studio/editor control plane, or the other crates/packages classified outside the tables
above. The portability report remains evidence; its other `Reference unchanged` or `Feature later`
rows are not part of this extraction contract.

## Slice gates

- **M9B / Rust:** the twelve accepted manifests resolve inside this repository; donor-focused tests
  run without `svc-levelgen`; Cargo metadata has no outside local package; the audit baseline removes
  all Cargo and Rust-source sibling entries.
- **M9C / TypeScript:** only local `@rusty-engine` render packages remain; renderer bridge/session
  imports, alias, linked packages, and donor build prerequisites are gone; the real Chromium gate
  still exercises Three/WebGL.
- **M9D / standalone:** the GLB/license and all operational paths are local, CI checks out one
  repository, the audit baseline is empty, and an isolated fresh clone passes the complete product
  verification without an Asha sibling or host-global links.

## Closeout result

M9B-M9D satisfy this contract. The operational baseline moved `55 -> 31 -> 17 -> 0`. The selected
GLB and CC0 text retain their pinned byte hashes. Local-path canonical regeneration retains source
SHA `6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00`, settings SHA
`98cb7d07a99015f5e759a39d89e77bb4f64cbdb0b3b5ed724bba9d35f95902ba`, eight cells, and four
sparse runs; its new provenance-sensitive content hash is
`086d81f12403192c6d7568289c2b47771741e5620a967e5b5fe5093fd5608ab7`.

Rusty Engine is the canonical repository. The pinned Asha repository and links in this document are
permitted immutable provenance, not operational dependencies or authority for future architecture.
