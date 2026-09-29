# NativeAOT staging without CoreCLR (#8776)

## Change

Before (as of #8775), `VerifyRustyEngineAot`:
1. depended on `StageRustyEngineCoreClrProduct`;
2. copied the whole staged bundle into `Product.next`;
3. added the native module and a combined manifest;
4. deleted the live bundle and copied everything back
   (`PromoteRustyEngineAotStagedProduct`).

Now each runtime artifact has its own staging target, and UI/content staging
is shared:

| Target | Stages | `product.json` lists |
|---|---|---|
| `StageRustyEngineCoreClrProduct` | `ValidateRustyEngineProduct`, `Build`, `coreclr/`, UI/content | `coreclr` |
| `VerifyRustyEngineAot` | `ValidateRustyEngineProduct`, NativeAOT publish, `native/`, UI/content | `nativeAot` |
| `StageRustyEngineCombinedProduct` (new, explicit) | both artifacts, UI/content | `coreclr` and `nativeAot` |

- **Manifest.** `_RustyEngineWriteProductManifest` writes `product.json` last.
  It lists exactly the artifacts that this invocation staged. Each artifact
  target deletes `product.json` first, so a host still admits a complete bundle
  or none.
- **UI and content.** `StageRustyEngineProductAssets` (#8743) stages them
  incrementally for every target (`SkipUnchangedFiles`).
- **Removed.** `Product.next`, `PromoteRustyEngineAotStagedProduct`,
  `_RustyEngineStagingDirectory`, and the unreferenced `StageRustyEngineProductBundle`.
- **Separate restore path.** The NativeAOT publish now restores into its own
  `MSBuildProjectExtensionsPath` (`obj/Rusty.Engine/NativeAotRestore/`).
  Without that, the outer restore (no RID) and the publish's restore (RID plus
  ILCompiler) rewrote the same `obj/project.assets.json` and `nuget.g.*` files.
  Every unchanged NativeAOT run then recompiled and relinked the product.
  `BaseIntermediateOutputPath` stays at `obj/`; moving it made the default
  compile items include `obj/` (see #8775).

**Retained.**
- **Fresh `native/` module.** Each NativeAOT stage copies the module into a
  fresh `native/` directory, even when unchanged. A running NativeAOT runtime
  maps the previous module, and one file per stage is cheap.
- **Nested `dotnet publish`.** It stays: the publish needs a RID-specific
  restore with the ILCompiler package. An in-process `MSBuild` task call would
  still need that restore, and forcing `PublishAot`/`RuntimeIdentifiers` onto
  every CoreCLR build would change ordinary product builds (AOT analyzers,
  restore graph).
- **The `VerifyRustyEngineAot` name.** Kept, because Dagger, CraftSurvive and
  Doom invoke it.

## Evidence

The measurement uses the packaged SDK consumer from
`scripts/test-csharp-sdk-package.sh`, comparing `origin/main` before #8775
(`9d393dcc`) with this change. Each run is
`dotnet msbuild Consumer.csproj -restore -t:VerifyRustyEngineAot` from a clean
copy, then again unchanged, under `strace -f -e trace=execve`. Copies are
counted from the Copy task's log.

| | origin/main clean | origin/main unchanged | #8776 clean | #8776 unchanged |
|---|---|---|---|---|
| `dotnet` processes | `msbuild` + nested `build` (CoreCLR composition) + nested `publish` (NativeAOT composition) | same | `msbuild` + nested `publish Consumer.csproj` | same |
| assemblies compiled | CoreCLR `Consumer.dll`, CoreCLR composition, NativeAOT composition | none | NativeAOT `Consumer.dll` (`linux-x64`) | none |
| CoreCLR build/stage | yes | yes | no | no |
| staged files copied | 84 (55 into `Product/`: 37 content, 14 coreclr, 2 ui, 1 native, 1 manifest; 29 into `Product.next/`) | 66 | 20 (18 content, 1 ui, 1 native) | 1 (native module) |
| manifest | `nativeAot` + `coreclr` | same | `nativeAot` only | same |

Wall times (shared machine, indicative only): origin/main 33.6 s clean and
7.2 s unchanged; #8776 16.6 s clean and 4.6 s unchanged.

`scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot` passed. Its
NativeAOT section now:
- runs `VerifyRustyEngineAot` on a clean copy of the consumer, with no prior
  build or stage;
- asserts that neither `bin/Debug/net10.0/Consumer.dll` nor a staged `coreclr/`
  appears;
- checks that the manifest lists only `native/Consumer.so` and that UI, content
  and the bundle inventory are staged;
- exercises the module through `rusty-product-host --exercise`;
- runs `StageRustyEngineCombinedProduct` in the same directory and checks that
  the manifest lists both loaders.

`scripts/test-runtime-pack.sh` passed.

`scripts/run-performance-regression.sh` is the one consumer that runs both
loaders against one bundle, and it now uses `StageRustyEngineCombinedProduct`.
I did not run it here: it needs the contributor browser runtime layout and is
a performance lane, not this task's question.

## Migration notes

- **NativeAOT-only manifest.** A product staged by `VerifyRustyEngineAot` no
  longer lists `coreclr`. Tooling that ran `--loader coreclr` against an AOT
  bundle should stage CoreCLR separately or use `StageRustyEngineCombinedProduct`.
- **Stale directories.** Staging one loader leaves any earlier `coreclr/` or
  `native/` directory in place, but unlisted. The manifest is the source of
  truth.
- **CraftSurvive.** It hooks its UI compiler `BeforeTargets="StageRustyEngineCoreClrProduct;VerifyRustyEngineAot"`.
  That still runs, but `RustyEngineProductUiBuildCommand` (#8743) is the
  intended route.

## Remaining

The NativeAOT operation still starts one nested `dotnet publish` process
because of the RID-specific restore.
