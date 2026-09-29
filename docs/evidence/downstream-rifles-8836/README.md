# Rifles moves to pair 91d7f9aa4f6b (#8836)

## Pair

`0.1.0-dev.a525a33ff441` → `0.1.0-dev.91d7f9aa4f6b`, the newest published
pair on 2026-09-29, via `rusty update`.

rusty-rifles `75b692c`: 7 files. The untracked art files in
`content/art/style/` are someone else's work and were left unstaged.

## Migrations applied

| Change | Source | Sites |
|---|---|---|
| Explicit inventory stacks: `Grant`, `Consume` and `TransferFungible` take an `InventoryStackId` | 1126859c8 | 7 |
| `InventoryStore.Prepare(expectedRevision)` → `Prepare()`; `InventoryEdit.Validate` is gone | #8741 | 4 |
| `EntityStore.Destroy(entity, null)` → `Destroy(entity)` | #8741 | 1 |
| `VoxelEditTransaction(session, before.SourceRevision, edits)` → `(session, edits)`; drop the `ReadScene` calls that only fetched it | #8739 | 2 |
| Weighted path: `PathLen` → `Path.Length`; the next cell is `Path.Span[1]` rather than `ReadNavigationPathCellAt` | #8840 | 1 |
| `Audio.Read().RetainedDiagnosticCount` plus `ReadDiagnosticAt` → `Audio.Read().Diagnostics` | #8840 | 1 |
| `AuthoredCatalogReadoutLeaseReceipt` → `…Result`; `NavigationStepReceipt` → `NavigationStepResult` | #8817, #8840 | 2 |

**Stack identity.** The party grid tokens (`s:<definition>`) and the saves
(unique by definition) already keep one fungible stack per definition per
owner. So `Stack(definition)` is `InventoryStackId.Parse(definition.Id)`, and
transfers name the same ID on both sides, which merges as before.

**Revision checks, decided in the product:**
- **Kept.** `Transfer`, `Equip`, `Unequip` and `Arrange` receive a revision
  from UI payloads (`RiflesProduct.Items.cs`), and a stale selection used to
  be refused by the Engine inside `Prepare(expectedRevision)`. The product
  keeps that behaviour with one `RequireRevision` helper. It is the same check
  `Arrange` already made in product code: "Inventory changed; select the item
  again."
- **Removed.** `PrepareUse(item, inventory.Revision)` passed a revision read
  on the same line and then published at once, so it becomes a direct `Use`.

**Removed:**
- **The 4,096-edit `EngineVoxelEditLimit` batching.** It worked around a cap
  that #8739 removed, so the floor build is now one transaction.
- **`RiflesProduct.Attach()` and `DungeonScene.Attach()`.** Only the product's
  `Attach` called the scene one (#8799).

## Checks

`bash scripts/check.sh` exits 0:
- `check:ui` passes.
- The Release build succeeds.
- The procgen and game checks all pass, including "Inventory checks passed:
  Engine item ledger, equipment, saves, party state, and world anchors".
- The procgen tool self-check passes.
- CoreCLR staging succeeds.

No CI workflow exists.

## `rusty dev`

This ran with CoreCLR on the new pair, headless.

- **View composition.** Each frame publishes one `view-composition` with one
  perspective camera (`fovY` 75, near 0.05, far 250) and one full-viewport
  primary view.
  - The current product source has no second view and no viewmodel layer:
    one active camera, and `RenderLayer.Scene` appearances only.
  - So the task's note (from #8774's scope) that Rifles uses composed views
    does not match this revision, and no composed view or viewmodel was there
    to check.
- **Audio.** `rifles.audio.read` answers through the migrated `AudioResult`
  (5 clips admitted, empty diagnostics).
- **The session.** It starts paused ("Expedition ready. Press P to begin").
  - `key-p` unpauses, and the world simulates: patrols move and a melee
    starts ("Blade took 1 damage.").
  - `key-e` turns the party from North to East.
- **Parity with the old pin.** The old pin `a525a33ff441`, run the same way
  from a detached worktree, gives the same turn and the same blocked step
  north.
- **Stepping was not observed** on either pin; the party stays at (99, 98). It
  is probably held by the melee that starts at spawn. Grid movement is covered
  by `check.sh`'s game checks ("grid actions/recovery").
- No `CSHARP_*` diagnostic appeared.

## Filed Engine tasks

None found.
