# SSE replay machinery removed (#8767)

After #8766, one runtime process owns the output bus. Every browser connection
now starts from a fresh complete baseline, and nothing is replayed.

## Removed

Rust: 1,524 lines removed, 552 added. The additions include the rewritten
tests and the new bus.

- **Resume and lag:**
  - `Last-Event-ID` resume and the separate `/outputs` resume route; only
    `/outputs/fresh` remains;
  - the `rusty-output-lag` event.
- **The shared history ring:**
  - `OutputBus.events`, floor cursor and retained-event limit;
  - `OutputPushStage` history trimming;
  - `MAX_OUTPUT_QUEUE_ITEMS`.
- **Fragments:** `OutputFragment`, transfer ids, `fragment_slices`,
  `MAX_OUTPUT_EVENT_BYTES` and `MAX_OUTPUT_FRAGMENT_DATA_BYTES`. Each batch is
  one SSE event.
- **Baselines spanning pushes:** `PendingBaseline` across pushes, and the
  isolated private-baseline bus. A baseline must now complete within one
  operation's outputs, and both publishing and connection baselines use one
  encoding path, `encode_output_batches`.
- **Retired-resource retention:**
  - the history lookup behind `/runtime/resource`;
  - resource sidecars on outputs and receipts;
  - `take_retired_renderer_resources`;
  - the widened delivery inventory;
  - the retired lists in the appearance and audio services and in the
    service composition.
- **Telemetry:** `outputQueueFloor`.

TypeScript: 507 lines removed, 46 added.

- **`local-transport.ts`:**
  - the lag listener and decoder;
  - fragment reassembly and decoding;
  - the 256 KiB per-event bound (the per-batch `maximumOutputBytes` budget
    stays optional and unbounded by default).
- **`product-browser-host.ts`:** the `output-lag` terminal-failure kind.
- **Fixtures and tests:**
  - the output-lag fixture button and spec;
  - six fragment tests;
  - `outputQueueFloor` in the live-debug client and Studio panel.

## What replaces it

- **Per-subscriber live queues.** A subscriber's queue starts when its
  baseline is captured, and all subscribers share one encoded batch. A
  subscriber that is 256 events behind, or that stops reading for the
  existing 750 ms write timeout, is closed; the browser reconnects fresh.
- **SSE ids** remain only as an output sequence: a debug command's
  `x-rusty-output-through` lets the playtest harness wait for its outputs.
- **The browser reconnect flow is unchanged:**
  - on stream error it publishes `fresh-baseline-required`, reopens
    `/outputs/fresh` and installs the new baseline;
  - a stream refused before its baseline is retried with backoff (#8766).
- **Resources** come from the runtime's current retained set only.
  Content-addressed responses stay immutable and browser-cached (#8734).

## Exercises: rusty-dagger under `rusty dev --engine-source --live-debug`

`scripts/exercise-dagger.mjs`, `results/dagger-exercise.json`. The page
reaches the host through a local TCP proxy, so a disconnect can really cut
its sockets. The proxy rewrites Host and Origin between same-length ports.

| Exercise | Result |
|---|---|
| Reload | fresh baseline and `ready` in 0.33 s, same incarnation (…623/1/1) |
| Transient disconnect: the proxy cut all 4 live sockets | one stream error; fresh reconnect (a third `/outputs/fresh`); `ready` in 4.3 s, same incarnation; then 62 batches/s |
| Stalled subscriber: a raw SSE client stopped reading for 10 s beside the page | the server closed it; the client got EOF as soon as it resumed reading at 10.0 s. The page was unaffected (59 batches/s) |
| Runtime replacement by source restage | five 503s while the new runtime started, then `ready` on the new incarnation (…624/1/1) 24.3 s after the edit, about 12.6 s of which was `rusty dev`'s rebuild |

- **No lost resources:** zero `/runtime/resource` 404s across all four
  (334 fetches). The same canvas and a live WebGL context throughout.
- **Console errors:** only the expected ones, `ERR_EMPTY_RESPONSE` from the
  cut and the replacement's 503s.
- **Warning capture** (`results/warning-capture.json`): 0 Engine events; the
  same two known browser warnings (dagger inventory art, Chromium
  ReadPixels). There is no baseline, so no clean delta is claimed.

**Supervisor paths on the new bus** (the #8766 exercise,
`results/supervisor-exercise.json`): each has fresh baselines on the new
incarnations.

| Path | Result |
|---|---|
| Replacement | 0.75 s |
| Automatic restart | 0.75 s |
| Pause | 503 |
| Recovery | 0.71 s |
| Stdin close | exit 0 |
| Direct-launch crash | exit 1 |

## Other checks

- **Rust tests:**
  - product-dev-host 43 + 29 loopback. New tests cover:
    - a subscriber's queue starting at join;
    - shared encodings;
    - overflow closing a subscriber;
    - pruning a dropped subscriber;
    - a large output staying one event;
    - a rejected publication sending nothing and fencing the binding;
    - a `Last-Event-ID` reconnect getting a fresh baseline.
  - csharp-engine-services 188;
  - csharp-product-runtime 44 + 26.
- **Clippy** is clean except the pre-existing #8757 lints.
- **`pnpm test`:**
  - product-browser-host 103, with a new test that a 4 MiB batch arrives as
    one event;
  - live-debug-client 5;
  - live-debug-panel 4 + artifact 3.
- **Also passing:**
  - `typecheck:browser`;
  - the product-browser-host Playwright specs (11);
  - the artifact rebundle;
  - `test-csharp-sdk-package.sh --coreclr-smoke`.

## Limits

- **Same-callback releases are no longer served.** A resource released in the
  same callback that published an operation using it is not retained. Audio
  reports that as a non-fatal realization diagnostic. The dagger exercises hit
  no case of it (zero 404s). A mesh or texture used and released in one
  callback is untested.
- **A reconnect is a full baseline.** A dropped connection re-sends the whole
  baseline rather than resuming, so reconnect cost scales with the world, not
  the gap. One-shot transients during a disconnect are not replayed.
- **Reconnect time is not broken down.** The 4.3 s reconnect on SwiftShader
  includes remounting the projection; its parts were not measured separately.
