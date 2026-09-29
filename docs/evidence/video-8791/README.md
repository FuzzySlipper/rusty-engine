# Video playback in the wgpu renderer (#8791)

The three video projection ops (play, stop, skip) play through `render-wgpu`.
Clips are decoded in pure Rust by the new `render-video` crate, and a
playing clip's Opus soundtrack plays on the device realizer. Dagger's opening
cinematics play in the desktop window.

## Decision: pure-Rust decode of the formats the products ship

**Formats first.** The Engine admits only `video/webm` (`render-presentation`
`VideoProjector::play`). Dagger ships 33 cinematics
(`content/worldrpg/media/cinematics`, 90 MB). Checked with ffprobe and an EBML
scan of every file (`ebml.py`):
- **Video.** Every clip is VP9 profile 0, 8-bit 4:2:0, with no colour
  description (so BT.601 limited range). Sizes are 320×200 (256×200 for
  `dag2`). Frame rates are variable, 4.7–14.5 fps, so timestamps drive
  playback.
- **Audio.** 17 clips carry mono 48 kHz Opus: CodecDelay 312, a final
  DiscardPadding block.
- **Container.** SimpleBlocks only; no lacing, VP9 superframes or hidden
  frames. Cues are present.

**Candidates, marked per format.**

| Route | VP9 p0 4:2:0 | WebM demux | Opus | License as shipped | Native deps |
|---|---|---|---|---|---|
| **`rusty_vp9` 0.1.1 + `matroska-demuxer` 0.8.1 (chosen)** | bit-exact against ffmpeg on all 33 clips | yes | the existing `opus-decoder` (render-audio) | Apache-2.0; Zlib/MIT/Apache-2.0 | none (pure Rust, MSRV 1.85/1.70) |
| `vp9dec` 0.1.1 | bit-exact | separate crate | same | MIT | none, but crashes rustc 1.95 in release builds without `RUST_MIN_STACK` |
| libvpx bindings | yes | separate crate | same | libvpx BSD-3 | a system or prebuilt libvpx; nasm for a source build |
| AV1 transcode at import + `rav1d` | not applicable | yes | same | BSD-2 | ffmpeg (GPL here) at import; C-ABI-only API; a generation loss |
| ffmpeg bindings | yes | yes | yes | LGPL, or GPL as Ubuntu builds it | system FFmpeg or a source build |
| gstreamer | via plugins-good | yes | yes | LGPL plugins | GStreamer runtime |
| `gpu-video` 0.4.0 | **no** (H.264 only) | no | no | MIT | Vulkan Video; pins wgpu 29 |

The chosen route carries no C library, build tool or license question. It adds
two small crates, owned by `render-video` in `EXTERNAL_DEPENDENCY_OWNERS`.
- **Frames.** Decoded frames arrive as CPU YUV planes (96 KB at 320×200)
  and are uploaded to three R8 textures per new frame; the shader converts
  BT.601 limited range to RGB.
- **Risk.** `rusty_vp9` is two months old with one maintainer. It is pinned
  exactly; the golden test and the product-clip test below catch a
  regression, and libvpx bindings stay the fallback.
- **Speed.** It decodes all of `dag2` (121.6 s, 1,164 frames) in 1.26 s
  (`dagger-clips-decode.txt`).

**No migration.** Nothing changes downstream: the admitted media type is
unchanged, and every shipped clip decodes. A clip in another codec now fails
as a playback (`decodeFailed`) instead of playing in the browser's decoder.

## Behaviour

- **Timeline.** A clip plays on the Engine presentation timeline from the
  first render after its play op, so a held simulation holds the picture.
- **Fresh renderer.** A renderer created on a new attachment plays an active
  clip from its start (#8769's rule). A playback's own video cursor in the
  Rust baseline is not needed for this.
- **Drawing.** The picture is letterboxed on black over the whole primary
  target.
- **Ending.** As the browser host did: `Completed` at the clip's end,
  `Skipped` for a skip (and for a skip with nothing playing), `Failed`
  (`decodeFailed` or `hostFailure`) for an unreadable clip. A stop is silent.
  Facts reach the Engine through the same video realization feedback the
  browser reported.
- **Layering.** The browser's video element sits at z-index 1000 in the
  application host, above the product UI (z-index 2). The desktop window
  therefore draws video after the UI overlay, and Dagger's title screen is
  covered as in the browser.
- **Streaming.** A streamed frame lies *under* the page UI, so the streaming
  mode leaves video to the browser's element (`RendererOptions::video =
  false` there; the stream surface keeps `video` among its browser domains).
  The streamed frame therefore does not carry video; the page shows it
  above, with its own sound.
- **Sound.** In the desktop window, the device realizer plays the clip's Opus
  track from the clip's start, outside the Engine buses, as the video element
  did. It pauses with the runtime and stops on stop or skip.

## Evidence

- **Fixture in the screenshot harness.**
  - `render-video/tests/fixtures/testsrc.webm` (ffmpeg `testsrc2` with a
    440 Hz Opus tone, 38 KB) decodes bit-exact against ffmpeg (FNV of all 15
    frames).
  - `render-wgpu/tests/video.rs` renders it over a room
    (`video-first-frame.png`, `video-frame-5.png`). Flat colours are within 3
    levels of ffmpeg's own RGB conversion.
  - The same test covers the held timeline, completion, restart on a fresh
    renderer, skip, stop, and the missing- and broken-clip failures.
- **Soundtrack.**
  - `render-audio` decodes the fixture's tone with the codec delay trimmed
    (880 zero crossings per second), seeks continuously, and a realizer
    soundtrack plays, pauses and stops on the mock backend.
- **Product clips.**
  - `RUSTY_VIDEO_CLIPS=<dagger cinematics> cargo test --release -p
    render-video -- --ignored` opens and decodes every one of the 33
    (`dagger-clips-decode.txt`).
- **Dagger in the desktop window.**
  - Clicking Begin plays `anim0000` over the title screen
    (`screenshots/dagger-anim0000-desktop.png`).
  - Its `Completed` fact advances the product to `dag2` (handle 3,
    `dagger-dag2-desktop.png`).
  - The runtime's null-sink recording matches `anim0000`'s own Opus track at
    r = 0.984, against r = 0.046 for `anim0001`.
- **Checks** as in the #8790 evidence; all suites named there pass.

## Limits

- **The streamed frame shows no video**, for the layering reason above. The
  browser plays it over the page instead, as before.
- **Soundtrack drift.** Picture and soundtrack start together; for long
  clips they are not resynchronized, since they run on the Engine timeline
  and the device clock respectively.
- **Windows and macOS** decoding was not built here.
