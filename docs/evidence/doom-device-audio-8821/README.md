# Doom on the current pair, with its sounds on the device path (#8821, Doom #8830)

**Question.** Does Doom build and play on the newest Engine pair? Do its weapon
and door sounds (Doom `c080e46`, #8814) reach an output device through the
Rust audio path (#8789)?

**Answer.** Yes.
- Doom moved from pair `e14db30ba217` to `f1d747afc01c`, which is Engine main
  on 2026-09-29.
- The port is mechanical, plus deleting two tuning values that existed only
  for removed Engine caps.
- In one room-study run on `RUSTY_AUDIO_OUTPUT=device`, every pistol shot and
  the north-wing door opening were recorded from a null sink.
- The recordings match the shipped WAV clips at correlation 0.999, and
  unplayed clips stay at 0.28 or below.

## The port (Doom `d209bc9`)

The pair move brought 34 compile errors. Every one is covered by a migration
note:

| Change | Engine source |
|---|---|
| `XLeaseReceipt` → `XResult` (`VoxelAssetSpatialPublish`, `VoxelSceneMaterialMapping`, `PerceptionReadout`, `Authored*`, `ImplicitAnalysisReport`), `DynamicsStepAndReadBody` → `DynamicsBodyFact` | #8744, #8817 |
| Headless test context drops `IContentStoreService ContentStore` | #8763 |
| `SpatialTriggerSetActiveRequest` / `SpatialTriggerRestoreRequest` lose the expected revision. The `ReadTrigger` calls that only fetched it are gone. | #8754 |
| `ReconcileTriggers(tick, cause)`: no entity or fact bounds, and no `FactsTruncated` | #8741 |
| `EntityKinematicMotion.Prepare(...).Apply()` → `Step(session, dt, selection)` | #8741 |
| `InventoryEdit.Validate()` removed from the pickup preflight. `Grant` checks capacity before and after each operation. | #8741 |
| `PublishAttachedSnapshot` → `PublishChanges(upserts, removals, attachments)` in `tools/attachment-example` | #8737 |

**Removed from Doom**, because they only fed Engine caps that no longer exist:
- the tuning values `MaximumPickupFactReadback` and `MaximumSpatialEntityBindings`;
- their HUD readout entries;
- their lifecycle-exercise assertion;
- a product-side hazard overlap-count check against the second value.

**Review.** Doom's three standing review lanes found no Engine reuse issue.
One runtime-trust finding was fixed:
- pickup settlement now collects before retiring the trigger;
- the old retire, collect, reactivate-on-failure sequence existed only
  because the revision-fenced call could fail.

Two product-reuse findings are Doom #8845, because the trigger-revision
mirror is in the save shape. The record is Doom
`docs/agent-review/engine-pair-f1d747af-20260929.md`.

**Checks.**
- `scripts/verify-csharp-spine.sh` passes: semantic catalog, build with 0
  warnings, recipe animation and lifecycle exercises, NativeAOT staging.
- `node scripts/audit-boundary.mjs` passes (109 operational files).

## Device audio run

**Setup.**
- The staged CoreCLR product, from `StageRustyEngineCoreClrProduct` with live
  debug on and port 0.
- The pair's runtime pack, run through `../audio-8789/device-proof.sh`: the
  host runs with `RUSTY_AUDIO_OUTPUT=device` into a PulseAudio null sink, and
  its monitor is recorded.
- [`drive-door-and-pistol.py`](drive-door-and-pistol.py) plays the room study
  with ordinary physical keyboard input posted to the runtime input route:
  - W to walk;
  - `ControlLeft` to fire;
  - E to use;
  - `playtest.look` to turn, which goes through the product's look rules.

**The route.**
- `navigation.route door-north-wing` returns `NoPath`, as #8719 recorded.
- The waypoints were read from `spatial.map-at` instead. They keep to the
  level walkway around the spawn room's recessed centre.
- At the door, the driver turns by the product's own aim guidance
  (`interaction.inspect` `yawDeltaDegrees`) before pressing E.

**Health.**
- The corridor trooper kills the player at 100 health before the corridor.
- The driver therefore sets armour to 100 once, and tops health back up to 100
  before each leg and shot with the debug command `loading-bay.set-track`.
- 100 is the room study's authored maximum. This is a sound recording, not a
  combat test.

**Events** ([`drive-events.jsonl`](drive-events.jsonl)). The script started
6.79 s into the recording. Each event is logged after the key's release, about
0.2 s after the press.
- **Pistol:** one shot at spawn, before any enemy is awake. Four more in the
  corridor:
  - trooper 31001 took 30 → 15 → down in two hits (`aimHit` names it at
    3.9 m);
  - the second trooper (31002) took two more shots.
- **Door:** the driver reached the vestibule, turned by the aim guidance, and
  pressed E. Door 20000 reported `open`, and the player walked through to
  x = 7.83.

**Recording** ([`clip-matches.txt`](clip-matches.txt), from
[`match-clips.py`](match-clips.py)). Each clip is slid over the mono recording
and scored by normalized correlation:

| Clip | Played | Best score | Matches above 0.6 |
|---|---|---|---|
| `DSPISTOL.wav` | 5 shots | 0.999 | 5, at 7.08, 34.41, 35.22, 36.03, 36.84 s (fire logged at 7.24, 34.58, 35.39, 36.20, 37.00 s) |
| `DSDOROPN.wav` | 1 door | 0.999 | 1, at 61.44 s (E pressed at about 61.5 s) |
| `DSSHOTGN.wav` | no | 0.141 | none |
| `DSPUNCH.wav` | no | 0.283 | none |
| `DSDORCLS.wav` | no | 0.068 | none |

An earlier run of the same drive recorded 13 pistol shots and one door
opening, also all at 0.999.

## Limits

- **Sound position.** The door sound is emitted from the door's position, but
  the device path has no listener pose or emitter positions yet (#8813). These
  sounds play unpositioned.
- **Assisted route.** The waypoints and health top-ups are assistance for the
  recording, not unaided play.
- **Doom's static route.** The route query still returns `NoPath` from spawn to
  the north-wing door. That is Doom's navigation data, not an Engine audio
  question, and it is unchanged by this task.
- **Run timing.** The recorded run came before the settlement reorder. The
  reorder touches only pickup settlement, and the lifecycle exercise covers
  it.
- **Future pairs.** Doom moves again with later pairs. #8744's remaining
  families and further #8723 changes will need the same kind of rename.
