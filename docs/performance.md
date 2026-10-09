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
second. The desktop window streams nothing, so it reports no frame timing.
`settings` gives the renderer settings in effect: what the product or its
manifest requested, what draws, and why each refused setting differs
([renderer settings](lighting-and-sky.md#renderer-settings)). Of its two
pipeline switches, clustered lighting pays once more than about ten ranged
lights are in view at the same time (at 50 the world pass is a third of the
light loop's; below ten the loop is as fast or faster, and cheaper on
llvmpipe), and GPU culling buys nothing while the CPU draw list is tens of
microseconds a frame, which it is up to a few thousand parts in view.
Where the adapter has timestamp queries, `gpu.passes` gives the median GPU
milliseconds of each renderer pass that is timed, with the adapter's
compute limits beside them: the ambient occlusion passes (`ao-prepass`,
`ao-occlusion`, `ao-blur`), then the first world view of a frame's `world`
pass, its `particles` pass (soft sprites and billboards, timed in the frames
that draw it), the frame's shadow-layer passes together (`shadows`, timed in
the frames that render layers), its `bloom-exposure` passes (with sun shafts) together and its
`finish` pass
([exposure, tone mapping and fog](lighting-and-sky.md#exposure-tone-mapping-and-fog)),
and a frame's share of building [the sky's
light](lighting-and-sky.md#the-skys-light) (`sky-light`). `gpu.indirectLight`
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
choices apply first. The image is the snapshot's own view, drawn before the
extra frames; the report's `camera` is that view. Compare
snapshots drawn on the same adapter; `WGPU_BACKEND` selects it as for any
wgpu program. A still camera reuses work a moving one repeats (culled lists,
a directional light's shadow cascades): add `--walk M` or `--turn D` to move
the snapshot's camera M metres forward or D degrees right before each extra
frame. A still snapshot also renders its shadow layers only once:
`--rerender-shadows` renders them all in every extra frame, so `shadows` and
the frame times show the uncached cost. The report's `shadows` gives the
layers, pages, atlas bytes and the layers and casters the last frame rendered.
On an RX 9070 XT at 1280×720, a Hotel floor's 138 layers (23 shadowed
lights, 2,053 caster draws) take 1.24 ms of GPU time re-rendered; the
atlas is 64 MiB.

Trusted product code reads the same statistics (`ProductHostRendererStatistics`
in `product-host`) as UTF-8 JSON bytes through
`IEngineContext.Diagnostics.ReadRenderer()`. The runtime refreshes them once a
second; the generated binding copies the bytes before returning. Reading them
never draws a frame or synchronizes the GPU.

## Feature gallery

How would this game look with a feature it leaves off? `rusty-scene-render
<snapshot> --gallery DIR [--width W] [--height H] [--frames N]` draws the
snapshot as it is, then once per renderer feature it leaves off or below its
top quality, each turned on at its Engine default:

- the [video options](lighting-and-sky.md#video-options) catalogue's
  features: render scale 1, 4× antialiasing, shadows, no shadow budget, each
  ambient occlusion mode not drawn, clustered lighting and GPU culling (vsync
  and occlusion strength and radius are tuning, not features);
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
occlusion in a mesh room) shows only its cost. A CraftSurvive meadow on an RX
9070 XT draws seven variants in about 6 seconds.

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
