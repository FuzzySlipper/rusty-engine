# Reload open pages after a UI asset reload (#8802)

## Decision

Yes. The smallest form is now implemented: one SSE event plus
`location.reload()`, with no hot-module machinery.

## What actual use showed

I ran a live `rusty dev --live-debug --headless` session on a private copy of
`docs/evidence/source-root-reload-8743/product`. The UI posted
`exercise.mark <label>` each time its module ran, and `exercise.marked` read
back every load in order.

Before this change, with a runtime pack from main at `53df9220`:

| Step | `exercise.identity` | `exercise.marked` |
|---|---|---|
| start, UI `ui-v1` | instance `d4431b1f…` | `ui-v1` |
| UI edit to `ui-v2` (`assets-reloaded`; server serves v2) | same | `ui-v1` |
| C# edit (`runtime-replaced`) | new instance `935b1ea3…` | empty; the page reconnected but never re-ran its UI |

`--headless` starts one Chromium with no remote-debugging port, and it
survived the runtime replacement: `RUSTY_HEADLESS_BROWSER started` appeared
once. Nothing can refresh that page, so for the agent path a UI edit never
reached the page for the rest of the session. For a person with a normal
browser, the event saves a manual refresh after every UI edit.

After this change, in the same kind of session:

| Step | instance | `exercise.marked` |
|---|---|---|
| start, UI `ui-v1` | `28dff8a5…` | `ui-v1` |
| UI edit to `ui-v2` | same | `ui-v1,ui-v2` |
| UI edit to `ui-v3` | same | `ui-v1,ui-v2,ui-v3` |

The product never restarted. The host logged no warnings, resyncs or faults.

## Change

- **Host** (`product-dev-host/src/host.rs`). After a successful
  `ProductDevAssetReload::reload`, each live SSE subscriber gets
  `event: rusty-ui-reloaded` with `data: {}`. The event has no output id and
  does not advance the output sequence. A failed reload sends nothing and keeps
  the old UI, as before. A content-bundle-only reload sends the event too, since
  the host cannot tell the two apart, and an extra page reload costs little.
- **Browser** (`local-transport.ts`). The fresh stream listens for the event
  and calls `reloadPage`. That is `location.reload()` by default, and tests can
  inject their own. Only the stream that currently owns the projection acts on
  it. The reloaded page attaches with a fresh baseline.

## Not changed

- **Runtime replacement.** The page still reconnects in place to the new
  incarnation. If an edit changes C# and UI together, it goes through a full
  restage and replacement, and the page keeps its old UI module. Reloading on
  every incarnation change would also fire on crash restarts during playtests
  and wipe page-side hooks, so it needs its own decision: #8825.
- **Scripted pages.** A Playwright page attached to the session reloads too.
  That only happens when a UI file is edited during the session.

## Found along the way

The runtime-pack shell (`runtime-pack-shell/main.js`) always passes
`realtimeAdvanceOwner: 'rust-host'`. A demand-lifecycle product, including the
#8743 exercise product as committed, therefore never mounts in the browser:
`Product Browser Host rust-host realtime advance ownership requires realtime
lifecycle mode`. The exercise above switched its copy to realtime (60 Hz, 4
catch-up steps). Filed as #8824.

## Checks

- `cargo test -p product-dev-host` passes. New tests:
  `an_asset_reload_tells_each_attached_page_to_reload` (loopback: the attached
  page receives the event with no id), and
  `a_failed_content_reload_keeps_the_served_ui_and_tells_no_page`, which asserts
  no event on failure, one event on success, and an unchanged output sequence.
  Clippy is clean.
- `product-browser-host` unit tests: 106 pass, including
  `a served UI reload reloads the page, and only from the current stream`.
- `pnpm run typecheck` passes, and `pnpm run test:browser` has 57 passed.
