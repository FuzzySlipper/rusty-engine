# Dungeon moves to pair 8101e06bc8cf (#8834)

## Pair

`0.1.0-dev.360a1ce508a9` → `0.1.0-dev.8101e06bc8cf`, the newest published
pair on 2026-09-29, via `rusty update`.

rusty-dungeon `a69b241` changes only the pin.

## Migrations applied

None were needed. The product and its tests use none of the renamed results,
the removed guards or the dropped `Attach` callback.

A grep for the reset's leftovers found nothing to remove either: no `Attach`,
taint handling, expected revisions, `Prepare(revision)`, `Read*At` or leases.
The one catch-all collects disposal failures in `DelveSceneRenderer.Dispose`
and is not a callback boundary.

## Checks

`./scripts/verify.sh` reports "all checks green":
- UI: 8 DOM tests pass.
- Architecture: 6 pass.
- Rulesets: 19 pass.
- Host: 19 pass.
- Import: 36 pass.
- Kit: 57 pass.
- CoreCLR staging succeeds.

The repository has no CI workflow.

## `rusty dev`

This ran with CoreCLR on the new pair, headless, driven through the runtime
input route and read from the `delve.hud` UI projection:

- The product starts in `phase: run` on dungeon level 1 (hp 12/12, one potion
  on the hotbar).
- A physical `key-w` press, held for 1.5 s, then released, drives the
  `move.forward` mapping. The next attach reports `playerY` 26 → 21.
- Left idle for about a minute, the player was killed by the level's creatures
  (hp 0). That is realtime gameplay, not a fault.
- No `CSHARP_*` diagnostic appeared.

## Filed Engine tasks

None found.
