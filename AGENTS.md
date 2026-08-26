# Rusty Engine C# NativeAOT trial guidance

## Scope

This guidance applies only to the experimental worktree
`/home/dev/worktrees/rusty-engine-csharp-runtime` on branch
`codex/csharp-nativeaot-trial`. It intentionally replaces the mainline Engine
guidance for this branch. Do not promote this branch to `main` as part of an
ordinary task.

The paired downstream experiment is
`/home/dev/worktrees/rusty-dagger-csharp-runtime` on branch
`codex/csharp-product-runtime`.

## Den

- Engine project: `rusty-engine`.
- Resolve live Den guidance before substantial work. The user's current task
  and its C#-trial task description override older project documents that
  assume downstream Rust, compiled TypeScript gameplay, Product Model, or the
  previous authority posture.
- If Den is unreachable, stop and report the failed tool. Do not invent local
  task records or reconstruct current task state from deleted documentation.

## Trial posture

> The product decides. The Engine guarantees.

- Downstream C# owns application and game logic, authoritative product state,
  orchestration, content meaning, and product policy.
- Engine Rust owns durable reusable infrastructure: lifecycle and host
  integration, input delivery, rendering and presentation mechanisms, spatial
  services, content/resource mechanisms, persistence primitives, diagnostics,
  and other named Engine capabilities.
- This division is not a contest over language authority. C# is expected to
  have substantial authority gravity. Engine mechanisms remain upstream so
  products do not recreate fragile infrastructure.
- NativeAOT product code is trusted first-party application code. Do not add
  compatibility negotiation, permission systems, hostile-input policy,
  canonical schemas, registries, generic command buses, or JSON invocation
  protocols merely to protect Engine from the product.
- Underlying Engine services retain their ordinary correctness invariants. The
  foreign boundary owns only real ABI and memory responsibilities: layout,
  pointer/length coherence, borrowed lifetime, copying retained data, no
  unwind/exception across the ABI, and exact ownership/release.

## Generated native API

- `rust/crates/csharp-product-runtime/src/native_api.rs` is the sole ABI source.
- `scripts/generate-csharp-native-bindings.sh` runs pinned cbindgen and
  ClangSharp and emits the C header plus raw/idiomatic C# API into a product's
  ignored `obj/Generated` directory.
- Generated files are never manually edited or checked in.
- Add named Engine capabilities to the Rust function table. Do not create
  method-name dispatch, reflection, a plugin registry, or parallel handwritten
  C# declarations.
- One explicit `rusty_product_bind` bootstrap is acceptable; lifecycle and
  Engine callbacks are compiler-checked through generated function tables.

## Rendering and TypeScript

- Engine owns renderer resources, retained handles, frame construction,
  backend realization, the canvas, and renderer lifecycle.
- C# publishes product facts through named Engine APIs. It must not build a
  second renderer, retained-frame implementation, resource loader, canvas, or
  browser-rendering substitute.
- TypeScript may own DOM UI, accessibility, and explicit Engine host/backend
  implementation. Downstream TypeScript must never render non-UI game elements
  or acquire application/gameplay state.
- When a product cannot express a needed presentation or mechanism through the
  generated API, stop and request the missing upstream capability. Inability to
  proceed is a valid task result and is preferred to a downstream substitute or
  fake proof.

## Work and verification

- Make the smallest coherent mechanism change that enables the current product
  task. Do not generalize ahead of demonstrated Dagger needs.
- Report a short milestone before expensive integration: goal advanced,
  necessary surfaces, proof scaffolding, drift/unsupported boundary, and any
  upstream request.
- Tests are optional evidence, not the deliverable. Run only generation,
  focused compilation, NativeAOT publish, or the direct exercise when those
  checks answer the task's actual question.
- Do not run old documentation, provider-wide, browser, packaging, security,
  conformance, or downstream-Rust gates unless the current task explicitly
  requires one.
- Preserve unrelated work. Commit and push only the experimental branch.

## Documentation status

The branch deliberately contains no `docs/` corpus. The deleted documents
remain available on stable `main` and in Git history. If the C# direction is
selected for main, a later focused documentation task will extract still-useful
durable mechanisms and write a new coherent architecture from demonstrated
behavior. Do not recreate aspirational documentation during the trial.
