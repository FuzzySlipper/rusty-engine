# CraftSurvive moves to pair 6faeaaaa619c (#8832)

## Pair

`0.1.0-dev.56c9322a179b` → `0.1.0-dev.6faeaaaa619c`, the newest published
pair on 2026-09-29, via `rusty update`. The move crosses 17 pairs, including
`a44170f62529`, which carries the campaign gate (#8744, #8799 and #8807). So
this one move already covers the gate, and CraftSurvive needs no second move.

rusty-craftsurvive `4e6cc93`: 14 files, 29 lines added and 145 removed.

## Migrations applied

Mechanical, from the pairs' notes:

| Change | Source | Files |
|---|---|---|
| `XLeaseReceipt` → `XResult` (`AuthoredCatalogReadout`, `VoxelSceneMaterialMapping`, `PerceptionReadout`) | #8817 | 6 |
| `NavigationPathReadout` → `NavigationPathResult`; `.PathLen` → `.Path.Length` | #8840 | 2 |
| `WorldOriginPrepareRequest` loses its three expected revisions | #8807 | 1 |
| `ReadAffectedAt(i)` → `ReadPrepared(...).Affected.Span[i]` (`WorldOriginAffectedTransform`, no `Present`) | #8818 | 1 |
| The rebase's hand-written motion shift → `CharacterMotion.Rebased(delta)` | #8806 | 1 |

Removed because the reset made them unnecessary:

- **The `Attach()` chain.** Six methods in the product, terrain, player, sky,
  ghost plate and microvoxels, 63 lines in all. The host has not called
  attach since presentation began being rebuilt from committed Engine state
  (b777eb3e4), and #8799 removed the callback.
- **The `Update` catch-all.** It swallowed any exception, dropped the frame and
  counted the failure for a `craft.product.health` debug probe, on the
  assumption that an escaping exception taints the runtime until respawn.
  Since #8736 an escape is logged with its stack, faults the lifecycle and
  can be resumed. So the catch now only hid failures, and it went with its
  probe.
- **The "did prepare keep my roots" check.** `prepare` maps every requested
  root, in request order, or fails, so the product indexes `Affected` directly.

## Checks

- `dotnet build src/CraftSurvive.Game -c Release`: succeeds. It shows the
  same nine warnings as the old pin, all product-local (nullable, unused
  fields, duplicate using).
- Managed checks (`dotnet run -c Release`) all pass: `DiscoveryCore`,
  `Procgen`, `RpgCore`, `SubstrateProof`, `TerrainResidency` and `Workbench`.
  (`EngineSurface` is a probe tool that needs a DLL path, not a check.)
- `pnpm run check:ui` and `pnpm run audit:textures` pass.
- The product's CI `verify` workflow passes on `4e6cc93` (run 36570544092).

## `rusty dev`

This run used `CRAFTSURVIVE_SCENE=traversal CRAFTSURVIVE_PROOF=substrate` on
the new pair (CoreCLR), driven headless through the runtime input route and
`rusty-live-debug`:

- **The live substrate proof passes.** It covers state cells, product-path
  and volume edits with undo, the chunk cache, generation, swimming,
  navigation replace and query, persistence, a second dimension session and
  entity projection.
  - The navigation query answered `StartNotWalkable` from its start cell. The
    proof accepts any typed outcome by design, because the terrain there came
    from a world overlay restored from an earlier run.
- **Walking.** A physical `key-w` press and release over 1.5 s was accepted
  (`DEV_HOST_INPUT_QUEUED`). The player moved from z=11.98 to z=-0.59.
- **The world-origin rebase (the migrated path).** `craft.player.teleport
  3000 40 -2500`, then walking:
  - the player component reads global `3000.000, 4.890, -2509.963`, while the
    controller's local position is about `0, 5.4, -9.4`, so the origin moved;
  - the lifecycle stayed `Running`;
  - no `CSHARP_*` diagnostics appeared.
- The courtyard rope rebase (`Dynamics.RebaseWorldOrigin`) was not exercised.
  Its call is unchanged by this move.

## Filed Engine tasks

None found. Every failure was a straight migration listed in the pairs'
notes.

One small inaccuracy in the notes: `a44170f62529` (#8744) says CraftSurvive's
`RopePlayground.Rebase` reads `DynamicsWorldReadout`. It hasn't since the
#8754 migration, so no rename was needed.

## Reusable notes for the other products (#8831)

1. Run `rusty update`. The newest pair already contains the gate, so a single
   move is enough.
2. Build first, and let the compiler list the renames. Then read the
   `## Migration` sections of these pairs:
   - `d189660a00b9` (#8817, the `LeaseReceipt` → `Result` rule);
   - `516912929c19` (#8840, the table of indexed reads);
   - `3d3d55ded594` (#8818, character contacts and world origin);
   - `a44170f62529` (#8744, #8799, #8807).
3. Delete the product's `Attach()` and everything reachable only from it. It
   compiles unchanged, which is why it silently survives.
4. Delete any catch-all around `Update` whose reason is taint or respawn.
5. Delete retries or revision bookkeeping for removed guards, and presence
   checks on results that are now spans.

Hits from a grep of each product at its current pin (the file counts are
hints, not a complete list):

| Product | `LeaseReceipt` | `Attach()` | taint | `Read*At(` | `Expected*Revision` |
|---|---|---|---|---|---|
| underworld | 4 | 1 | 3 | 0 | 1 |
| roguelike | 1 | 2 | 0 | 1 | 1 |
| crawler | 0 | 1 | 2 | 0 | 1 |
| rifles | 1 | 1 | 0 | 2 | 0 |
| space | 1 | 1 | 0 | 2 | 0 |
| d20 | 0 | 1 | 0 | 0 | 0 |
| dungeon | 0 | 0 | 0 | 0 | 0 |
