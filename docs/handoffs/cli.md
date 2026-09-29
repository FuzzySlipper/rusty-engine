# Lane: cli

**Tasks, in order:** #8781, #8810, #8800. **Start:** now.
Campaign #8777; read the campaign task and its coordination section. #8800 is
under #8723. Shared protocol: [README.md](README.md).

Round 1 of this lane landed #8779 (`e0f9545e`, `12d517f7`, `3d7ce4f0`, in
review). **It also left local downstream branches that are not pushed:**
- template `d7630ec`;
- Dagger `3097397`;
- CraftSurvive `4fd8a12`;

all on `cli-workflow-8779`. Start by checking them; this round finishes the
downstream moves.

## Tasks

- **#8781: move the product template and bootstrap guidance onto the CLI.**
  - Template: `/home/agent/dev/rusty-template`, pinned at
    `0.1.0-dev.360a1ce508a9`.
  - Split `docs/csharp-sdk.md` into a short run/update/troubleshoot entry page
    plus linked capability references. This is a hot-file edit: do it in one
    commit, rebase just before landing, and keep every section other lanes
    added.
  - Update the stale Den downstream briefing.
  - Automate template adoption of newly published pairs, with a visible
    failure when an update breaks.
  - Evidence: a fresh product from the template via the CLI, plus one
    automated template update and one failed update.
- **#8810: move the remaining products onto the rusty CLI pair pin.** For
  each product, delete its installer scripts and its `sdk-feed` copy in
  `NuGet.Config`:
  - rusty-dungeon: `scripts/install-engine.sh`;
  - rusty-crawler: `install-engine-pair.sh`, `update-engine-pin.sh`;
  - asset-pipeline: `install-engine-pair.sh`, `install-workbench-cli.sh`;
  - Doom: blocked until a pair is published.

  The same `sdk-feed` pattern also appears in d20, underworld, roguelike,
  rifles, space and rc-live-b. Move those too, or list them in the task.
  Evidence: `rusty status` ready and `rusty dev` serving for each product.
- **#8800: adopt the SDK UI build target in Dagger and CraftSurvive.**
  - Branches `engine-8743-ui-build` already exist:
    - Dagger (`src/WorldRpg.Host/WorldRpg.Host.csproj`, `57d0a6f`);
    - CraftSurvive (`src/CraftSurvive.Game/CraftSurvive.Game.csproj`,
      `9b1f7f1`).
  - Each needs a pair that contains #8743 and its review fix `fd466f035`.
    Dagger main is now at pair `db2bb445aeaa`, which contains #8743 but not
    the fix. Move both products to a current pair with `rusty update`, then
    merge.
  - Evidence: compiler invocations show `tsc` runs only for UI edits.
  - #8800 and #8810 both edit the same downstream project files, which is why
    they share this lane.

## Files

- **Owns:** see the README table.
- **Leave alone:** `Rusty.Engine.targets` (the tooling lane may touch it for
  #8808).

## Evidence

Per task, as above. Follow AGENTS.md on downstream pairs: move products to the
exact current pair; don't pin back.
