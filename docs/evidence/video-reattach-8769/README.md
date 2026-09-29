# #8769: a page attaching mid-clip sees the Engine's position

## What changed since the task was filed

#8769 was filed when the browser played video. The video baseline carried no
playback cursor, so each fresh attachment (a reload, a dropped stream, a
fresh-stream retry) started the active clip from 0.

Since #8792 the runtime draws video with wgpu (`render-wgpu/src/video.rs`),
and the renderer outlives page attachments:
- **A connect rebuilds nothing.** `connect` on a running runtime only tags a
  complete baseline for the page (`csharp-product-runtime` `connect`).
- **Rebuilds happen for other reasons.** `SceneDriver::rebaseline` replaces
  the renderer only for a start, a restart, a reported fault, or a product
  call whose renderer work was lost.

So the case the task names no longer happens. One gap remained:
- **The gap.** The renderer took a clip's start from its first *draw*, not
  from its play op. Nothing draws while no viewer watches, so a clip that
  started unwatched would begin from 0 when a page attached. Its soundtrack,
  which `audio_output` starts on the play op, would already be playing.
- **The fix.** A clip now starts at the Engine time its play op applied at,
  the same stamp animation controllers use. This removes the lazy
  `started_at: Option<f64>`.

## Behavior

| Case | Result |
|---|---|
| A page leaves and another attaches mid-clip | The clip is at the Engine's position. The first frame the new page gets is the last one drawn before the gap; the renderer then catches up. |
| A clip starts while no page watches | It runs from its play op; an attaching page sees it where the Engine is, in step with its sound. |
| A clip ends while no page watches | It completes once, at the first draw. That draw needing a viewer is #8871. |
| The renderer is rebuilt (fault, lost renderer work, restart) | The baseline applies at the current Engine time, so the active clip plays again from its start, and `audio_output`'s rebaseline restarts its sound with it. The old renderer's unreported facts are dropped, so the clip completes once, later. |

The rebuild row is kept on purpose. Rebuilds happen only on failure paths or
on a restart, where the world itself is re-established. Carrying a playback
position through them would need a cursor in the video baseline and a seek,
and nothing yet observed needs that.

## Tests

`render-wgpu/tests/video.rs`, with the real renderer and the `testsrc` clip:
- **`an_undrawn_clip_keeps_to_the_engine_timeline`.**
  - A play applies at 10.0 s, and nothing draws until 10.55 s. That first
    draw is clip frame 5, not frame 0.
  - The clip then ends with nothing drawn. The next draw reports `Completed`
    once, and a later draw reports nothing.
- **`a_rebuilt_renderer_plays_an_active_clip_from_its_start`.** The renderer
  takes the current time before its baseline's play, as
  `SceneDriver::rebaseline` does, and shows frame 0.

`cargo test -p render-wgpu -p render-stream` passes. So do `cargo fmt --check`
and stable clippy with `-D warnings` on both crates.

## Live run on Dagger

Dagger `608502f` ran under `rusty dev --runtime` on a runtime pack built from
this change (ABI `2ccdb0e6…`, pair `0.1.0-dev.b67d90d5b601`). The first
opening clip, `anim0000.webm`, is 45.4 s long.

**Reattach mid-clip** ([reattach.mjs](reattach.mjs), [reattach.json](reattach.json)).
- **The sequence.** A page clicked Begin and watched for 8 s, then left. A
  new page attached 23.2 s after Begin.
- **Matching.** [match.py](match.py) compares each screenshot's letterboxed
  clip with the clip's own frames at 4 fps (160×100 grey, lowest mean
  squared error). The clip shows a book, so a page stays on screen for
  several seconds, and a match gives a span rather than an instant.

| Screenshot | Seconds since Begin | Matching clip span | MSE at the 3 s frame (a restart) |
|---|---|---|---|
| first page, before leaving | 8.0 | 5.25–12.25 s (title page) | 3422 |
| new page, first frame | 23.3 | 5.25–12.25 s (the last frame drawn before the gap) | — |
| new page, 3 s later | 26.4 | 24.75–31.0 s (illustrated page) | 3428 |

- **Result.** 3 s after reattaching, the new page shows the clip where the
  Engine is (26.4 s, inside the matching span). It does not show it from the
  start, or where the first page left it (about 11 s would be the title
  page, MSE 634). The tolerance is the span a page stays on screen, about
  ±3 s here.
- **Pictures.** [reattach-strip.jpg](reattach-strip.jpg) shows the three
  screenshots.

**The opening completes once per clip** ([finish.mjs](finish.mjs), [finish.json](finish.json)).
- **Setup.** The runtime then ran unwatched until about 62 s after Begin, so
  `anim0000` (45.4 s) ended with no page watching. A page then attached.
- **Result.** The attaching page's first draw completed `anim0000`, and
  `anim0011` (5.6 s) and `dag2` (121.6 s) played in turn. Dagger went from
  `Title` to `Playing` 128.3 s after the attach, which is their 127.2 s plus
  about a second.
- **Why that shows single completion.** Dagger's opening policy fails on a
  completion for any clip but the one it expects, so a doubled or replayed
  completion would have faulted it.

This live run shows clips whose play op applied while a page watched. The
changed case, a clip that starts unwatched, is covered by the renderer test
above: in Dagger the next clip starts only after the previous one completes,
which needs a draw.

## Not changed

- **Catching up.** After a gap, the first draw decodes every frame between
  the old position and the new one (VP9 frames depend on their
  predecessors). For Dagger's 320×200 clips this is quick. A long,
  high-resolution clip left unwatched would make that first draw slow; a
  keyframe seek would fix it if it ever matters.
- **Completion without a viewer.** An unwatched clip still completes only
  when something draws. That is #8871.
