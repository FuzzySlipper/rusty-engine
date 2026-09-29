# Precompiled managed product bridge (#8764)

## Change

The product generator used to embed three generated files (`Interop.g.cs`,
`EngineServiceImplementations.g.cs`, `AbiIdentity.g.cs`) and add them, together
with a 634-line `ProductExports.g.cs` template, to every product compilation.
That compilation was the generated composition project, which needed
`AllowUnsafeBlocks` and a `NoWarn 0649`.

Now:

- `Rusty.Engine.csproj` compiles the three generated files from its ignored
  `obj/Generated/GeneratedInputs` output. The native function-table fields carry
  a file-scoped `#pragma warning disable CS0649`, so no project-wide `NoWarn`
  is needed.
- The export template became handwritten SDK code:
  [`ProductBridge.cs`](../../../csharp/Rusty.Engine/NativeProduct/ProductBridge.cs).
  It holds the engine context, product lifetime, debug-execution context and
  every `NativeProductApi` callback. Its one public entry is
  `ProductBridge.Bind(nint host, nint product, createProduct, createDebugCatalog)`.
- The generator emits only what depends on the product: the debug catalog
  (unchanged) and a 13-line safe export.

  ```csharp
  [UnmanagedCallersOnly(EntryPoint = "rusty_product_bind_v1", CallConvs = [typeof(CallConvCdecl)])]
  internal static int BindV1(nint host, nint product)
      => ProductBridge.Bind(host, product, static context => new global::Product(context),
                            GeneratedDebugCommandCatalogFactory.Create);
  ```

  The host's type and method (`Rusty.Engine.NativeProduct.ProductExports.BindV1`)
  and the NativeAOT symbol are unchanged. The export lives in the root (product)
  assembly, so NativeAOT still exports it without `UnmanagedEntryPointsAssembly`.
- The generator no longer depends on generated bindings. Its project dropped
  the binding-generation target and embedded resources, and
  `pack-csharp-sdk.sh` no longer rebuilds it per ABI shape.

The ABI, host and function table are untouched.

## Evidence

Packaged consumer (`scripts/test-csharp-sdk-package.sh`, fixture product
`SdkPackageConsumer.Product`). Counts come from `EmitCompilerGeneratedFiles`
output. Baseline is `origin/main` at `9d393dcc`.

| | origin/main | #8764 |
|---|---|---|
| Generated C# added to each product compilation | 19,888 lines / 1,202,716 bytes (5 files) | 394 lines / 30,301 bytes (2 files) |
| `Rusty.Engine.ProductGenerator.dll` in the package | 1,238,528 bytes | 35,840 bytes |
| `lib/net10.0/Rusty.Engine.dll` in the package | 1,955,840 bytes | 2,480,640 bytes |
| Product compilation needs `AllowUnsafeBlocks` | yes (composition project) | no |

The 394 remaining lines are the product's debug catalog (381) and its export
(13). The bridge is now compiled once, when the SDK is packed, not once per
product compilation. This does not claim every earlier edit recompiled 19k
lines: CoreCompile already skipped an unchanged composition. What changed is
that each C# edit, which recompiles the composition, now compiles 394 generated
lines rather than 19,888.

Checks ran on the #8764 tree before #8775, with `DOTNET_ROOT=/home/agent/.dotnet`
(nethost cannot find a per-user dotnet otherwise):

- `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot` passed. The
  CoreCLR and NativeAOT host exercises cover create/start/update/pause/resume/
  restart/shutdown/fault and named services (Graphics, Voxel,
  VoxelScenePresentation, Spatial, Ui, Content, particles, caught refusals).
  There were no CS/RS warnings in either product build. The host exercise does
  not call the debug catalog. The catalog's describe/execute path through
  `ProductBridge` is exercised with `rusty-live-debug` in the
  [#8775 evidence](../single-build-graph-8775/README.md), where the bridge code
  is identical.
- `dotnet publish fixtures/csharp-nativeaot-trial -r linux-x64`:
  `nm -D` shows `rusty_product_bind_v1` exported from `CsharpNativeAotTrial.so`.
- `fixtures/csharp-debug-command-catalog` and `fixtures/csharp-entity-store-debug`
  harnesses run and pass without `AllowUnsafeBlocks`.

## Removed with the bridge

`fixtures/csharp-debug-execution-context` had a `Main` that acted as a fake
host through the raw native types. Those types are now internal to
`Rusty.Engine`, and no script ran that `Main`. `scripts/test-runtime-pack.sh`
only builds the fixture as its CoreCLR product, and the real host drives its
lifecycle. The fixture is now a plain component library.

## Remaining generator choice

Not exercised. After the move, cbindgen/ClangSharp run only when the SDK itself
is built, and #8735 already skips unchanged generation. The product side has no
ABI-dependent generation left. Replacing the toolchain would change contributor
tooling without removing per-product work, so this task leaves it in place.
Caller-provided result buffers remain #8744.

Second emitter (#8793): the current model is the C header's `#[repr(C)]`
layout (cbindgen → ClangSharp AST → C# emitter). The DTOs #8793 needs (UI
projections, input facts, diagnostics, live-debug shapes, frame stream header)
are serde-shaped Rust types that never cross the C ABI. A serde-derived emitter
on those crates (ts-rs or typeshare) is the natural route, not this ABI model.

## Migration

Products that already consume the package need nothing new for #8764 alone.
#8775 removes the `EngineProduct` attribute and the composition project; its
notes cover that step.

## Follow-ups

- #8799: make every product ABI callback required under exact ABI identity. The
  compiled bridge always fills the full table, so the host's optional pairs and
  the `create`/`create_with_error` split are dead compatibility code.
