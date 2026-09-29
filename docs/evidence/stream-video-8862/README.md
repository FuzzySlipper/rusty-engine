# Video in the streaming mode from the runtime renderer (#8862)

Before this change, the streaming mode left video to the browser. The runtime
turned its renderer's video off (`RendererOptions::video = false`), and the
stream surface kept `video` among its browser domains, so the browser host's
`<video>` element played clips over the page. That element goes away with
#8792, and the #8791 review asked that cinematics play through the Rust
renderer in streamed frames.

## Change

- **`render-wgpu`.** `RendererOptions::video` is removed: every renderer plays
  video ops. `draw_video` reports whether a clip's picture covered the target,
  and `FrameStats::video` carries that out of `render_view_composition` and the
  window paths. The ghost-plate source renderer never received video ops;
  its `video: false` went with the field.
- **`render-stream`.** `DrawnFrame::video` records it for each drawn frame, and
  the published frame carries it.
- **`product-dev-host`.** In the `RSF1` header, bit 1 of the flags byte is set
  while a video clip covers the frame. The header layout and length are
  unchanged.
- **`csharp-product-runtime`.**
  - Stream mode no longer turns video off. The drawn frames' video facts reach
    the Engine through the same `FrameOutput::report` path as window mode.
  - The clip's own sound now plays whenever the runtime draws video, in either
    output. It plays with the runtime's device audio realizer
    (`RUSTY_AUDIO_OUTPUT=device`).
  - `engine.renderer.presentation` reports the last frame's `video`.
- **`product-browser-host` (`streamed-frame-surface.ts`).**
  - `video` is no longer a browser domain in either runtime-rendered mode, so
    the browser video host receives no ops, and the surface reports no video
    facts. The runtime is the single fact source.
  - A frame with the video flag is shown with the canvas at z-index 1000, the
    old video element's layer, above the product UI (z-index 2). A frame
    without the flag puts the canvas back to its resting z-index.
  - The canvas also carries `data-rusty-frame-video`.

## Dagger in the streaming mode

- **Setup:**
  - Dagger at rusty-dagger `ce8af48` (pair `8101e06bc8cf`, ABI fingerprint
    `6297d14a…`);
  - `rusty dev --runtime <pack built from this change>` with
    `RUSTY_RENDER_OUTPUT=stream` and `RUSTY_AUDIO_OUTPUT=device`;
  - audio into a PulseAudio null sink whose monitor was recorded;
  - viewed in headless Chromium through Playwright (`scripts/drive.mjs`). The
    driver clicks Begin, samples the canvas every 250 ms, and screenshots at
    fixed times.
- **Title (`screenshots/01-title.jpg`).** Before Begin: video flag off, canvas
  at z-index 0, UI at 2.
- **Begin → `ANIM0000`.**
  - Within one sample the video flag is on and the canvas is at z-index 1000.
  - The clip draws letterboxed over the whole view, and the title screen and
    its Begin button are covered (`screenshots/02-anim0000.jpg`).
  - `engine.renderer.presentation` reports `video: true`
    (`presentation-during-video.json`).
  - The page never creates a `<video>` element.
- **Sequence.**
  - The flag stays on through `ANIM0000` (45.36 s), `ANIM0011` (5.56 s) and
    `DAG2` (121.56 s) (`screenshots/04-dag2.jpg`). It turns off at 172.9 s after
    Begin, against 172.5 s of clips.
  - Dagger advances its opening sequence only on each clip's terminal
    realization fact. That the sequence reached play therefore shows the
    runtime's `Completed` facts advancing `ANIM0000` → `ANIM0011` → `DAG2`.
- **After the cinematics.** The canvas is back at z-index 0 under the game HUD,
  and the world draws in play (`screenshots/06-end.jpg`).
- **Soundtrack.** The null-sink recording contains each clip's own Opus track.
  A 20 s excerpt of each clip's decoded audio matches the recording:

  | Clip | Where it matched in the recording | r | Control r (10 s later) |
  |---|---|---|---|
  | `anim0000` | clip 5 s at 26.27 s | 1.000 | −0.012 |
  | `dag2` | clip 5 s at 77.26 s | 1.000 | 0.009 |

  `dag2` starts 50.99 s after `anim0000`, against 45.36 + 5.56 = 50.92 s of
  clips in between.

## Checks

- **Tests:**
  - `cargo test` for `render-wgpu`, `render-stream`, `product-dev-host` and
    `csharp-product-runtime`;
  - `tests/video.rs` now also asserts `FrameStats::video` before, during and
    after a clip;
  - `frames.rs` asserts the video flag bit.
- **Lints:** workspace clippy `-D warnings`, stable clippy on the touched
  crates, and fmt.
- **TypeScript:** `product-browser-host` typecheck and 106 tests.

## Accepted difference

With browser-realized audio (no `RUSTY_AUDIO_OUTPUT`), a clip in the streaming
mode plays silent. The browser's `<video>` element used to carry the clip's
sound, and the browser audio host plays only audio ops. The audio family moves
into Rust (campaign #8782); `renderer-host`, which holds the browser audio
host, is deleted by #8792. After that, the device realizer is the streaming
mode's audio path, and it already plays the clips' sound.
