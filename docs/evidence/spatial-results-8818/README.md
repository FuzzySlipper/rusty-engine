# Spatial character and world-origin reads as borrowed results (#8818)

## What changed

Both index calls (one FFI crossing per element) become one call that returns a
borrowed `Native*Result`, the shape #8744 used for Dynamics. The backing sits
in the Spatial bridge's `BorrowedResult` until the next result on that bridge,
and the generated binding copies it before returning.

- **Character.** `ReadCharacterController` returns `NativeCharacterControllerResult`:
  the latest proposal's `contacts` plus the former readout scalars. The
  following are removed:
  - `ReadCharacterContactAt`, with `NativeCharacterContactAtRequest`/`Receipt`;
  - `ReadCharacterDynamicImpulseAt`, with its request/receipt and
    `NativeCharacterDynamicImpulse`;
  - `NativeCharacterControllerReadout`, with its `contact_count` and
    `dynamic_impulse_count` (the length now carries the first).
- **Dynamic impulses are not carried over.** The bridge's `CharacterStepWorld`
  reports no rigid bodies (`rigid_body_linear_velocity` returns `None`), so the
  impulse list was always empty through the C# ABI, and nothing read it. The
  step receipt's always-zero `dynamic_impulse_count` stays for now, because two
  downstream tests pass it by name; #8840 removes it.
- **World origin.** `ReadPrepared` returns `NativeWorldOriginPreparedResult`:
  the `affected` root transforms (`NativeWorldOriginAffectedTransform`) plus
  the target cell and envelope. Removed: `ReadAffectedAt`, with
  `NativeWorldOriginAffectedAtRequest`/`Receipt`, and
  `NativeWorldOriginPreparedReadout`, with its always-true `present` and
  `affected_entity_count`.
- `EntityOriginRebaser.Prepare` makes one read instead of 1 + N.
  `EntityOriginRebaserPrepared.Receipt` is now the generated
  `WorldOriginPreparedResult`, and the `EntityOriginRebaserPrepareReceipt`
  wrapper is deleted.

## Evidence

- `cargo test -p csharp-engine-services` passes. The world-origin bridge test
  copies `affected` from the borrowed result (entity, translation, scale,
  rotation). The moving-platform character test reads the controller result
  after a proposal: one contact, matching the step receipt's
  `contact_count`, source entity and kind.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
  Clippy on the touched crates is clean.
- Bindings regenerated.
- `Rusty.Engine.Entities.Example` runs clean: `EntityOriginRebaser` reads
  `Receipt.Affected` from one call.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes.
- `scripts/test-runtime-pack.sh` passes. It publishes and launches the
  NativeAOT-trial fixture, whose constructor runs `ExerciseCharacterController`
  and now checks `ReadCharacterController(...).Contacts` against the step
  receipt.

## Migration

| Before | After |
|---|---|
| `Spatial.ReadCharacterController(req)` → `CharacterControllerReadout` | → `CharacterControllerResult` (same scalars, plus `Contacts`; no `ContactCount`/`DynamicImpulseCount`) |
| `Spatial.ReadCharacterContactAt(new(session, i))` → `.Present`, `.Contact` | `ReadCharacterController(new(session)).Contacts.Span[i]` |
| `Spatial.ReadCharacterDynamicImpulseAt(...)` | removed (always empty through the C# bridge) |
| `WorldOrigin.ReadPrepared(req)` → `WorldOriginPreparedReadout` | → `WorldOriginPreparedResult` (`Affected`, target cells, `LocalEnvelope`) |
| `WorldOrigin.ReadAffectedAt(new(prepared, i))` → `.Present`, `.EntityId`, `.LocalTransform` | `ReadPrepared(new(prepared)).Affected.Span[i]` → `.EntityId`, `.LocalTransform` |
| `EntityOriginRebaserPrepared.Receipt.Native` / `.Affected` | `Receipt` is the `WorldOriginPreparedResult`; `Receipt.Affected` |

Downstream:
- **rusty-craftsurvive** `Modules/Player/PlayerController.cs` (around line 577)
  reads two roots with `ReadAffectedAt`. At its next pin move:
  - read `engine.WorldOrigin.ReadPrepared(new(prepared)).Affected.Span` once,
    and check `EntityId`, not `Present`;
  - drop the three revision arguments from `WorldOriginPrepareRequest` (#8807);
  - optionally replace its hand-written motion shift with
    `motion.Rebased(localTranslation)` (#8806).

  Its current pin (`56c9322a179b`) predates both changes, so it builds today.
- rusty-dagger, the template and the other products use none of these calls.
