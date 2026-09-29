# Lane: build

**Tasks, in order:** #8775, #8776, #8764. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

All three edit `Rusty.Engine.targets` and the product generator, so they are
one lane. #8744 (result buffers) is the natural follow-on, but it is not
assigned yet. It starts after the service lanes are quiet.

## Tasks

- **#8775:** remove the generated CoreCLR composition project and the nested
  `dotnet build`/restore.
  - Current shape: `rusty-cli` `stage_product` runs `dotnet msbuild` on the
    product; `csharp/Rusty.Engine/buildTransitive/Rusty.Engine.targets`
    generates a composition project and builds it inside a target.
  - Keep the assembly-file lifetime needed during runtime replacement.
  - **Land this first and say so on #8779 in Den.** The cli lane is waiting
    for it.
- **#8776:** NativeAOT publishing without a CoreCLR stage.
  - `VerifyRustyEngineAot` depends on `StageRustyEngineCoreClrProduct`, copies
    the staged bundle into `.next`, then `PromoteRustyEngineAotStagedProduct`
    deletes and recopies it.
- **#8764:** precompile the shared managed bridge into the SDK, and keep
  generated code to the product factory, debug catalog and minimal
  bind/export glue.
  - `Interop.g.cs`, `EngineServiceImplementations.g.cs` and
    `AbiIdentity.g.cs` are currently added to every product compilation.
  - Record the one-paragraph second-emitter note the task asks for; don't emit
    TypeScript.

## Coordination

- **content lane (#8743)** owns making UI edits skip managed compilation, and
  migrating Dagger's and CraftSurvive's `tsc` hooks. You meet at the target and
  command boundary; agree the shape in Den rather than both editing the same
  target.
- **cli lane (#8779)** extends `rusty-cli` after #8775. Keep your `rusty-cli`
  edits to `stage_product` and the build invocation.
- **#8742** (`25dfd514`, in review) removed the generator's 64 KiB debug
  command/result caps (`ProductGenerator.cs`). The main lane is fixing one
  remaining HTTP truncation in `product-dev-host`, not in your files.

## Evidence

- Record the build and restore processes actually invoked, and the generated
  inputs, for a clean build, an unchanged build and a one-edit build.
- For #8776, show a NativeAOT publish from clean with no CoreCLR build.
- Run after each landing: `scripts/test-csharp-sdk-package.sh --coreclr-smoke`
  and `scripts/test-runtime-pack.sh`.
- Build a local pair when you need one:
  `scripts/pack-csharp-sdk.sh <version> <feed>`, then
  `scripts/build-runtime-pack.sh --output <dir>`.
