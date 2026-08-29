# Managed C# capability migration

Status: synthesized planning record for Den campaign #7502. This file is not
authorization to delete, port, or deprecate a capability. Tasks #7503–#7507
populated the inventory and task #7508 recorded the owner decisions. Task #7509
may create dependency-ordered implementation work from this map.

## Direction

The product decides. The Engine guarantees.

- Downstream C# owns application and gameplay state, meaning, policy,
  orchestration, and the product entity/component model.
- Rust owns durable native, host, renderer, spatial, resource, persistence, and
  other reusable mechanisms whose lifetimes, shared state, performance, or
  cross-call invariants belong upstream.
- Rust may use an internal ECS or specialized data layout for a native
  mechanism. That does not require the product world to be mirrored across the
  ABI.
- The managed SDK remains one default `Rusty.Engine` assembly until measured
  build evidence shows that another assembly boundary pays for its complexity.
  Namespaces remain useful organization inside that assembly. Binding and
  product source generators remain separate build-time tools.
- Existing crate, package, project, ABI-table, and helper boundaries are
  walking-spike or legacy evidence. They are not automatically the target
  architecture.

The migration preserves capabilities that still matter; it does not preserve
machinery merely because that machinery once made TypeScript resemble an
ordinary application language, serialized C#-natural code into JSON, or worked
around the absence of inheritance, virtual dispatch, or direct product-owned
types in Rust.

## Placement test

Apply these questions to each capability, not to a whole crate by default:

1. Who owns its semantic truth? Product meaning, authored policy, orchestration,
   and authoritative application state normally belong in C#.
2. Does it require native/host lifetime, shared physical-world state, renderer
   or platform integration, substantial reusable performance work, or atomic
   invariants spanning calls? That mechanism normally remains in Rust.
3. Is it a reusable deterministic calculation independent of legacy Product
   Model packaging? It may remain in Rust when sharing or performance justifies
   the crossing; otherwise an ordinary managed implementation may be simpler.
4. Does its shape primarily exist for compiled TypeScript, canonical JSON
   artifacts, package identity, compatibility negotiation, generic registries,
   or imitation of C# language features? Preserve any useful semantics, not the
   scaffolding.
5. Would the proposed split create two long-lived owners for the same domain
   state? If so, redesign it. Product facts and native mechanism state may
   coexist only through an explicit projection boundary.

Using declarations select namespaces; they do not justify separate assemblies.
Current downstream imports are ergonomic evidence, not a vote on whether a
capability has value.

## Disposition vocabulary

| Disposition | Meaning |
| --- | --- |
| `retain-rust` | Keep the capability's durable mechanism in Rust and expose only the coherent product-facing surface needed across the generated boundary. |
| `port-managed` | Move the capability's semantic owner to ordinary safe C#; remove the native owner after consumers and prerequisites move. |
| `adapt-managed` | Preserve selected semantics in a C#-natural shape, without porting the legacy architecture or protocol literally. |
| `abandon` | Intentionally discard the capability or behavior. This requires an explicit owner decision and a recorded reason. |
| `defer` | Preserve the current implementation while a prerequisite or product decision remains unresolved. |
| `owner-decision-needed` | Evidence cannot establish whether the capability is valuable or which semantics matter. Ask; do not infer from current usage. |

`Quarantine` may describe a temporary publication posture, but it is not a
final disposition. A quarantined surface still needs one of the dispositions
above and explicit removal gates.

## Required record for every capability

Each final inventory entry states, directly or through its keyed matrix row:

- capability and current source owner/files;
- observable semantics and invariants;
- recoverable original intent, with source/Den/Board/Git pointers and an
  explicit distinction between owner direction and agent proposal;
- current production, tooling, fixture, generated, and downstream dependents;
- native/host/shared-state dependency and present C# exposure;
- proposed disposition and target namespace/type shape when applicable;
- semantics to preserve and legacy baggage not to port;
- neutral extractions, consumer moves, ABI changes, and other removal gates;
- evidence confidence and any owner question.

The survey tables below record semantics, evidence, disposition, and local
gates. The keyed dependency/exposure matrix supplies the cross-cutting consumer,
managed-surface, confidence, and decision fields that would be unreadable if
repeated inside every prose cell. Read the two views together.

No source family disappears from the ledger because it has no current
downstream consumer. Conversely, presence in the current ABI or fixture does
not make a family recommended SDK architecture.

## Capability family index

This index defines survey coverage. The detailed tables below will split these
families into independently disposable capabilities.

| Survey family | Current owners included | Planning task |
| --- | --- | --- |
| Rules and authoring | `gameplay-rules`, `gameplay-standard`, `gameplay-continuous-mechanics`, `rules/packages/*`, rule/standard ABI bridges | #7504 |
| Product Model and compiled TS | `product-model`, `product-kernel`, `product-assembly`, `product-materializer`, `runtime-composition`, `runtime-vm`, `runtime-standard-capabilities`, related CLI/dev-host paths | #7504 |
| Mechanics and numeric evaluation | `gameplay-mechanics`, exact/continuous evaluators, stats, tracks, effects, damage, inventory, equipment, RNG-facing mechanics | #7505 |
| Resolution and state machines | `gameplay-resolution`, `state-machine`, structural sessions, policy/evaluation helpers | #7505 |
| Product/runtime state | `entity-state`, managed entity helpers, runtime lifecycle/input/mutation/schedule/timeline/UI, update and scheduler helpers | #7506 |
| Persistence composition | native persistence primitives plus managed codecs, stores, restore/import coordination, and Product Model coupling | #7506 |
| Managed SDK shape | all `csharp/*.csproj`, namespaces, generated service/context surface, product/bootstrap generators, examples, and downstream reference patterns | #7507 |
| Adjacent tooling and facades | `rusty-engine` reexports, `csharp-engine-abi`, `csharp-engine-services`, `csharp-product-runtime`, inspector/developer-command/CLI/fixtures and build scripts | #7504–#7507 |

The following are checked for accidental coupling but are presumed durable Rust
mechanism families unless source evidence shows product semantics embedded in
them: rendering/presentation, browser host integration, spatial/collision/
navigation/physics, assets/content/resources, voxel mechanisms, and core math,
space, time, identity, and value primitives.

## Intent provenance

These handles explain the major historical goals without making old proposals
current mandates:

| Family | Historical intent evidence | Current interpretation |
| --- | --- | --- |
| Rules and authoring | Board #18, #33, #42; Den #7176 and #7180 | Stop downstream hardcodes through bounded definitions, provenance, and named mechanisms. The JSON/TS package path is not itself durable C# architecture. |
| Gameplay Standard and numeric domains | Board #30–#34, #42, #96; Den #7175 and #7177–#7198 | Preserve useful exact/continuous semantics and explicit evidence; reject a mandatory Standard facade, universal number, or consumer-count promotion rule. |
| Mechanics | Board #18; Den #6284–#6289; historical closeout at `41ce7c4` | Reusable stats/tracks/sources/effects/damage/inventory/equipment were separated from attacks, turns, AI, scheduling, complete saves, and product meaning. |
| Structural resolution | Board #18, #32, #42; Den #7032; historical reduction at `ea15f9a` | Preserve only bounded structural coordination and evidence where useful; it was never intended as a combat system or universal rules language. |
| Product entity/state split | Den #6284–#6286; Board #99 and #106; managed world commit `5b1c1a0` | Native mechanisms may own specialized state; C# owns the ordinary product entity/component model. Avoid two canonical worlds. |
| Lifecycle, schedule, and timeline | Board #99 and #106–#109; Den #7247 and #7255–#7258 | Lifecycle/input/ticket fencing remains useful. Rust-owned mandatory product scheduling/orchestration belonged to the cancelled Product Model direction. |
| Product Model | Board #99–#109; cancelled task #7247 | Retain only neutral mechanisms and generated bootstrap lessons. Compiled TS, Product Kernel, Runtime Composition, and Rust-owned product meaning are not the C# baseline. |
| C# SDK and helpers | Board #132, #135, comments #137–#138; Den #7304–#7310 | One safe generated SDK, direct named services, generated NativeAOT bootstrap, ordinary product C#, and optional small helpers. Board #133/#134 remain proposal/donor material, not a module/analyzer framework mandate. |
| Deferred long-horizon ideas | Board #35–#41 and #96 | Developer preview/play/admin, catalog overlays/epochs, scratch/checkpoint/fault isolation, a fact-to-intent VM experiment, and continuous cadence/residual persistence must receive explicit dispositions rather than vanish with their old provider. |

## Capability ledger

Task #7508 reconciled these entries with the owner decisions below. Explicit
`defer` entries are consumer-driven deferrals, not unanswered architecture
questions.

Survey baseline: Engine `main` at `391bf25`. Historical Board posts and Git
commits explain why code was written; most Board posts are agent-authored
proposals, not owner instructions. Current owner direction, current source, and
the #7502 task contract take precedence.

### Rules, Standard, Product Model, and TypeScript

| Capability and evidence | Current semantics and boundary | Disposition, target, and gates |
| --- | --- | --- |
| Rules artifact envelope, identity, canonical JSON, provenance, and diagnostics ([`gameplay-rules`](../rust/crates/gameplay-rules/src/lib.rs)) | Admits bounded immutable packages, canonicalizes bytes, fingerprints content, and correlates sources. It never evaluates gameplay meaning. The generated `Rules` service exposes this artifact protocol directly. | `abandon`. There is no separately supported TypeScript authoring or package lane. Do not port the JSON envelope, compatibility ceremony, or package registry into C#. Removal requires resolving inspector, Standard, ABI, fixture, generator, and CLI consumers. |
| Rule package-set dependency resolution ([`resolve.rs`](../rust/crates/gameplay-rules/src/resolve.rs)) | Deterministic bounded topological ordering, duplicate/version/fingerprint checks, and cycle diagnostics. Useful only when rule packages remain a real product artifact. | `abandon` with the Rules artifact lane. Reintroduce a normal typed dependency helper only if a future managed content system demonstrates the need. |
| Rule payload subtree selection ([`selection.rs`](../rust/crates/gameplay-rules/src/selection.rs)) | Copies a canonical, fingerprint-fenced subtree for inspection/tooling; it is not product rule evaluation. | `abandon` with the Rules artifact lane. A future tooling/content consumer may request a typed read API; do not preserve generic JSON-pointer gameplay access speculatively. |
| Exact expression/value evaluation ([`exact.rs`](../rust/crates/gameplay-standard/src/exact.rs)) | Pure bounded integer expression compilation/evaluation over `MechanicsScalar`, explicit input/roll evidence, checked arithmetic, comparisons, and work receipts. No host or shared-state requirement. | `adapt-managed` as ordinary typed C# values/functions. Preserve useful checked arithmetic, rounding, quotas, duplicate-evidence rejection, and vectors without serialized expression trees or a native call boundary. |
| Continuous expression/value evaluation ([`continuous.rs`](../rust/crates/gameplay-standard/src/continuous.rs)) | Pure finite-binary64 values, bit identity, explicit input bundles, bounded expressions/comparisons, and work receipts. | `adapt-managed` as ordinary C# where a named consumer needs these semantics. Keep exact and continuous domains distinct; do not create one universal `Number` abstraction. |
| Standard definition/package admission, composed leaves, and extension artifacts ([`package.rs`](../rust/crates/gameplay-standard/src/package.rs), [`composed.rs`](../rust/crates/gameplay-standard/src/composed.rs), [`extension.rs`](../rust/crates/gameplay-standard/src/extension.rs)) | Wraps typed definitions and caller-compiled product leaves in Rules provenance/schema machinery. This was primarily a build-time authoring seam. | `abandon`. Preserve selected typed semantics in product C# without porting generic leaf registries, schema dispatch, or opaque runtime JSON. |
| Standard mechanics operation planning and generic role binding ([`resolution.rs`](../rust/crates/gameplay-standard/src/resolution.rs)) | Binds actor/target-style roles, evaluates operands, plans track/damage/effect/inventory/equipment requests, and validates read sets. It composes existing mechanisms but has no native/host requirement. | `adapt-managed` for any useful product coordinator/read-set concepts; `abandon` the universal operation enum/role grammar unless a concrete product proves it. Direct domain C# calling named mechanisms is the baseline. |
| Quantization, residual carry, cadence, and exact deadlines ([`quantization.rs`](../rust/crates/gameplay-standard/src/quantization.rs), [`cadence.rs`](../rust/crates/gameplay-standard/src/cadence.rs)) | Pure conversion policies and caller-owned continuation records. It owns no clock, loop, resource, component, cap, or save format. Historical experiments found partition drift and deliberately left cadence ownership downstream. | `adapt-managed` for small conversion helpers; `defer` residual/cadence continuation until a product-neutral persisted resource exists. Preserve rounding/provenance/policy-version fields if adopted. Do not create an Engine cadence loop. |
| Bounded evidence/sample plans ([`bounded_evidence.rs`](../rust/crates/gameplay-standard/src/bounded_evidence.rs)) | Validates named caller-supplied sample ranges and receipts; it does not generate randomness or assign product meaning. | `adapt-managed` only when an Engine managed capability actually consumes the contract; otherwise leave validation in ordinary product C#. |
| Standard presets and fixed vocabulary ([`presets.rs`](../rust/crates/gameplay-standard/src/presets.rs)) | Emits inert actor/vitality/resource and destructible/integrity/impact catalog fragments. Names/defaults are product policy. | `abandon` as Engine vocabulary. Preserve only as donor/sample code moved into a product namespace. |
| Standard projections, receipts, and inspection ([`projection.rs`](../rust/crates/gameplay-standard/src/projection.rs), [`engine-inspector`](../rust/crates/engine-inspector)) | Read-only evidence over packages, evaluation, mechanics, and resolution. The developer console work also planned preview/play/admin/provenance lanes. | `retain-rust` for mechanism inspection where the mechanism remains; `adapt-managed` typed readouts for C# tooling. Do not retain an obsolete evaluator only to retain its inspector. |
| TypeScript Rules/Standard authoring and contract generation ([`rules/packages`](../rules/packages), [`rules/scripts`](../rules/scripts)) | Build-time immutable drafts, Rust-shaped validation, canonical output, and Rust/TS convergence fixtures. No runtime evaluator. | `abandon`. C# definitions are C# code; do not port the Node serializer or convergence fixture suite. Any future arbitrary-code development console is a separate tooling problem and is not a reason to retain this authoring architecture. |
| Product Layout manifest and compiled composition ([`product-model`](../rust/crates/product-model/src/lib.rs)) | `rusty.toml`, input maps, five schedule phases, opaque definitions/timelines, capability links, quotas, and static artifact admission. This was the cancelled Product Model composition root. | `abandon`; there is no separately supported Rust/TS product lane. Extract only neutral input, identity, JSON-limit, schedule, or timeline primitives still required by retained Rust mechanisms. |
| Product Kernel and Runtime Composition ([`product-kernel`](../rust/crates/product-kernel/src/lib.rs), [`runtime-composition`](../rust/crates/runtime-composition/src/lib.rs)) | Static source-linked owners and a five-lane lifecycle/input/schedule/timeline/mutation/UI root. Even without a runtime registry, it duplicates the direct C# product root. | `abandon` from the C# path. Preserve useful phase/token and named mechanism concepts individually; do not port macros, module registration, generic dispatch, or a `ProductApplication` framework. |
| Runtime VM ([`runtime-vm`](../rust/crates/runtime-vm/src/lib.rs)) | Bounded fresh QuickJS realm with fixed exports and atomic revisioned JSON state. It is a distinct TS product option, not needed by NativeAOT C#. | `abandon` with the TypeScript product lane. A future development-only arbitrary-code facility must be evaluated on its own requirements rather than inheriting QuickJS or JSON state by default. |
| Product materializer, assembly, and Product-Model CLI paths ([`product-materializer`](../rust/crates/product-materializer/src/lib.rs), [`product-assembly`](../rust/crates/product-assembly/src/lib.rs), [`rusty-cli`](../rust/crates/rusty-cli)) | Build/package tools for TS/Rust products, generated Rust assembly source, product layouts, and contract outputs. | `abandon` after removing retained consumers. C# uses normal .NET build/content paths plus the generated NativeAOT bootstrap; do not port Node/materialization ceremony into the SDK. |
| Product development host ([`product-dev-host`](../rust/crates/product-dev-host/src/lib.rs)) | Loopback HTTP/SSE, serialized runtime calls, lifecycle/input/timeline/UI host models. It owns host integration, not gameplay. | `retain-rust`; remove Product Model type contamination from the retained runtime lanes. Do not move browser/host machinery into product C#. |

### Mechanics, resolution, state, and numeric helpers

| Capability and evidence | Current semantics and boundary | Disposition, target, and gates |
| --- | --- | --- |
| Mechanics IDs, exact scalar/ratio arithmetic, and immutable catalog ([`gameplay-mechanics`](../rust/crates/gameplay-mechanics/src/lib.rs), [`scalar.rs`](../rust/crates/gameplay-mechanics/src/scalar.rs), [`catalog.rs`](../rust/crates/gameplay-mechanics/src/catalog.rs)) | Checked ±1e12 integers, normalized ratios, canonical typed IDs, product-authored definitions, catalog validation/version/fingerprint. Pure admission/arithmetic feeds stateful services. | `adapt-managed` into the Engine C# SDK. Preserve useful checked arithmetic and stable typed identities without keeping a native call boundary, immutable package protocol, or universal registry. |
| Seven exact mechanics component families and codecs ([`component.rs`](../rust/crates/gameplay-mechanics/src/component.rs)) | Stats, tracks, intrinsic sources, effects, inventory, item, and equipment live in the shared native `EntityState`, with revisions, catalog versions, codecs, and exact replacement. | `adapt-managed` for the reusable families other than damage. Product-owned managed state becomes canonical; retire the native mechanics mirror after managed revisions, containment, and atomic publication cover the selected semantics. Do not preserve native codecs as the product save format. |
| Modifier/source attribution and collection ([`source.rs`](../rust/crates/gameplay-mechanics/src/source.rs)) | Expands intrinsic/effect/equipment/request sources; validates provenance and revisions; orders deterministically; reports applied/suppressed/inapplicable decisions. | `adapt-managed` as an Engine C# mechanism. Preserve deterministic attribution/order where useful; product C# owns definitions, meaning, and consequences. Do not add an ambient source registry. |
| Stats and tracks ([`stat.rs`](../rust/crates/gameplay-mechanics/src/stat.rs), [`track.rs`](../rust/crates/gameplay-mechanics/src/track.rs)) | Modifier evaluation, checked scale/add/clamp, prospective no-stranding validation, read/spend/restore/set/reconcile, revision-guarded exact publication. Product decides what a value means and when actions are legal. | `adapt-managed` as Engine C# mechanisms. The Engine supplies generic stat calculation and track mutation/current-value invariants; downstream supplies concepts such as health, damage, healing, stamina, and action legality. Preserve selected ordering, rounding, stale, and no-stranding behavior in managed form. |
| Effects ([`effect.rs`](../rust/crates/gameplay-mechanics/src/effect.rs)) | Provenance-aware independent/refresh/replace stacking, prospective track validation, and source activation. No duration, timer, callback, or scheduler. | `adapt-managed` as a reusable Engine C# stacking/lifecycle mechanism. Product C# owns duration, timing, names, meaning, and consequences. Do not turn it into an effect bus or hidden scheduler. |
| Damage and restoration ([`damage.rs`](../rust/crates/gameplay-mechanics/src/damage.rs)) | Preview/apply pipeline of prevention, flat reduction, one rounded scale, ordered absorption, and target-track application with bounded multipart receipts and one exact publication. It owns no attack, hit, target, reaction, death, or score semantics. | `abandon` as an Engine concept. Damage and healing are downstream domain meanings assembled from generic Engine track/stat/effect mechanisms. The existing pipeline remains donor evidence only and must not hold up native mechanics removal. |
| Inventory, unique items, containment, equipment, and capacity ([`item.rs`](../rust/crates/gameplay-mechanics/src/item.rs)) | Canonical fungible stacks, entity-backed unique items, maintained containment, typed capacity costs, multislot/exclusivity rules, source activation, explicit unequip/transfer/destruction policy. | `adapt-managed` as reusable Engine C# mechanisms. Preserve maintained containment, capacity, multislot/exclusivity, and cross-component atomicity where selected; downstream owns item/equipment meaning and policy. |
| Mechanics views, snapshots, imports, and receipts ([`snapshot.rs`](../rust/crates/gameplay-mechanics/src/snapshot.rs), [`view.rs`](../rust/crates/gameplay-mechanics/src/view.rs)) | Strict registry/catalog-aware mechanism snapshots and bounded readouts; not a complete product save format. | `adapt-managed` into ordinary typed readouts and product-composed persistence. Preserve only semantics selected for the managed mechanisms; retire native catalog/snapshot formats with their owner. |
| Continuous mechanics ([`gameplay-continuous-mechanics`](../rust/crates/gameplay-continuous-mechanics/src/lib.rs)) | Separate finite-binary64 stats/tracks/sources/effects catalog and four durable component families sharing the exact entity binding. No continuous damage/inventory/equipment/cadence. | `adapt-managed`. Continuous product values live in C#; preserve useful finite-value and dependent-track validation without a second native component authority. |
| Structural resolution lifecycle ([`gameplay-resolution`](../rust/crates/gameplay-resolution/src/lib.rs), [`structural.rs`](../rust/crates/gameplay-resolution/src/structural.rs)) | Bounded Admit/Gather/Check/Plan/BeforeCommit/Commit/Consequences traversal, correlation, child budgets, preview/apply, traces, and transaction finalization; product owns all meaning and state. | `adapt-managed` only as ordinary C# helpers or donor concepts. Direct domain methods, interfaces, and virtual dispatch are the baseline; retire the native per-node ABI and mandatory coordinator. Preserve structural bounds or transaction separation only where a real managed consumer uses them. |
| Generic Rust policy/program/resolver and Standard operation enum ([`policy.rs`](../rust/crates/gameplay-resolution/src/policy.rs), [`resolver.rs`](../rust/crates/gameplay-resolution/src/resolver.rs)) | Generic traits/program nodes and staged downstream effects; Standard adds a mechanics-shaped operation grammar. These primarily compensate for static Rust composition and are not exposed as the C# product API. | `abandon` as a mandatory Engine framework; `adapt-managed` only selected read-set/receipt concepts. Ordinary virtual methods, interfaces, resolvers, and domain methods are the C#-natural lane. |
| State-machine transition validation and stores ([`state-machine`](../rust/crates/state-machine/src/lib.rs)) | Bounded graph admission, guarded transitions, and revisions. Current C# stores instances in managed `EntityWorld`; the ABI retains definitions and validates detached transitions, while older `StateMachineStore` can retain native instances. | `adapt-managed` as ordinary C#. Managed product state is the sole instance owner; retire the native `StateMachineStore` and native validator after callers move. |
| Deterministic RNG ([`svc-rng`](../rust/crates/svc-rng/src/lib.rs)) | Seeded stateless keyed draws and mutable scoped streams behind native handles; no ambient entropy, time, or global state. | `retain-rust` for shared authoritative randomness and scoped streams. Stream save/restore is optional future work for an identified consumer, not a migration gate. Preserve versioned vectors, framing, unbiased mapping, and fork/counter semantics. |
| Standard bounded roles, evidence, extensions, and presets | Pure validation and authoring concepts around the families above. | Preserve only named useful concepts in C# product code. Do not port generic registries, opaque operation dispatch, TypeScript callbacks, or fixed actor/vitality vocabulary. |

### Entity/component, update, timeline, and persistence composition

| Capability and evidence | Current semantics and boundary | Disposition, target, and gates |
| --- | --- | --- |
| Native entity/component state ([`entity-state`](../rust/crates/entity-state/src/lib.rs)) | Typed component registry/stores, lifecycle, revisions, atomic commands, transforms, containment/relationships, projections, and mechanism snapshots. It has no Product Model dependency and is used by mechanics, spatial, rendering, content, and tooling. | `retain-rust` for mechanism-owned native state. Do not wholesale-port its layout or snapshot schema to C#. Every retained native consumer must identify the subset it owns; ordinary product facts remain C# authority. |
| Managed product entity world ([`EntityWorld.cs`](../csharp/Rusty.Engine.Entities/EntityWorld.cs), [`ComponentType.cs`](../csharp/Rusty.Engine.Entities/ComponentType.cs)) | Product-owned typed storage with stable IDs, lifecycle, containment, revisions, deterministic queries, batches, snapshots, and restore candidates. It deliberately does not mirror Rust `EntityState`. | `adapt-managed`; merge into the one assembly under `Rusty.Engine.Entities` as an optional default storage helper, not a mandatory ECS/framework. It avoids a native call for ordinary product facts and gives cross-owner adapters one guarded publication boundary. Revisit reserved Engine component IDs and clone-on-batch cost only when measured use makes them relevant. |
| Named managed/native entity adapters ([`Rusty.Engine.Entities`](../csharp/Rusty.Engine.Entities)) | Explicit projections for mechanics, dynamics, kinematic/motion, spatial, character, world origin, appearance, and state machines. They use copied facts, guard rechecks, and one managed publication rather than a registry. | `adapt-managed` only where an adapter standardizes a reusable Engine mechanism crossing or a real cross-owner transaction. Spatial/character/dynamics/kinematic/motion/world-origin/appearance are plausible upstream helpers; mechanics and state-machine adapters should retire or simplify as those owners move to C#. Product-specific component mappings remain downstream. Direct service calls remain valid. |
| Runtime lifecycle ([`runtime-lifecycle`](../rust/crates/runtime-lifecycle/src/lib.rs)) | Product-Model-free lifecycle state, generations/control revisions, realtime/demand/external admission, simulation/presentation facts, pause/restart/fault/shutdown, and typed phase tokens. | `retain-rust`. C# consumes copied `ProductUpdate` facts and owns product orchestration; it must not create another host clock or central Engine loop. |
| Managed update pipeline and scheduler ([`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application)) | Optional named callback phases and deterministic due-callback ordering driven only by admitted update facts; no clock, host loop, native state, persistence, or rollback. | `adapt-managed` in the default `Rusty.Engine` assembly under `Rusty.Engine.Application`; it remains an optional convenience. Follow-on continuation work must not own a second clock or require the Product Model scheduler. |
| Product Model runtime schedule ([`runtime-schedule`](../rust/crates/runtime-schedule/src/lib.rs)) | Compiles a closed five-phase authored schedule and validates dependency/access/cadence before caller-supplied dispatch. It stores no callbacks or clock but is tightly coupled to Product Model types. | `abandon` with Product Model. Do not port the five-phase DSL, static access graph, or generic dispatcher. The optional managed update/scheduler/coroutine helpers consume Engine-admitted update facts directly. |
| Runtime timeline and completion tickets ([`runtime-timeline`](../rust/crates/runtime-timeline/src/lib.rs)) | Bounded step queue, deterministic release order, finite recurrence, issue-ordered completion tickets, lifecycle fencing, snapshot/restore of mechanism state, and C# completion callback. It owns no clock, executor, callback, or product meaning. | `retain-rust` as a reusable mechanism; extract Product Model identity/opaque-data/template coupling. Add named C# schedule/cancel/read surfaces only for identified callers/use cases. Do not port opaque Product Model templates as the product API. |
| Runtime mutation ([`runtime-mutation`](../rust/crates/runtime-mutation/src/lib.rs)) | Closed capability descriptors, caller planner, guarded owned candidate, and one fail-atomic assignment. It is coupled to Product Model/Kernel operation envelopes. | `abandon` as generic C# product mutation infrastructure; `retain-rust` only for a named native authority that genuinely needs the mechanism. Managed `EntityWorld` typed candidates are the product-side shape. |
| Runtime input ([`runtime-input`](../rust/crates/runtime-input/src/lib.rs)) | Host-normalized physical/direct facts, held/edge synthesis, focus/restart/overflow clearing, sequence and binding fences. Product meaning remains downstream. | `retain-rust`, extracting input enums/value kinds/bounds from Product Model. C# gets copied typed events/configuration. Do not move DOM translation or add an input event bus. |
| Runtime UI projection ([`runtime-ui`](../rust/crates/runtime-ui/src/lib.rs)) | Bounded copied DTO/value transport with streams, sequences, and staged publication. It owns no DOM renderer, product state, callback, clock, or scheduler. | `retain-rust`; extract identity/JSON validation from Product Model. C# publishes typed facts through the named UI service; TypeScript remains DOM UI only. |
| Persistence primitives and product composition ([`ProductStateStore.cs`](../csharp/Rusty.Engine.Persistence/ProductStateStore.cs), [`content-store`](../rust/crates/content-store/src/lib.rs)) | Product C# owns DTO schemas, codecs, migrations, scope/key, and restore meaning. Rust owns durable bytes/storage and mechanism snapshots. Composition is an ordered product-selected adapter list. | `retain-rust` native storage; `adapt-managed` codecs/stores in the one assembly. Do not port a generic Rust archive or automatically persist every managed/native world. |
| Product Kernel/Runtime Composition root | A separate closed orchestration root over lifecycle, input, schedule, timeline, mutation, and UI. | `abandon` from the direct C# runtime. Extract retained lanes individually so the generated C# product root remains the only product orchestrator. |

### Managed SDK and one-assembly target

There is no measured compile-time problem supporting an assembly split. Current
Dagger, Space, and CraftSurvive runtime/game projects reference only
`Rusty.Engine`; none references the four helper assemblies. Their NativeProduct
projects also reference `Rusty.Engine.ProductGenerator` correctly as a separate
build-time analyzer dependency.

| Current project/surface | Survey finding | Target |
| --- | --- | --- |
| [`Rusty.Engine`](../csharp/Rusty.Engine/Rusty.Engine.csproj) | Safe generated contracts/values plus three handwritten Mechanics helpers. It compiles no raw bindings. | The one default runtime assembly. Retain `Rusty.Engine` as the generated root namespace. |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | Small optional update/scheduler convention, compiled with the default runtime assembly. | Retained under `Rusty.Engine.Application`; it remains optional. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine.Entities/Rusty.Engine.Entities.csproj) | About 5,026 lines: product world plus many service adapters and paired mechanics composition. Valuable pieces and spike-shaped pieces are mixed. | Fold accepted sources into the runtime assembly under `Rusty.Engine.Entities`. Per-helper disposition precedes consolidation; do not make this the canonical Engine ECS. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine.Persistence/Rusty.Engine.Persistence.csproj) | About 649 lines: strong product-codec/native-store split plus entity/mechanics/continuous composition. | Fold accepted sources under `Rusty.Engine.Persistence`; keep product codecs explicit and adapter composition opt-in. |
| [`Rusty.Engine.Resolution`](../csharp/Rusty.Engine.Resolution/Rusty.Engine.Resolution.csproj) | About 437 lines around a native structural session and product transaction. | Fold or rewrite accepted concepts under `Rusty.Engine.Resolution`; do not preserve ABI-per-node ceremony merely to preserve this project. |
| [`Rusty.Engine.BindingGenerator`](../csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj) | ClangSharp/cbindgen executable build tool; unsafe AST interop is appropriate here. | Keep separate as build-time tooling. |
| [`Rusty.Engine.ProductGenerator`](../csharp/Rusty.Engine.ProductGenerator/Rusty.Engine.ProductGenerator.csproj) | Roslyn analyzer/source generator that embeds raw ABI/service implementation inputs and emits the NativeAOT bootstrap. | Keep separate, analyzer-only. Unsafe, pointers, `GCHandle`, exports, and raw tables remain generated/boundary-local. |
| Example projects and NativeAOT fixture | Proof/scaffolding; the broad capability exercise is not product architecture. The Application example already drifts from the current `IEngineContext`. | Keep separate or retire individually. Update references only when the one-assembly implementation occurs; do not use example breakage to retain obsolete services. |

The current generated surface contains 27 service families, roughly 148 enums,
725 record structs, and 45 disposable owners. That is surface-volume evidence,
not compilation-pressure evidence. Generation remains systemic: ABI declaration,
cbindgen, ClangSharp, safe generation, and generated bootstrap. The migration
must not replace it with handwritten interop or a method inventory.

### Adjacent mixed families and explicit exclusions

Every `rusty-engine` facade reexport or C# ABI module must eventually have a
ledger row or an explicit exclusion. These high-risk adjacent families already
need a split decision:

| Family | Required split |
| --- | --- |
| Voxel annotations and material authority ([`voxel-annotation`](../rust/crates/voxel-annotation), [`asset-catalog`](../rust/crates/asset-catalog)) | `retain-rust` for canonical content/material state, admission, and collision/occlusion/structural facts. Product C# proposes typed mutations, reads facts, and owns gameplay interpretation such as what a hazard or spawn marker causes. |
| Authored scenes, prefabs, and content load plans ([`authored-scene`](../rust/crates/authored-scene), [`content-store`](../rust/crates/content-store)) | `retain-rust` for bounded atomic admission, artifact roles, load ordering, resources, and publication. C# proposes typed content mutations and owns which entity/prefab/content facts mean what. Do not collapse this into generic entity persistence. |
| Environment authoring and seeded materialization ([`environment-authoring`](../rust/crates/environment-authoring), [`Studio owner map`](../studio/owner-adoption.tsv)) | `retain-rust` for deterministic generation/admission and reusable materialization invariants over Engine-owned authored content. Product C# supplies typed requests, reads results, and owns authored gameplay meaning and selection policy. Generator definition placement may follow concrete authoring workflows; it is not a migration blocker. |
| Spatial, collision, navigation, character, voxel residency, and world origin ([`engine-spatial`](../rust/crates/engine-spatial), [`svc-pathfinding`](../rust/crates/svc-pathfinding)) | `retain-rust` native indexes, queries, leases, revisions, and high-frequency mechanisms; `adapt-managed` product movement policy, goals, and copied facts. These are not candidates for a downstream replacement system. |
| Animation and explicit-time voxel playback ([`voxel-object-runtime`](../rust/crates/voxel-object-runtime), [`render-presentation`](../rust/crates/render-presentation)) | `retain-rust` for playback/clip state, renderer/resource realization, and reusable sampling. Product C# proposes typed playback mutations, reads state, and owns why an animation plays. Collision selection remains explicit rather than following visual frames accidentally. |
| Runtime `observe-pairs` ([`runtime-standard-capabilities`](../rust/crates/runtime-standard-capabilities)) | `retain-rust` for the reusable distance/facing/occlusion query and deterministic per-target reduction, but do not publish the Product Model name or wrapper as the managed API. Surface discoverable `Perception`/`Visibility` C# utilities for direct custom use plus optional `EntityWorld` vision/target adapters. C# supplies observer/target facts and owns AI, stealth, and awareness policy. Retire Product Model schedule/mutation coupling. |
| Input, time, and RNG ([`runtime-input`](../rust/crates/runtime-input), [`core-time`](../rust/crates/core-time), [`svc-rng`](../rust/crates/svc-rng)) | Retain normalized host facts and deterministic services in Rust. Keep host time, admitted simulation steps, animation timestamps, and product scheduler time distinct. |
| Rendering, audio, UI host/backend, assets/resources, voxel mechanisms, core math/space/IDs | Explicitly excluded from gameplay-language migration except where a mixed file embeds product semantics. These remain durable Engine mechanisms and generated service families. |
| Developer command, inspector, CLI, dev host, Studio, and renderer TypeScript | Tool/host/editor lanes, not C# gameplay owners. Survey their dependency closures before deleting a legacy provider, but do not port them merely for language uniformity. |

## Dependency and managed-exposure matrix

This matrix keys the current closure back to the ledger. “None” means no direct
managed API, not that the capability is unused or valueless.

| Capability key | Current production/tooling dependents | Current managed exposure | Confidence and decision gate |
| --- | --- | --- | --- |
| Rules envelope/set/selection | `csharp-engine-services`, Standard, inspector, facade, contract generator, fixture | Generated `IRulesService` and leases/readouts | High; abandon after listed dependents move or retire. |
| Standard exact/continuous evaluation and admission | `csharp-engine-services`, Mechanics/Resolution/Rules via Standard, Continuous Mechanics, inspector/developer command, facade | Generated `IStandardExactService` and `IStandardContinuousService` | High; selected pure semantics move to ordinary C#. |
| Standard operation planning, presets, cadence, evidence, extensions | Standard internals, developer tooling, TS authoring/contracts, fixtures | No direct managed planner/preset/cadence API | High; preserve only named managed concepts, not the legacy coordinator/vocabulary. |
| TS Rules/Standard authoring | Rules workspace, Rust-driven contract generators, product-conformance fixtures, materializer/CLI output | None at runtime | High; abandon the lane and remove its closure. |
| Product Model manifest/composition/catalog | input, mutation, schedule, timeline, UI, Kernel, assembly/materializer, dev host, CLI, facade, C# runtime type imports | No named service; some creation/update types still derive from its vocabulary | High; abandon after neutral extraction for retained Rust mechanisms. |
| Product Kernel/Runtime Composition | legacy generated assembly source, runtime lanes, facade | None in direct C# product root | High; abandon after retained mechanisms are separated. |
| QuickJS VM | Rust facade and separate VM/assembly fixtures or commands | None | High; abandon. Future developer arbitrary-code work is a separate capability. |
| Product materializer/assembly/CLI | Each other, Product Model, dev host/content tools, generated source and CI paths | None as runtime SDK | High; abandon legacy product paths while preserving unrelated current CLI/content tools. |
| Product development host | C# product runtime models, renderer/host, input/timeline/UI | Host integration, not `IEngineContext` gameplay service | High; retain after neutral extractions. |
| Exact Mechanics | C# services, Standard, developer command, inspector, facade, managed definitions/entity/persistence helpers | Generated `IMechanicsService` plus handwritten helpers | High; selected Engine mechanisms move to managed C#, damage/restoration does not. |
| Continuous Mechanics | C# services, inspector, combined native registry, facade, managed entity/composed persistence helpers | Generated `IContinuousMechanicsService` plus helpers | High; move selected semantics to managed C# and retire the native authority. |
| Structural resolution | C# services, Standard, developer command/inspector, facade | Generated `IResolutionService` and `StructuralResolutionSession` | High; direct ordinary C# replaces the native coordinator. |
| State machines | facade, ABI/service bridge, managed entity adapter, NativeAOT fixture | Generated `IStateMachineService` | High; managed C# is the sole product instance owner. |
| RNG | ABI/service bridge, facade, NativeAOT fixture and downstream products | Generated `IRandomService` with scoped owners | High; retain Rust, defer continuation persistence to an identified need. |
| Native `EntityState` | Mechanics, Continuous Mechanics, spatial, render projection, content store, runtime standard capability, tools | No generic managed world; exposed indirectly through named services | High; retain only per-native-mechanism state after mechanics/continuous consumers migrate. |
| Managed `EntityWorld` and adapters | Engine helper projects/examples; no current Dagger/Space/CraftSurvive helper-project reference | Four separate helper assemblies today | High on shape; keep an optional one-assembly helper and retain only generic Engine-crossing adapters. Downstream adoption remains voluntary. |
| Runtime lifecycle | C# product runtime, dev host, Kernel and all runtime lanes | Generated `IEngineProduct` lifecycle/update facts | High; retained Rust mechanism. |
| Managed update pipeline/scheduler | Application helper/example only | Optional `Rusty.Engine.Application` namespace in the default assembly | High on target shape; add optional coroutine-like continuations only in follow-on work. |
| Runtime schedule/mutation | Product Model/Kernel/Composition and standard capability paths, facade | None | High; abandon Product Model owners, retain only named native mutation authority required by surviving mechanisms. |
| Runtime timeline | C# product runtime, dev host, Runtime Composition, facade | `IEngineProduct.CompleteTimeline`; no general schedule/cancel API | High; retain the mechanism and grow the managed API only for identified callers. |
| Runtime input | C# runtime, dev host, Runtime Composition, facade | Copied product creation/update configuration and events | High; neutral Product Model type extraction required. |
| Runtime UI | C# UI bridge, dev host, Runtime Composition, facade | Generated `IUiService` | High; neutral identity/JSON extraction required. |
| Persistence composition | native persistence/content owners plus managed product/entity/mechanics stores | Generated `IPersistenceService` and separate helper assembly | High; preserve product codecs and retained-native adapters, retire native mechanics composition after the managed move. |
| Annotations/materials/scenes/content | asset/content/scene/voxel/spatial/render/Studio/tooling owners | Generated AuthoredContent, Content, ContentStore, Voxel, and related service families cover subsets | Medium; retain Rust state/admission and expose typed C# proposal/read surfaces. |
| Environment authoring | authored scene/content/voxel/annotation owners, Studio owner map, tools/fixtures | No dedicated managed generator service | Medium; retain Rust generation/admission, add typed managed requests only as workflows require. |
| Spatial/navigation/physics/world origin | spatial/service crates, render/content consumers, multiple downstream products | Generated named Spatial, Motion, Kinematic, Dynamics, Character, Voxel, and WorldOrigin families | High; native mechanisms retained, managed adapter breadth remains per-helper. |
| Animation/playback | voxel-object runtime, render presentation/projection, content/tooling | Generated Animation/Presentation and voxel presentation subsets | Medium; retain Rust playback state/mechanism with typed C# proposal/read APIs. |
| Developer/inspector surfaces | gameplay providers, CLI/host wire schemas, tooling | Mostly Rust/CLI; selected readouts cross named services | Medium; preserve debugger, observation, and trusted mutation outcomes rather than the legacy provider shape. |

## Compile-time removal closure

No surveyed legacy family is currently an orphan. A coherent removal may need
all of the following updates in one dependency-ordered sequence:

- Cargo dependencies and `rusty-engine` facade reexports;
- `csharp-engine-abi` modules and `NativeEngineApi` fields;
- `csharp-engine-services` bridge storage, construction, and function-table
  publication;
- generated C# context/service/value/bootstrap output and the binding script;
- handwritten managed helpers, entity adapters, paired persistence stores, and
  examples;
- developer-command and inspector projections;
- Rules workspace contracts/authoring packages and Rust-driven generators;
- Product Model/materializer/assembly/CLI-generated source;
- fixtures, cross-language convergence vectors, and workflow path gates; and
- downstream consumers after their semantic owner has moved.

The ABI and generator sources are authoritative; ignored generated files and
fixture calls are regenerated/proof closure, not separate architecture.

## Removal gates

A legacy owner can be removed only when all applicable gates are explicit:

1. every capability it owns has a disposition;
2. preserved semantics have a named replacement owner and target API shape;
3. every production, tooling, generated, fixture, and downstream dependency is
   migrated, intentionally retired, or accepted as a deliberate break;
4. neutral primitives required by retained Rust infrastructure are extracted
   from Product Model or other legacy owners;
5. generated ABI tables, bridge construction, facade reexports, build scripts,
   workspaces, and ignored generated artifacts agree with the new ownership;
6. documentation stops advertising the removed lane;
7. abandonment decisions cite owner approval rather than agent inference; and
8. focused compile or direct-consumer evidence is chosen for the actual changed
   boundary. Old provider-wide, browser, security, packaging, or conformance
   gates do not become requirements by inertia.

## Owner decisions and remaining gates

These decisions were recorded on 2026-08-28. They govern migration planning;
they do not authorize deleting a legacy owner before its dependency closure and
replacement surface are ready.

1. **Rules and TypeScript authoring: abandon.** Retire the Rules package,
   provenance, Product Model, and TypeScript authoring lane after dependents
   move. A possible future arbitrary-code development console is a separate
   tooling capability, not a reason to keep QuickJS or the authored JSON path.
2. **Mechanics: keep reusable Engine concepts, move them to managed C#.** Stats,
   tracks, modifier/source attribution, effects, inventory, equipment, and
   their useful atomicity are Engine SDK candidates. They are not required to
   remain Rust. Damage and healing are downstream meanings built from generic
   track/stat/effect mechanisms, not Engine concepts.
3. **Continuous mechanics: managed C#.** Do not retain a second native
   component authority. Carry over only useful finite-value and dependent-track
   behavior.
4. **Exact/continuous evaluators: ordinary C#.** Retire their native service
   boundary and serialized authoring shape.
5. **Structural resolution: direct C#.** Interfaces, virtual methods, and
   domain methods replace the native coordinator. Selected bounds, receipts,
   or transaction-separation ideas may survive as ordinary helpers.
6. **Managed `EntityWorld`: retain as an optional SDK default.** Its use cases
   are typed product-owned component storage without per-access ABI crossings;
   stable entity identity/lifecycle/containment; deterministic joins; revision
   guards; atomic batches; and product-composed snapshot/restore. Upstream
   adapters are justified only when they standardize a reusable Engine service
   crossing or cross-owner transaction. Spatial, character, dynamics,
   kinematic/motion, world-origin, and appearance adapters are plausible;
   product-specific mappings stay downstream. Mechanics and state-machine
   adapters should shrink or retire as those authorities move to C#.
7. **State machines: managed C#.** Retire the older native
   `StateMachineStore` and validator after callers migrate.
8. **RNG continuation: consumer-driven.** Rust RNG remains useful, but stream
   save/restore is not a general migration requirement. Add it when a concrete
   save/replay consumer needs exact continuation.
9. **Timeline API growth: consumer-driven.** Add public C# schedule, cancel, or
   read operations only for identified callers and use cases.
10. **Legacy product runtime lane: abandon.** Product Model schedule/mutation,
    Product Kernel, Runtime Composition, QuickJS VM, materializer, and Assembly
    are not supported product variants. The managed SDK does need optional
    coroutine-like logic driven by Engine-admitted steps. Extend the existing
    C# scheduler with simple continuations rather than preserving the five-phase
    Product Model scheduler.
11. **`observe-pairs`: retain the mechanism under discoverable sensing names.**
    Its reusable core serves stealth visibility, AI target acquisition, sentry
    vision, and per-target awareness/threat accumulation by combining distance,
    facing, occlusion, and deterministic aggregation. Rust owns the spatial
    query/reduction. The managed SDK exposes direct `Perception`/`Visibility`
    utility methods for custom implementations and optional default vision and
    perceivable-target adapters over `EntityWorld`; it does not own AI policy or
    automatically mutate AI state. Remove the Product Model schedule/mutation
    wrapper and the `observe-pairs` public vocabulary.
12. **Content and playback mechanisms: retain Rust state.** Voxel annotations,
    scene/prefab/content admission, and animation playback/clip state may remain
    Rust Engine mechanisms. C# proposes typed mutations, reads state, and owns
    application meaning and orchestration.
13. **Developer tooling: preserve outcomes, not legacy providers.** Required
    outcomes are a practical debugger path for the real product, runtime state
    observation, and trusted direct mutation/override for testing. Preview,
    admin, checkpoint, fault isolation, or overlays survive only where they
    help those outcomes. [.NET NativeAOT diagnostics](https://learn.microsoft.com/dotnet/core/deploying/native-aot/diagnostics)
    support ordinary managed debugging during non-AOT development builds and
    native source debugging of published code when symbols are retained; the
    [deployment contract](https://learn.microsoft.com/dotnet/core/deploying/native-aot/)
    does not support runtime code generation or dynamic assembly loading. The
    current Rust-hosted shared-library path has not yet demonstrated a normal
    managed-debugger workflow. A focused implementation task must prove that
    workflow instead of assuming the old command architecture is its
    replacement.

No owner question above blocks #7508 synthesis. Items 8 and 9 are explicitly
deferred to identified consumers; item 13 defines a result to prove rather
than prescribing its implementation.

### Public lifecycle terminology follow-up

`ProductUpdateMode` names the admission mode (`Realtime`, `Demand`, or
`External`), and `ProductUpdateResult` names the explicit post-`Update` outcome
(`None` or `ReportFault`). The runtime intentionally commits staged Engine
service work before applying `ReportFault`, whereas an exception makes the
callback fail and discards the staged call. Do not convert this distinction into
exception control flow merely to obtain a `void Update` signature.

The broader `Product` prefix is a separate readability question. Audit only the
public managed lifecycle/input/content surface and downstream call sites before
renaming it; do not commission a repository-wide terminology rewrite without a
clear replacement vocabulary and migration benefit.

## Implementation dependency order

Task #7509 should turn these phases into bounded tasks without collapsing the
whole migration into one proof campaign:

1. **Consolidate the managed foundation.** Fold accepted Application,
   `EntityWorld`, persistence, and helper sources into the one `Rusty.Engine`
   assembly. Add optional admitted-step coroutine-like continuations. Keep the
   build-time generators separate.
2. **Build managed gameplay mechanisms beside the native owners.** Adapt
   exact/continuous values, stats, tracks, sources/modifiers, effects,
   inventory/equipment, selected resolution helpers, and state machines into
   ordinary C#. Do not port damage/restoration, generic Standard operation
   grammar, serialized expression trees, or package registries.
3. **Move consumers before owners.** Migrate managed adapters, product
   persistence, downstream products, inspector/developer surfaces, examples,
   and fixtures to the selected managed mechanisms. Shrink or remove mechanics,
   continuous-mechanics, resolution, and state-machine ABI families only after
   their callers move.
4. **Neutralize retained Rust runtime mechanisms.** Extract identity, bounded
   value, input, timeline, UI, lifecycle, and dev-host primitives still coupled
   to Product Model. Preserve their named Engine behavior without the Product
    Kernel/Runtime Composition root. Extract `observe-pairs` into the retained
    spatial sensing mechanism and publish discoverable managed
    `Perception`/`Visibility` utilities and optional EntityWorld adapters.
5. **Remove the abandoned legacy closure.** Delete Rules/Standard artifact and
   TypeScript authoring packages, generators, Product Model schedule/mutation,
   Product Kernel, Runtime Composition, QuickJS VM, materializer, Assembly, and
   their obsolete ABI/services/facade/CLI/fixture/workflow edges in dependency
   order. Preserve unrelated current CLI/content tooling.
6. **Finish publication and documentation cleanup.** Regenerate the C#/C ABI
   surface, remove obsolete helper projects and examples, update downstream
   references, and replace this migration plan with durable architecture/API
   documentation when the implementation has landed.

Standalone task #7511 owns the C# debugger, precompiled-command console, and
trusted live inspection/mutation plan. It is intentionally outside campaign
#7502 and must not block unrelated managed migration or preserve the legacy
developer-command architecture.

Each phase uses focused compilation or a direct consumer exercise for the
changed boundary. Legacy provider-wide gates do not become prerequisites for
deletion merely because they once existed.
