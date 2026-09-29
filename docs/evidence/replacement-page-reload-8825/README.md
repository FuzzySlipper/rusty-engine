# A page reloads when a new runtime incarnation answers (#8825)

## Decision

When a page's fresh output baseline comes from a different runtime instance
than the one its UI module was loaded against, the page calls
`location.reload()` instead of attaching. A reconnect to the same incarnation
still attaches in place.

The trade-off is small because new incarnations exist only under `rusty dev`.
A source restage replaces the runtime, or a crash restarts it. A direct launch
exits on a crash instead (`docs/architecture.md`). Either way, a new
incarnation is a fresh product, so the page starting over loses nothing that
still exists. That includes page-side hooks after a crash, since the product
state they observed is gone too. The same rule works for the desktop shell
(#8790): a UI surface belongs to one runtime incarnation.

The alternative was a signal only when the restage touched UI, from the
supervisor or through a UI digest in the baseline. That needs new state and a
new contract field, and it still misses loose content that replacement
delivers. It would save about 300 ms per C#-only edit (below).

## What actual use showed

`replace-probe.mjs` drives one Chromium page on the committed
`source-root-reload-8743` exercise product (demand lifecycle) under
`rusty dev --live-debug`. It makes a C#-only edit, then a C# and UI edit
together. It polls the page's `data-rusty-product-host-state`, the UI label the
page runs, its navigation count, and the product instance
(`exercise.identity`). Raw timelines are in [before.txt](before.txt) and
[after.txt](after.txt).

| Edit | Before (current main) | After |
|---|---|---|
| C#-only | re-attached in place; 503 → `ready` in 317 ms | page reloaded; 503 → `ready` in 601 ms |
| C# + UI together | `ready` on the new instance, but the page still ran `ui v1` for the whole 90 s window while the server served `ui v2` | page reloaded; `ready` with `ui v2` 414 ms after 503 |

The reload costs about 300 ms more than re-attaching on this small product.
The restage before it takes 2–3 s here, and far longer for a real product's C#
build. The host logged no warnings or resyncs in either session.

## Change

- `local-transport.ts` records the instance of the first completed baseline.
  A later baseline from a different instance calls `reloadPage`: the injectable
  seam from #8802, `location.reload()` by default.
- `product-browser-host.ts` is unchanged. Its incarnation checks also handle a
  generation change within one instance (lifecycle restart), so none of that
  code becomes dead.
- `docs/architecture.md` describes the reload.

## Checks

- `local-transport.test.ts`: the three cross-incarnation disconnect cases now
  assert one page reload, no new-incarnation baseline attached, and no replayed
  mutation. The same-incarnation reconnect test fails if the page reloads.
- `product-browser-host`: 106 tests pass. `pnpm run typecheck` passes, and
  `pnpm run test:browser` has 57 passed.
