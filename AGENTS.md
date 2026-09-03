# Rusty Engine downstream guidance

## Direction and ownership

Rusty Engine's current downstream lane is ordinary C#. CoreCLR through the
packaged SDK and `rusty dev` is the normal development path; NativeAOT is a
separate fidelity/release path. The paired
proving product is `/home/dev/rusty-dagger`.

> The product decides. The Engine guarantees.

- A C# product owns application and game logic, authoritative product state,
  orchestration, content meaning, and product policy.
- Rust owns reusable Engine mechanisms: lifecycle and host integration, input
  delivery, rendering and presentation infrastructure, spatial mechanisms,
  resources/content, persistence primitives, diagnostics, and other named
  Engine services.
- This is not a language-authority contest. C# is expected to carry substantial
  application authority; Engine mechanisms stay upstream so products do not
  recreate fragile infrastructure.
- Product code is trusted first-party code. Do not add JSON protocols,
  generic dispatch, registries, compatibility negotiation, permission systems,
  or hostile-input ceremony to protect Engine from the product.

Read [the architecture overview](docs/architecture.md) and
[the C# SDK guide](docs/csharp-sdk.md) before changing this boundary.

## Den and missing capabilities

- Engine project: `rusty-engine`. Resolve current Den guidance before
  substantial work; the current task overrides stale documentation and
  superseded downstream-language or authoring assumptions.
- If Den is unreachable, stop and report the failed tool. Do not invent local
  task records.
- If a needed mechanism is not expressible through the generated API, identify
  the exact upstream capability, file or link the owning request when
  authorized, and stop. Do not recreate it in C#, TypeScript, or browser code
  merely to complete a task.

## Generated C# boundary

- `rust/crates/csharp-engine-abi` is the sole ABI declaration source.
  `rust/crates/csharp-engine-services` implements named Engine bridges, and
  `rust/crates/csharp-product-runtime` owns loaded-product binding, lifecycle,
  and host integration.
- `scripts/generate-csharp-native-bindings.sh` runs pinned cbindgen and
  ClangSharp, generating the header plus raw and safe C# inputs under ignored
  `obj/Generated` paths. Generated files are never edited or checked in.
- `csharp/Rusty.Engine` is the public safe service/value surface;
  `csharp/Rusty.Engine.ProductGenerator` generates the internal CoreCLR and
  NativeAOT bind entrypoints and service implementations. Ordinary products
  consume the immutable SDK package and must not handwrite P/Invoke, exported
  entrypoints, unsafe native calls, parallel declarations, or checked
  composition projects.
- A matching runtime pack owns the Rust host and browser shell. `rusty dev`
  stages the loose Product bundle and uses CoreCLR. Products do not copy Engine
  browser assets or invoke Cargo to host themselves.
- Engine source use must be an explicit contributor override. Never teach
  adjacent-checkout discovery as ordinary downstream setup.
- Add coherent named Engine capabilities to the Rust function table and the
  generator path. Do not create method-name dispatch, reflection, plugin
  registries, or a task-specific callback list.
- The boundary handles actual ABI/lifetime concerns only: layout,
  pointer/length coherence, copied retained data, no unwind across ABI, and
  exact release. Underlying Engine services retain their normal invariants.

## Rendering and TypeScript

- Engine owns renderer resources, retained handles, frame construction,
  backend realization, canvas ownership, and renderer lifecycle.
- C# publishes product facts through named Engine APIs. It must not build a
  second renderer, retained-frame implementation, resource loader, canvas, or
  browser-rendering substitute.
- TypeScript may own DOM UI, accessibility, and explicit Engine host/backend
  implementation. Downstream TypeScript must never render non-UI game elements
  or acquire application/gameplay state.

## C# product style

The following are product-facing recommendations, not an unimplemented Engine
module framework. See [C# product style](docs/csharp-product-style.md) for the
full guide.

- Organize code by gameplay domain, with one clear mutable state owner for each
  domain. Keep systems thin: **Read → Decide → Apply → Publish**.
- Use views, requests, receipts, and facts when a boundary benefits from an
  explicit typed contract; do not introduce a bus or framework where a direct
  method is clearer.
- Use Engine-provided update facts, input, and random services for gameplay
  behavior. Do not retain native pointers, create a second loop, or depend on
  browser state for game meaning.
- Prefer file-scoped namespaces, nullable reference types, `internal` and
  `sealed` defaults, records/value types for small immutable data, and explicit
  composition. Keep unsafe/PInvoke out of ordinary product projects.
- Never bury numeric or string tuning/identities in behavior. At minimum give a
  local value a named `const` or `static readonly` declaration; prefer typed
  definitions or product-owned configuration adapters for values that need
  tuning. Avoid a giant global constants dump unless a value is genuinely
  cross-domain.

## Work and evidence

- Add capabilities as coherent service families informed by real downstream
  needs. The generated surface is not a claim that every Rust source API is
  available in C#.
- Report a short milestone before expensive integration: goal advanced,
  necessary surfaces, proof scaffolding, drift/unsupported boundary, and any
  upstream request.
- Tests are evidence, not the deliverable. Run generation, focused compilation,
  CoreCLR staging, NativeAOT publish, or a direct exercise only when it answers the task's
  actual question. Do not chase old documentation, provider-wide, browser,
  packaging, security, conformance, or downstream-Rust gates without a task
  requirement.
- Preserve unrelated work and follow the task's branch and promotion
  instructions.

### Playtest warning deltas

- Use `scripts/capture-playtest-warning-delta.mjs` for report-only warning
  capture on required browser exercises when its Engine host is available. A
  new warning must be understood and fixed, or linked to the exact owning
  Engine or product task before making a clean claim. A linked, recoverable,
  nonfatal warning may coexist with completion when acceptance remains valid.
- Do not demand absolute zero historical warnings. Terminal, safety-critical,
  or unknown-provenance diagnostics, and missing, lagged, dropped, or failed
  capture, block a clean claim. An explicitly compatible baseline is required
  for a delta; no baseline is report-only and comparison remains unavailable.
- Any allowlist is narrowly scoped, owned, reasoned, linked, and expiring; it
  is never a blanket permission to ignore errors. Clippy's warnings-as-errors
  policy is separate and must not be weakened incidentally. Do not blanket
  continue past compiler errors, ABI failures, or explicit Error/Fatal findings.

## Documentation

The intentionally small documentation set is rooted in current source, not in
the superseded corpus preserved in Git history. Keep it concise and truthful;
use history as donor material only. Start at [docs/README.md](docs/README.md).
