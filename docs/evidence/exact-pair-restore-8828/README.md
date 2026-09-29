# Plain dotnet restores exactly the pinned SDK (#8828)

## Problem

Products referenced `Version="$(RustyEnginePackageVersion)"`, which NuGet reads
as a minimum. The pair's feed came only from the CLI's
`RestoreAdditionalProjectSources` environment variable (`rusty build`,
`rusty dev`, `export $(rusty env)`). Any other restore of a pin missing from
`~/.nuget/packages` therefore took a higher-sorting cached dev pair; hash
versions sort as strings. That happened right after every `rusty update`.

## What changed

**Products.** Each product's `Directory.Build.props` declares the pinned pair's
feed beside the pin, with a target that fails an uninstalled pin with the fix:

```xml
<RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(RUSTY_ENGINE_CACHE)</RustyEngineCache>
<RustyEngineCache Condition="'$(RustyEngineCache)' == '' and '$(XDG_CACHE_HOME)' != ''">$(XDG_CACHE_HOME)/rusty-engine</RustyEngineCache>
<RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(HOME)/.cache/rusty-engine</RustyEngineCache>
<RestoreAdditionalProjectSources>$(RestoreAdditionalProjectSources);$(RustyEngineCache)/pairs/$(RustyEnginePackageVersion)/sdk-feed</RestoreAdditionalProjectSources>
…
<Target Name="RequireRustyEnginePair" BeforeTargets="Restore;_GenerateRestoreGraph" Condition="!Exists('…/sdk-feed')">
  <Error Text="Rusty Engine pair $(RustyEnginePackageVersion) is not installed: run `rusty install` in this repository." />
</Target>
```

Every `Rusty.Engine` reference is exact:
`Version="[$(RustyEnginePackageVersion)]"`. The cache lookup mirrors the CLI:
`RUSTY_ENGINE_CACHE`, then `XDG_CACHE_HOME/rusty-engine`, then
`~/.cache/rusty-engine`. `rusty update` still rewrites only the version element.

**CLI.**

- `rusty status` reports `project shape`. It checks for the feed declaration in
  the pin file and scans the repository's `.csproj`/`.props`/`.targets` for
  `Rusty.Engine` references that are not exact. Each problem prints the exact
  snippet, and the product is reported not ready.
- `rusty build` and `rusty dev` print the same findings as
  `RUSTY_PROJECT_SHAPE:` warnings and continue.
- **Removed:** the CLI's `RestoreAdditionalProjectSources` environment
  injection and the `rusty env` command. Nothing needs them once the product
  declares its feed. Scripts that ran a host directly and took `DOTNET_ROOT`
  from `rusty env` now derive it from `dotnet` themselves.

**Docs.** The SDK entry page, the distribution guide and the product-project
reference show the full `Directory.Build.props` and exact reference. The
troubleshooting entry for `rusty env` became: "not installed" means
`rusty install`, and NU1101/NU1603 means check `rusty status`.

## Applied

| Product | Pin | Commit |
|---|---|---|
| rusty-template | `8f08ab04275f` | `9d9c1fd` |
| rusty-dagger | `a44170f62529` | `06b51c9` |
| rusty-craftsurvive | `56c9322a179b` | `37e87e3` (CI drops `rusty env`) |
| rusty-crawler | `eedd406900d5` | `d5740e7` |
| rusty-underworld | `360a1ce508a9` | `8e63b5d` |
| rusty-dungeon | `360a1ce508a9` | `0f89dae` |
| rusty-rifles | `a525a33ff441` | `2d7c082` |
| rusty-space | `681ec653ab6a` | `3cdee15` |
| rusty-d20 | `bcf02594620c` | `fdfc9a2` |
| rusty-roguelike | `bcf02594620c` | `4aca8e3` |

Other notes:

- rusty-crawler's standalone `tools/portable-assets-example` needs
  `PortableAssetContent`, which is newer than the product pin. It now has its
  own `Directory.Build.props` pinning `feec788503fe`; MSBuild and `rusty` both
  take the nearest one. It previously restored from asset-pipeline's product
  feed.
- The architecture tests in Dagger and underworld asserted the old minimum
  form; they now assert the exact form.
- `rc-live-b` is a live checkout of rusty-crawler with local edits. It picks
  this up when it pulls `main`.
- asset-pipeline and Doom move with #8822.

## Evidence

For each product, `rusty status` reported the new shape as ready. A restore of
one Engine-referencing project was then run with no
`RestoreAdditionalProjectSources` in the environment. Its NuGet package
folder held all 108 cached packages and every cached `Rusty.Engine` version
except the pin: 188 competing versions, symlinked from `~/.nuget/packages`.
A second restore used an empty `RUSTY_ENGINE_CACHE`.

| Product (project) | Resolved | Uninstalled pin |
|---|---|---|
| template (Game) | `8f08ab04275f` | error: pair … is not installed: run `rusty install` |
| dagger (Kit.Tests) | `a44170f62529` | same error |
| craftsurvive (RpgCore) | `56c9322a179b` | same error |
| crawler (Host.Tests) | `eedd406900d5` | same error |
| underworld (Kit.Tests) | `360a1ce508a9` | same error |
| dungeon (Host.Tests) | `360a1ce508a9` | same error |
| rifles (Game) | `a525a33ff441` | same error |
| space (Game) | `681ec653ab6a` | same error |
| d20 (Core.Checks) | `bcf02594620c` | same error |
| roguelike (Product.Checks) | `bcf02594620c` | same error |

Before the change, `rusty status` on Dagger listed the missing feed
declaration, with the snippet, plus its four minimum references.

Also run:

- underworld and dungeon `verify.sh` pass without `rusty env`.
- Dagger `WorldRpg.Kit.Tests` passes 140/140 on exactly `a44170f62529`, with no
  NU16xx warning. `WorldRpg.Rulesets.Daggerfall.Tests` passes 1250 of 1251;
  the one failure is rusty-dagger #8811.
- `cargo test -p rusty-cli` passes (24 + 1, including a shape-check test), and
  clippy is clean.

## Migration

For a product still on the old shape:

- Add the feed block and target above to `Directory.Build.props` (or copy the
  lines `rusty status` prints).
- Bracket every `Rusty.Engine` version.
- Delete any `export $(rusty env)` or `rusty env >> "$GITHUB_ENV"` line; the
  command no longer exists.

## Review fix: order-independent reference detection

The first check read a reference only as `Include="Rusty.Engine"` followed by
`Version=` inside the same tag. It therefore passed
`<PackageReference Version="…" Include="Rusty.Engine" />` and a child
`<Version>…</Version>` element, both minimum references. The check now reads
each `PackageReference` and `PackageVersion` start tag as a whole:

- `Include` or `Update`, with attributes in any order;
- single or double quotes;
- the `Version` attribute, or else a child `<Version>` element.

A unit test covers nine forms, including a longer package name and an
unrelated `<Version>` element, neither of which may match.

The reviewer's probe, rerun with this build (the Dagger feed declaration plus a
minimal project):

| Reference | `rusty status` |
|---|---|
| `Include` then minimum `Version` | needs changes, exit 1 |
| minimum `Version` then `Include` | needs changes, exit 1 |
| child `<Version>` minimum | needs changes, exit 1 |
| `Version="[$(RustyEnginePackageVersion)]"` then `Include` | exact pin, pair feed declared, exit 0 |

All twelve migrated products report `exact pin, pair feed declared`, checked
on asset-pipeline's pushed `main`.

The rerun found one real case that `~/.local/bin/rusty` had missed because
that build predated the check. asset-pipeline's obsolete
`legacy/tools/micro-voxel-studio` `MicroVoxel.Host` pinned its own unpublished
`0.1.0-dev.8752652edcef` as a minimum. It now references the repository pin
exactly (asset-pipeline `4ab29c9`). It does not compile on current pairs
(`IEngineContext.Appearance` is gone), but it now fails there instead of
restoring an arbitrary cached SDK. Nothing builds it; the recipe job uses
`MicroVoxel.Cli`.
