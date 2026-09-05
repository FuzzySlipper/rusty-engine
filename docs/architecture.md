# Engine architecture

Rusty Engine hosts an ordinary C# product. The product decides game
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
  │  generated CoreCLR/NativeAOT binds, copied values, explicit leases
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
| Retained graphics intent | Rust | `render-presentation::PresentationWorld` owns the committed graphics graph, snapshots, and publication revision. Existing appearance and voxel projectors feed typed changes into it. |
| Session serialization and recovery facts | Rust | `runtime-session` owns the serialized runtime guard and mutation/scope/recovery vocabulary; `product-dev-host` adapts these to its development transport. |
| Binding generation | Engine tooling | [`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh) runs cbindgen, ClangSharp, and the binding generator. |
| Safe C# contracts | Generated C# | [`Rusty.Engine`](../csharp/Rusty.Engine) compiles generated contracts and values from ignored `obj/Generated` output. |
| Product bootstrap | Generated C# | [`Rusty.Engine.ProductGenerator`](../csharp/Rusty.Engine.ProductGenerator) produces the internal versioned bind path and service implementations for CoreCLR and NativeAOT. |
| Product lifecycle and host | Rust | [`csharp-product-runtime`](../rust/crates/csharp-product-runtime) loads the product and drives its lifecycle. |
| Product logic | Downstream C# | The product implements the generated `IEngineProduct` contract and owns its own state and code organization. |
| UI and host/backend implementation | TypeScript/host | DOM UI and explicit Engine host/backend work only; not downstream gameplay ownership or game-rendering substitution. |

The generated ABI surface is deliberately a capability surface, not a claim of
full source-level Rust API coverage. Its current shape is defined by the Rust
ABI crate and binding generator; use current Den coverage guidance for
planning, rather than copying a volatile service table into this document.

## Lifecycle and data movement

1. The packaged Rust runtime loads the staged Product through CoreCLR during
   ordinary development, or through its NativeAOT module during an explicit
   fidelity/release check, and calls the same generated versioned bind.
2. That bind verifies the exact SDK/runtime ABI identity before product
   construction. The generated bootstrap then constructs an `IEngineContext`
   from named safe Engine services, copies product content/input configuration,
   and creates the product with `ProductCreateContext`.
3. Rust sends lifecycle calls and update facts. The generated bootstrap copies
   input events into C# values and forwards `ProductUpdate` to the product.
4. The product uses named services to read or publish facts. Explicit leases
   make retained native resources disposable on the C# side; borrowed data must
   not be stored past its documented call/lease boundary.
5. The Engine turns admitted product presentation facts into its renderer
   state. DOM UI observes Engine-supported UI/projection paths and emits
   semantic input; it does not become a second game implementation.

Ghost plates and standalone microvoxel objects illustrate this boundary. C#
selects a retained `Appearance` source, placement, capture/configuration, and
ordinary material bindings. The Engine performs the retained capture or voxel
mesh projection, owns renderer resources and cleanup, and returns copied
observation/readout facts. Ghost direction uses an Engine-selected hard snap
among 1/4/8/16 captured sectors with optional hysteresis; a plate is a frozen
source pose. The MagicaVoxel object path uses bounded v150 model admission,
ordinary matte-capable materials, and the generated greedy surface's
axis-aligned face normals. Neither path asks a downstream product to supply a
voxel shader, browser renderer, or TypeScript game implementation.

The Engine fixtures exercise provider generation, ABI, lifecycle, and both
loaders. They are not downstream product architecture or launch templates.

## Reconstructible presentation

C# selects presentation facts through named services. Rust commits the resulting
graphics intent into `PresentationWorld`; TypeScript realizes that intent in
browser/GPU objects. Fresh browser attachment reads committed Rust snapshots
without calling the product's `Attach` callback. Graphics snapshots preserve
active handles and resource dependencies, with a `presentation-world`
continuation revision. The runtime session guard covers snapshot capture and
the output cursor handover, so subsequent deltas follow that snapshot.

The TS `render-projection` model has no Three or DOM dependency. A mounted
`renderer-host` surface and its `renderer-three` backend share one neutral
projection, installing a baseline or advancing a delta only after successful
realization. `product-browser-host` owns connection and attachment epochs;
uncertain derived state uses a fresh baseline. This does not undo spatial
mutations or make an uncertain C# callback safe to replay.

The initial common world covers ordinary graphics and voxel render output.
Camera, UI, and retained effects have read-only Engine snapshots alongside it.
Exact audio/animation time anchoring and frozen ghost-capture reconstruction
remain task #7779: current snapshots preserve retained intent, not elapsed GPU
or browser playback history. Further ABI-owner extraction and TS cleanup are
#7782; composition and the C# `Appearance` to `Graphics` rename are #7780.

## Packaging and development

The Engine publishes two matched artifacts: an immutable `Rusty.Engine` SDK
package and a runtime pack containing `rusty`, `rusty-product-host`, and the
Engine-owned browser shell. The package generates composition below `obj` and
stages a loose Product directory. `rusty dev` asks the package to stage that
directory, launches CoreCLR, and watches only the declared Product inputs.

Neither artifact contains product meaning. A Product repository does not carry
Engine JavaScript, generated bindings, a checked native bootstrap, or an Engine
Rust host. Selecting `--engine-source` is an explicit Engine-contributor
override that still uses the packaged contract; normal downstream development
never searches for an adjacent checkout.

## What stays out of downstream code

- Handwritten P/Invoke declarations, `UnmanagedCallersOnly` exports, raw
  function-table use, pointer ownership, and browser/native host adaptation.
- A custom renderer, retained frame format, browser canvas owner, TypeScript
  game renderer, or downstream TypeScript gameplay path.
- JSON invocation protocols, generic command buses, reflection/discovery
  frameworks, or policy/security layers at the trusted product boundary.

When a product needs a mechanism that current generated services cannot
express, name that mechanism precisely and create or link the upstream request.
Stopping there is preferable to a local substitute.
