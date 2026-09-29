# Dagger and CraftSurvive build their UI through the SDK target (#8800)

## What landed

**rusty-dagger** (pin `0.1.0-dev.db2bb445aeaa`, the #8743 pair itself):

- `433ada1`: #8743's prepared `WorldRpg.Host` change, rebased onto #8779's
  CLI migration. `tsc` moved into `RustyEngineProductUiBuildCommand`, with
  `package.json`/`package-lock.json` as `RustyEngineProductUiInput`.
- `5e12378`: the same for `WorldRpg.SpriteWorkbench`, which the prepared
  branch had missed.
  - Its `tsc` hook ran before `GenerateRustyEngineProductComposition`. That
    target is gone since #8775, so the hook would silently stop at the next
    pin.
  - The atlas and stylesheet copy reads the per-run content root. It stays a
    small copy target, but now runs `BeforeTargets="BuildRustyEngineProductUi"`.

**rusty-craftsurvive** had to move off `0.1.0-dev.1aecde636cd3` first:

- `0e0702d` adopts `0.1.0-dev.56c9322a179b`, the latest pair at the time:
  - **#8739:** edits and residency apply in place. The expected revision, the
    stale-revision outcome, the residency revision/policy/hashes and the
    chunk-lease map all go.
  - **#8741:** `Destroy`/`Commit`/`Prepare`/`Publish` lose their guard and cap
    arguments.
  - **#8754:** the rope rebase no longer passes the dynamics generation.
  - **The live substrate proof** now admits a distant chunk with
    `ApplyResidency`. Its background-preparation and cancellation steps went
    with the mechanism.
  - **`tests/SubstrateProof`:** its checks for refused stale commits and
    publishes went with the guards they asserted.
- `0c3eb65` fixes the proof's entity-projection stage. On current pairs a
  standalone projection publish is accepted, so the proof disposed a marker
  appearance that its published snapshot still held
  (`CSHARP_APPEARANCE_IN_USE`). It now publishes an empty projection first.
  The same stage also failed on `1aecde636cd3`, so this repairs an older
  failure rather than a bump regression.
- `96bdebe`: #8743's prepared `CraftSurvive.Game` change, rebased onto the bump.

## Evidence

Timestamps of the UI output (`main.js`, changed only by `tsc`) and of the
product assembly (changed only by a C# build) were compared against
`rusty dev` events on the live products:

| Case | Dagger `WorldRpg.Host` | CraftSurvive |
|---|---|---|
| warm `rusty dev` start (no source change since the last build) | no `tsc`, no C# build, served | no `tsc`, no C# build, served |
| C#-only edit | C# build, no `tsc`, `runtime-replaced` | C# build, no `tsc`, `runtime-replaced` |
| UI-only edit | `tsc`, no C# build, `assets-restaged`, no replacement, same host pid; new UI served after about 14 s | `tsc`, no C# build, `assets-restaged`, no replacement, same host pid; new UI served |

Dagger's sprite workbench, run through `src/scripts/run-sprite-workbench.sh`,
served `workbench.js` and `workbench.css`, and 386 atlas pages were staged
into its UI root.

CraftSurvive on the new pair:

- CI run 36547459760 (bootstrap + `rusty env`) passed on `96bdebe`. All eight CI checks also passed locally: Procgen, Workbench, Procgen tool
  self-check, TerrainResidency, SubstrateProof, RpgCore, DiscoveryCore, and the
  Release product build.
- The live substrate proof (`CRAFTSURVIVE_SCENE=traversal CRAFTSURVIVE_PROOF=substrate`)
  printed `residency admission: … resident chunks 54 -> 55` and
  `live substrate proof PASSED`.
- Its navigation query reports `StartNotWalkable` from cell (16, 4, 16) on
  both the old and the new pin. It is pre-existing, and the proof does not
  gate on it.

## Notes

- Dagger's first warm start in one run hit the host's 30-second
  `DEV_HOST_RUNTIME_STARTUP_TIMEOUT`, then served after the supervisor's
  automatic restart. It did not recur in two later runs, and staging did not
  change in this task.
- Dagger's `scripts/verify.sh` still stops at the product test tracked in
  rusty-dagger #8811.

## Migration

Other products with a `tsc` target hooked to
`GenerateRustyEngineProductComposition` or `StageRustyEngineCoreClrProduct`
should declare `RustyEngineProductUiSourceRoot` and
`RustyEngineProductUiBuildCommand` instead. Keep any non-compile asset copy as
a target that runs `BeforeTargets="BuildRustyEngineProductUi"`.
