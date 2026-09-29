# D20 moves to pair 551e695886c2 (#8838)

## Pair

`0.1.0-dev.bcf02594620c` → `0.1.0-dev.551e695886c2`, the newest published
pair on 2026-09-29, via `rusty update`. The old pin predates per-pair release
notes, so the compiler and the evidence READMEs were the guide.

rusty-d20 `0479342`: 8 files, 19 lines added and 29 removed.

## Migrations applied

| Change | Source | Sites |
|---|---|---|
| `EntityStore.PrepareBatch(batch, Entities.Revision).Publish()` → `Entities.Commit(batch)` | #8741 | 6 |
| `Entities.Set(entity, component, value, expectedComponentRevision)` → `Set(entity, component, value)` | #8741 | 1 |
| `VoxelEditTransaction(session, scene.SourceRevision, edits)` → `(session, edits)`; delete the `ReadScene` calls that only fetched the revision | #8739 | 2 |
| `ProductStateStore<T>(engine, scope, codec, [])` → `(engine, scope, codec)` | — | 1 |
| `NavigationPathReadout` → `NavigationPathResult` (the check fake builds `Path` from the goal cell) | #8840 | 2 |

The #8741 rule applied cleanly because every prepared batch was published on
the next statement. No call site held a detached candidate or published one
conditionally, so losing all-or-nothing batches changes nothing here.

**Removed:**
- `RustyD20Product.Attach()`. Nothing called it (#8799).
- The pair names in `docs/source-provenance.md` and
  `docs/csharp-migration-map.md`. They named `cbf35130d06c` and the obsolete
  `.runtime` feed; both documents now point at the pin in
  `Directory.Build.props`.

**Kept:**
- The command-rejection catch in `Apply`. It is product policy that reports
  `rejected:<command>:<reason>`, not a callback boundary.
- The persistence revision guard on `Save`, which #8741 keeps as a real
  on-disk check.

## Checks

- `dotnet build RustyD20.sln -c Release` succeeds.
- `RustyD20.Core.Checks`: 25 of 25 pass.
- `RustyD20.Product.Checks`: "focused checks passed".

## `rusty dev`

This ran with CoreCLR on the new pair, headless. D20 runs its lifecycle in
**demand** mode, and its UI claims digital intents with no key mappings. So the
driver posts intents and then `POST /__rusty/product/runtime/admit-demand-step`
with `{}`, as the browser shell would.

- **Begin.** `d20.begin` changes `campaign.phase` from `Camp` to `Exploration`.
- **Movement.** Repeated `d20.forward` moves `exploration.location` from 1,1 to
  5,1 (log `explore:StepForward`).
- **Save and load.** `d20.save`, then `d20.back` (to 4,1), then `d20.load`
  restores 5,1. This round-trips through the migrated `ProductStateStore`.
- **A rejection.** A repeated `d20.begin` is rejected with the product's own
  message ("Command requires Camp, current phase is Exploration.").
- No `CSHARP_*` diagnostic appeared.

**A driving note for headless checks.** In demand mode, intents are accepted
and reported consumed, but nothing updates until a demand step is admitted.

## Filed Engine tasks

None found.
