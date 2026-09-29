# NativeAOT trial publish emitted a CoreCLR app (#8808, #8804)

## Cause

The cause was in the repo, not the environment. `fixtures/csharp-nativeaot-trial`
held two projects in one directory: `CsharpNativeAotTrial.csproj` (the
NativeAOT product) and `CsharpNativeAotTrial.Game.csproj` (a library holding
`Product.cs`). Both used the default `obj/`, so both restores wrote the same
`obj/project.assets.json`. Whichever restore finished last won the race.

When the Game library won, the product project read assets that had no
ILCompiler package, so `IlcCompile` never ran. `PublishAot=true` then produced a
self-contained CoreCLR app with no `CsharpNativeAotTrial.so`, and the build
still reported success.

Four clean publishes on unmodified `origin/main` (`b2b912f4`):

| Run | `.so` produced | `project.assets.json` belongs to |
|---|---|---|
| 1 | yes | `CsharpNativeAotTrial` |
| 2 | no | `CsharpNativeAotTrial.Game` |
| 3 | no | `CsharpNativeAotTrial.Game` |
| 4 | no | `CsharpNativeAotTrial.Game` |

`scripts/test-csharp-sdk-package.sh --aot` passed because its consumer is a
single project. #8776's restore-path change in `Rusty.Engine.targets` is not
involved: this fixture uses `ProjectReference`, not the package targets.

## Change

Removed the Game project and the `NativeProduct.cs` file, which contained only
a `using` line. `Product.cs` now compiles into the product project itself.

The split dated from when a separate composition assembly carried the product
export. Since #8775, the project that declares `RustyEngineProductEntryType`
compiles its own export. Every downstream product (Dagger, Doom, CraftSurvive,
template) declares the entry type in the project that defines it, so the split
covered no layout that anyone uses.

## Evidence

- Three clean publishes (`rm -rf obj bin`, then
  `dotnet publish … -c Release -r linux-x64`) each produced
  `CsharpNativeAotTrial.so` with no warnings.
- `scripts/test-runtime-pack.sh` passes end to end:
  `moved runtime pack launched CoreCLR and NativeAOT Product V1 bundles; UI was staged and content remained non-static`.
