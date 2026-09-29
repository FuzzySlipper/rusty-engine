# Every product callback required (#8799)

## Why the optional handling could go

The host binds a product only after an exact ABI fingerprint match. The one
thing that fills a `NativeProductApi` is the SDK's generated `ProductBridge`,
and it fills every field. A product with a partial or older table cannot pass
the handshake, so the host's compatibility branches were unreachable.

## Removed

- **ABI (`csharp-engine-abi/src/product.rs`):**
  - The plain `create(args, handle)` slot. The single `create` now carries the
    `NativeProductCallError` result that `create_with_error` used to carry.
  - The `create_with_error` slot and the `NativeProductCreateWithError`
    typedef.
  - The `attach` slot. The host has not called it since b777eb3e4, when
    presentation began being rebuilt from committed Engine state.
  - The "appended so older products keep their offsets" comments.
- **Host (`csharp-product-runtime/src/lib.rs`):**
  - `optional_callback_pair` and `optional_describe_callback`, along with the
    `CSHARP_CALLBACK_PAIR` error.
  - The `create_with_error`/`create` fallback in `call_create`, including its
    `expect("create diagnostics require call-error callbacks")`.
  - The `CSHARP_DEBUG_UNSUPPORTED` branch in `execute_debug` and the
    "unavailable catalog" branch in `describe_debug`.
  - The absent-observer early return in `observe_product_runtime`.
  - The `Option` in `read_product_call_error`.
  - `LoadedProductApi` now holds the nine callbacks directly, not in `Option`
    pairs.
- **SDK:**
  - `ProductBridge.Create` without an error pointer, the separate
    `CreateWithError` export, and the null-error branches in create.
  - `ProductBridge.Attach`.
  - The default `IEngineProduct.Attach()`.
- **Tests:** the assertions that an older product may omit the debug pair or
  the describe callback.

## Kept

`NativeProductApi` fields stay `Option<unsafe extern "C" fn …>`, and
`from_bound_product` requires each one through `required_function`. That is
16 null checks, once per product load.

- **Why keep the checks.** A non-`Option` Rust function pointer that the
  product left null would be undefined behaviour the moment the table is
  copied. The check turns it into a `CSHARP_REQUIRED_FUNCTION` load error
  and costs nothing per frame.
- **Why the fields stay inline.** cbindgen cannot render `Option<NamedAlias>`
  as a nullable function pointer. It emits an undefined `Option_NativeProductCreate`
  type, which was observed and broke the SDK compile. So the table keeps its
  inline `Option<unsafe extern "C" fn …>` fields rather than typedef aliases.

## Evidence

- **SDK smoke.** `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot`
  passes. Product create, the call-error path in the caught-refusal and
  hardware-exception checks, debug and lifecycle all run under CoreCLR and
  NativeAOT.
- **Runtime pack.** `scripts/test-runtime-pack.sh` passes on main after #8808
  ("moved runtime pack launched CoreCLR and NativeAOT Product V1 bundles").
- **Rust tests.** `cargo test -p csharp-engine-abi -p csharp-engine-services -p csharp-product-runtime`
  passes.
  - `failed_create_copies_named_engine_diagnostic_and_destroys_the_product`
    still covers a failing create. It copies the product error and releases it
    exactly once.
  - The runtime fixtures now supply a full table (neutral describe, observe and
    call-error callbacks).
- **Clippy.** `cargo clippy -p csharp-product-runtime -p csharp-engine-abi --all-targets -- -D warnings`
  passes.

## Migration

- **ABI.** The fingerprint and table layout change, so a product must rebuild
  against the matching SDK. That is the normal pair rule, and the generated
  bridge needs no product edits.
- **`IEngineProduct.Attach()`** is gone. A product that still defines a public
  `Attach()` compiles unchanged, but that method was not called before this
  change and is not called now. Move any work in it into `Start`/`Update`
  publication and delete it.
- **Affected products.** Dagger (`WorldRpgProduct`, `SpriteWorkbenchProduct`),
  Doom (`LoadingBayProduct`) and several Engine fixtures define such a method.
  Only an explicit `void IEngineProduct.Attach()` implementation would stop
  compiling, and none exists in the known products.
