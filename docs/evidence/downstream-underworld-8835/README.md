# Underworld moves to pair 0c95655d07c0 (#8835)

## Pair

`0.1.0-dev.360a1ce508a9` → `0.1.0-dev.0c95655d07c0`, the newest published
pair on 2026-09-29, via `rusty update`. The move includes the gate pair
`a44170f62529` (#8744, #8799, #8807).

Pairs older than `a3b433435cef` publish no release notes, so the compiler and
the evidence READMEs were the guide there.

rusty-underworld `7edee9b`: 15 files, 46 lines added and 52 removed.

## Migrations applied

| Change | Source | Files |
|---|---|---|
| `PerceptionReadoutLeaseReceipt` → `PerceptionReadoutResult`; `SpritePlaybackAdvanceLeaseReceipt` → `…Result` in a test fake | #8817 | 4 |
| `NavigationStepReceipt` → `NavigationStepResult` | #8840 | 2 |
| `CharacterStepReceipt` in a test: drop `RevisionBefore`, `RevisionAfter` and `DynamicImpulseCount`; add `Movement` and `Tether` | #8754, #8840 | 1 |
| The `GraphicsDouble` test fake implements `ReadTextureInfo` and `PublishChanges` | new members | 1 |
| `InventoryStore.Prepare(expectedRevision)` → `Prepare()` | #8741 | 1 |

Removed because the reset made them unnecessary:

- **The inventory revision chain.** The container coordinator and
  `CorpseLootCoordinator` took `expectedWorldRevision` only to pass it to
  `Prepare`. The level-item callers passed `_coordinator.Read(x).StoreRevision`,
  read on the line before, so the guard could never refuse.
  - The parameters are gone from `Transfer`, `TransferAll` and their callers.
  - Dagger's copy of the coordinator (per #8741's notes) still accepts the
    parameter and ignores it.
- **The empty `AbyssProduct.Attach()`** (#8799).
- **The "taints the runtime" rationale.** It is gone from two source comments,
  two test comments and `docs/gpu-playtesting.md`. A refused character step now
  faults and pauses the product (#8736).
  - The behaviour it justifies stays: the tile probe refuses closed tiles, and
    movement is subdivided within the controller's step range.

## Checks

`scripts/verify.sh` exits 0 on the new pair:
- UI: 14 DOM tests pass.
- Architecture: 5 pass.
- Kit: 19 pass.
- Rulesets: 169 pass.
- Host: 108 pass.
- Import: 41 pass.
- CoreCLR staging succeeds.

The Import suite reads operator data that is not in Git (`local/extracted/uw`
and `content/abyss/imports`). In the task worktree, both were symlinked from
the main checkout.

The repository has no CI workflow.

## `rusty dev`

This ran from the worktree on its own port, with CoreCLR on the new pair,
driven headless through `rusty-live-debug`:

- **Start.** The product starts in `Playing` on level 1 (34/34 hp, 71 actors).
- **Travel.** `abyss.travel 2` reports "You descend to level 2." (75 actors).
- **Save and load.** `abyss.save` writes `quicksave/0`. `abyss.load` resumes
  `autosave/level-2`; the level-item restore moves items through the changed
  transfer path.
- **Other commands.** `abyss.rune`, `abyss.cast` (it answers `NotASpell` for an
  empty shelf) and `abyss.damage 5` (34 → 29 hp) all answer.
- **Operator probe.** `abyss.goto` refuses a closed tile with the product's
  own message.
- No `CSHARP_*` diagnostic appeared and the product never faulted.

**Physical keys do not move the avatar in a headless run, on either pin.**
- `key-w` and `key-d` presses and releases are accepted and consumed
  (`consumedThrough` advances), but `abyss.where` does not change.
- The old pin `360a1ce508a9`, run the same way from a second worktree, behaves
  identically. So this is not a result of the move; it probably depends on the
  product UI entering gameplay mode, which headless driving skips.
- Keyboard walking on the new pair was therefore not observed.
- **Correction (#8855).** The cause was product-side, not headless:
  `UuLocomotionPolicy` never passed the frame's movement into the step's
  `PlanarIntent`. Underworld `dcdbddf` fixes it; see
  `docs/evidence/character-substeps-8855/README.md`.

The long-running orphaned session on port 4177 (main checkout, pre-#8810
`.runtime` pack) was left alone. The main checkout still has the old working
tree; fast-forwarding it restages that session, so it is the session owner's
call.

## Filed Engine tasks

- **#8855** (#8723, priority 4): reconcile the product fixed-step rate with
  the character controller's 1/15 s step limit. Classification: unnecessary
  restriction or fail-closed behaviour.
  - The runtime accepts a fixed step below 15 Hz, and the controller then
    refuses every step.
  - Underworld (60 Hz) stays inside the range but mirrors the private bound as
    `MaximumStepSeconds`/`MinimumStepSeconds`, with a substep loop that never
    subdivides at 60 Hz.

## Notes for Dungeon (#8834, same old pin)

- Dungeon's source has no `LeaseReceipt`, `Attach()`, taint, `Read*At(` or
  `Expected*Revision` hits. Expect the move to be mostly the pin, plus any
  fakes implementing SDK service interfaces.
- The fakes are the likely break: `IGraphicsService` gained `ReadTextureInfo`
  and `PublishChanges`, and `CharacterStepReceipt` changed shape.
