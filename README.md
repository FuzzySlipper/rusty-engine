# Rusty Engine — C# NativeAOT trial branch

This is the experimental Engine worktree for testing ordinary C# as the
downstream application/game language:

- worktree: `/home/dev/worktrees/rusty-engine-csharp-runtime`
- branch: `codex/csharp-nativeaot-trial`
- paired product: `/home/dev/worktrees/rusty-dagger-csharp-runtime`
- paired product branch: `codex/csharp-product-runtime`

It is not stable `main` and must not be promoted incidentally.

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
rust/crates/csharp-product-runtime/src/native_api.rs
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
  --port 0 --exercise
```

Generated sources belong under `obj/Generated` and are not committed.

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

The prior mainline documentation was intentionally removed from this branch so
it cannot steer the experiment back toward superseded assumptions. Stable
`main` and Git history retain it. If this direction is selected, a later focused
task will recover useful durable mechanisms and write new documentation from
the proven architecture.
