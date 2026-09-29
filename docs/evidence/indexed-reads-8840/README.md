# The last indexed …At reads become borrowed results (#8840)

## What changed

The ABI no longer has any `Native*AtRequest`. Each read that used to return a
count, with a second call fetching one element per FFI crossing, now returns the
elements in one borrowed `Native*Result`. It is the #8744/#8818 shape: backing
held in the bridge's `BorrowedResult` until the next result on that bridge,
and copied by the generated binding before returning.

| Family | Before | After |
|---|---|---|
| Spatial triggers | `ReconcileTriggers` → count, then `ReadTriggerFactAt(i)` | `ReconcileTriggers` → `SpatialTriggerReconcileResult.Facts` |
| | `SetTriggerActive` → count, then `ReadTriggerFactAt(i)` | `SetTriggerActive` → `SpatialTriggerLifecycleResult.Facts` |
| | `ReadTrigger` → count, then `ReadTriggerOverlapAt(i)` or `ReadTriggerOverlapPage` | `ReadTrigger` → `SpatialTriggerReadResult.Subjects` |
| Navigation | path requests / steps → `PathLen`, then `ReadNavigationPathCellAt(i)` | each returns `Path` (`NavigationPathResult`, `NavigationWeightedPathResult`, `NavigationVolumetricWeightedPathResult`, `NavigationStepResult`) |
| Audio | `Read` → count, then `ReadDiagnosticAt(i)` | `Read` → `AudioResult.Diagnostics` |
| | `ReadRealization` → count, then `ReadRealizationFactAt(i)` | `ReadRealization` → `AudioRealizationResult.Facts` |
| Video | `ReadRealization` → count, then `ReadRealizationFactAt(i)` | `ReadRealization` → `VideoRealizationResult.Facts` |
| Animation | `ReadRealization` → count, then `ReadRealizationFactAt(i)` | `ReadRealization` → `AnimationRealizationResult.Facts` |
| Presentation | `Read` → two counts, then `ReadDiagnosticAt(domain, i)` | `Read` → `PresentationFactsResult.BillboardDiagnostics`, `.ParticleDiagnostics` |

## Removed with the index calls

- **Session state that existed only for index reads.** The Spatial session's
  `last_trigger_facts` and the navigation `last_path` are gone; the facts and
  the path now come back from the call that produced them.
- **`ProposeNavigationStep`.** Without `last_path` it was identical to
  `EvaluateNavigationStep`, which every product already uses.
- **The trigger overlap paged read**, with its revision fence,
  `MAX_TRIGGER_OVERLAP_PAGE_ITEMS`, engine-spatial's `current_overlaps_page`,
  `MAX_TRIGGER_READ_ITEMS` and the `QuotaExceeded`/`StaleRevision` trigger
  codes. `ReadTrigger` returns every subject. No product used the page.
- **Voxel `ReadResidentChunkAt` and `ReadDirtyChunkAt`, deleted rather than
  converted.** Nothing in the SDK, fixtures or products called them, and
  folding them into the edit and residency results would have renamed receipts
  that CraftSurvive and roguelike use by name. The counts stay. The session's
  retained dirty-chunk list is gone.
- **`NativeCharacterStepReceipt.dynamic_impulse_count`.** It was always 0,
  because the C# bridge's step world reports no rigid bodies.
- **Always-zero or redundant fields:** `NativeSpatialTriggerRestoreReceipt.fact_count`
  (restore never produces edges), and every `retained_*_count` and `path_len`
  whose value is now the slice length.

## Generator change

`NativeAnimationRealizationFact` carries inline `NativeAnimationFeedbackText`
fields, fixed `uint8_t[96]` arrays. The result-element validator did not accept
fixed arrays, and the copy emitter tried to build a metadata helper for the
text struct. `Rusty.Engine.BindingGenerator` now:
- validates a fixed-array field in a result element as the fixed-type
  validator does;
- converts `NativeAnimationFeedbackText` through its existing `FromNative`
  instead of a generated metadata copy.

## Evidence

- `cargo test -p engine-spatial -p csharp-engine-services` passes. Tests:
  - triggers: enter/stay/exit from `Facts`, exits in the lifecycle result,
    `ReadTrigger` subjects after restore;
  - navigation: the evaluated path's cells;
  - audio: diagnostics and realization facts;
  - video: realization facts;
  - presentation: billboard diagnostic identity.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
  Clippy on the touched crates is clean.
- Bindings regenerated.
- `Rusty.Engine.Entities.Example` runs clean, including `EntityTriggerProjection`
  facts.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes. Its packaged
  consumer compiles the navigation-mapping and spatial-artifact checks.
- `scripts/test-runtime-pack.sh` passes. It runs the NativeAOT-trial fixture,
  including its host, voxel and volumetric path checks.
- A local release pair (`scripts/build-csharp-release-pair.sh`) passes
  `scripts/test-csharp-release-pair.sh`, which compiles `scripts/fixtures`
  into its clean consumer.
- The audio-containers and video-playback fixtures compile against a locally
  packed SDK.
- `fixtures/csharp-lease-release` does not compile on main before this change
  either: its handle constructors predate #8817. It is not built by any
  script.

## Migration

The type renames are mechanical. The generator requires a borrowed result to
end in `Result`.

| Before | After |
|---|---|
| `SpatialTriggerReceipt` + `ReadTriggerFactAt(new(session, i))` (`.Present`, `.Enter`, …) | `SpatialTriggerReconcileResult r`; `r.Facts.Span[i]` (`SpatialTriggerFact`, no `Present`) |
| `SpatialTriggerLifecycleReceipt` (`.FactCount`) | `SpatialTriggerLifecycleResult` (`.Facts`) |
| `SpatialTriggerRestoreReceipt.FactCount` | removed (always 0) |
| `SpatialTriggerReadReceipt.OverlapCount`, `ReadTriggerOverlapAt`, `ReadTriggerOverlapPage` | `SpatialTriggerReadResult.Subjects` |
| `NavigationPathReadout` / `NavigationWeightedPathReadout` / `NavigationVolumetricWeightedPathReadout` | `NavigationPathResult` / `NavigationWeightedPathResult` / `NavigationVolumetricWeightedPathResult` |
| `NavigationStepReceipt` | `NavigationStepResult` |
| `.PathLen` | `.Path.Length` |
| `ReadNavigationPathCellAt(new(session, i))` (`.Present`, `.Cell`) | `.Path.Span[i]` on the path or step result that produced it |
| `ProposeNavigationStep` | `EvaluateNavigationStep` |
| `AudioReadout`, `.RetainedDiagnosticCount`, `ReadDiagnosticAt(new(i))` | `AudioResult`, `.Diagnostics.Span[i]` (`AudioDiagnostic`) |
| `AudioRealizationReadout`, `.RetainedFactCount`, `ReadRealizationFactAt(new(i))` | `AudioRealizationResult`, `.Facts.Span[i]` (`AudioRealizationFact`) |
| `VideoRealizationReadout`, `ReadRealizationFactAt` → `VideoRealizationFactAtReceipt` | `VideoRealizationResult`, `.Facts` → `VideoRealizationFact` |
| `AnimationRealizationReadout`, `ReadRealizationFactAt` → `AnimationRealizationFactAtReceipt` | `AnimationRealizationResult`, `.Facts` → `AnimationRealizationFact` |
| `PresentationFactsReadout`, `.BillboardDiagnosticCount`, `.ParticleDiagnosticCount`, `ReadDiagnosticAt` | `PresentationFactsResult`, `.BillboardDiagnostics`, `.ParticleDiagnostics` |
| `CharacterStepReceipt(..., ContactCount, DynamicImpulseCount, CastCount, ...)` | drop `DynamicImpulseCount` |
| `EntityTriggerProjectionReconcileReceipt.Trigger` (`SpatialTriggerReceipt`) and `.Facts` (`SpatialTriggerFactAtReceipt`) | `.Trigger` is `SpatialTriggerReconcileResult`; `.Facts` is `SpatialTriggerFact` |

Downstream users found (at their current pins):
- **rusty-dagger:** the trigger reconcile loop and its test fake, the video
  cinematic realization loop, and `NavigationStepReceipt` names. Dagger moves
  to the exact new pair with this change.
- **rusty-roguelike:** `FloorEngineProjection` re-reads the retained path
  (`ReadNavigationPathCellAt` over `retainedPathLength`). It should keep the
  `Path` from its `RequestNavigationPath` result instead, which is product-owned
  state now.
- **rusty-rifles:** the weighted-path next cell becomes `path.Path.Span[1]`,
  and its audio diagnostics become `Audio.Read().Diagnostics`.
- **rusty-craftsurvive, rusty-d20, rusty-crawler, rusty-underworld, rc-live-b:**
  type renames, and `DynamicImpulseCount` in two test constructors.
- **rusty-space tests:** audio fakes.
- **asset-pipeline:** the animation realization loop.
