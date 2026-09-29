# Roguelike moves to pair 551e695886c2 (#8839)

## Pair

`0.1.0-dev.bcf02594620c` → `0.1.0-dev.551e695886c2`, the newest published
pair on 2026-09-29, via `rusty update`. D20 (#8838) moved from the same pin.

rusty-roguelike `faa5586`: 7 files.

## Migrations applied

| Change | Source | Sites |
|---|---|---|
| `VoxelEditTransaction(session, before.SourceRevision, edits)` → `(session, edits)`; drop the `ReadScene` that only fetched the revision | #8739 | 1 |
| `PerceptionReadoutLeaseReceipt` → `PerceptionReadoutResult` | #8817 | 1 |
| `NavigationStepReceipt` → `NavigationStepResult` | #8840 | 1 |
| The `ProbeRandom` check fake implements `IRandomService.DrawLcg15` | new member | 1 |
| Exercise scripts: `/__rusty/product/runtime/outputs` → `/outputs/fresh` with an `Origin` header; find UI projections inside `runtime-output-batch` events | #8767 | 2 |
| `exercise-product.sh` takes its binding from a first attachment instead of `lifecycle/start` with a null binding | host behaviour | 1 |

**Why the last row changed.** The staged CoreCLR host has already started the
product when it answers, and `start` with no binding is accepted only in
`Created`, so it was refused with `CSHARP_CONTROL_BINDING`. The NativeAOT
probe host still accepts that start.

**Removed:**
- **The retained-path half of `NavigationAtomicityProbe`.** The probe seeded
  an Engine-retained path, then re-read it cell by cell with
  `ReadNavigationPathCellAt`, to show that a failed move left it unchanged.
  Since #8840 the Engine retains no path: a path belongs to whoever requested
  it. So `SeedNavigationPath`, `FloorNavigationState.RetainedPath` and the
  `retainedPathLength`/`pathHash` projection fields are gone.
  - The probe still proves that a command whose settlement fails leaves the
    product checkpoint and the Engine navigation and spatial projections
    unchanged.
- **The `Attach` callbacks** in the product and the probe (#8799).

**Kept:** the perception paging (`expectedProjectionIdentity`, `pairCursor`).
The ABI still has it.

## Checks

- `dotnet build RustyRoguelike.sln -c Release` succeeds.
- `RustyRoguelike.Product.Checks`: "focused product checks passed".
- **`bash src/scripts/exercise-product.sh`** (staged CoreCLR host, demand
  steps) passes end to end. It covers inactive begin, save before begin,
  begin, move, wait, save and load, pause and resume, restart, and shutdown.
- **`bash src/scripts/exercise-navigation-atomicity.sh`** (NativeAOT publish
  and host) prints "Packaged NativeAOT navigation atomicity proof passed".
  - The first attempt failed with `CSHARP_PRODUCT_ABI_MISMATCH`: stale ignored
    `obj/Rusty.Engine/Composition` output from an old SDK was still being
    built.
  - Deleting that directory fixed it. The current SDK no longer generates
    composition projects (#8775).
  - Other products that ran `VerifyRustyEngineAot` on a pre-#8775 pin can hit
    the same stale output.

No CI workflow exists.

## `rusty dev`

This ran with CoreCLR on the new pair, in demand mode, using intents plus
`admit-demand-step`:

- `roguelike.begin` moves the phase from `preparation` to `partydecision`.
- `roguelike.move.east` moves `partyCellX` from 17 to 18.
- No `CSHARP_*` diagnostic appeared.

## Filed Engine tasks

None found.
