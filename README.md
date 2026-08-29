# Rusty Engine

Rusty Engine is reusable Rust infrastructure for NativeAOT C# products. A
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
| Product runtime | [`rust/crates/csharp-product-runtime`](rust/crates/csharp-product-runtime) | Loads NativeAOT products and integrates their lifecycle with the host. |
| Binding generation | [`scripts/generate-csharp-native-bindings.sh`](scripts/generate-csharp-native-bindings.sh) and [`csharp/Rusty.Engine.BindingGenerator`](csharp/Rusty.Engine.BindingGenerator) | Generates native declarations, safe contracts/values, and generator inputs. |
| Safe C# surface | [`csharp/Rusty.Engine`](csharp/Rusty.Engine) | Generated contracts plus handwritten managed helpers. |
| Product bootstrap | [`csharp/Rusty.Engine.ProductGenerator`](csharp/Rusty.Engine.ProductGenerator) | Generates the internal NativeAOT exports and safe service implementations. |
| Optional C# helpers | [`Rusty.Engine.Application`](csharp/Rusty.Engine/Application), [`Entities`](csharp/Rusty.Engine.Entities), [`Persistence`](csharp/Rusty.Engine.Persistence), [`Resolution`](csharp/Rusty.Engine.Resolution) | Reusable managed scheduling, entity, persistence, and resolution helpers. |
| Working fixture | [`fixtures/csharp-nativeaot-trial`](fixtures/csharp-nativeaot-trial) | Minimal buildable product and direct runtime exercise. |

## NativeAOT quick start

Generate bindings into a product-local ignored directory:

```bash
scripts/generate-csharp-native-bindings.sh /absolute/product/obj/Generated
```

The fixture is the smallest current end-to-end reference. Its game project
references `Rusty.Engine`; its composition project references the source
generator that supplies the internal native bootstrap.

```bash
dotnet publish fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj \
  -c Release -r linux-x64 -o /tmp/rusty-nativeaot-trial

cargo run -p csharp-product-runtime --locked -- \
  --library /tmp/rusty-nativeaot-trial/CsharpNativeAotTrial.so \
  --bundle-dir fixtures/csharp-nativeaot-trial/browser \
  --content-dir fixtures/csharp-nativeaot-trial/content \
  --mode demand --persistence-root /tmp/rusty-nativeaot-persistence \
  --content-store-root /tmp/rusty-nativeaot-content-store \
  --direct-intent runtime.exercise=payload:runtime.exercise.payload \
  --port 0 --exercise
```

`obj/Generated` is build output: do not edit or commit it. Persistence and
content-store roots are separate developer-host choices. The Engine never
deletes a selected root on shutdown.

## Boundary in one page

- C# owns product state, rules, orchestration, content meaning, and policy.
- Rust owns lifecycle/host integration, input delivery, renderer and
  presentation infrastructure, spatial mechanisms, resources, persistence
  primitives, and other named Engine capabilities.
- The generated API is the green path. A missing capability is an upstream
  request and a valid stopping point—not permission to recreate an Engine
  mechanism downstream.
- TypeScript is limited to DOM UI, accessibility, and explicit Engine
  host/backend work. It does not own gameplay state or render non-UI game
  elements.

For details and the product coding lane, use the linked guides rather than
duplicating architecture here.
