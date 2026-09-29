# Streaming browser mode (#8786)

With `RUSTY_RENDER_OUTPUT=stream`:
- the runtime renders the world with `render-wgpu` in its own process;
- the browser shell shows those frames on the Engine canvas, under the unchanged product UI;
- input, UI projections, audio, live-debug and the crew-services playtest harness work as before.

The design summary is in [architecture: streaming browser mode](../../architecture.md#streaming-browser-mode).

All measurements are from 2026-09-29 on one workstation:
- AMD RX 9070 XT (RADV) for both the runtime's wgpu and Chromium's WebGL (ANGLE on Vulkan);
- 20 cores, shared with other lanes' builds (load average 5 to 13 during the runs).

## What changed

**Runtime** (`csharp-product-runtime/src/frame_output.rs`):
- The env var turns on a `render-stream::FrameStreamer` when the runtime loads.
- Each finished product call's `Frame`, `Presentation` and `ViewComposition` publications are applied to the renderer (`realize`).
- A call whose renderer work was lost, or a world replacement, rebuilds the renderer from `snapshot_outputs` (`rebaseline_frames`).
- Animation facts from drawn frames go to `ingest_animation_realization_feedback`.
- The renderer takes the manifest's `defaultLights`.
- `engine.renderer.*` adds a `stream` object: adapter, frame rate, per-stage medians, bytes, skipped ops.

**`render-stream`** (new crate, the only owner of `jpeg-encoder` in `dependency_boundary_check.py`):
- The `Renderer` lives behind one mutex. The runtime thread applies deltas; a render thread draws, reads back and encodes.
- It draws only when a change was applied: every simulation step while running, once per change while held (paused or manual/action-driven time).
- It draws only while a viewer is watching.

**`product-dev-host`** (`frames.rs`, plus a small route in `host.rs`): `GET /__rusty/product/runtime/frames?after=N&width=W&height=H` answers with the latest frame newer than N, or 204 after 1 s. Each frame is one `RSF1` frame:

| offset | size | field |
|---|---|---|
| 0 | 4 | magic `RSF1` |
| 4 | 4 | header bytes (40) |
| 8 | 8 | sequence |
| 16 | 8 | simulation step |
| 24 | 8 | width, height |
| 32 | 1 | format (1 JPEG, 2 RGBA8) |
| 33 | 1 | flags (bit 0: held) |
| 36 | 4 | payload bytes |

The desktop lane (#8790) presents to a window and does not read frames, so the header was not shared (Den #8786, messages 35353 and 35370).

**Browser:**
- `product-bootstrap.json` gains `renderer.output: "stream"`.
- The runtime-pack shell passes `mountStreamedFrameSurface` (`product-browser-host`) through a new `renderer.mountSurface` option of `application-host`.
- The surface pulls frames and paints them on the Engine canvas. The canvas stays the first `<canvas>`, the focus and pointer-lock target, and the input target, and it carries `data-rusty-frame-sequence`, `-step` and `-held`.
- It acknowledges world ops. It forwards audio, video and telemetry-overlay ops to the browser hosts as before.
- Its rAF loop is the Engine cadence: it samples input and advances the browser presentation hosts.

**Not realized in this mode yet:**
- observer camera, on-demand drawing, pick and the presentation observation (#8841);
- ghost plate feedback (#8842);
- `RenderOutput` image jobs (#8826), since realized: the runtime runs them in every mode;
- billboard labels (#8827), since realized: the streamed renderer draws them.

## Encoding decision: JPEG q80, pure Rust

A development viewer is a browser on localhost or one LAN hop. The decision was taken from these numbers.

Offline encode on the captured Doom room study frame (`scripts/encode-bench`; median of 20; CPU only, one thread):

| Encoding | 1280x720 ms | 1280x720 bytes | 1920x1080 ms | 1920x1080 bytes | MB/s at 60 fps (720p / 1080p) |
|---|---|---|---|---|---|
| raw RGBA | 0 | 3,686,400 | 0 | 8,294,400 | 221 / 498 |
| **jpeg-encoder q80 (chosen)** | 4.0 | 88,231 | 8.8 | 175,310 | 5.3 / 10.5 |
| jpeg-encoder q70 | 3.8 | 69,812 | 8.6 | 139,200 | 4.2 / 8.4 |
| jpeg-encoder q90 | 7.2 | 170,011 | 15.5 | 336,542 | 10.2 / 20.2 |
| libjpeg-turbo q80 (Pillow; stands in for `turbojpeg`) | 1.5 | 87,268 | 3.4 | 172,909 | 5.2 / 10.4 |
| WebP lossless (image-webp) | 5.5 | 344,296 | 8.6 | 563,008 | 20.7 / 33.8 |
| PNG, fastest | 3.0 | 905,046 | 5.3 | 1,621,021 | 54 / 97 |
| lz4 of raw | 2.2 | 488,284 | 3.9 | 822,058 | 29 / 49 |
| zstd -1 of raw | 3.1 | 248,909 | 5.3 | 412,877 | 15 / 25 |

Browser decode plus draw into the 2D canvas (Chromium, GPU; `scripts/decode-bench.mjs`):

| Payload | 720p | 1080p |
|---|---|---|
| JPEG | 7.5 ms | 12.7 ms |
| raw RGBA | 2.5 ms | 4.3 ms |

- **Why JPEG q80 with `jpeg-encoder`.** It is the smallest simple option: a 5.3 MB/s stream at 720p decoded natively by the browser, with no JS decoder, and it keeps up with 60 fps at both sizes.
- **`jpeg-encoder`'s licence** is (MIT OR Apache-2.0) AND IJG. The IJG terms ask a binary distribution's documentation to credit the Independent JPEG Group. This matters only if the streaming mode ever ships outside development.
- **libjpeg-turbo would be the next step if encode time mattered.** It is about 2.5 times faster, at the same size. Through `turbojpeg` it needs cmake and nasm (absent here) or the system `libturbojpeg` dev package, plus a Windows build story. Not needed at the current cost.
- **Hardware video encoding** (VA-API, NVENC and similar) was not built. Frame bytes and encode time are not the constraint at these sizes; the page's decode is (below).
- **Lossless options are 4 to 10 times the bytes.** lz4 and zstd would also need a JS decoder on the page.
- **Raw RGBA is not viable even on localhost.** It is 221 MB/s at 720p and 1.8 Gbit/s, over 1 GbE. End to end (`measurements/latency-stream-rgba-*.json`, measured with the earlier push transport), 720p took 296 MB/s with 45.6 ms median and 145 ms p90 input-to-display, and 1080p took 484 MB/s with 181 ms median and 459 ms p90. `RUSTY_RENDER_STREAM_FORMAT=rgba` stays only to repeat that measurement.

## Latency and bandwidth beside the Three path

`scripts/latency-bandwidth.mjs` runs headless Chromium with the crew-services GPU flags against Doom's room study on the same build, with the Three path (no env var) and the streaming path:
- **Bandwidth:** every byte the page receives over a quiet 5 s window (CDP `Network.dataReceived`).
- **Latency:** a keydown ("l" turns right) timestamped in the page, to the first rAF whose canvas pixels differ from the frame before the key. 40 trials.

Final build, rebased on main at 516912929 (render-wgpu with 4x MSAA):

| Path | Size | Median | p90 | Max | Page receives |
|---|---|---|---|---|---|
| Three | 1280x720 | 27.4 ms | 32.7 ms | 98.9 ms | 64 KB/s SSE |
| Stream | 1280x720 | **30.9 ms** | 40.6 ms | 50.8 ms | 64 KB/s SSE + 5.3 MB/s frames |
| Three | 1920x1080 | 31.4 ms | 50.2 ms | 84.1 ms | 68 KB/s SSE |
| Stream | 1920x1080 | 65.5 ms | 164.9 ms | 356.1 ms | 67 KB/s SSE + 10.7 MB/s frames |

Runtime stage medians (`engine.renderer.status` → `stream`; `measurements/stream-status-*.json`). Both sizes run 60 fps with 0 skipped frames (`frame-probe.py`):

| Size | render (encode passes) | readback (GPU wait) | JPEG encode | Frame |
|---|---|---|---|---|
| 720p | 0.53 ms | 1.16 ms | 4.48 ms | 88 KB |
| 1080p | 0.40 ms | 1.92 ms | 8.77 ms | 176 KB |

- **720p is at parity with Three** (within about 4 ms median), and it is the development default the harness uses.
- **At 1080p the page is the bottleneck.** JPEG decode takes 12.7 ms per frame, and under the machine's load the page occasionally stalls: `scripts/draw-gaps.mjs` drew about 45 of 60 fps, with gaps up to 170 ms. That gives the p90 tail. The runtime side is not saturated.
- **Measurements vary with other lanes' builds.** Repeated 720p stream runs gave 25 to 42 ms medians; Three gave 27 to 31 ms.
- **Not measured over a LAN hop.** No second machine was reachable. #8844 records the run. At the measured sizes one hop adds one frame's transfer: 88 KB is about 0.7 ms on 1 GbE.

### How the transport got here

The route first sent one long response of back-to-back frames, drawn on a free-running 60 Hz clock:

| Change | 720p median | 1080p |
|---|---|---|
| First version | 57 ms | 75 ms |
| Draw when a change is applied, instead of on the free clock | 25–28 ms | long tail (p90 320–540 ms): frames queued in socket buffers whenever the page fell behind |
| Pull, one request per frame | bounded tail | ~10 ms added to 720p by Chrome's per-request path under load; the host answers in 0.18 ms |
| Two outstanding requests (the shipped design): one takes the next frame, the other already waits for the one after | 30.9 ms | 65.5 ms |

With two outstanding requests, at most two frames are in flight and request setup is off the display path.

## Doom playtest through crew-services

The run used:
- the installed `playtest` service and its existing `rusty-doom` profile, unchanged (`http://127.0.0.1:4394/`);
- the runtime pack built from this branch with `RUSTY_RENDER_OUTPUT=stream`;
- a Doom build staged against this branch's SDK.

That Doom build is a private, uncommitted copy of `/home/agent/dev/rusty-doom`, migrated only enough to compile against current main. Doom's own pin is 120 commits behind; its migration is #8821. The call sites involved: #8821's list, `*LeaseReceipt`→`*Result` (#8817), and the #8840 borrowed-result renames. No downstream repository changed.

Captures (`captures/`, crew-services originals re-encoded to JPEG):

| # | Harness step | Result |
|---|---|---|
| 01 | `playtest start rusty-doom`, `observe` | `phase: connected`, one 1280x720 canvas, no console or page errors; the streamed frame under Doom's HUD |
| 02 | `assist act forward 2000ms` (realtime) | moved 12.18 units; new view |
| — | `assist look` + `act attack` ×2 at trooper 31001 | `kills: 1`; the attack captures show the sprite in the streamed frame |
| — | `playtest input hold [W, Space]` | jumped onto the pool ledge |
| 03 | `capture {engine_presentation:true}` in front of the north wing door | door closed; `interaction.inspect` selects door 20000, `Ready` |
| 04 | `assist act use`, then `capture` | door 20000 goes `opening` → `open` (`raised: 2.5`); the corridor behind is visible |
| 05 | `assist time manual`, `look yaw -150`, `advance 500` | Held: one frame, flag set, no further frames for 1.5 s. The inspection look redraws (sequence +1, same step); advance draws the new step. |
| 06 | `frame-probe.py` | a raw 1280x720 runtime frame (JPEG payload) |
| 07 | rebased build: `act forward`, `capture` | moved 6.15 units; same presentation |

Live-debug commands went through the product's existing debug route with no harness change:
- `rusty-live-debug --command "combat.observe"`, `engine.renderer.status` and `engine.time.mode realtime`;
- the harness's own `engine.renderer.presentation` query inside every `capture` (`measurements/crew-capture-*.json`). It reports `available: false`, since there is no presentation observation in this mode (#8841), plus the `stream` facts.

**Screenshots:** crew-services capture is Playwright CDP on headless Chromium (GPU). `scripts/page-check.mjs` repeats the check on SwiftShader headless Chromium, the runtime's own `--headless` flags. Both capture the streamed frame, because it is painted into the DOM canvas.

**Found along the way (not streaming):** held-time `act` (manual or action-driven) reports `advancedMs` but does not move the player. This happens with the Three path too, on this build and private Doom copy; #8843. The playtest therefore moved in realtime.

## Checks run

- `cargo test -p product-dev-host -p render-stream -p csharp-product-runtime -p csharp-engine-services`: all pass. `render-stream/tests/stream.rs` drives the render thread on a real headless device: first frame, frames per applied step, and nothing more once held.
- `cargo clippy --no-deps -D warnings` on the touched crates: clean.
- `python3 scripts/dependency_boundary_check.py`: passed.
- `pnpm run build` in `render/`: passes, including the product-browser-host artifact type check.
- `scripts/build-runtime-pack.sh`: builds.
- **Not run:** `scripts/test-runtime-pack.sh` and the C# SDK smoke. No ABI, generator or C# SDK file changed.

## Reproduce

```sh
# Runtime pack from this checkout, a product staged against its SDK, then:
RUSTY_RENDER_OUTPUT=stream rusty-product-host --product <staged Product> --loader coreclr
python3 docs/evidence/streaming-8786/scripts/frame-probe.py http://127.0.0.1:4394 1280 720 4 frame.jpg
node docs/evidence/streaming-8786/scripts/latency-bandwidth.mjs http://127.0.0.1:4394 stream-720 1280 720 40
```
