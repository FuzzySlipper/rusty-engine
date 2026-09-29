# Space moves to pair 91d7f9aa4f6b (#8837)

## Pair

`0.1.0-dev.681ec653ab6a` → `0.1.0-dev.91d7f9aa4f6b`, the newest published
pair on 2026-09-29, via `rusty update`.

rusty-space `d1d8695` and `b1e04bd`: 9 files.

## Migrations applied

| Change | Source | Sites |
|---|---|---|
| Hull contacts: `ReadWorld(...).ContactCount` plus `ReadContactAt(i)` → one `ReadWorld(...).Contacts.Span` | #8744 | 1 |
| Test fixtures: `DynamicsContactAtReceipt(Present: true, ...)` → `DynamicsContact(...)` | #8744 | 9 |
| The recording Engine doubles follow the current services: results in place of leases and indexed reads; rope, tether and chain members; `ReadTextureInfo`; `PublishChanges`; `SetBackgroundColor`; `SetSkyBackgroundBlend`; `Input`, `RenderOutput` and `Video`; no `ContentStore` | #8744, #8817, #8840, #8763 | 1 file |

**Changed because the rollback is gone (#8736).**
- `BridgeTheater` and `SpacePresentation` deliberately released nothing when
  their constructors failed partway, because "the failed create call … the
  Engine discards whole". Since #8736 a failed call keeps what it did, and an
  owner's `Dispose` releases at once.
- Both constructors now release what they had opened before rethrowing, and
  `SpacePresentation.Dispose` tolerates a partial owner.
- Two composition tests asserted the old contract. One was named
  `ATheaterThatFailsItsFirstVoiceLeavesItsClipsToTheFailedCreate` and
  expected 0 clip releases. They now assert the new one: the theater releases
  its 2 clips, and the projection its 17 appearances.
- The `Shutdown` and `Dispose` comments named the removed lease coordinator
  and staged calls, and were corrected (`b1e04bd`).

**Removed:**
- `MaximumContactsToRead = 8`, which only bounded the per-contact read calls.
- `SpaceProduct.Attach()` (#8799).

## #8798 bounds

#8798 removed the 1e6 translation and 1e4 velocity bounds. Space has no
workaround for either:
- no origin shifting;
- no coordinate or velocity clamping;
- no reference to those limits.

The only speed limit is `FlightTuning.MaximumSpeed` (12), a gameplay tuning
value for the ship's drive. Nothing needed removing.

## Checks

- Release build of `Product.Game` and the tests succeeds.
- `Product.Game.Tests`: 248 of 248 pass.
- `StageRustyEngineCoreClrProduct` stages the product.

No CI workflow exists.

## `rusty dev`

This ran with CoreCLR on the new pair, headless.
- **Diagnostics.** `space.telemetry`, `space.impacts` (chart
  `kestrel-approach`, 5 authored obstacles, no faults) and
  `space.controller` answer.
- **Flight.** A physical `key-w` held for 2 s (`space.flight.thrust`) raises
  `space.attitude` speed from 2.881 to 7.260 and moves the ship from
  (65.98, 35.25) to (74.41, 48.27).
- No `CSHARP_*` diagnostic appeared.

## Filed Engine tasks

None found.

**A note for the Dagger lane.**
`rusty-dagger/src/WorldRpg.SpriteWorkbench/SpriteWorkbenchProduct.cs:352`
still says "Failed Create calls can return a wrapper whose native side was
never committed". That is the same pre-#8736 assumption. Its cleanup code is
still correct, but the comment is stale.
