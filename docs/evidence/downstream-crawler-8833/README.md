# Crawler moves to pair 551e695886c2 (#8833)

## Pair

`0.1.0-dev.eedd406900d5` → `0.1.0-dev.551e695886c2`, the newest published
pair on 2026-09-29, via `rusty update`.

rusty-crawler `ba45176`: 11 files. The work was done in a separate worktree,
because the main checkout runs a long-lived `rusty dev` loop (see "Left
alone").

## Two pins, now one

The task named two pins, `feec788503fe` and `eedd406900d5`:
- The product pin in `Directory.Build.props` was `eedd406900d5`.
- `tools/portable-assets-example/Directory.Build.props` pinned `feec788503fe`
  for a small standalone example. It pinned a newer pair only because it
  needed `PortableAssetContent` before the product pin had it.

The product pair now has that API, so the example's own `Directory.Build.props`
is deleted and it inherits the repository pin. Its README says so.
- `rusty status --project tools/portable-assets-example/PortableExample.csproj`
  resolves the root pin.
- The example builds against `551e695886c2`.

## Migrations applied

| Change | Source | Sites |
|---|---|---|
| `NavigationStepReceipt` → `NavigationStepResult` | #8840 | 1 |
| `EntityStore.Destroy(id, expectedRevision)` → `Destroy(id)` | #8741 | 1 |
| `CharacterStepReceipt` test value: drop `RevisionBefore`, `RevisionAfter` and `DynamicImpulseCount`; add `Movement` and `Tether` | #8754, #8840 | 1 |
| The fake `IEngineContext` drops `ContentStore` | #8763 | 1 |
| UI build: the `tsc` target moves to `RustyEngineProductUiSourceRoot`, `RustyEngineProductUiBuildCommand` and `RustyEngineProductUiInput` | #8800 note, #8775 | 1 |

**The UI build row, in detail.**
- The old target ran `BeforeTargets="GenerateRustyEngineProductComposition"`.
  #8775 removed that target, so `tsc` silently never ran.
- Staging worked in the main checkout only because an old `src/ui/generated`
  was still on disk. A clean worktree failed with `RustyEngineProductUiRoot
  does not exist`.
- With the SDK properties, staging compiles the UI.

**Removed:** `CrawlerProduct.Attach()` (#8799), and the four test calls to it.
Those tests read the projection that `Start` and `Update` already publish.

## Checks

- **Builds.** Every product and test project builds in Release.
- **Suites.**
  - Architecture: 6 pass.
  - Kit: 530 pass.
  - Host: 169 pass.
  - Import: 139 pass.
- **Staging.** `StageRustyEngineCoreClrProduct` stages the product.
- **UI test.** `scripts/verify.sh` stops at its first step, the node UI test.
  - `session-panel.test.mjs` imports `src/ui/main.ts`, and this machine's Node
    v22.22.1 has no TypeScript support (`ERR_NO_TYPESCRIPT`).
  - The unchanged checkout at the old pin fails identically.
  - Filed as rusty-crawler #8863. The steps after it were run by hand and are
    listed above.

## `rusty dev`

- **Pin resolution.** The task asked whether a pair's `rusty dev` without
  `--runtime` resolves through the pin. It does:
  `RUSTY_DEV {"event":"pin-resolved","pin":"0.1.0-dev.551e695886c2","runtimePack":"~/.cache/rusty-engine/pairs/0.1.0-dev.551e695886c2/runtime-pack"}`.
- **Default content.** The committed default bundle names no content packs,
  so a stock run creates a party but has no world (`contentPacks: 0`,
  `place: ""`).
- **The walk.** For the walk, the bundle temporarily named the operator's
  local packs `mm7-tables`, `mm7-world` and `mm7-turn-scenario`. That edit was
  not committed. `rusty dev` restaged on the content change.
  - Space accepts creation.
  - The party stands in place 57 at x = -415.
  - A physical `key-w` press, held for about 1.5 s, moves it to x = 263. That
    matches `docs/live-checks.md`'s figure of about 382 units per second.
- No `CSHARP_*` diagnostic appeared.

## Left alone

A `while true; do sleep infinity | .runtime/runtime-pack/bin/rusty dev ...;
done` loop (since 2026-09-27) serves the main checkout on
192.168.1.10:4176, with the pre-#8810 `.runtime` pack. The main checkout also
has an uncommitted `docs/live-checks.md` edit.

The checkout's working tree is still at `d5740e7`. Fast-forwarding it restages
that session against the new SDK, so it is the session owner's call. The
`rc-live-b` loop on :4178 serves a different checkout and is out of scope.

## Filed tasks

- **Engine:** none found.
- **Product:** rusty-crawler #8863, for the UI test and Node TypeScript support.
