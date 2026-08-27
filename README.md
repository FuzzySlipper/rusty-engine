# Rusty Engine — C# downstream runtime

Rusty Engine uses ordinary C# as its downstream application/game language:

- Engine checkout: `/home/dev/rusty-engine`
- paired proving product: `/home/dev/rusty-dagger`

The current NativeAOT implementation proves the direction but still has
walking-spike organization. Engine ABI, host, generator, and public C# SDK
ownership are the next architecture-planning focus before broad API expansion.

## Current model

> The product decides. The Engine guarantees.

C# owns downstream gameplay, state, orchestration, and product meaning. Rust
provides direct named Engine mechanisms. The NativeAOT product is trusted; the
boundary is a generated ABI, not a JSON protocol or policy sandbox.

The Engine continues to own rendering infrastructure. Downstream C# publishes
product facts through generated APIs; TypeScript remains limited to DOM UI and
Engine-owned host/backend implementation.

## Generated bindings

The sole ABI source is:

```text
rust/crates/csharp-engine-abi
```

Generate a product's C header and C# bindings with:

```bash
scripts/generate-csharp-native-bindings.sh /absolute/product/obj/Generated
```

The NativeAOT fixture invokes generation automatically:

```bash
dotnet publish fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj \
  -c Release -r linux-x64 -o /tmp/rusty-nativeaot-trial

cargo run -p csharp-product-runtime --locked -- \
  --library /tmp/rusty-nativeaot-trial/CsharpNativeAotTrial.so \
  --bundle-dir fixtures/csharp-nativeaot-trial/browser \
  --content-dir fixtures/csharp-nativeaot-trial/content \
  --mode demand --direct-intent runtime.exercise=payload:runtime.exercise.payload --port 0 --exercise
```

Generated sources belong under `obj/Generated` and are not committed.

`csharp-engine-services` owns the concrete Engine bridges; the product runtime
owns loaded-product binding and lifetime management.

## Trial rules

- Prefer adding a direct upstream capability over recreating Engine mechanisms
  downstream.
- A missing upstream capability is a valid stopping result.
- Do not introduce JSON invocation/results, generic dispatch, registries,
  version negotiation, or adversarial boundary ceremony.
- Do not use TypeScript or browser code to render non-UI game elements.
- Use narrow compilation/direct-exercise evidence only when it answers the
  current task.

## Documentation

The prior architecture documentation was intentionally removed so it cannot
steer implementation back toward superseded assumptions. Git history retains
it. A later focused task will recover useful durable mechanisms and write new
documentation from the proven architecture.
