# Greenfield downstream product path

This is the starting point for a **new downstream product repository**. It is
not a migration plan and it is not a recipe for making a web app. Rusty Engine
provides host-neutral mechanisms; the product owns the game it makes with them.

The shortest useful rule is:

> Rust owns live meaning. TypeScript may author or present it, but does not
> evaluate, schedule, save, or mutate it.

For an existing repository being simplified or moved, use [downstream gameplay
adoption](../gameplay/downstream-adoption.md) instead. The exact provider
contracts remain in [the design](../../design.md), the relevant code maps, and
the public source they link. Use [upstream promotion and authoring
DSL](upstream-promotion-and-authoring-dsl.md) when deciding whether a new
capability belongs in Engine, downstream Rust, or an authoring facade.

## Choose owners before folders

| Concern | Engine provides | Downstream product owns |
|---|---|---|
| Object facts | Typed components, exact-slot mutation, relationships, snapshots | Entity roles, spawn/despawn policy, game services, full save meaning |
| Reusable mechanics | Stats, tracks, effects, damage, inventory, equipment, named service receipts | What an ID means, action rules, timing, consequences, and orchestration |
| Rules artifacts | Bounded opaque envelopes, canonical bytes, fingerprints, provenance, diagnostics | Payload schema, semantic compilation, publication, catalog admission, and execution |
| Standard gameplay pieces | Opt-in static capability metadata, exact/continuous expression families, standard mechanics operation plans | Product leaves, target selection, operation meaning, facts, transaction, and policy |
| Runtime attempt lifecycle | A reusable structural resolution lifecycle when it fits | Intent admission, fact gathering, predicates, effects, events, scheduling, and one product transaction |
| Spatial/render mechanisms | Canonical spatial services; retained frames and renderer/application-host contracts | World meaning, resource admission, camera/input semantics, window lifecycle, and product acceptance |
| Product UI and host | One public Engine application-host boundary or a Rust-only webview adapter | UI vocabulary, menus, accessibility content, transport, packaging, and host lifecycle |

The table is deliberately not a proposed universal game grammar. Start with
the smallest product-specific Rust types that name the actions, actors, facts,
and consequences actually needed. Promote a neutral Engine mechanism only when
its owner and correctness value are clear; do not add a product service,
scheduler, save format, or rule vocabulary to Engine as a convenience.

## Recommended repository layout

One possible layout makes the authority boundaries obvious without requiring
any particular framework:

```text
my-product/
  Cargo.toml
  crates/
    product-runtime/          # Rust: service, state, fixed-step/turn policy
    product-gameplay/         # Rust: definitions, compiler, facts, operations
    product-host/             # Rust: chosen browser or desktop adapter
  authoring/                  # optional build-time TypeScript workspace
    src/                      # constructors, pure macros, catalogs
    scripts/                  # deterministic materialization plumbing only
  content/
    gameplay/                 # checked canonical artifacts, not TS source
    runtime/                  # admitted product resources/content closure
  ui/                         # optional TypeScript DOM product presentation
    src/
  desktop/                    # optional Tauri packaging/window configuration
  tests/                      # product service, adapter, and host evidence
```

The names are illustrative. The separation is the contract:

- Rust crates hold the authoritative live product state and semantic compiler.
- `authoring/` is optional build-time TypeScript, never the live evaluator.
- `content/gameplay/` contains the canonical materialized artifacts the Rust
  compiler admits; source and artifact provenance stay inspectable.
- `ui/` owns local presentation state only. It does not become a gameplay,
  catalog, save, or renderer authority.
- A product host adapts the selected process/window transport to one named
  Rust product service; it is not an Engine-owned universal product runtime.

## Code philosophy: use the smallest authority surface

### Rust only

Use Rust directly for a compact product whose rules are clearer as typed code,
or when the content does not need a separate authoring language. Rust defines
the action types, owns the data/capability calls, gathers facts, stages effects,
and publishes a complete product result. Engine mechanisms are direct named
service calls, not callbacks or ambient command routing.

### Rust-first semantics with optional generated TypeScript authoring

Use a TypeScript DSL when authored variation benefits from pure composition.
Rust first defines the serialized vocabulary, bounded decoder, semantic
compiler, and runtime interpretation. TypeScript then offers strict builders,
catalogs, and pure macros that lower only to that already-defined vocabulary.
It materializes a canonical artifact, and Rust re-admits and compiles it before
anything reaches the live catalog.

This is the normal route for `gameplay-rules` packages and generated strict
contracts. A generated TypeScript type describes a Rust-owned wire boundary; it
does not create new serialized meaning and cannot be a runtime evaluator.

### TypeScript-only pure macros or presentation

Pure TypeScript can make a catalog readable, lower repeated authoring forms,
format a diagnostic, assemble a bounded UI panel, or adapt physical DOM input
to a typed product request. It may not quietly acquire semantic validation that
belongs in Rust merely because a build script is convenient.

### Forbidden authority moves

Do not put a live expression evaluator, game clock, command scheduler, save
model, canonical catalog, mutable world state, or gameplay transaction in
TypeScript. Do not use the DOM, a URL, browser storage, or an offscreen control
as world state. Do not add HTTP just because the development adapter happens to
use it. Browser code asks a named Rust product service to do work; Rust returns
typed readouts and receipts.

### How a change should land

| Change | First landing | Optional follow-up |
|---|---|---|
| New live mechanic, semantic rule, unit, target meaning, evaluator behavior, or serialized node | Rust definition, validation, compiler, and focused tests | Strict generated or handwritten TypeScript authoring facade after the Rust meaning is settled |
| Neutral reusable mechanism with a clear Engine owner | Owning Engine Rust crate and facade | Engine-owned generated TypeScript contract only when build-time authors need it |
| Product-specific fact or operation | Downstream Rust closed type and product transaction | Product TypeScript constructor/codec that lowers to the Rust-owned wire shape |
| New catalog entry or tuned authored values | Existing TypeScript authoring syntax or direct Rust construction | No new Rust type when the meaning is unchanged |
| Repeated authoring shorthand | Pure TypeScript macro that expands entirely to admitted nodes | None; it must not add a private serialized kind |
| HUD, menu, accessibility, or input presentation | TypeScript component inside the supplied bounded UI root | Typed request/readout adapter to the Rust product service |

Do not add TypeScript merely for symmetry. A Rust-only capability is complete
when direct Rust construction is the clearest product route. Add the authoring
facade when it materially improves authored variation or gives authors one
strict route to an Engine-owned mechanism; the facade follows the Rust
contract and does not become a prerequisite for the Rust capability.

## A small Dagger-like extension, without a local universal grammar

Suppose the product needs an exact check composed from ordinary Engine
arithmetic plus three closed product facts: the equipped tool profile, a target
guard, and a situation modifier. It does **not** need to fork an entire local
expression/predicate/program language.

1. The product defines a closed Rust `ProductLeaf` enum for those three facts
   and one `ComposedExactLeafCodec`. Its strict decode/encode and compilation
   are static Rust code. Compilation turns each leaf into the ordinary exact
   expression plus explicit input/role requirements.
2. The authored definition uses `ComposedExactExpr<ProductLeaf>` and
   `ComposedExactComparison<ProductLeaf>`. Standard owns the literal, input,
   add/subtract/multiply/divide/min/max structure and exact evaluator;
   `Product(...)` is the only product arm.
3. Optional TypeScript authoring uses the generated strict composed-exact
   contract and one matching strict product codec. It can materialize and
   validate the bounded artifact, but it never evaluates the check in play.
4. Product Rust first admits canonical bytes with
   `decode_canonical_rule_package` (or an in-memory candidate with
   `admit_rule_package`). It then passes that `AdmittedRulePackage` to
   `compile_composed_exact_package`. For an aggregate, it selects a bounded
   `SelectedRulePayloadSubtree` from the already-admitted parent before calling
   `compile_composed_exact_embedded`. Only then does it gather current product
   facts and evaluate the compiled comparison through the Rust owner.

The product may pair that predicate with Engine standard mechanics operations
where they fit, and with its own closed operations where they do not. It should
use `ComposedPredicate` and `ComposedOperation<Product>` (or a direct product
policy) to retain product types, rather than sending kind strings and JSON
through a generic extension tunnel.

### Operation mapping and the product transaction

```text
typed player/AI intent
  -> product Rust admits intent and gathers current typed facts
  -> Rust evaluates compiled exact comparison and plans standard/product operations
  -> product ResolutionTransaction stages a complete candidate
  -> named Engine services execute on that private candidate where applicable
  -> product validates guards and publishes once, or publishes nothing
  -> typed receipt/events/projected frame
```

The `ResolutionTransaction` in that flow is product-owned. Engine does not own
the aggregate, world clone, target discovery, scheduler, turn, effects timing,
save, or final publication. A planned standard mechanics operation is not an
authorization to mutate authoritative components during planning. Both player
and AI submit the same typed intent path; AI chooses an attempt, not a damage
delta or a pre-applied mutation.

## Product presentation is not a web-app architecture

For a rich DOM product, import only the public
`@rusty-engine/application-host` artifact from Engine. It mounts one
application frame containing the **sole Engine-owned canvas** and one bounded
downstream UI root. The product supplies Rust-projected frames/content and
mounts its HUD, menus, or forms only inside that UI root.

- Keep one viewport/application frame. Do not make the document grow, rely on
  offscreen controls, or create a second canvas/render loop.
- For a game-style product, choose and pass finite positive
  `presentationAspectBounds` so the canvas, indicators, UI, loading, and
  failure states share one centered clipped frame across browser and WebView
  hosts. Omitting it preserves the legacy full-host layout; it is not a reason
  to let descendants establish their own page geometry.
- A panel may intentionally scroll internally. Ordinary descendants must not
  expand the document or calculate/copy renderer geometry.
- UI input is local presentation input. Every global DOM listener must call
  `context.ui.allowsGameplayInput(event)` before assigning gameplay meaning;
  use interaction modes for menus and modals, then route the resulting typed
  command to product Rust.
- Use the Rust-only `RendererWebviewAdapter` instead when the product does not
  need arbitrary product DOM. Do not inject markup into its fixed Engine-owned
  document.

An ordinary browser adapter may use a development HTTP/WebSocket transport,
but HTTP is not a runtime requirement. For a packaged Tauri product, the
default is one in-process named Rust product service and one WebView with typed
IPC. A loopback sidecar is a product choice only when a measured isolation or
lifecycle need justifies it; it is not the default.

Read the full [downstream renderer and Studio boundary](downstream-renderer-and-studio.md)
before choosing a presentation host. For optional typed diagnostic commands,
read the [runtime developer console](runtime-developer-console.md); its shell
is a bounded diagnostic presentation over a product-owned safe-point path, not
a generic live editor.

## Evidence: prove the owner that changed

| Claim | Evidence that supports it | Does not prove it |
|---|---|---|
| Rust compiler/admission is correct | Focused Rust unit/integration tests, canonical artifact fixtures, rejection/atomicity cases | TypeScript build success |
| Generated/strict authoring facade converges | Rust contract generation plus isolated TypeScript decode/materialization tests | A TypeScript evaluator or runtime JSON object |
| Engine mechanics/resolution integration is correct | Product service tests exercising named owners and stale/failure paths | A renderer screenshot |
| DOM/application-host behavior is correct | Real browser evidence through normal UI, one-frame/canvas and cleanup assertions | Headless Rust tests |
| Packaged desktop behavior is correct | Selected host build plus headed installed WebView evidence for lifecycle, content, input, and shutdown | Browser success or mocked IPC |
| Product behavior is correct | Product-owned scenarios through its actual supported controls and host | Engine provider tests alone |

Run the narrowest checks first, then the owner’s gate. Keep browser and
packaged-host proof separate: Chromium validates the browser adapter; it does
not prove a packaged host, and a package build does not prove a headed WebView.
