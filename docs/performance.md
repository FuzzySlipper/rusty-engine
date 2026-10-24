# Performance diagnostics

Rusty Engine keeps one checked performance probe that divides a frame-sized
operation into independently attributable layers:

- `rust-appearance-call-stage` measures beginning and ending a Rust graphics
  call with an 8 MiB retained renderer resource. A call owns the state for its
  duration and never copies it.
- `managed-csharp-update` measures a stable allocation-free managed update
  without Rust or browser work.
- `csharp-rust-crossover` measures generated CoreCLR and NativeAOT callbacks,
  the Engine service call, and output conversion.
- `product-host-http` adds the local product-host HTTP admission path: with
  time held, each sample is one `engine.time.advance` request admitting one
  step.

Run the complete probe from the repository root:

```sh
./scripts/run-performance-regression.sh
```

Each result is printed as a single `RUSTY_PERF` JSON record. Override the
crossover sample count with `RUSTY_PERF_ITERATIONS` (1 through 256).

The crossover and HTTP lanes use the same canonical staged Product under both
CoreCLR and NativeAOT. `fixtures/csharp-crossover-performance` consumes the
public SDK staging targets with an explicit Engine contributor override. The
runner packs the current SDK, stages one Release bundle for both loaders
through `StageRustyEngineCombinedProduct`, and launches the current Rust host
with `--product`, each `--loader` and `--performance-probe`. This measures one
small one-step UI publication; it is not a full game or graphics workload.
Loader, runtime and workload identity keep the two lanes distinct.

The runner needs an existing runtime pack for its browser shell, at
`target/runtime-pack/linux-x64` (the `scripts/build-runtime-pack.sh` default)
or at `RUSTY_PERF_RUNTIME_PACK`. It copies that shell into a disposable layout
beside the current source host. The probe draws nothing and does not execute
browser JavaScript. No release pair is published.

## Interpreting regressions

Compare records from the same machine and build mode. There is intentionally
no universal millisecond gate: a CI runner and a developer workstation are
different performance classes.
Use the first lane that regressed to localize investigation:

1. Rust staging points to Engine call-state or resource ownership work.
2. Managed C# points to downstream-style update cost or managed allocation.
3. Crossover points to generated ABI, lifecycle, or Engine service commit cost.
4. HTTP points to product-host transport or synchronization.

## Renderer statistics

The probe is deliberately bounded and synthetic. On a running product started
with `--live-debug`, run `engine.renderer` in the live-debug panel or
`rusty-live-debug --origin ORIGIN --command engine.renderer`. It prints the
runtime renderer's adapter and output (`stream` or `window`), retained
operations skipped by kind, and, for streamed frames, the recent frame rate,
median render, readback and encode milliseconds, and bytes per frame and per
second. For the desktop window, `window` gives the recent presented frame
rate, the median milliseconds spent acquiring, waiting for the scene, drawing
and presenting, and `shown`: each recent frame's presentation time (Unix
milliseconds), the step it showed and the `presentationSeconds` it drew at
([presentation time](csharp-lifecycle.md#presentation-time)). While the world
runs, `presentationSeconds` should advance every frame, even across frames
that show the same step.
`settings` gives the renderer settings in effect: what the product or its
manifest requested, what draws, and why each refused setting differs
([renderer settings](lighting-and-sky.md#renderer-settings)). Of its two
pipeline switches, clustered lighting pays once many ranged lights are in
view at the same time (with few, the light loop is as fast or faster), and
GPU culling buys nothing while the CPU draw list is cheap, which it is up to
a few thousand parts in view.
Where the adapter has timestamp queries, `gpu.passes` gives the median GPU
milliseconds of each renderer pass that is timed, in the frames that draw it,
with the adapter's compute limits beside them:
- the compute passes: `gpu-cull`, `occlusion`, `scatter-select`,
  `light-clusters`, `volumetric-fog`, the ambient occlusion passes
  (`ao-prepass`, `ao-occlusion`, `ao-blur`) and a frame's share of building
  [the sky's light](lighting-and-sky.md#the-skys-light) (`sky-light`);
- the frame's shadow-layer passes together (`shadows`, and `backdrop-shadows`
  for [the backdrop](lighting-and-sky.md#the-backdrop)'s cascades);
- the first world view's `world` pass, with the
  [water](lighting-and-sky.md#water)'s copy of the opaque scene inside it
  (`water-copy`), and its `clouds`, `backdrop`, `particles` (soft sprites and
  billboards) and `precipitation` passes;
- its `bloom-exposure` passes (with sun shafts) together and its `finish`
  pass
  ([exposure, tone mapping and fog](lighting-and-sky.md#exposure-tone-mapping-and-fog));
- a product [image effect](lighting-and-sky.md#image-effects)
  (`image-effect`), or the `upscale` stage, which brings a frame drawn at a
  render scale below 1, or with
  [temporal anti-aliasing](lighting-and-sky.md#temporal-upscaling-taau) or
  [DLSS](lighting-and-sky.md#dlss), to the output's size.

`gpu.upscale` names that stage and the last frame's render and output
resolutions, and why DLSS is refused while asked for. `gpu.water.refused`
says why a device has no copy of the scene's colour for water, if it has
none. `gpu.decals` is the [projected decals](lighting-and-sky.md#projected-decals)
(their count, grid entries, busiest cell, bytes and last rebuild),
`gpu.wetness` the [wetness](lighting-and-sky.md#wet-surfaces) and
`gpu.snow` the [snow cover](lighting-and-sky.md#snow-cover) in force, whose
shading is part of the `world` pass. `gpu.indirectLight`
is [the probe volume](lighting-and-sky.md#indirect-light-the-probe-volume):
its probes, bricks and bricks pending, triangles, the last brick's and
batch's bake times, the last frame's upload bytes and the texture's bytes.

To measure the renderer's frame cost without the product, take a scene
snapshot on the running product (`engine.renderer.snapshot <path>`, see
[presentation capture](presentation-capture.md#scene-snapshots)) and draw it
again on a fresh renderer:

```sh
rusty-scene-render scene.rscene out.png --width 1280 --height 720 --frames 600
```

It prints the adapter, the time to open and apply the snapshot, the mean
and median milliseconds of the extra frames, each with readback, the
median draw calls, instances and CPU microseconds spent building draw lists
per frame, the `gpu` pass medians, and the renderer `settings` drawn (the snapshot's,
changed by the flags below) with what the adapter refused. `--ambient-occlusion off|compute|raster|field` draws the
snapshot with that ambient occlusion path (`field` cone-traces the voxel
chunks' distance fields), `--ambient-occlusion-strength S` and
`--ambient-occlusion-radius R` with that strength and radius, `--render-scale S` at that fraction of the
output size, and `--indirect-light cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]`
with a probe volume baked before the frames (reported under `gpu.indirectLight`), to
compare the paths on one scene. `--choose ID=VALUE` (repeatable) sets any
[video option](lighting-and-sky.md#video-options) as a player's choice, as
`--choose antialiasing=off` or `--choose shadows=true`: choices, like the
flags above, hold over the snapshot's own settings and over a product's
`RendererSettings.Set` recorded in it, and a snapshot's recorded player
choices apply first. `--motion OUT.png` draws the extra frames with their
[motion target](lighting-and-sky.md#temporal-inputs), as a temporal upscale
stage has them drawn, and writes the last frame's motion there; with
`--walk` or `--turn` it shows the camera's motion, and its `world` median
against a run without it is the target's cost. `--last-frame OUT.png`
writes the last extra frame instead, as a temporal stage shows it once it
has gathered the frames before it, and `timing` reports the frames' render
and output resolutions, their `upscale` stage and how often its history
started again. The image is the snapshot's
own view, drawn before the extra frames; the report's `camera` is that view. Compare
snapshots drawn on the same adapter; `WGPU_BACKEND` selects it as for any
wgpu program. A still camera reuses work a moving one repeats (culled lists,
a directional light's shadow cascades): add `--walk M` or `--turn D` to move
the snapshot's camera M metres forward or D degrees right before each extra
frame. The snapshot holds presentation time; `--advance S` moves it on S
seconds before each extra frame, from the snapshot's, so clouds drift and
wind sways (in the motion too) and every run draws the same moments. A still snapshot also renders its shadow layers only once:
`--rerender-shadows` renders them all in every extra frame, so `shadows` and
the frame times show the uncached cost. The report's `shadows` gives the
layers, pages, atlas bytes and the layers and casters the last frame rendered.

Trusted product code reads the same statistics (`ProductHostRendererStatistics`
in `product-host`) as UTF-8 JSON bytes through
`IEngineContext.Diagnostics.ReadRenderer()`. The runtime refreshes them once a
second; the generated binding copies the bytes before returning. Reading them
never draws a frame or synchronizes the GPU.

If the wgpu device is lost (a driver reset, say), `gpu.deviceLost` gives
wgpu's reason and the driver's message, and the host's stderr logs it once.
Nothing draws after that, but the runtime keeps stepping and answering: every
later wgpu call fails quietly. A mesh the device has no room for, out of
memory or larger than one buffer may hold, is refused: it draws nothing, and
its operation counts among those skipped, with the error as the last skip.
That holds for static, voxel and animated meshes, an instance's skinned
vertices and a mesh's morph targets; a skinned motion-history buffer without
room draws without motion history, and wireframe edges without room draw as
triangles.
Any other wgpu error is a renderer bug and panics.

## Feature gallery

How would this game look with a feature it leaves off? `rusty-scene-render
<snapshot> --gallery DIR [--width W] [--height H] [--frames N]` draws the
snapshot as it is, then once per renderer feature it leaves off or below its
top quality, each turned on at its Engine default:

- the [video options](lighting-and-sky.md#video-options) catalogue's
  features: render scale 1, 4× antialiasing, shadows, backdrop shadows, no
  shadow budget, each ambient occlusion mode not drawn, low volumetric fog,
  clouds and water reflections, clustered lighting, GPU culling and
  vegetation occlusion (vsync and occlusion strength and radius are tuning,
  not features);
- indirect light, when the scene has no probe volume: one 48×24×48 m around
  the camera, labelled as a stand-in, since a game places its own;
- everything on together.

The list comes from the catalogue: each entry's `gallery` experiment names
the values to try (in turn, each that would turn the feature on or up from
the scene's own; for alternative paths, such as the two occlusion modes, each
the scene is not on), the one "everything on" takes, a setting it needs on
(the shadow budget needs shadows), and what the scene must hold. Volumetric
fog lights the scene's fog, so a scene without any gets a stand-in haze,
labelled. Volumetric clouds draw only what a product sets up, so their image
is labelled "needs product setup". A feature added to the catalogue with its
experiment joins the gallery with no gallery change. Each variant renders in its own process (20 timed frames unless
`--frames` says otherwise), so its GPU timings are its own. `DIR` gets each
variant's image and report, `sheet.png` (every image labelled with its
frame-time change and the share of pixels it changed by more than 2 levels,
or "no visible change"), and `gallery.md` and `gallery.json` with the frame
times, the GPU passes that rose most, refusals and notes. A feature with no
visible change in a scene (GPU culling, clustered lighting, distance-field
occlusion in a mesh room) shows only its cost.

Live, `engine.renderer.gallery <directory> [width height]` snapshots the
current view into `<directory>/scene.rscene` and starts the same gallery with
the `rusty-scene-render` beside the host, writing its log to
`gallery.log`; the game keeps running while it draws.

## Repeatable baseline artifacts

The baseline tools retain raw samples and repeat runs. Measurements are
report-only; increases are candidates for investigation, not statistical proof
of a regression. There is no machine-independent CI timing gate, and no
baseline artifact is currently recorded in the repository.

CPU/service/voxel lanes (release builds):

```sh
./scripts/run-cpu-performance-baseline.sh /tmp/engine-cpu-baseline workstation-release
```

This repeats the service/C#/HTTP probes three times and adds three runs each
of deterministic 32³ sculpted and 40³/56³ noisy/carved DC meshes. The voxel
timing covers `mesh_scalar_samples` only; constructing the scalar field is
outside the timed region. Output geometry counts and seeds are retained. It
does not measure gameplay generation, erosion, or collision construction.

Compare artifacts from the same execution environment and workload:

```sh
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json
# Only after choosing a workload-appropriate policy:
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json --fail-percent 20
```

The comparison reports median/p95 changes and spread between complete runs.
A different environment label, host CPU/OS/Node, runtime or workload
configuration, or a missing lane produces an incompatible result, not a pass.
Exit 2 means incomplete/incompatible; exit 1 means the optional regression
policy failed (or the command failed). Without a threshold, compatible measured
increases remain report-only, exit 0. Repeat suspicious runs on an otherwise
idle host before selecting a threshold.

Artifacts identify the capture host's CPU/OS/Node and Git revision/dirty
state. Use a stable explicit environment label for each machine and build
configuration.
