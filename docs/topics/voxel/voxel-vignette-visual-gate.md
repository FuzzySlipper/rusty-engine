# Voxel vignette visual gate

This focused report records the first user-facing visual decision gate for Den
#6925. It is an isolated `render/voxel-vignette-playtest` product, not a
durable asset admission profile or an Engine gameplay feature.

## Verdict and route disposition

The corrected ordinary-mesh route passed a bounded real-Chromium visual check:
the terrain is horizontal, and the shrine, tree, and door are upright, visibly
palette-coloured, and grounded. The owner recorded visual acceptance of this
palette/geometry route in Den message `30143`; the scene correction resolved
only its orientation integration bug.

The route intentionally remains temporary. Each static GLB is admitted by the
public `@rusty-engine/application-host` `defineAnimatedMesh` operation with an
empty clip list and `defaultClip: null`; no private renderer or Three import is
used. It demonstrates ordinary voxel-authored GLB meshes only.

| Candidate route | Disposition | Exact missing or temporary boundary |
| --- | --- | --- |
| Palette-unlit ordinary GLB mesh | Visually passed, temporary | Uses zero-clip animated-mesh admission; no canonical public static-GLB profile exists. |
| Accepted run-003 VOX/reference JSON as runtime voxel object | Unavailable | The accepted `.vox`/`-voxel.json` inputs are not canonical Engine packed voxel-object data, and `application-host` has no resource route for them. Terrain has no paired `-voxel.json`. |
| Styled conventional mesh comparator | Absent negative evidence | No accepted styled conventional comparator artifact was supplied; this gate does not fabricate one. |
| Collision/walkability | Unavailable | No collision proxy was supplied or admitted. The controls are deliberately free-flight. |

The focused successor should be a neutral public static-GLB admission/profile
decision (including the producer's `KHR_materials_unlit` presentation
requirement), rather than extending this comparison product. Runtime voxel
admission and a conventional comparator each need their own accepted inputs and
owner-bound task.

## Exact inputs and staging

The checked input manifest is
[`render/voxel-vignette-playtest/staging-manifest.tsv`](../../../render/voxel-vignette-playtest/staging-manifest.tsv).
`scripts/stage-voxel-vignette-playtest-assets.sh` copies only after matching
the source SHA-256/length and verifies the staged copy again. Staged binaries
are ignored and remain local experimental visual inputs; no licence/source
envelope was recorded.

| Asset | SHA-256 | Bytes | Primitives | Triangles | Materials | Embedded textures |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `terrain-128-palette-unlit.glb` | `254c7ed1d5376d3fbeaff1239a9125b769dbf3d1182e349956c69d7584c30c1a` | 1,067,908 | 3 | 15,370 | 4 | 1 |
| `shrine-nano-128-palette-unlit.glb` | `9f97e2700962191ea015f87bb1f63670919581e3cf9f32807a72ca585ad563f5` | 14,260,896 | 255 | 203,606 | 255 | 1 |
| `gpt-tree-128-palette-unlit.glb` | `cc5b1f15c31dbf114717c5f078637ef84752224f9f32734da7abc0794c595460` | 13,374,324 | 255 | 190,502 | 255 | 1 |
| `door-t3-128-palette-unlit.glb` | `f3c8c66bc7b97bfbccdf19452b8042589b13209614c3e8a53098a4f5c32e03b5` | 5,856,864 | 255 | 81,944 | 255 | 1 |
| **Total** | **four checked resources** | **34,559,992** | **768** | **491,422** | **769** | **4** |

The source and staged output hashes are identical by that script. Its default
source is `/home/dev/asset-pipeline/live-evidence/palette-unlit-6925-20260824-001/`;
`VOXEL_VIGNETTE_ASSET_SOURCE_ROOT` is the documented override for an explicit
equivalent producer output. On a missing source it names that stable location
and `/home/dev/asset-pipeline/experiments/voxels/palette-unlit/README.md`, whose
producer command is `node experiments/voxels/palette-unlit/palette_unlit_glb.mjs`.
The producer tool is committed on Asset Pipeline `main` at
`2d1b00bd805ba55a1bb4ac3e2b3f501334ca5cbc`; the staging script does not
generate assets. The producer receipts label the inputs
local experimental palette-unlit derivatives and retain the original
BIN/accessors/bufferViews/meshes/nodes. The source gallery run is
`voxel-vignette-128-20260823-003`. Its layout file retains stale
`vignetteId: 002`; this gate presents `run 003`. The source layout's normalized
`1/128` display scale is not the per-object producer `voxelSize: 0.2`
convention, so neither is silently treated as world authority.

## Scene correction and reproducible observation

The assets have identity roots and centered mesh translations. Terrain is the
local-Z-up exception and retains `-90°` about X, scale `[16/128, 16/128,
1/128]`, and translation `[0, 1/128, 0]`. Shrine, tree, and door retain identity
rotation, uniform `1/128` scale, and derived vertical lifts `64/128`, `51/128`,
and `64/128` respectively. Their layout positions are `[0, 0, 0]`, `[-4, 0,
5]`, and `[4, 0, 6]`; no object-specific visual hand tuning or mirroring was
used.

The observed world bounds were terrain `x[-8,8], y[0,0.0234375], z[-8,8]`,
shrine `x[-0.3984,0.3906], y[0,1.0078125], z[-0.28125,0.2891]`, tree
`x[-4.4141,-3.5938], y[0,0.796875], z[4.5,5.5078]`, and door
`x[3.6016,4.3984], y[0,1.0078125], z[5.8281,6.1797]`.

The evidence run used Chromium `148.0.7778.215` on Arch Linux in its broker
virtual display, via Vite `8.1.5` / Node `v26.2.0`. Navigation returned HTTP
200 with approximately 197 ms navigation, DOMContentLoaded, and load timing.
One click captured the Engine canvas; a two-second `W` hold and pointer-look
produced a close shrine view without an observed failure. The initial and
after-input screenshots are indexed in the product-playtest run
`rusty-engine-voxel-vignette-playtest-20260824T021619.743265738Z-1594618`.
This is browser evidence, not physical-display certification.

`application-host` publicly reports admission resource count and bytes, hence
the four-resource / 34,559,992-byte total above. It does not expose admission
duration separately from browser navigation, nor live renderer draw calls,
material/resource residency, triangle counters, GPU memory, or GPU frame
timers. The private Three `webgl.info` path was intentionally not inspected.
The bounded moving-camera sample is therefore a behavioural frame check, not a
numeric frame-time claim. The accepted composition has no repeated instance;
public host readouts do not establish resource sharing, so no sharing claim is
made.

The product calls the public host `dispose()` once on `pagehide`; its UI owner
cancels its animation frame and removes input listeners. The evidence browser,
driver, and virtual display were then cleanly stopped. That establishes bounded
host/UI teardown only, not a renderer-residency metric.

## Reproduction and provenance

Engine base: `88d05ebd81e20fcf9e78868a8f9651edd3bc2e63`
(`codex/task-6925`).

```bash
cd /home/dev/worktrees/rusty-engine-6925
./scripts/stage-voxel-vignette-playtest-assets.sh --check
pnpm --dir render run check:voxel-vignette
pnpm --dir render run typecheck
den-serve up rusty-engine-voxel-vignette -repo "$PWD/render/voxel-vignette-playtest"
```

If the stable evidence directory has not yet been copied, pass the explicit
producer-output root without changing the script:

```bash
VOXEL_VIGNETTE_ASSET_SOURCE_ROOT=/absolute/palette-unlit-6925-20260824-001 \
  ./scripts/stage-voxel-vignette-playtest-assets.sh --check
```

The first correction check used that override while the producer was still in
its task worktree. The final check passed through the stable default above
after the producer tool was promoted and its ignored local evidence copied to
the stable Asset Pipeline checkout.

The live comparison URL is
`http://192.168.1.22:37100/voxel-vignette-playtest/`. Controls are click canvas,
WASD free-flight, pointer-look, and Escape to release. The viewport HUD keeps
the route, caveats, run identity, controls, ready state, and bounded failures
inside the canvas presentation area.
