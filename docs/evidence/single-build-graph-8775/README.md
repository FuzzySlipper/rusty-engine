# One build graph for a C# product (#8775)

## Change

Before this task, staging (`rusty dev`, or `-t:StageRustyEngineCoreClrProduct`)
worked in three steps:

1. It wrote a composition project to `obj/Rusty.Engine/Composition/coreclr/`
   with a `ProductEntry.g.cs` carrying `[assembly: EngineProduct(typeof(...))]`.
2. It launched `dotnet build` on that project from inside the MSBuild target.
3. That nested process restored the composition, built the product project
   again as a `ProjectReference`, and compiled `Rusty.Engine.Product.dll` around
   it.

Now the product project is the only project:

- **The product compiles its own export.** The SDK targets add the generator
  analyzer, and `CompilerVisibleProperty RustyEngineProductEntryType`, only to
  the project that declares `RustyEngineProductEntryType`. The generator reads
  that property directly, so the attribute and `ProductEntry.g.cs` are gone.
  Library projects that reference `Rusty.Engine` get neither.
- **The product builds as a CoreCLR component.** The same condition sets
  `EnableDynamicLoading`, so the ordinary build writes
  `<Product>.runtimeconfig.json`, `<Product>.deps.json` and copy-local
  dependencies.
- **One MSBuild invocation.** `StageRustyEngineCoreClrProduct` depends on
  `ValidateRustyEngineProduct;Build` and runs in the same process that
  `rusty dev` started (`dotnet msbuild -restore -t:StageRustyEngineCoreClrProduct`).
  `rusty-cli` needed no change.
- **Staging copies the build output.** It copies `$(OutDir)` into a fresh
  `coreclr/` directory, and `product.json` names
  `coreclr/$(TargetFileName)`.

**Assembly-file lifetime.** Kept. The running worker maps the previously
staged files, so staging still removes and recreates `coreclr/` and never
rewrites a mapped file in place. The build output in `bin/` is never loaded, so
the next build may overwrite it. This is the one copy staging keeps.

**NativeAOT.** `VerifyRustyEngineAot` publishes the product project itself
with `PublishAot`/`NativeLib=Shared` for `linux-x64`. The export lives in the
root assembly, so the shared library still exports `rusty_product_bind_v1`.

That publish is still a nested `dotnet publish` and still follows the CoreCLR
stage. #8776 owns that. An attempt to give the publish its own
`BaseIntermediateOutputPath` failed: the moved intermediate directory no longer
excludes the normal `obj/` from default compile items, so generated files
compiled twice (CS0101/CS0579). The RID already separates the publish's
intermediate and output directories. Only `obj/project.assets.json` is shared,
and CoreCLR's next `-restore` rewrites it.

## Evidence

### Staging processes, restores and compilations

Measured with the packaged SDK consumer from `scripts/test-csharp-sdk-package.sh`.
Three packages were compared:
- `origin/main` at `9d393dcc`;
- #8764 alone (its commit, before this one);
- this change.

Each stage ran the `rusty dev` staging command under
`strace -f -e trace=execve`. Compiled assemblies are the intermediate
`obj/**/Debug/**/*.dll` files the stage wrote. The edit adds one class to
`Product.cs`.

| Stage | origin/main | #8764 only | #8775 |
|---|---|---|---|
| clean: `dotnet` processes | `msbuild -restore` + nested `build Rusty.Engine.Product.csproj` | same | `msbuild -restore` only |
| clean: restores | Consumer + composition project | same | Consumer |
| clean: compiled | `Consumer.dll`, composition `Rusty.Engine.Product.dll` | same | `Consumer.dll` |
| unchanged: `dotnet` processes | `msbuild` + nested `build` | same | `msbuild` only |
| unchanged: compiled | none | none | none |
| one edit: `dotnet` processes | `msbuild` + nested `build` | same | `msbuild` only |
| one edit: compiled | `Consumer.dll`, `Rusty.Engine.Product.dll` (with 19,888 generated lines) | `Consumer.dll`, `Rusty.Engine.Product.dll` (394 generated lines) | `Consumer.dll` (with its 394 generated lines) |
| staged `coreclr/` | `Consumer.dll`, `Rusty.Engine.Product.{dll,deps.json,runtimeconfig.json}`, `Rusty.Engine.dll` | same | `Consumer.{dll,deps.json,runtimeconfig.json}`, `Rusty.Engine.dll` |

Wall times, for context only (other agents share this machine):

| | clean | unchanged | one edit |
|---|---|---|---|
| origin/main | 17.0 s | 5.7 s | 8.1 s |
| #8764 only | 6.7 s | 11.1 s | 6.7 s |
| #8775 | 3.8 s | 2.5 s | 4.8 s |

The structural result is exact: one process, one restore graph, one
compilation per C# edit. The timings are not a benchmark; the #8764-only
unchanged run shows the noise.

### Checks

Run with `DOTNET_ROOT=/home/agent/.dotnet`, because nethost does not find a
per-user dotnet otherwise, and `TMPDIR` on disk, because the shared `/tmp`
quota was full.

- **Package smoke.** `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot`
  passed. CoreCLR loads `coreclr/Consumer.dll`, and NativeAOT loads
  `native/Consumer.so`, each through `rusty-product-host --exercise`. Neither
  build produced CS/RS warnings. The script now also checks that no
  `Composition/` directory appears, that `ProductExports.g.cs` is in the
  product's own `obj`, and that a library referencing `Rusty.Engine` gets no
  export.
- **NativeAOT warnings now visible.** The NativeAOT publish now compiles the
  product itself, so its compiler output reaches the smoke's warning check.
  The earlier nested composition publish reused an already-built product
  assembly. This surfaced CS8632 in `fixtures/csharp-hardware-exceptions/HardwareExceptionChecks.cs`
  (#8753: `string?` without a nullable context in the non-nullable consumer).
  The fixture now starts with `#nullable enable`.
- **`rusty dev` launch, replacement and debug catalog.** The runtime pack came
  from `scripts/build-runtime-pack.sh` on this tree. The product was the
  package consumer plus a product module with
  `[DebugCommand("product.revision")]`, run with `--live-debug`.
  - `rusty-live-debug complete product` printed
    `product.revision — Reports which product build is running.` (describe path).
  - `--command product.revision` returned `r1` (execute path).
  - Editing the constant to `r2` produced `change-detected`, one in-process
    restore/build/stage, then runtime replacement. `product.revision` returned
    `r2`.
  - Ctrl+C stopped the session cleanly.
  - Repeated after rebasing onto #8743's asset routing, with a fresh runtime
    pack and SDK. A UI-only edit logged `assetsOnly:true`, `assets-restaged`
    and `assets-reloaded`, with no C# build and no replacement. A C# edit
    rebuilt once, replaced the runtime and returned `r2`.
- **Dagger.** Dagger's product project `WorldRpg.Host` (`TreatWarningsAsErrors`,
  `AllowUnsafeBlocks=false`, `EnforceCodeStyleInBuild`) was built from a
  scratch copy against this package.
  - It built with 0 warnings and 0 errors, and `WorldRpg.Host.dll` carries the
    export.
  - Staging produced `coreclr/WorldRpg.Host.dll` with its dependency closure.
  - `rusty-product-host --product … --loader coreclr` bound it and listened.
  - The copy needed one unrelated source patch: #8741 changed
    `InventoryStore.Prepare(expectedRevision)` to `Prepare()`, and Dagger's
    `MechanicsInventoryContainerCoordinator` still passes a revision. That
    migration belongs to #8741's downstream notes, not this task.
- **Runtime pack.** `scripts/test-runtime-pack.sh` passed. Its source build
  of the generator reported RS2000 for the new `RUSTY001` descriptor; the rule
  is now listed in `AnalyzerReleases.Unshipped.md`, and the generator builds
  clean. A wrong entry type fails with `error RUSTY001: RustyEngineProductEntryType
  'Missing.Product' does not name a type …`.

## Migration notes

- **Remove the attribute.** Delete `[assembly: EngineProduct(typeof(...))]` if a
  product wrote it by hand; the attribute type no longer exists.
  `RustyEngineProductEntryType` alone selects the product.
- **Move UI build hooks.** `GenerateRustyEngineProductComposition` no longer
  exists, so targets hooked with `BeforeTargets` on it silently stop running
  (Dagger's `BuildWorldRpgProductUi` and `BuildSpriteWorkbenchProductUi`).
  Declare the compiler as `RustyEngineProductUiBuildCommand` instead, the route
  #8743 introduced. The SDK then runs it only when UI inputs change.
- **Staged assembly name.** The staged managed assembly keeps the product's
  own name. Tooling that hard-coded `coreclr/Rusty.Engine.Product.dll` or
  `native/Rusty.Engine.Product.so` should read `product.json` instead.
- **Plain `dotnet build` output.** It now includes a runtimeconfig, deps.json
  and copy-local dependencies, because the product project is a CoreCLR
  component.
- **Engine source fixtures.** They declare `RustyEngineProductEntryType` plus
  `<CompilerVisibleProperty Include="RustyEngineProductEntryType" />` next to
  their analyzer reference.

## Remaining structural work

- **#8776.** NativeAOT still runs after the CoreCLR stage, as a nested
  `dotnet publish`, and copies the bundle through `.next`.
- **#8743.** UI edits still go through a managed build and restage.
- **Staging copy.** Each stage copies the managed output into a fresh
  `coreclr/`, even when unchanged. That copy is the retained lifetime rule
  above, and it is small.
