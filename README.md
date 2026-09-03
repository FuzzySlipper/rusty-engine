# Rusty Engine

Rusty Engine is reusable Rust infrastructure for ordinary C# products. A
product owns its game and application meaning; the Engine supplies durable
mechanisms and the host that makes that product run.

> The product decides. The Engine guarantees.

The paired proving product is `/home/dev/rusty-dagger`.

## Start here

- [Architecture overview](docs/architecture.md) — ownership, layers, and data
  flow.
- [C# SDK guide](docs/csharp-sdk.md) — build a product through the current
  generated safe API.
- [C# product style](docs/csharp-product-style.md) — recommended downstream
  organization and coding conventions.
- [Agent guidance](AGENTS.md) — task-time boundary and verification rules.

## Repository map

| Owner | Location | Responsibility |
| --- | --- | --- |
| Rust ABI declaration | [`rust/crates/csharp-engine-abi`](rust/crates/csharp-engine-abi) | The single C ABI/function-table source. |
| Rust Engine bridges | [`rust/crates/csharp-engine-services`](rust/crates/csharp-engine-services) | Concrete named Engine service bridges. |
| Product runtime | [`rust/crates/csharp-product-runtime`](rust/crates/csharp-product-runtime) | Loads staged CoreCLR or NativeAOT products and integrates their lifecycle with the host. |
| Binding generation | [`scripts/generate-csharp-native-bindings.sh`](scripts/generate-csharp-native-bindings.sh) and [`csharp/Rusty.Engine.BindingGenerator`](csharp/Rusty.Engine.BindingGenerator) | Generates native declarations, safe contracts/values, and generator inputs. |
| Default managed C# assembly | [`csharp/Rusty.Engine`](csharp/Rusty.Engine) | The single runtime assembly containing generated contracts/values and handwritten managed helpers. |
| Product bootstrap | [`csharp/Rusty.Engine.ProductGenerator`](csharp/Rusty.Engine.ProductGenerator) | Generates the internal CoreCLR/NativeAOT bind entrypoints and safe service implementations. |
| Managed namespaces | [`Application`](csharp/Rusty.Engine/Application), [`Entities`](csharp/Rusty.Engine/Entities), [`Mechanics`](csharp/Rusty.Engine/Mechanics), [`Persistence`](csharp/Rusty.Engine/Persistence), [`Resolution`](csharp/Rusty.Engine/Resolution), [`StateMachine`](csharp/Rusty.Engine/StateMachine) | Optional reusable scheduling, entity, mechanics, persistence, resolution, and state-machine helpers inside `Rusty.Engine`; these are namespace boundaries, not separate runtime assemblies. |
| Working fixture | [`fixtures/csharp-nativeaot-trial`](fixtures/csharp-nativeaot-trial) | Minimal buildable product and direct runtime exercise. |

## Product quick start

Ordinary product projects reference one immutable `Rusty.Engine` package and
declare their product metadata in that project. With the matching runtime pack,
the normal development command is:

```bash
/path/to/runtime-pack/bin/rusty dev \
  --project /path/to/Product.Game.csproj \
  --runtime /path/to/runtime-pack
```

`rusty dev` builds and atomically stages a loose Product bundle, starts it
through CoreCLR, and restarts it when declared C#, UI, or content inputs change.
The runtime pack supplies the Engine host and browser assets; a Product bundle
contains only managed Product output, Product UI, Product content, and
`product.json`.

The SDK package and runtime pack must describe the same generated ABI identity.
The bind rejects a mismatch with a rebuild/select-the-matching-pack diagnostic;
there is no compatibility negotiation or adjacent-checkout discovery.

NativeAOT is a separate fidelity/release operation:

```bash
dotnet msbuild /path/to/Product.Game.csproj -t:VerifyRustyEngineAot
```

Engine contributors may explicitly select source with `rusty dev
--engine-source /home/dev/rusty-engine`; this is an exception, not a downstream
setup requirement. Binding generation, Cargo hosting, and the fixtures in this
repository are provider development and proof infrastructure. Products do not
copy them or check in generated interop/composition code.

## Boundary in one page

- C# owns product state, rules, orchestration, content meaning, and policy.
- Rust owns lifecycle/host integration, input delivery, renderer and
  presentation infrastructure, spatial mechanisms, resources, persistence
  primitives, and other named Engine capabilities.
- The generated API is the green path. A missing capability is an upstream
  request and a valid stopping point—not permission to recreate an Engine
  mechanism downstream.
- TypeScript is limited to Engine-side DOM UI, accessibility, and explicit
  host/backend work. It is not a downstream gameplay lane and does not render
  non-UI game elements.

For details and the product coding lane, use the linked guides rather than
duplicating architecture here.
