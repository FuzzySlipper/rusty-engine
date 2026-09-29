# Control fences keep the renderer and media mounted (#8765)

A visible check of #8746. Pause, resume, control replace and input remapping
should publish an in-place rebind: no world snapshot, no renderer
replacement, and held input cleared.

Since #8792 the page no longer renders: it shows the runtime's wgpu frames on
a 2D canvas. There is no WebGL context or `replaceFrame` to count. The
equivalents are one output stream, the same canvas, and one runtime renderer
(`engine.renderer.presentation` `surfaceId`) across every fence.

## Streamed run on the current pair

### Setup

- **Engine.** This tree (`bc0e45e80` plus the frame-view fix below), packed
  with `scripts/build-runtime-pack.sh --output …`. The product ABI
  fingerprint is `2ccdb0e6…`, the same as pair `0.1.0-dev.b67d90d5b601`.
- **Dagger.** rusty-dagger `608502f` on that pair, run with
  `rusty dev --runtime <pack> --live-debug`. The runtime drew with wgpu on
  RADV.
- **Browser.** [exercise-fences-stream.mjs](exercise-fences-stream.mjs) ran
  inside `scripts/capture-playtest-warning-delta.mjs`, which drives headless
  Chromium, so the capture window spans every fence.
  - An init script counts output streams, bindings and UI projections, with
    their control revisions.
  - A request listener reads the input posts for key facts and clears.
  - Positions, mode and steps come from live debug `playtest.observe` and
    `engine.time`.
- **Start.** The run started from Dagger's title: Begin, then the three-clip
  opening, then play.

### Results

[stream-fences-result.json](stream-fences-result.json). Each walk held a key
for 1.2 s.

| Checkpoint | Control revision | Streams | Same canvas | Frame | Renderer | Projection revisions seen |
|---|---|---|---|---|---|---|
| playing | 1 | 1 | yes | 10758 | `runtime-stream-2` | 1 |
| after menu | 1 | 1 | yes | 11441 | `runtime-stream-2` | 1 |
| after control replace | 2 | 1 | yes | 11929 | `runtime-stream-2` | 1, 2 |
| rebound to K | 3 | 1 | yes | 12140 | `runtime-stream-2` | 1–3 |
| paused, observer | 4 | 1 | yes | 12445 | `runtime-stream-2` | 1–4 |
| resumed, walked K | 5 | 1 | yes | 12631 | `runtime-stream-2` | 1–5 |
| restored W | 6 | 1 | yes | 12981 | `runtime-stream-2` | 1–6 |

- **Gameplay input works.** Walking with W moved 5.05 m.
- **The menu clears held input.** With W held, the player moved 5.04 m, then
  the menu opened.
  - It moved 0 m while the menu was open.
  - It moved 0 m after the menu closed with W still down, because the menu's
    `interaction-mode-loss` clear released the key.
  - A fresh press then walked 5.13 m.
- **A control replace clears held input.** `control/replace` gave revision 2
  and a `control-revision-change` clear. W held across it moved 4.97 m
  before and 0 m after. A fresh press walked 5.00 m.
- **A new mapping fires, and the old one stops.** Rebinding move.forward to
  K in Dagger's Controls UI gave revision 3.
  - W moved 0 m and K moved 5.06 m.
  - [stream-rebound-k.jpg](stream-rebound-k.jpg).
- **Pause holds the world.** `lifecycle/pause` gave revision 4, and the mode
  was `Paused` with 0 steps advanced.
  - The menu opened while the world was held.
  - An observer camera 4 m above the eye, pitched down 35°, drew frame 12445
    at step 13848 with `held: true` and `observer: true`
    ([stream-paused-observer.jpg](stream-paused-observer.jpg)).
  - Clearing the observer drew frame 12446 at the same step.
- **Resume, and restore.**
  - `lifecycle/resume` gave revision 5, and K walked 5.13 m.
  - Rebinding back to W gave revision 6. W walked 1.31 m before a dungeon
    wall stopped it ([stream-restored-w.jpg](stream-restored-w.jpg)).
- **Nothing was rebuilt.** There was one output stream throughout. The page
  never reconnected, and the canvas and runtime renderer never changed.
  - UI projections arrived under every new binding.
  - The page sent 16 key facts, and a `control-revision-change` clear at
    each of revisions 2 to 6.

### Warning capture

[stream-warning-capture.json](stream-warning-capture.json). Browser and
Engine capture both completed, with no lag and no drops. It found two items.

- **Dagger product art.** "Inventory frame art is not published", as in the
  #8746 run.
- **`CSHARP_INPUT_STATE`, once.** This was `rejected-recoverable`. While
  Dagger was paused, the page posted input (the menu's clears), and the
  runtime refuses input unless it is running. The page treats the refusal as
  recoverable, and resume clears input anyway, so only the warning is
  unwanted. Filed as #8872.

Differences from the #8746 run:
- Chromium's ReadPixels stall message is gone, because the page no longer
  uses WebGL.
- The `CSHARP_INPUT_STATE` warning is new because input now reaches the
  runtime at all. In the #8746 run the page sent no key facts.

No baseline exists, so no clean delta is claimed.

**Four findings fixed.** The same exercise on the pair's own shell reported
four `PLAYWRIGHT_REQUEST_FAILED net::ERR_ABORTED` items on
`/__rusty/product/runtime/frames`.
- **Cause.** When no newer frame is ready within its 1 s wait, the route
  answers 204. The shell's frame loop skipped a 204 without reading its empty
  body, and Chromium reports an unread response as an aborted request.
- **Reproduction.** A page on the same origin fetched two 204s. The one left
  unread was reported `net::ERR_ABORTED`; the one read was not.
- **Where it showed.** It happened every second while the world was held.
- **Fix.** `frame-view.ts` now reads the body. The run above, with the fix,
  had none.

That earlier run also logged `CSHARP_INPUT_STALE_DROPPED`, which is input a
fence made stale and is dropped by design. This run happened to post none.

### A product fault found on the way

The first attempt on this pair faulted Dagger on its first pointer look. The
first delta after pointer lock exceeded the per-update bound, and
`PlayerInputSystem` used `Look.Integrate`, which throws
`DeltaLimitExceeded`. Dagger `15ec724` saturates pointer look with
`Look.IntegrateClamped`, as it already did for the stick. The Engine's look
API was right; the product used the failing variant.

## First run: Engine `677082c9`, before #8766, #8767 and #8792

### Setup

- Engine `677082c9` (#8746 plus its media-owner fix), with a source runtime
  pack built by `scripts/build-runtime-pack.sh --output
  target/runtime-pack/linux-x64`.
- rusty-dagger `dea2968` in a private clone, launched with `rusty dev
  --engine-source … --live-debug`. The clone had its own persistence and
  content store.
- Headless Chromium (SwiftShader) driven by Playwright. An init script
  counts WebGL contexts and context losses, `AudioBufferSourceNode`
  start/stop/end, and `<video>` creation/removal. The network listener
  counts `/outputs/fresh` connections (every projection recovery or remount
  reconnects fresh) and renderer-resource fetches.
- Player position, mode and simulation steps come from live debug
  `playtest.observe` and `engine.time`.

### Results

**Phase A: fences while dagger's retained opening video plays**
(`exercise-intro-fences.mjs`, `intro-fences-result.json`, fresh world).
The fences were pause, resume, a direct control replace, and a move.forward
rebind through the Controls UI (control revisions 1 → 5).

| Checkpoint | video time | same `<video>` | fresh conns | contexts / lost | resource fetches |
|---|---|---|---|---|---|
| intro playing | 3.3 s | yes | 1 | 3 / 0 | 168 |
| after pause | 5.2 s | yes | 1 | 3 / 0 | 168 |
| after resume | 6.9 s | yes | 1 | 3 / 0 | 168 |
| after control replace | 8.9 s | yes | 1 | 3 / 0 | 168 |
| after rebind | 14.1 s | yes | 1 | 3 / 0 | 168 |

- The video element was never paused or removed by a fence.
- Each clip's completion fact was reported under the *new* binding with
  `replaceOwner: false` and accepted. In a separate run with pause/resume
  during the intro (control revisions 12 → 14), the report was
  `acceptedThroughFactId: 1`.
- Dagger's three-clip opening then advanced into play (`mode: Playing`).
  Three videos were created, and each was removed only when it finished.

**Phase B: gameplay** (`exercise-gameplay.mjs`, `gameplay-result.json`)

- Pause (revision 8) held the world: 0 simulation steps advanced over
  1.5 s, and the mode was `Paused`.
- Resume (revision 9) returned to `Playing`, and steps advanced again.
- Across pause and resume: the same canvas, a live WebGL context (3
  contexts, 0 lost), still one fresh connection, and no new resource fetches.
- The one audio source started in gameplay was stopped before the fences;
  no fence stopped a source.

**Warning capture** (`scripts/capture-playtest-warning-delta.mjs`,
`warning-capture.json`, report-only):
- Engine events: 0.
- Browser warnings: 2. One is dagger product art ("inventory frame art is
  not published"); the other is Chromium's ReadPixels GPU-stall message.
- No baseline exists, so no clean delta is claimed.

### Limits of the first run

- **New mappings firing in gameplay was not shown visibly.** Under headless
  automation, dagger's UI moved to interface mode on attach
  (`interaction-mode-loss` clear) and never returned it to gameplay, so the
  browser sent no key facts. The Controls rebind did change the product's
  mapping (move.forward W ↔ K, control revision advanced) in Phase A.
  Fresh edges after a mapping change are covered by the csharp-product-runtime
  test `update_callback_staged_mapping_rebinds_the_browser_and_delivers_fresh_edges`.
- The observer camera and a menu opened during held time were not exercised
  separately.
- **A fresh attachment restarts an active video from the beginning.** The
  video baseline carries no playback cursor. This is pre-existing and
  unrelated to fences; it is filed as #8769.

The streamed run above answers the first two limits. Since #8792 the runtime
owns the renderer, so a page attaching no longer restarts a video (#8769).
