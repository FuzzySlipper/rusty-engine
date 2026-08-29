# Managed C# capability migration

Status: active survey and planning record for Den campaign #7502. This file is
not authorization to delete, port, or deprecate a capability. Tasks #7503–#7507
populate the inventory; task #7508 owns synthesis and owner decisions; task
#7509 may create implementation work only after those decisions.

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

Each final inventory entry synthesized under #7508 must state:

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

The survey tables below record semantics, evidence, tentative disposition, and
local gates. The keyed dependency/exposure matrix supplies the cross-cutting
consumer, managed-surface, confidence, and owner-question fields that would be
unreadable if repeated inside every prose cell. Task #7508 must join both views
before treating any row as final.

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

The entries in this section are provisional until task #7508 reconciles the
parallel surveys. `owner-decision-needed` is preferable to a confident guess.

Survey baseline: Engine `main` at `391bf25`. Historical Board posts and Git
commits explain why code was written; most Board posts are agent-authored
proposals, not owner instructions. Current owner direction, current source, and
the #7502 task contract take precedence.

### Rules, Standard, Product Model, and TypeScript

| Capability and evidence | Current semantics and boundary | Provisional disposition, target, and gates |
| --- | --- | --- |
| Rules artifact envelope, identity, canonical JSON, provenance, and diagnostics ([`gameplay-rules`](../rust/crates/gameplay-rules/src/lib.rs)) | Admits bounded immutable packages, canonicalizes bytes, fingerprints content, and correlates sources. It never evaluates gameplay meaning. The generated `Rules` service exposes this artifact protocol directly. | `abandon` as the ordinary C# product-definition path; `owner-decision-needed` for a separately supported artifact/mod/TS lane. Do not port the JSON envelope, compatibility ceremony, or package registry into C#. Removal requires resolving inspector, Standard, ABI, fixture, generator, and CLI consumers. |
| Rule package-set dependency resolution ([`resolve.rs`](../rust/crates/gameplay-rules/src/resolve.rs)) | Deterministic bounded topological ordering, duplicate/version/fingerprint checks, and cycle diagnostics. Useful only when rule packages remain a real product artifact. | `defer` behind the artifact-lane decision. If retained, keep it as a typed Rust packaging mechanism, not a global gameplay catalog. |
| Rule payload subtree selection ([`selection.rs`](../rust/crates/gameplay-rules/src/selection.rs)) | Copies a canonical, fingerprint-fenced subtree for inspection/tooling; it is not product rule evaluation. | `adapt-managed` only for a named tooling/content need, otherwise `abandon` with the package lane. Do not replace it with generic JSON-pointer gameplay access. |
| Exact expression/value evaluation ([`exact.rs`](../rust/crates/gameplay-standard/src/exact.rs)) | Pure bounded integer expression compilation/evaluation over `MechanicsScalar`, explicit input/roll evidence, checked arithmetic, comparisons, and work receipts. No host or shared-state requirement. | `adapt-managed` or `retain-rust`; #7508 must decide whether cross-product sharing/performance justifies the ABI. A C# form should be ordinary typed values/functions, not serialized trees as its authoring model. Preserve overflow, rounding, quotas, duplicate-evidence rejection, and vectors. |
| Continuous expression/value evaluation ([`continuous.rs`](../rust/crates/gameplay-standard/src/continuous.rs)) | Pure finite-binary64 values, bit identity, explicit input bundles, bounded expressions/comparisons, and work receipts. | `owner-decision-needed`, coupled to the continuous-mechanics decision. Never collapse exact and continuous domains into one universal `Number` abstraction. |
| Standard definition/package admission, composed leaves, and extension artifacts ([`package.rs`](../rust/crates/gameplay-standard/src/package.rs), [`composed.rs`](../rust/crates/gameplay-standard/src/composed.rs), [`extension.rs`](../rust/crates/gameplay-standard/src/extension.rs)) | Wraps typed definitions and caller-compiled product leaves in Rules provenance/schema machinery. This was primarily a build-time authoring seam. | `abandon` as mainline C# infrastructure; `defer` only for a named external artifact compiler. Preserve selected typed semantics in product C# without porting generic leaf registries, schema dispatch, or opaque runtime JSON. |
| Standard mechanics operation planning and generic role binding ([`resolution.rs`](../rust/crates/gameplay-standard/src/resolution.rs)) | Binds actor/target-style roles, evaluates operands, plans track/damage/effect/inventory/equipment requests, and validates read sets. It composes existing mechanisms but has no native/host requirement. | `adapt-managed` for any useful product coordinator/read-set concepts; `abandon` the universal operation enum/role grammar unless a concrete product proves it. Direct domain C# calling named mechanisms is the baseline. |
| Quantization, residual carry, cadence, and exact deadlines ([`quantization.rs`](../rust/crates/gameplay-standard/src/quantization.rs), [`cadence.rs`](../rust/crates/gameplay-standard/src/cadence.rs)) | Pure conversion policies and caller-owned continuation records. It owns no clock, loop, resource, component, cap, or save format. Historical experiments found partition drift and deliberately left cadence ownership downstream. | `adapt-managed` for small conversion helpers; `defer` residual/cadence continuation until a product-neutral persisted resource exists. Preserve rounding/provenance/policy-version fields if adopted. Do not create an Engine cadence loop. |
| Bounded evidence/sample plans ([`bounded_evidence.rs`](../rust/crates/gameplay-standard/src/bounded_evidence.rs)) | Validates named caller-supplied sample ranges and receipts; it does not generate randomness or assign product meaning. | `owner-decision-needed`. Keep only if it proves to be a reusable evidence contract; otherwise ordinary product C# validation is clearer. |
| Standard presets and fixed vocabulary ([`presets.rs`](../rust/crates/gameplay-standard/src/presets.rs)) | Emits inert actor/vitality/resource and destructible/integrity/impact catalog fragments. Names/defaults are product policy. | `abandon` as Engine vocabulary. Preserve only as donor/sample code moved into a product namespace. |
| Standard projections, receipts, and inspection ([`projection.rs`](../rust/crates/gameplay-standard/src/projection.rs), [`engine-inspector`](../rust/crates/engine-inspector)) | Read-only evidence over packages, evaluation, mechanics, and resolution. The developer console work also planned preview/play/admin/provenance lanes. | `retain-rust` for mechanism inspection where the mechanism remains; `adapt-managed` typed readouts for C# tooling. Do not retain an obsolete evaluator only to retain its inspector. |
| TypeScript Rules/Standard authoring and contract generation ([`rules/packages`](../rules/packages), [`rules/scripts`](../rules/scripts)) | Build-time immutable drafts, Rust-shaped validation, canonical output, and Rust/TS convergence fixtures. No runtime evaluator. | `abandon` from the main C# product path; `defer` only as an explicitly supported TS artifact producer. C# definitions should be C# code, not a port of the Node serializer or fixture suite. |
| Product Layout manifest and compiled composition ([`product-model`](../rust/crates/product-model/src/lib.rs)) | `rusty.toml`, input maps, five schedule phases, opaque definitions/timelines, capability links, quotas, and static artifact admission. This was the cancelled Product Model composition root. | `abandon` as C# application architecture. Extract neutral input, identity, JSON-limit, schedule, and timeline primitives that retained Rust lanes still import. `owner-decision-needed` for a separately supported Rust/TS product lane. |
| Product Kernel and Runtime Composition ([`product-kernel`](../rust/crates/product-kernel/src/lib.rs), [`runtime-composition`](../rust/crates/runtime-composition/src/lib.rs)) | Static source-linked owners and a five-lane lifecycle/input/schedule/timeline/mutation/UI root. Even without a runtime registry, it duplicates the direct C# product root. | `abandon` from the C# path. Preserve useful phase/token and named mechanism concepts individually; do not port macros, module registration, generic dispatch, or a `ProductApplication` framework. |
| Runtime VM ([`runtime-vm`](../rust/crates/runtime-vm/src/lib.rs)) | Bounded fresh QuickJS realm with fixed exports and atomic revisioned JSON state. It is a distinct TS product option, not needed by NativeAOT C#. | `defer` and request an owner decision on separate long-horizon TS support. Never port it to C# or keep it accidentally through the facade alone. |
| Product materializer, assembly, and Product-Model CLI paths ([`product-materializer`](../rust/crates/product-materializer/src/lib.rs), [`product-assembly`](../rust/crates/product-assembly/src/lib.rs), [`rusty-cli`](../rust/crates/rusty-cli)) | Build/package tools for TS/Rust products, generated Rust assembly source, product layouts, and contract outputs. | `defer` only while the legacy/separate product lane is supported. C# should use normal .NET build/content paths plus the generated NativeAOT bootstrap; do not port Node/materialization ceremony into the SDK. |
| Product development host ([`product-dev-host`](../rust/crates/product-dev-host/src/lib.rs)) | Loopback HTTP/SSE, serialized runtime calls, lifecycle/input/timeline/UI host models. It owns host integration, not gameplay. | `retain-rust`; remove Product Model type contamination from the retained runtime lanes. Do not move browser/host machinery into product C#. |

### Mechanics, resolution, state, and numeric helpers

| Capability and evidence | Current semantics and boundary | Provisional disposition, target, and gates |
| --- | --- | --- |
| Mechanics IDs, exact scalar/ratio arithmetic, and immutable catalog ([`gameplay-mechanics`](../rust/crates/gameplay-mechanics/src/lib.rs), [`scalar.rs`](../rust/crates/gameplay-mechanics/src/scalar.rs), [`catalog.rs`](../rust/crates/gameplay-mechanics/src/catalog.rs)) | Checked ±1e12 integers, normalized ratios, canonical typed IDs, product-authored definitions, catalog validation/version/fingerprint. Pure admission/arithmetic feeds stateful services. | `adapt-managed` for product definitions and C#-natural value builders; `retain-rust` only for contracts shared by retained native mechanics. Preserve arithmetic vectors and catalog identity. Do not create a universal registry/modifier framework. |
| Seven exact mechanics component families and codecs ([`component.rs`](../rust/crates/gameplay-mechanics/src/component.rs)) | Stats, tracks, intrinsic sources, effects, inventory, item, and equipment live in the shared native `EntityState`, with revisions, catalog versions, codecs, and exact replacement. | `retain-rust` where native mechanics owns the state; `adapt-managed` copied facts/adapters. Do not mirror the same mechanics state as an independent C# authority. Removal requires an explicit replacement for revisions, codecs, containment, and atomic publication. |
| Modifier/source attribution and collection ([`source.rs`](../rust/crates/gameplay-mechanics/src/source.rs)) | Expands intrinsic/effect/equipment/request sources; validates provenance and revisions; orders deterministically; reports applied/suppressed/inapplicable decisions. | `retain-rust` with native mechanics; product C# owns definitions and interpretation. Preserve provenance/order/cost bounds. Do not add callback registries or ambient source lookup. |
| Stats and tracks ([`stat.rs`](../rust/crates/gameplay-mechanics/src/stat.rs), [`track.rs`](../rust/crates/gameplay-mechanics/src/track.rs)) | Modifier evaluation, checked scale/add/clamp, prospective no-stranding validation, read/spend/restore/set/reconcile, revision-guarded exact publication. Product decides what a value means and when actions are legal. | `retain-rust` provisionally for shared stateful invariants; `adapt-managed` domain methods and definitions. A managed replacement would need exact ordering, rounding, stale, and no-stranding equivalence before native removal. |
| Effects ([`effect.rs`](../rust/crates/gameplay-mechanics/src/effect.rs)) | Provenance-aware independent/refresh/replace stacking, prospective track validation, and source activation. No duration, timer, callback, or scheduler. | `retain-rust` for lifecycle/invariants if exact mechanics remains; product C# owns duration, timing, meaning, and consequences. Do not port an effect bus or scheduler. |
| Damage and restoration ([`damage.rs`](../rust/crates/gameplay-mechanics/src/damage.rs)) | Preview/apply pipeline of prevention, flat reduction, one rounded scale, ordered absorption, and target-track application with bounded multipart receipts and one exact publication. It owns no attack, hit, target, reaction, death, or score semantics. | `retain-rust` provisionally as a reusable deterministic stateful mechanism; C# owns combat admission and consequences. Preserve stage order and late-failure atomicity. |
| Inventory, unique items, containment, equipment, and capacity ([`item.rs`](../rust/crates/gameplay-mechanics/src/item.rs)) | Canonical fungible stacks, entity-backed unique items, maintained containment, typed capacity costs, multislot/exclusivity rules, source activation, explicit unequip/transfer/destruction policy. | `retain-rust` for native containment/capacity/equipment invariants; `adapt-managed` definitions and product policy. Removal must replace maintained child indexes and cross-component atomicity, not merely copy records into C#. |
| Mechanics views, snapshots, imports, and receipts ([`snapshot.rs`](../rust/crates/gameplay-mechanics/src/snapshot.rs), [`view.rs`](../rust/crates/gameplay-mechanics/src/view.rs)) | Strict registry/catalog-aware mechanism snapshots and bounded readouts; not a complete product save format. | `retain-rust` with the native mechanism; `adapt-managed` outer save schema through product codecs. Preserve catalog identity, containment, and fresh revision remapping. |
| Continuous mechanics ([`gameplay-continuous-mechanics`](../rust/crates/gameplay-continuous-mechanics/src/lib.rs)) | Separate finite-binary64 stats/tracks/sources/effects catalog and four durable component families sharing the exact entity binding. No continuous damage/inventory/equipment/cadence. | `owner-decision-needed`. Either keep a deliberate second native family or move continuous product values to C#; never keep both as unexamined duplicate authorities. Preserve bit identity and dependent-track validation if retained. |
| Structural resolution lifecycle ([`gameplay-resolution`](../rust/crates/gameplay-resolution/src/lib.rs), [`structural.rs`](../rust/crates/gameplay-resolution/src/structural.rs)) | Bounded Admit/Gather/Check/Plan/BeforeCommit/Commit/Consequences traversal, correlation, child budgets, preview/apply, traces, and transaction finalization; product owns all meaning and state. | `retain-rust` or `adapt-managed`. The current managed `StructuralResolutionSession` is a thin optional wrapper, but native per-node ABI calls may be ceremony C# no longer needs. Preserve structural limits and transaction separation if ported. |
| Generic Rust policy/program/resolver and Standard operation enum ([`policy.rs`](../rust/crates/gameplay-resolution/src/policy.rs), [`resolver.rs`](../rust/crates/gameplay-resolution/src/resolver.rs)) | Generic traits/program nodes and staged downstream effects; Standard adds a mechanics-shaped operation grammar. These primarily compensate for static Rust composition and are not exposed as the C# product API. | `abandon` as a mandatory Engine framework; `adapt-managed` only selected read-set/receipt concepts. Ordinary virtual methods, interfaces, resolvers, and domain methods are the C#-natural lane. |
| State-machine transition validation and stores ([`state-machine`](../rust/crates/state-machine/src/lib.rs)) | Bounded graph admission, guarded transitions, and revisions. Current C# stores instances in managed `EntityWorld`; the ABI retains definitions and validates detached transitions, while older `StateMachineStore` can retain native instances. | `adapt-managed` for ordinary product machine instances; `retain-rust` only for a shared detached validator if useful. `owner-decision-needed` for the old native store. Choose exactly one instance owner per domain. |
| Deterministic RNG ([`svc-rng`](../rust/crates/svc-rng/src/lib.rs)) | Seeded stateless keyed draws and mutable scoped streams behind native handles; no ambient entropy, time, or global state. | `retain-rust` for shared authoritative randomness and scoped streams. Define save/restart continuation before claiming streams are durable. Preserve versioned vectors, framing, unbiased mapping, and fork/counter semantics. |
| Standard bounded roles, evidence, extensions, and presets | Pure validation and authoring concepts around the families above. | Preserve only named useful concepts in C# product code. Do not port generic registries, opaque operation dispatch, TypeScript callbacks, or fixed actor/vitality vocabulary. |

### Entity/component, update, timeline, and persistence composition

| Capability and evidence | Current semantics and boundary | Provisional disposition, target, and gates |
| --- | --- | --- |
| Native entity/component state ([`entity-state`](../rust/crates/entity-state/src/lib.rs)) | Typed component registry/stores, lifecycle, revisions, atomic commands, transforms, containment/relationships, projections, and mechanism snapshots. It has no Product Model dependency and is used by mechanics, spatial, rendering, content, and tooling. | `retain-rust` for mechanism-owned native state. Do not wholesale-port its layout or snapshot schema to C#. Every retained native consumer must identify the subset it owns; ordinary product facts remain C# authority. |
| Managed product entity world ([`EntityWorld.cs`](../csharp/Rusty.Engine.Entities/EntityWorld.cs), [`ComponentType.cs`](../csharp/Rusty.Engine.Entities/ComponentType.cs)) | Product-owned typed storage with stable IDs, lifecycle, containment, revisions, deterministic queries, batches, snapshots, and restore candidates. It deliberately does not mirror Rust `EntityState`. | `adapt-managed`; merge into the one assembly under `Rusty.Engine.Entities` if retained. Keep it optional and ordinary C#, not a mandatory ECS/framework. Clarify whether its reserved Engine component range and broad clone-on-batch behavior are still useful. |
| Named managed/native entity adapters ([`Rusty.Engine.Entities`](../csharp/Rusty.Engine.Entities)) | Explicit projections for mechanics, dynamics, kinematic/motion, spatial, character, world origin, appearance, and state machines. They use copied facts, guard rechecks, and one managed publication rather than a registry. | `adapt-managed` per adapter while its Rust mechanism remains. Keep only adapters that simplify a real multi-entity/cross-owner transaction; direct service calls remain valid. Never infer a hidden binding or second durable world. |
| Runtime lifecycle ([`runtime-lifecycle`](../rust/crates/runtime-lifecycle/src/lib.rs)) | Product-Model-free lifecycle state, generations/control revisions, realtime/demand/external admission, simulation/presentation facts, pause/restart/fault/shutdown, and typed phase tokens. | `retain-rust`. C# consumes copied `ProductUpdate` facts and owns product orchestration; it must not create another host clock or central Engine loop. |
| Managed update pipeline and scheduler ([`Rusty.Engine.Application`](../csharp/Rusty.Engine.Application)) | Optional named callback phases and deterministic due-callback ordering driven only by admitted update facts; no clock, host loop, native state, persistence, or rollback. | `adapt-managed`; fold into the one assembly/namespace as an optional convenience. It must remain realtime-neutral and ignorable, not a hidden Product Model scheduler. |
| Product Model runtime schedule ([`runtime-schedule`](../rust/crates/runtime-schedule/src/lib.rs)) | Compiles a closed five-phase authored schedule and validates dependency/access/cadence before caller-supplied dispatch. It stores no callbacks or clock but is tightly coupled to Product Model types. | `owner-decision-needed`; current C# baseline rejects an upstream gameplay scheduler. Extract a narrow reusable ordering/token primitive only if a real consumer needs it. Do not port the five-phase DSL or generic dispatcher. |
| Runtime timeline and completion tickets ([`runtime-timeline`](../rust/crates/runtime-timeline/src/lib.rs)) | Bounded step queue, deterministic release order, finite recurrence, issue-ordered completion tickets, lifecycle fencing, snapshot/restore of mechanism state, and C# completion callback. It owns no clock, executor, callback, or product meaning. | `retain-rust` provisionally as a reusable mechanism; extract Product Model identity/opaque-data/template coupling. Add named C# schedule/cancel/read surfaces only if products need them. Do not port opaque Product Model templates as the product API. |
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
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine.Application/Rusty.Engine.Application.csproj) | Small optional update/scheduler convention, 385 handwritten lines. | Fold sources into the runtime assembly under `Rusty.Engine.Application`; keep the convention optional. |
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
| Voxel annotations and material authority ([`voxel-annotation`](../rust/crates/voxel-annotation), [`asset-catalog`](../rust/crates/asset-catalog)) | Rust retains canonical content/material admission and collision/occlusion/structural facts. Product C# owns the gameplay meaning of spawn areas, cover, hazards, and navigation hints. `owner-decision-needed` on authoring/API shape. |
| Authored scenes, prefabs, and content load plans ([`authored-scene`](../rust/crates/authored-scene), [`content-store`](../rust/crates/content-store)) | Rust retains bounded atomic admission, artifact roles, load ordering, resources, and publication. C# owns which entity/prefab/content facts mean what. Do not collapse this into generic entity persistence. |
| Environment authoring and seeded materialization ([`environment-authoring`](../rust/crates/environment-authoring), [`Studio owner map`](../studio/owner-adoption.tsv)) | Seeded generators materialize voxel/scene facts, annotations/markers, transforms, collision participation, and provenance into Engine-owned authored content. Rust should retain deterministic generation/admission and reusable materialization invariants; product C# owns authored gameplay meaning and selection policy. `owner-decision-needed` on whether generator definitions live in product C#, Engine content tools, or both through a typed seam. Survey Studio, CLI, authored-scene, asset/content, and fixture consumers before removal or relocation. |
| Spatial, collision, navigation, character, voxel residency, and world origin ([`engine-spatial`](../rust/crates/engine-spatial), [`svc-pathfinding`](../rust/crates/svc-pathfinding)) | `retain-rust` native indexes, queries, leases, revisions, and high-frequency mechanisms; `adapt-managed` product movement policy, goals, and copied facts. These are not candidates for a downstream replacement system. |
| Animation and explicit-time voxel playback ([`voxel-object-runtime`](../rust/crates/voxel-object-runtime), [`render-presentation`](../rust/crates/render-presentation)) | Rust retains renderer/resource realization and reusable sampling. `owner-decision-needed` on whether clip/loop/scrub state is product gameplay state or presentation-only state. Collision selection must remain explicit rather than follow visual frames accidentally. |
| Runtime `observe-pairs` ([`runtime-standard-capabilities`](../rust/crates/runtime-standard-capabilities)) | Likely split into retained Rust spatial sensing/reduction plus C# product policy/operation meaning. Do not retain its Product Model schedule/mutation coupling just to keep the useful query. |
| Input, time, and RNG ([`runtime-input`](../rust/crates/runtime-input), [`core-time`](../rust/crates/core-time), [`svc-rng`](../rust/crates/svc-rng)) | Retain normalized host facts and deterministic services in Rust. Keep host time, admitted simulation steps, animation timestamps, and product scheduler time distinct. |
| Rendering, audio, UI host/backend, assets/resources, voxel mechanisms, core math/space/IDs | Explicitly excluded from gameplay-language migration except where a mixed file embeds product semantics. These remain durable Engine mechanisms and generated service families. |
| Developer command, inspector, CLI, dev host, Studio, and renderer TypeScript | Tool/host/editor lanes, not C# gameplay owners. Survey their dependency closures before deleting a legacy provider, but do not port them merely for language uniformity. |

## Dependency and managed-exposure matrix

This matrix keys the current closure back to the ledger. “None” means no direct
managed API, not that the capability is unused or valueless.

| Capability key | Current production/tooling dependents | Current managed exposure | Confidence and decision gate |
| --- | --- | --- | --- |
| Rules envelope/set/selection | `csharp-engine-services`, Standard, inspector, facade, contract generator, fixture | Generated `IRulesService` and leases/readouts | High; owner question 1 decides whether the artifact lane remains. |
| Standard exact/continuous evaluation and admission | `csharp-engine-services`, Mechanics/Resolution/Rules via Standard, Continuous Mechanics, inspector/developer command, facade | Generated `IStandardExactService` and `IStandardContinuousService` | High on closure; questions 3–4 decide managed/native shape. |
| Standard operation planning, presets, cadence, evidence, extensions | Standard internals, developer tooling, TS authoring/contracts, fixtures | No direct managed planner/preset/cadence API | High on source shape, medium on future value; questions 3–5 and 13. |
| TS Rules/Standard authoring | Rules workspace, Rust-driven contract generators, product-conformance fixtures, materializer/CLI output | None at runtime | High; questions 1 and 10. |
| Product Model manifest/composition/catalog | input, mutation, schedule, timeline, UI, Kernel, assembly/materializer, dev host, CLI, facade, C# runtime type imports | No named service; some creation/update types still derive from its vocabulary | High; question 10 and neutral-extraction gates. |
| Product Kernel/Runtime Composition | legacy generated assembly source, runtime lanes, facade | None in direct C# product root | High; question 10. |
| QuickJS VM | Rust facade and separate VM/assembly fixtures or commands | None | High on isolation, low on future product intent; question 10. |
| Product materializer/assembly/CLI | Each other, Product Model, dev host/content tools, generated source and CI paths | None as runtime SDK | High; question 10. |
| Product development host | C# product runtime models, renderer/host, input/timeline/UI | Host integration, not `IEngineContext` gameplay service | High; retain after neutral extractions. |
| Exact Mechanics | C# services, Standard, developer command, inspector, facade, managed definitions/entity/persistence helpers | Generated `IMechanicsService` plus handwritten helpers | High; question 2. |
| Continuous Mechanics | C# services, inspector, combined native registry, facade, managed entity/composed persistence helpers | Generated `IContinuousMechanicsService` plus helpers | High; question 3. |
| Structural resolution | C# services, Standard, developer command/inspector, facade | Generated `IResolutionService` and `StructuralResolutionSession` | High; question 5. |
| State machines | facade, ABI/service bridge, managed entity adapter, NativeAOT fixture | Generated `IStateMachineService` | High; question 7 chooses the instance owner. |
| RNG | ABI/service bridge, facade, NativeAOT fixture and downstream products | Generated `IRandomService` with scoped owners | High; question 8 covers continuation. |
| Native `EntityState` | Mechanics, Continuous Mechanics, spatial, render projection, content store, runtime standard capability, tools | No generic managed world; exposed indirectly through named services | High; question 6 and per-native-consumer ownership audit. |
| Managed `EntityWorld` and adapters | Engine helper projects/examples; no current Dagger/Space/CraftSurvive helper-project reference | Four separate helper assemblies today | High on current use, medium on desired SDK breadth; question 6. |
| Runtime lifecycle | C# product runtime, dev host, Kernel and all runtime lanes | Generated `IEngineProduct` lifecycle/update facts | High; retained Rust mechanism. |
| Managed update pipeline/scheduler | Application helper/example only | Separate `Rusty.Engine.Application` assembly | High on shape, medium on future adoption; merge namespace only after #7508. |
| Runtime schedule/mutation | Product Model/Kernel/Composition and standard capability paths, facade | None | High; questions 10–11. |
| Runtime timeline | C# product runtime, dev host, Runtime Composition, facade | `IEngineProduct.CompleteTimeline`; no general schedule/cancel API | High; question 9. |
| Runtime input | C# runtime, dev host, Runtime Composition, facade | Copied product creation/update configuration and events | High; neutral Product Model type extraction required. |
| Runtime UI | C# UI bridge, dev host, Runtime Composition, facade | Generated `IUiService` | High; neutral identity/JSON extraction required. |
| Persistence composition | native persistence/content owners plus managed product/entity/mechanics stores | Generated `IPersistenceService` and separate helper assembly | High; per-adapter decision under questions 2, 3, and 6. |
| Annotations/materials/scenes/content | asset/content/scene/voxel/spatial/render/Studio/tooling owners | Generated AuthoredContent, Content, ContentStore, Voxel, and related service families cover subsets | Medium until #7508 splits semantic authoring from admission; question 12. |
| Environment authoring | authored scene/content/voxel/annotation owners, Studio owner map, tools/fixtures | No dedicated managed generator service | Medium; question 12 must choose typed C# authoring versus tool-owned generation. |
| Spatial/navigation/physics/world origin | spatial/service crates, render/content consumers, multiple downstream products | Generated named Spatial, Motion, Kinematic, Dynamics, Character, Voxel, and WorldOrigin families | High; native mechanisms retained, managed adapter breadth remains per-helper. |
| Animation/playback | voxel-object runtime, render presentation/projection, content/tooling | Generated Animation/Presentation and voxel presentation subsets | Medium; question 12 separates product clip state from presentation realization. |
| Developer/inspector surfaces | gameplay providers, CLI/host wire schemas, tooling | Mostly Rust/CLI; selected readouts cross named services | Medium on future scope; question 13. |

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

## Owner decision queue

Questions discovered during survey live here until answered. A question must
name the capability and consequence of each plausible answer; vague requests
for general approval are not useful.

1. Are Rules packages/provenance/mod-style artifacts still a supported product
   feature outside the main C# lane, or may that entire package/TS authoring
   path be retired after dependents move?
2. Which exact mechanics capabilities remain canonical native mechanisms:
   stats/tracks/sources/effects/damage/inventory/equipment, or a smaller set?
3. Is continuous mechanics a durable second component family, or should
   continuous gameplay values live in C# with only selected pure math retained?
4. Do the pure exact/continuous evaluators justify a native call boundary, or
   should their semantics be adapted into ordinary C#?
5. Is structural resolution valuable as a native bounded coordinator, or are
   its useful phase/receipt concepts better expressed directly in C# now that
   virtual methods/interfaces are available?
6. Should managed `EntityWorld` remain an optional SDK storage helper, and if
   so which adapters belong upstream rather than in individual products?
7. Is managed `EntityWorld` the sole owner for product state-machine instances;
   may the older native `StateMachineStore` be retired after caller migration?
8. Must stateful RNG streams survive save/restart, and what exact continuation
   state is durable if they do?
9. Which timeline operations need public C# schedule/cancel/read APIs beyond
   the existing completion callback?
10. Does any supported product still require Product Model runtime schedule,
    mutation, Product Kernel, Runtime Composition, QuickJS VM, materializer, or
    Assembly as a separate Rust/TS lane?
11. Is `observe-pairs` a durable named spatial mechanism once its Product Model
    schedule/mutation wrapper is removed?
12. Who owns the gameplay meaning and authoring shape of voxel annotations,
    scene/prefab semantics, and animation playback/clip state?
13. Which planned developer inspection capabilities—preview/play/admin,
    catalog overlays/epochs, scratch/checkpoint/fault isolation—remain useful
    after their legacy provider surfaces move?

## Planned sequence

1. Establish this ledger and enumerate every relevant owner (#7503).
2. Complete the four independent source/intent surveys (#7504–#7507).
3. Reconcile overlaps, omissions, dispositions, namespace targets, and owner
   questions (#7508).
4. Return unresolved intent decisions to the owner before any destructive plan.
5. Create dependency-ordered implementation tasks only from the accepted map
   (#7509).

Likely implementation ordering will be neutral primitive extraction, managed
foundation, selected capability ports/adaptations, downstream consumer moves,
publication cleanup, and only then legacy deletion. Task #7508 must establish
the exact sequence; this survey does not pre-approve it.
