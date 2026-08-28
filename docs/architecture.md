# Engine architecture

Rusty Engine hosts an ordinary NativeAOT C# product. The product decides game
meaning; the Engine guarantees reusable mechanisms and integration.

## Ownership flow

```text
C# product application and state
  │  product rules, orchestration, content meaning, policy
  ▼
Rusty.Engine safe services and optional managed helpers
  │  generated contracts/values plus product-side composition helpers
  ▼
generated product bootstrap and C ABI function table
  │  generated NativeAOT exports, copied values, explicit leases
  ▼
Rust Engine services and runtime host
  │  lifecycle, input, renderer, spatial mechanisms, content, persistence
  ▼
browser/host implementation and DOM UI
```

The arrows describe a cooperation boundary, not a hierarchy of game authority.
C# does not need to imitate Rust. Rust mechanisms remain upstream so a product
does not grow its own renderer, platform host, resource loader, or native ABI.

## Source owners

| Layer | Current owner | Source of truth |
| --- | --- | --- |
| ABI declarations | Rust | [`csharp-engine-abi`](../rust/crates/csharp-engine-abi) defines the C ABI and named function tables. |
| Concrete Engine bridges | Rust | [`csharp-engine-services`](../rust/crates/csharp-engine-services) implements ABI-backed named capabilities. |
| Binding generation | Engine tooling | [`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh) runs cbindgen, ClangSharp, and the binding generator. |
| Safe C# contracts | Generated C# | [`Rusty.Engine`](../csharp/Rusty.Engine) compiles generated contracts and values from ignored `obj/Generated` output. |
| NativeAOT product bootstrap | Generated C# | [`Rusty.Engine.ProductGenerator`](../csharp/Rusty.Engine.ProductGenerator) produces the internal `rusty_product_bind` path and service implementations. |
| Product lifecycle and host | Rust | [`csharp-product-runtime`](../rust/crates/csharp-product-runtime) loads the product and drives its lifecycle. |
| Product logic | Downstream C# | The product implements the generated `IEngineProduct` contract and owns its own state and code organization. |
| UI and host/backend implementation | TypeScript/host | DOM UI and explicit Engine host/backend work only; not gameplay ownership or game rendering. |

The generated ABI surface is deliberately a capability surface, not a claim of
full source-level Rust API coverage. Its current shape is defined by the Rust
ABI crate and binding generator; use current Den coverage guidance for
planning, rather than copying a volatile service table into this document.

## Lifecycle and data movement

1. The Rust runtime loads the NativeAOT library and calls the generated product
   bind entrypoint.
2. The generated bootstrap constructs an `IEngineContext` from named safe
   Engine services, copies product content/input configuration, and creates the
   product with `ProductCreateContext`.
3. Rust sends lifecycle calls and update facts. The generated bootstrap copies
   input events into C# values and forwards `ProductUpdate` to the product.
4. The product uses named services to read or publish facts. Explicit leases
   make retained native resources disposable on the C# side; borrowed data must
   not be stored past its documented call/lease boundary.
5. The Engine turns admitted product presentation facts into its renderer
   state. DOM UI observes Engine-supported UI/projection paths and emits
   semantic input; it does not become a second game implementation.

The working reference is
[`fixtures/csharp-nativeaot-trial`](../fixtures/csharp-nativeaot-trial). It is
an ABI/lifecycle exercise, not a product architecture template.

## What stays out of downstream code

- Handwritten P/Invoke declarations, `UnmanagedCallersOnly` exports, raw
  function-table use, pointer ownership, and browser/native host adaptation.
- A custom renderer, retained frame format, browser canvas owner, or TypeScript
  game renderer.
- JSON invocation protocols, generic command buses, reflection/discovery
  frameworks, or policy/security layers at the trusted product boundary.

When a product needs a mechanism that current generated services cannot
express, name that mechanism precisely and create or link the upstream request.
Stopping there is preferable to a local substitute.
