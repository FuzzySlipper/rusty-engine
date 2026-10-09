# Verification notes

## CI lanes

CI follows the current C# product, Rust Engine, and browser shell ownership.
Changes route to their owning lanes. A pull request's newer check run cancels
its older one; every commit pushed to `main` keeps its own runs, so each can
gate a review. Pair publications run one at a time.

| Lane | Default evidence |
| --- | --- |
| Rust | Formatting, standalone and Cargo dependency boundary checks, workspace tests, Clippy with warnings as errors |
| C# | Binding generation and a disposable packaged SDK consumer staged and exercised through the Rust CoreCLR host |
| Browser | Browser shell and live-debug bundle build, closed-bundle check, compiled unit tests, performance result tooling tests |
| Docs | Local links and CI owner routing |
| Pair | On `main`: build, exercise with a packaged consumer, and publish the matched SDK/runtime pair and desktop runtime pack |

Run the corresponding `scripts/verify.sh`, `scripts/verify-csharp.sh`,
`pnpm --dir render run verify` (the browser shell and live-debug panel), or
`scripts/verify-docs.sh` locally. The browser shell needs its pnpm
dependencies; C# requires .NET, Clang/libclang and the pinned binding tools.
The render-wgpu screenshot tests run on llvmpipe (`mesa-vulkan-drivers`) in CI.

## GPU verification

GitHub's runners have no GPU, so CI renders on llvmpipe, a software Vulkan
adapter. There it proves that the renderer starts, draws correctly and
refuses what the adapter cannot do; it says nothing about how a feature looks
or what it costs on a GPU. A renderer feature may refuse on a software adapter
(`Gpu::is_software`) and report the refusal through the settings readout, as
the compute features do without compute shaders. Look and cost are verified
on machines with GPUs, against that machine's own accepted images and
timings. `rusty-scene-render` says on stderr and in its report
(`softwareAdapter`) when it ran on a software adapter, so such numbers are not
mistaken for GPU evidence. llvmpipe timings in the docs describe what a
software adapter costs; they are not acceptance gates.

### The GPU lane

`scripts/gpu-lane.sh` runs the lane on the machine it is on, for the commit
checked out. It builds `rusty-scene-render` and `rusty-gpu-lane`, then renders
every scene of the lane directory twice over: with the machine's accepted
baseline renderer and with the candidate, alternating which goes first, three
times each (`--repeats`). It prints a table and writes the run under
`runs/<machine>/<time>-<commit>/`: `report.md`, `report.json`, and for every
scene `baseline.png`, `candidate.png` and `diff.png`. It exits 0 when every
scene passes, 1 when one is flagged and 2 when the candidate could not render
one. `--scene NAME` (repeatable) runs a subset.

The lane directory (`RUSTY_GPU_LANE_ROOT`, `/data/rusty-engine-gpu-lane` on
den-agents) holds:

| Path | What |
| --- | --- |
| `scenes.json` | The scenes and the thresholds (below) |
| `scenes/*.rscene` | Scene snapshots, from `engine.renderer.snapshot` on a fixture or a product |
| `machines/<machine>/` | The machine's accepted baseline: `baseline.json` (its source and why) and a copy of that renderer; earlier records stay beside it, numbered |
| `runs/<machine>/` | Every run's report and images |

A scene names its snapshot, the `rusty-scene-render` flags it renders with
(an occlusion path, clustered lighting, GPU culling), notes for the review,
and any settings it overrides. The defaults:

| Setting | Default | Meaning |
| --- | --- | --- |
| `width`, `height` | 1280, 720 | Render size |
| `frames`, `turn` | 30, 0.25 | Frames per render, and degrees the camera turns each, so timings cover more than one view; the image is the last frame |
| `repeats` | 3 | Renders per renderer |
| `changedAbove` | 2 | A pixel counts as changed when a colour channel moves more than this many levels |
| `changedShareFlag` | 0.0005 | Flag when more than this share of pixels changed |
| `ssimFlag` | 0.999 | Flag when the luminance SSIM falls below this |
| `passRatioFlag`, `passDeltaFlagMs` | 1.15, 0.05 | Flag a GPU pass slower by both, in its median, with every candidate repeat slower than every baseline repeat |
| `frameRatioFlag`, `frameDeltaFlagMs` | 1.10, 0.2 | The same for the frame median |

Rendering is deterministic on one adapter (two runs of one build give
identical images), so any image difference comes from the change. A scene
whose own repeats differ is reported as not deterministic and judged above
twice that noise. Timings only flag; a one-off stall on a shared machine does
not, because every candidate repeat must be slower than every baseline one.

**Review.** When a scene is flagged, the run also writes `review-prompt.md`:
a brief for a reviewing agent listing each flagged scene's images and the
`git log` and `git diff` of the change. The agent running the lane hands it
to a subagent, which opens the images and answers, per scene, intended,
regression or noise with the evidence. Only flagged scenes are reviewed.

**Gate.** `rusty-gpu-lane status --run RUN --repo FuzzySlipper/rusty-engine
--sha SHA` posts the run as the check run `gpu-render/<machine>` on the
commit, through the `gpu-lane-status` workflow (a check run needs the Actions
token). A pass posts success, a failed render failure, and a flagged run
neutral until reviewed. After review, post again with `--conclusion success`
or `failure` and `--review FILE`, the subagent's verdicts, which head the
check's summary. A review submission can then require
`gpu-render/<machine>` beside `csharp`.

**Baselines.** `rusty-gpu-lane accept --root DIR --renderer
target/release/rusty-scene-render --source SHA --reason TEXT` makes a renderer
this machine's baseline. Accept when a reviewed change has landed on `main`,
so the next run compares against it. Each machine keeps its own baseline,
since images differ between GPUs and drivers. When the snapshot format
changes, an older baseline cannot open newer scenes; the lane flags those
scenes (nothing compared) until the scenes are recaptured and a new baseline
is accepted with that reason.

On Windows, run the two `cargo build` lines of the script and then
`rusty-gpu-lane run` directly, or the script from Git Bash.

NativeAOT is a separate fidelity path: `scripts/verify-csharp.sh --aot`, the
C# workflow dispatch option, and `scripts/test-csharp-release-pair.sh PAIR --aot`.

Boundary checks inspect current dependencies and artifacts, not symbol names
or exact build command text. Deliberate ABI adapter signatures may carry a
local, reasoned Clippy exception; warnings-as-errors remains active.

## Playtest warning deltas

`scripts/capture-playtest-warning-delta.mjs` is a small report writer for one
named Playwright exercise against the product's browser page. It launches
headless Chromium through the `render` workspace's `@playwright/test`, so it
needs those pnpm dependencies and a Playwright Chromium. It covers the default
streamed output; the desktop window's UI is not a Playwright page.

It independently listens for Playwright console warnings/errors,
`pageerror` events and failed requests, and, when an Engine host origin is
provided, reads structured Engine diagnostics through
`/__rusty/product/runtime/diagnostics/read`. That route answers only when the
product was started with `--live-debug`. The report retains normalized,
512-character messages and stable fingerprints with occurrence counts; it never
forwards console arguments, request/response bodies, or stacks. Failed browser
resources carry their observed URL and status.

Warnings/errors remain findings. A later confirmed browser-host baseline can
resolve an exact attachment's settled response-delivery warning
(`PRODUCT_HOST_RESPONSE_WRITE_RESYNC`); the report preserves the original warning
and records the matching baseline sequence. A queued input acknowledgement
does not prove C# execution, so queued-input or unknown delivery certainty
remains unresolved. New tabs, missing correlation, incomplete capture, and
terminal errors cannot clear that warning.

Run a named navigation-only exercise against a product host on `PORT`:

```sh
node scripts/capture-playtest-warning-delta.mjs \
  --url http://127.0.0.1:PORT/ \
  --exercise-id renderer-smoke \
  --engine-origin http://127.0.0.1:PORT \
  --output warning-delta.json
```

For a multi-step required path, pass `--exercise path/to/exercise.mjs`. That
module must export `async function exercise({ page, url })`; it owns the
explicit navigation and interactions. The capture helper owns only listeners
and report construction.

The Engine read drains a private checkpoint to its current `throughSequence`
just before the exercise, then drains that same private cursor after it.
`--engine-cursor` can make the initial checkpoint detect an already-lagged
cursor, but does not share or advance another reader's cursor. Engine capture
is optional to configure, but an omitted, failed, lagged, dropped, or
incomplete Engine capture prevents the report from saying a clean claim is
eligible.

Compare against an explicitly named prior report:

```sh
node scripts/capture-playtest-warning-delta.mjs \
  --url http://127.0.0.1:PORT/ \
  --exercise-id renderer-smoke \
  --engine-origin http://127.0.0.1:PORT \
  --baseline previous-warning-delta.json \
  --output warning-delta.json
```

Only matching schema/protocol, exercise ID, page origin, Engine origin, and
complete baseline capture are compatible. The JSON report explicitly says
`unavailable` when no baseline was supplied, `incompatible` when it cannot be
compared, or lists new/resolved/unchanged fingerprints. The helper is
report-only: warning findings and deltas do not themselves choose an exit code;
a failed browser run still exits nonzero. `cleanClaimEligible` is false for
incomplete capture, incompatible comparison, and terminal or unknown Engine
diagnostics; explicit Error findings also require disposition before a clean
claim.
