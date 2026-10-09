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
