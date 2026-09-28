# Control fences keep the renderer and media mounted (#8765)

A visible check of #8746. Pause, resume, control replace and input remapping
should publish an in-place rebind: no world snapshot, no renderer
replacement, and retained media keeps playing.

## Setup

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

## Results

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

## Limits

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
