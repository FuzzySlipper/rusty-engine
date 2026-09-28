# Readouts on change, no per-tick progress event (#8768)

## Change

- **`runtime-progress` is deleted** from the host output model, the worker
  scheduler, the browser decoder and the Product Browser Host. It had two
  consumers:
  - The Rust-host cadence pulse (`pulseRustHost`) drained browser input once
    per tick. This duplicated the renderer's animation-frame cadence, and
    input availability already wakes the lane (`pulseInput`).
  - The browser health counter (`data-rusty-product-runtime-progress`,
    `runtimeProgress` in the browser status report). It now counts only
    browser-owned realtime advances. When Rust owns the clock, liveness is
    the Rust scheduler's own telemetry (`runtimeProgressRateMillihertz` in
    live-debug diagnostics).
- The 250 ms DOM-write throttle that existed only for those per-tick pulses
  is removed.
- **`runtime-readout` is published only when it changes what a browser
  shows** (`ProductDevRuntimeReadout::changes_browser_view`):
  - runtime binding, mode, state, fault or inspection time;
  - the step counter, but only while inspection time is not realtime (the
    browser derives held simulation time from it).
- Per-tick counters and clock samples stay in live debug (`engine.time`).
  Both the scheduler and the debug path go through the same last-published
  readout.
- A fresh connection's private baseline now carries the current readout. A
  reconnecting browser therefore never keeps a stale one.

## Idle traffic and CPU

The SDK fixture (`fixture.sdk-package`, realtime 60 Hz), with one SSE
subscriber for 10 s (`scripts/idle.sh`, `results/idle-*.txt`).
- Before: runtime pack from the #8740 prototype, which has the same host code
  as `main` at `365ecf6a`.
- After: this change.
- `owned` is the runtime-owned I/O mode (`--worker-owned-io`), whose
  in-process host scheduler published a readout every tick.

| topology | per-tick batch, before | after | idle SSE, before → after | CPU (one core), before → after |
|---|---|---|---|---|
| worker (default) | frame, ui-projection, runtime-progress (398 B) | frame, ui-projection (350 B) | 23.9 → 21.0 KB/s | 2.2% → 2.2% |
| owned | frame, ui-projection, runtime-readout, runtime-progress (808 B) | frame, ui-projection (350 B) | 48.0 → 20.8 KB/s | 1.6% → 1.4% |

After the change, each connection receives one readout, and a second one at
the first scheduled tick in `owned` mode. The CPU difference is within noise
on this shared machine.

The remaining per-tick batch is an empty frame (`ops: []`) plus an unchanged
UI projection (`value: null`, new sequence). Both change nothing. Removing
them is #8770.

## Visible exercise: rusty-dagger

`scripts/exercise-dagger.mjs`, `results/dagger-exercise.json`.
- Setup: `rusty dev --engine-source … --live-debug` on the default worker
  topology, headless Chromium.
- An init script counts SSE output kinds and keeps the last readout the
  browser received.

- **Idle:** 290 batches in 5 s, with 0 `runtime-readout` and 0
  `runtime-progress`. The page received one readout, at attach.
- **Playtest time inspection** (`__rustyPlaytest`):
  - `time manual` published a readout with `inspectionTime: ["manual", 60]`,
    and the world held (0 steps in 1.5 s);
  - `advance 500` published one with steps +30;
  - `time realtime` published one with `["realtime", 60]`.
  - Each was one readout; nothing arrived between them.
- **Input through the renderer cadence (no pulses):**
  - dagger's Controls UI remapped `move.forward` from W to K and back;
  - 24 input POSTs produced 24 `runtime-input-result`s and two in-place
    rebinds;
  - `playtest.observe` reported `["KeyK"]` and then `["KeyW"]`;
  - the Begin click was also delivered (accepted input results).
- **Renderer:** the same canvas throughout, with a live WebGL context and the
  intro video playing; host state stayed `ready`.
- **Live debug:**
  - `engine.time` worked throughout;
  - diagnostics telemetry reported `runtimeProgressRateMillihertz: 55004`
    (about 55 Hz on SwiftShader) and a progress age of 17 ms.

**Warning capture** (`results/warning-capture.json`, report-only):
- Engine events: 0.
- Browser warnings: the same two as in #8765, dagger's missing inventory art
  and Chromium's ReadPixels GPU stall.
- No baseline exists, so no clean delta is claimed.

## Other checks

- Rust: `cargo test -p product-dev-host -p csharp-product-runtime`, including
  a new `changes_browser_view` test.
- Clippy: clean except the pre-existing `collapsible_match` at `model.rs:609`
  (#8757).
- `pnpm test` in product-browser-host: 107/107.
- `pnpm run typecheck:browser`.
- Playwright product-browser-host specs: 12/12.
- Product-browser-host artifact rebundled.
- `test-csharp-sdk-package.sh --coreclr-smoke` passed.

## Limits

- **In-process readouts on dagger were not shown.** The dagger exercise used
  the default worker topology, where scheduled readouts never reached the
  browser. There, the change removes only `runtime-progress`. The
  in-process change-only readout was measured on the SDK fixture in `owned`
  mode, which has no browser UI, so it has no visible exercise.
- **Keyboard gameplay keys were not driven.** As in #8765, headless dagger
  stays in interface input mode. Input was shown through UI intents instead,
  which use the same cadence lane.
