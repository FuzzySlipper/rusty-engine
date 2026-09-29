# No empty frames or unchanged UI on idle ticks (#8770)

## Before

On every tick, an idle product sent one SSE batch with two outputs that
changed nothing:
- an empty frame (`ops: []`);
- a UI projection whose value was unchanged, with only its sequence advanced.

**The UI projection came from the product.** Both the SDK fixture
(`PublishUi()` in `Update`) and Dagger (`DaggerfallHudProjection.Publish` on
each session update) call publish every update.

**The empty frame came from the Engine,** which emitted one for every product
call, whether or not anything changed.

## Change

**UI service** (`csharp-engine-services/src/ui.rs`). A publish whose value
equals the stream's last published value, under the same binding, is not
republished.
- The product's sequence still advances, so the next publish must still be
  higher.
- `latest` keeps the envelope the browser actually has, so baselines and
  rebinds send that envelope.

**Per-call frames** (`finish_product_call` in `csharp-product-runtime`). A
frame or presentation frame with no operations and no publication revision is
dropped. A frame with a publication revision is kept even when empty, because
the browser chains those revisions. Baselines are built separately and are
unchanged.

**Lifecycle rebinds** (`CsharpProductRuntime::action`). Start, Pause, Resume
and Restart change the binding, and the browser clears UI whenever the binding
changes. When the binding changes, each stream's current projection now
follows the new binding (`snapshot_ui_projections`), as faults and in-place
rebinds already did.

Before, only what the lifecycle callback happened to publish was rebound.
- **With the UI change:** a callback's unchanged publish would otherwise be
  dropped, and the UI would stay blank while paused.
- **Without the UI change:** the fixture's `Pause()` publishes nothing, so its
  UI was blank while paused even before this change.

**Nothing else to remove.** The host already sends no batch for a tick with
no outputs (`host.rs`), so the batch disappears with its two outputs.

## Consumers checked

- **UI sequences** (`application-host/src/ui-projection.ts`): they must
  strictly increase within one binding. Gaps are fine.
- **The initial renderer frame gate** (products with animation preloads): it
  settles on the first frame, which arrives in the complete baseline.
  Baselines are not filtered.
- **Renderer diagnostics:** sampled on the renderer's animation-frame cadence
  (`observeRendererCadence`), not on received frames.
- **The host exercise:**
  - it asserted that Start carries both a create-time and a Start projection;
    it now asserts that Start carries the current projection under the Start
    binding;
  - a new assertion checks that Pause carries the current projection even
    though the fixture's `Pause()` publishes nothing.

## Idle traffic, SDK fixture

`scripts/idle.sh`, adapted from #8768's script for the removed worker
topology and `--content-store-root`. Debug hosts, before (main) and after, on
the same staged fixture, with one subscriber for 10 s (`results/idle-*.txt`):

| | Per-tick batches | Idle SSE | CPU (one core, noisy shared host) |
|---|---|---|---|
| Before | 600 × `frame,ui-projection` (209 KB) | about 20.9 KB/s | 4.3% |
| After | none | 9 keep-alive comments (180 B) | 2.2% |

The attach baseline and the one readout are the same in both runs.

## Visible exercise: rusty-dagger

`scripts/exercise-dagger.mjs`, `results/dagger/`.
- **Setup:** Dagger `9e422ae` plus its #8743 UI-build migration (`57d0a6f`, from
  #8800), built against an SDK packed from this change. It ran under
  `rusty dev --live-debug` with this change's runtime pack, in headless
  Chromium.

**Results:**
- **Idle:** 0 batches in 5 s on the title screen, and 0 in a second 5 s window.
  #8768 measured about 290 per 5 s here.
- **UI follows product state.** Through Dagger's Controls panel,
  `move.forward` was remapped to K:
  - the product binding became `KeyK`;
  - the HUD projection arrived twice: the rebind's current projection, then
    the first update's new value under the new binding;
  - the DOM showed `move.forward: KeyK` (`04-rebound-k.png`).

  Remapping back to W did the same.
- **Page:** `ready` throughout, with no page errors.
- **Begin** leaves the title screen in neither this run nor #8768's run: the
  headless intro video blocks it. No gameplay HUD change is claimed.

## Validation

- Tests pass for `csharp-engine-services` (183, including the extended UI
  publication test), `csharp-product-runtime` and `product-dev-host`.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes, including the
  host exercise's new Start and Pause UI assertions.
- Clippy `--no-deps -D warnings` is clean on `csharp-product-runtime`.
  `csharp-engine-services` has the known #8757 lints only.
