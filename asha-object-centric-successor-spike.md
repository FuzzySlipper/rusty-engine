# Asha Object-Centric Successor Spine

Status: exploratory architecture; headless phases 0-2 implemented, broader falsification in progress
Working location: `/home/dev/rusty-engine`
Asha donor snapshot inspected: `a431974330589761c9e35fc4f8a55996a1b5ee48`
Decision scope: a possible successor for Asha's runtime/application spine, not a decision to replace the whole engine

## Decision in one sentence

Test a small sibling runtime centered on a host-neutral Rust entity/capability kernel, an object-centric session model, explicit services/systems, typed post-commit events, and thin projection adapters; compare direct Rust services with trusted executable TypeScript as the game-code host, preserve useful Asha features by transplanting bounded lower-level crates, and do not carry forward `GameplayRuntimeHost`, Gameplay Fabric, universal reaction/replay contracts, or the full `RuntimeSession` facade as the new center.

## Executive position

Asha is too large for a coherent clean-room successor, but its most difficult problems are concentrated in its structural spine rather than evenly distributed across all of its feature work. That creates a plausible middle path:

1. Build a deliberately small, separately launchable authority runtime in this repository.
2. Give it a conventional, object-centric center that is familiar from PixelPhantasm and RuleWeaver.
3. Transplant only Asha code that can sit below that center without importing the old center with it.
4. Prove the result with one mechanic in both candidate game-code hosts, a data-only variation, a project-code variation, and one genuinely new reusable engine capability.
5. Make a migration decision only after the spike demonstrates lower change amplification in practice.

This is bolder than another corrective wave inside Asha, but much smaller than rewriting Asha. The unit of succession is the runtime spine. Foundation types, asset work, spatial services, content, renderer work, and tooling remain potential donors.

The intended center is not Unity's runtime model and not a strict ECS. It is the architecture that the older projects were already reaching for beneath Unity:

> an authority core of entities and data components, behavior owned by explicit services/systems, typed events for consequential facts, data-driven definitions, and a thin host/render shell.

## What “object-centric” means here

Object-centric is a discoverability and authority goal, not a demand for classical object-oriented implementation.

For any gameplay entity, a developer or agent should be able to answer these questions without reconstructing contract routing:

- What is this entity? Inspect its `EntityDefinition`, identity, and components.
- What data gives it this behavior? Inspect its typed configuration/state components.
- Who is allowed to change that data? Follow the component's feature module to its named service or explicit project behavior binding.
- What caused the last change? Inspect a typed committed event and, when needed, its diagnostic trace.
- What happens each frame? Read one short, centrally composed phase list.
- How does it reach the screen? Follow one derived projection path from committed state.

That does **not** mean:

- entities independently own arbitrary mutable object graphs;
- components contain `Update`, `Tick`, event-subscription, service-location, or I/O behavior;
- every entity is a Unity `GameObject`;
- every behavior is a `MonoBehaviour` callback;
- Unity serialization, prefabs, or scene objects define authority;
- systems obtain broad mutation rights merely because they declare an ECS query;
- a generic event bus hides which behavior runs next.

“Unity-legible” is therefore useful only at the level of broad landmarks: a session/scene, entities, components, systems/services, definitions, input, events, and presentation. The actual precedent is the custom entity/component architecture in PixelPhantasm, where pooled Unity objects and transforms were attached only when Unity features were needed.

## The recovered project lineage

### PixelPhantasm / old RPG

The old RPG under `/home/stash/research/old-rpg` had the recognizable center that Asha currently lacks:

- `EntityController` owned entity identity and per-type component storage.
- `ComponentArray<T>` associated concrete component values with entities.
- `DataDefinition` supplied `ID`, `Name`, parents, and component data, then materialized an entity.
- named systems owned behavior outside components.
- `UnityToEntityBridge` mapped colliders, rigidbodies, identifiers, and pooled Unity objects back to authoritative entities.
- `EntityPhysicsCollider` translated a Unity collision into typed entity events.

That is not evidence for restoring its implementation unchanged. It also demonstrates the failure modes to avoid:

- `EntityController`, `World`, and several systems were ambient static authorities;
- `SystemBase<T>.Get` was a service locator hidden behind global singleton access;
- `EntityEventHub` used priority-sorted receivers and an additional string-named post hook;
- `RuleEvent` was a pooled mutable object routed through a global `RulesSystem`;
- reflection-heavy serialization and inheritance restoration were difficult to trust;
- `ISystemUpdate` allowed update-style behavior to spread into Unity-attached objects.

The successor should recover the **shape**—entity data, definitions, systems, and a thin shell—while replacing ambient ownership, hidden ordering, reflection, and scattered polling.

### RuleWeaver

RuleWeaver refined the same architecture in a headless-friendly form:

- `SessionState` was the explicit authority for entities and typed component stores.
- `ActorView` made an entity convenient to inspect without creating another authority.
- `GameBootstrap` and `AppRuntime` made construction and lifecycle visible.
- `GridService`, `TurnService`, `ZoneService`, `ReactionService`, and other named services gave behavior clear homes.
- `RulesEngineService` kept collaborative rule resolution specialized rather than turning it into the application message bus.
- typed bus events, rule traces, an event journal, combat-log projection, and presentation timelines were recognized as different concerns.
- the Core/App/Engine dependency direction kept the FNA/Myra host outside the simulation core.

RuleWeaver also supplies cautions:

- `AppRuntime` can become an oversized service bag if passed everywhere.
- independently setter-injected scenario collaborators create lifecycle drift.
- priority-ordered handlers mutating one resolution workspace remain hard to reason about.
- a typed bus still becomes ambient if subscription and disposal are not explicit.
- views that expose broad mutation can become a second informal authority API.
- explicit registries are useful only when each fact has one authoritative list.

### Asha

Asha preserved several valuable principles:

- Rust is authoritative; TypeScript is expression, tooling, and presentation.
- stored project content, runtime state, and rendered projection are distinct.
- public language/process borders are typed and generated.
- accepted state should be distinguishable from proposed intent.
- many low-level capabilities and services are already isolated in crates.

The problem is that these principles became coupled to a high-cost universal route. At the inspected snapshot, ordinary gameplay can pass through `GameplayRuntimeHost`, static module composition, Gameplay Fabric coordination, owner routing, declared reads, proposal/fact envelopes, reaction frames, decision receipts, hashes, admission proofs, and bridge facades before producing a visible result. Each piece is defensible locally; collectively they obscure the domain object and its behavior owner.

The successor hypothesis is that the valuable principles survive with a much more ordinary execution center.

## Pre-implementation fork: who owns game-specific decisions?

The independent Den proposal `asha/external-object-owned-gameplay-runtime-spike` agrees with most of this document's structural diagnosis but challenges one assumption: that ordinary game-specific behavior should normally be implemented by Rust services.

It proposes a different authority split:

> trusted executable TypeScript owns Game Project decisions and behavior-local state; a smaller Rust world kernel owns reusable engine invariants and atomically applies typed capability commands.

This is not a terminology disagreement. It is the largest unresolved design choice before implementation begins.

Object-centric and object-owned are also not synonyms:

- **object-centric** means entities, their data, relationships, responsible behavior owners, and recent facts are the primary way to understand the runtime;
- **object-owned behavior** additionally means executable behavior instances are attached to an entity/scope and may own project state and lifecycle handlers.

The first property is a requirement of this spike. The second is a hypothesis to compare. PixelPhantasm was object-centric while putting most behavior in external systems; its history therefore does not by itself justify attached per-entity scripts.

### The two credible game-code hosts

| Question | Rust service-owned gameplay | Trusted executable TypeScript gameplay |
|---|---|---|
| Game-specific decisions | Named Rust services and domain systems | Project services/behaviors using ordinary TypeScript control flow |
| Reusable engine invariants | Rust services/kernel | Rust world/capability kernel |
| Game-specific state | Typed Rust components/session aggregates | Bounded versioned project-state records, stored by Rust but interpreted by project code |
| World mutation | Direct in-process service API; reusable engine capabilities may use a typed applier | Read-only invocation views produce one atomic typed command batch |
| Object locality | Entity definition/components lead to the owning service | Entity definition/components/relationships lead to an explicitly bound behavior or project service |
| Main strength | Familiar direct call stacks, typed state, straightforward debugging, no cross-language game loop | Ordinary branching/composition without growing a Rust-authored behavior language or changing Engine schemas for each rule |
| Main risk | Every new game meaning requires a Rust compile and can drift back upstream into Engine | Split-language debugging, persistence/tooling complexity, bridge cost, opaque state, and a slide toward MonoBehaviour-like lifecycle scattering |

The external proposal's strongest criticism should be accepted regardless of which host wins: if data-only authoring requires Rust to acquire event filtering, branching, local state, queries, cancellation, error handling, and continuations as public schema, the engine is building a second programming language. This successor must not repeat that path.

Its other useful additions are:

- distinguish **engine authority** from **trusted Game Project rule authority** rather than treating all authority as one indivisible concept;
- use one batched Rust mutation boundary for reusable engine capabilities;
- compare change amplification instead of assuming Rust or TypeScript is simpler;
- measure bridge calls, copying, allocations, scale, and edit-to-run time;
- preserve versioned project state and stable scheduled messages without serializing callbacks;
- keep the experimental repository outside Asha's workspace and governance pressure.

### Common kernel that does not predetermine the winner

The initial kernel should be usable by either host and should contain only shared world mechanics:

- entity identity/lifecycle and concrete reusable capabilities;
- bounded read-only entity/capability views;
- one typed atomic command applier for reusable engine mutations;
- accepted engine facts and central projection reconciliation;
- stable time plus one durable scheduler;
- snapshot mechanics and resource bounds;
- a narrow batched host boundary.

A Rust service may call this applier in process without serialization. A TypeScript host may receive one bounded event/view batch and return one command/state/schedule batch. Neither path gets mutable renderer objects, browser globals, a hidden service locator, or a clone of the complete world.

The common kernel must not contain door, quest, encounter, or behavior-language semantics merely to make the comparison possible. It also must not require all Rust domain state to be reduced to generic capability commands. The narrow shared boundary is for genuinely reusable engine-owned state.

An illustrative external-host exchange is:

```text
typed event batch + bounded Entity views + due messages
  -> explicitly composed trusted Game Project code
  -> atomic capability-command batch
     + versioned project-state updates
     + stable scheduled messages
  -> Rust validation/application once
  -> accepted facts and projection
```

This is a runtime host boundary, not an authored AST, interpreter, or universal behavior graph.

### Guardrails on the TypeScript hypothesis

If executable TypeScript is tested:

- organize behavior as explicit project services/controllers where possible; attaching one behavior instance to every object is not mandatory;
- an entity-to-behavior binding must be visible in its definition or relationships, never discovered through magic methods;
- behavior-instance state has one owner and is never mirrored semantically in Rust;
- handlers are invoked from explicit event/message composition, not ambient subscription;
- per-entity ticking is off by default; real-time work uses a named bounded project system/host phase;
- the hot path is batched and does not cross the bridge once per read or command;
- project events remain typed and project-owned without requiring a new central Rust enum case for every field;
- scheduled work addresses a stable behavior/service instance and typed message, never a JavaScript closure;
- the first profile assumes trusted bundled product code, not adversarial mods or competitive network clients;
- a dual Rust/TypeScript host is not a presumed permanent architecture. It requires distinct named consumers or one host must be retired after the experiment.

The decision should be made from the walking comparison below, not from analogy to Unity or from an abstract preference for either language.

## Proposed runtime spine

```text
stored project content
        |
        v
EntityDefinition loader/validator -----> explicit snapshot codecs
        |
        v
  +--------------------- GameRuntime ----------------------+
  |                                                       |
typed input ---> named game-code owner ---> SessionState   |
  |                 |                 |                    |
  |                 +---- outcome ----+                    |
  |                                   |                    |
  |                         typed committed Events         |
  |                                   |                    |
  |                         explicit Dispatcher            |
  |                         /        |        \             |
  |                 another owner    Scheduler  Journal    |
  |                                   |                    |
  |                         named frame Systems             |
  |                                   |                    |
  +------------------------ Projection --------------------+
                                      |
                                      v
                           TS/Three/DOM host adapters
```

Diagnostics and recording observe this path. They do not define the path.

In the Rust-service baseline, “named game-code owner” is an in-process Rust service. In the executable-project comparison, the same position may be a trusted TypeScript project service/behavior behind one batched host boundary; all actual reusable world mutation still terminates at the Rust kernel.

### `GameRuntime`: private composition and lifecycle root

`GameRuntime` owns the active `SessionState`, service instances, optional game-code host, scheduler, event queue, ports, and lifecycle. It is the one place where the frame sequence and cross-service orchestration are visible.

It is not passed wholesale into domain code. A service receives only the state and collaborators needed by that operation. Replaceable session/scenario collaborators are rebuilt together at a visible lifecycle boundary, never installed piecemeal through long-lived setters.

### `SessionState`: authoritative mutable truth

`SessionState` owns durable gameplay truth for one running session. It contains:

- entity identity and lifecycle;
- concrete typed component tables keyed by `EntityId`;
- explicit session aggregates that do not naturally belong to one entity;
- optional bounded/versioned project-state records when the executable project-code host is under test;
- authoritative time/random state when required;
- enough state to produce a supported snapshot.

Service-owned caches and derived indexes may exist outside `SessionState` when their owner and rebuild lifecycle are explicit. A spatial broadphase or render-handle map is a derived index, not a second version of entity truth.

The initial spike should use concrete typed tables, not build a dynamic ECS or `Any`-based component framework. A domain module may own a small group of tables, and the session composes those groups explicitly. Repetition can justify a helper or code generator later; it does not justify a framework in advance.

### `Entity` and entity views

An `Entity` is an identity plus its composed data in the session. It is not a separately owned mutable heap object.

The API should nevertheless be entity-legible. A read-only `EntityRef` or domain view may gather familiar accessors around an `EntityId`, such as `door()`, `transform()`, `health()`, and `definition_source()`. Such a view is a lens over `SessionState`, not another state owner.

Mutation remains inside named service operations. If an `EntityEdit` helper proves useful, it should be short-lived, scoped to one service call, and incapable of locating other services or publishing arbitrary events.

### `EntityDefinition`: durable authored shape

`EntityDefinition` is the durable description from which runtime entity data is created. It may contain:

- identity/name/source information;
- common first-class fields where nearly every domain benefits from them;
- a set of concrete typed configuration components;
- typed references to other definitions/entities;
- authoring diagnostics and source provenance.

There is no requirement to force every field into a generic component solely for architectural purity. The old `DataDefinition` and RuleWeaver's `EntityDefinition` both show that common first-class RPG/engine fields can improve legibility.

Definition inheritance is not assumed. If retained, inheritance is resolved and flattened during project admission so runtime code sees one concrete definition. No runtime parent walking or reflection-based restoration belongs in the baseline.

### Components: mostly data

Components contain entity-local configuration and state. They may have constructors, validation helpers, pure calculations, and invariant-preserving value methods. They do not own the application loop.

A component must not:

- register itself with a global world or event bus;
- define an implicit update callback;
- locate services;
- perform bridge, renderer, file, or network I/O;
- mutate unrelated entities;
- become authoritative merely because some code holds a mutable reference to it.

“Component” is the default conventional term. “Capability” may remain where it genuinely communicates optional engine-level ability, but no vocabulary firewall should enforce the distinction. `Entity`, `EntityDefinition`, `Component`, `Service`, `System`, `Event`, `SessionState`, `Adapter`, and `Projection` are the working vocabulary.

### Services: intent-driven behavior owners

A service owns a coherent family of operations and invariants. Typical examples are `InteractionService`, `DoorService`, `CombatService`, `InventoryService`, `SaveLoadService`, and `SchedulerService`.

The familiar baseline is a Rust service. The comparison may place game-specific decision logic in an explicitly composed trusted TypeScript project service/behavior, but that host must preserve the same visible responsibility boundary and may mutate reusable engine state only through the batched Rust command applier.

The ordinary operation is direct and typed:

```text
typed intent
  -> service validates against SessionState
  -> service changes its owned state atomically
  -> service returns a typed outcome plus committed events
  -> runtime dispatches those facts
```

Ownership should primarily be expressed by module boundaries, private mutation APIs, and obvious composition—not by a runtime owner matrix, registry digest, proposal codec, or per-call proof object. Architecture tests may enforce dependency direction. They should not make ordinary local mutation look like a network protocol.

### Systems: bounded, centrally scheduled batch work

“System” is reserved for behavior that genuinely needs a batch or frame phase across multiple entities, such as fixed-step physics, lifetime expiry, spatial-index reconciliation, or render extraction.

Systems are invoked by `GameRuntime` in a short named sequence. Components do not opt themselves into updates, and there is no open scheduler that discovers arbitrary readers/writers.

A reasonable initial frame is:

1. normalize external input into typed intents;
2. execute due scheduled intents;
3. run the small set of fixed-step simulation systems;
4. drain committed event consequences at defined boundaries;
5. reconcile derived indexes;
6. extract render/UI projection;
7. publish diagnostics and presentation output.

The exact phases are a spike result. The architectural rule is that the list is short, central, explicit, and contains no component-local polling.

### Typed events with real weight

Events are immutable typed facts about committed state. They may cause significant consequences: open an encounter, schedule future work, update a quest, trigger audio, append history, or prompt another service operation.

The Rust-service baseline should use an explicit dispatcher over closed/nested Rust event enums, not dynamic subscription or string topics. Domain modules may own nested event enums, while the application composition root exposes the finite routing between them. This makes “what handles `EnemyDefeated`?” a code search with a small answer.

The executable project-code host must preserve the same property without forcing each project event into a central Rust enum. Project-owned events are typed by the bundled project package and routed through explicit project composition; only engine-owned facts and the small versioned host batch need kernel-level representation. Crossing the host boundary must not erase event identity into free-form `SendMessage` strings.

The distinctions are:

| Concept | Meaning | Default transport |
|---|---|---|
| Intent/command | A request that may be rejected | Direct typed service call |
| Outcome | The immediate result for the caller | Typed return value |
| Event | A fact that is now true and may have consequences | Explicit bounded event queue/dispatcher |
| Trace | Why a service or rule reached its answer | Optional diagnostic sink |
| Journal | Selected committed history for inspection | Recorder observing events |
| Projection | Derived render/UI/presentation data | Narrow host adapter |

An event is never mutated by receivers. Event processing is bounded, ordered, and traceable. Handlers may emit later events, but they do so through the runtime's queue rather than recursively posting to an ambient bus.

### Rules and reactions are specialized tools

Most operations should not pass through a generic rules engine. A direct service method is the normal path.

When a domain truly needs collaborative resolution—combat modifiers, interrupts, replacement effects, or preview/simulation—it may create a domain-specific `ResolutionWorkspace`. Handlers may revise that in-flight workspace, then one owning service commits the final authoritative changes and emits facts. This recovers RuleWeaver's useful deferred-authority boundary without making every door, movement, definition load, or render update a rule resolution.

Priorities, phases, cancellation, and traces must be local to that resolution domain. They do not become a universal event topology. Mutable pooled `RuleEvent` objects and global rule systems do not return.

### Scheduling

There is one `SchedulerService`. For Rust-owned behavior it stores typed future intents such as `CloseDoor { entity }`. For a trusted project-code host it may instead address a stable behavior/service instance with a versioned typed message and bounded payload. It never stores callbacks, script closures, browser timers, unbounded data, or replay frames. Due work re-enters the same explicitly composed behavior path as immediate work.

The supported snapshot records pending scheduled intents when product behavior requires them.

### Projection and host boundary

Render and UI projection are derived from committed authority state and changes. Gameplay services do not each remember to update render visibility, collision visualization, scene handles, and UI mirrors separately.

One projection system extracts stable render operations or state deltas for the existing TypeScript/Three/DOM shell. The render host owns ephemeral render objects and input devices; it never owns gameplay truth. This is the modern equivalent of `UnityToEntityBridge`, without Unity objects leaking back into components.

Generated contracts remain appropriate at the real Rust/TypeScript or process boundary. They are not required between neighboring Rust services.

### Persistence, replay, and observability

The baseline persistence contract is an explicit snapshot at a documented quiescent point. Concrete Rust feature modules contribute explicit typed snapshot data/codecs. If the trusted project-code host is selected, the snapshot also contains behavior/service type and version, stable instance identity, bounded structurally validated state, pending stable messages, and an explicit migration or incompatible-version result. Rust need not interpret project-state semantics, but there must never be a second mirrored copy. Restore rebuilds derived indexes and host projections. No reflection discovery is used.

Replay remains available as an assurance product, not the application spine:

- baseline: focused deterministic tests, snapshots, typed events, and useful traces;
- recordable profile: external intents, seeds/time, selected events, and checkpoints for regression/debugging;
- certified replay profile: exhaustive hashes and stronger proof only for a named consumer that justifies its cost.

Turning off certified replay must not disable ordinary gameplay, restart, or supported save/load. A reaction frame, registry proof, or full event history is persisted only when execution or a selected assurance profile actually requires it.

## Suggested initial crate shape

Start coarse and split only along real ownership or dependency boundaries. An initial workspace should be closer to six to ten crates than Asha's roughly one hundred assignment-sized crates.

```text
crates/
  foundation/       ids, math, time, errors, assets
  model/            SessionState, EntityStore, definitions, component tables
  gameplay/         Rust product/domain components, services, typed events
  engine-services/  spatial, collision, physics, pathfinding, mesh/voxel adapters
  runtime/          GameRuntime, lifecycle, dispatcher, scheduler, save/load
  bridge/           narrow native/TS input and projection contracts
```

This is a starting dependency shape, not a naming mandate. A feature deserves its own crate when it has an independently useful ownership boundary, substantial implementation, or a real downstream consumer—not merely to serve as an agent assignment cell.

The executable-TypeScript comparison may add one narrow game-code host/bridge crate and one project runtime package. It must not reproduce Asha's protocol-package topology. The headless Rust kernel and Rust-service baseline must remain runnable without Node or a browser.

## Asha preservation and bypass map

No candidate is reused merely because it already exists. Each donor must pass a dependency and semantic audit against the new spine.

| Asha area at the inspected snapshot | Successor posture | Reason/caution |
|---|---|---|
| `core-ids`, `core-math`, `core-time`, `core-error`, `core-collections`, `core-space`, `core-assets` | Strong transplant candidates | Low-level value types with little reason to reinvent them. Copy with tests; do not retain a path dependency back to Asha. |
| `core-entity` | Selectively transplant and reshape | It already has entity identity, lifecycle, typed capability tables, relations, and useful values. Its hard-coded tables, movement ownership, save/replay hashing, and tombstone policy must be chosen afresh rather than inherited automatically. |
| `core-scene`, project bootstrap, `EntityDefinition` authoring | Adapt behind a new loader | Preserve stored/runtime separation and reference validation. Materialize directly into the new `SessionState`, not through the old runtime host. |
| `svc-spatial`, `svc-collision`, `svc-physics`, `svc-pathfinding`, mesh/voxel/asset services | Feature donors behind successor interfaces | These represent expensive feature work. Reuse only if they compile below the successor runtime and do not pull in owner/fabric/replay assumptions. |
| #6088 direct authority-verb executor and existing scheduler semantics | Extract selected kernel mechanics | They are valuable evidence for one typed atomic engine-capability mutation boundary, but currently sit inside or near the high-level host under evaluation. Copy/extract the exercised pieces rather than importing that topology. |
| `svc-serialization`, project content/bundle codecs | Adapt narrowly | Keep strict public-border decoding, versions, migrations, and canonical project content. Do not make artifact hashes or admission receipts prerequisites for local service calls. |
| `protocol-input`, `protocol-render`, `render-bridge`, native bridge pieces | Border candidates | The language/process boundary still benefits from typed generated contracts. Prefer a smaller successor facade and central projection extraction. |
| TS `renderer-three`, `renderer-host`, `ui-dom`, browser/electron host work | Preserve aggressively | The old-project retrospective and RuleWeaver both validate keeping the browser/Three/DOM shell rather than replacing it with a native UI stack. |
| assets, examples, fixtures, diagnostics, importers | Preserve or port as feature value | These are product investment, not evidence that the old runtime route must remain. |
| `gameplay-runtime-host`, `gameplay-module-sdk`, `rule-gameplay-fabric`, `svc-gameplay-fabric` | Do not use as the successor center | Their composition, read-plan, proposal/fact, reaction, proof, and routing obligations are the structure under test. Depending on them would make the spike a facade over Asha rather than a successor. |
| AuthoredBehavior DTO/IR/compiler/interpreter | Exclude from the experiment | Both candidate hosts use ordinary code for game-specific control flow; the spike must not grow a universal behavior language to mediate them. |
| full `runtime-bridge-api` / `RuntimeSession` surface | Replace with a walking-slice facade | Carry over only border operations a real consumer exercises. Avoid mirroring every existing method for compatibility. |
| `protocol-replay`, mandatory state hashes, reaction frames, decision receipts | Optional donor for an earned assurance profile | They must not be on the baseline gameplay path. |

The transplant rule is strict:

> A copied crate may depend on successor foundation/model contracts, but the successor runtime must never call back into Asha's high-level runtime to get its authority work done.

Every copied or materially adapted donor should be recorded in a small provenance ledger with its source repository/path, exact commit, copied symbols or scope, reason for copying instead of referencing, and meaningful successor changes. This makes selective transplantation auditable without importing Asha's broader governance machinery.

Asha can remain an implementation reference and behavior oracle. The two runtimes can be launched separately against equivalent fixtures. They must not share live mutable authority or delegate operations to one another.

## Walking falsification spike

### Phase 0: common kernel, no game-code verdict

Build only enough host-neutral machinery to support a fair comparison:

- explicit switch and door entities with visible relationships;
- entity lifecycle and the minimum transform/collision/renderable capabilities;
- bounded entity/capability views;
- one typed atomic engine-command applier;
- accepted facts, scheduler, snapshot, and projection reconciliation;
- a headless runner before the browser host.

Do not put door, encounter, branching, or behavior-IR semantics into the kernel. Do not build the TypeScript bridge until a headless Rust caller proves the kernel boundary.

### Slice 1: Rust service-owned security door

Implement the familiar baseline through explicit Rust services.

Representative entity data:

- switch entity: `Interactable`, `ControlsTargets`, optional `SwitchState`;
- door entity: `DoorConfig`, `DoorState`, `Transform`, `ColliderShape`, `Renderable`;
- typed relationships contain actual `EntityId`/definition references after admission;
- `DoorConfig` includes an optional auto-close duration.

Representative behavior owners:

- `InteractionService` validates player interaction and commits switch activation;
- explicit dispatch routes `SwitchActivated` to the configured domain consequence;
- `DoorService` owns legal door transitions and emits `DoorOpened`/`DoorClosed`;
- `SchedulerService` stores a typed `CloseDoor` future intent;
- collision/spatial reconciliation derives participation from committed state;
- `ProjectionSystem` derives visible door state and render operations.

The important call path should fit on one screen:

```text
Interact { actor, switch }
  -> InteractionService
  -> SwitchActivated { switch, actor }
  -> explicit event route
  -> DoorService::open(door)
  -> one atomic reusable-capability apply where needed
  -> DoorOpened { door }
  -> schedule CloseDoor when configured
  -> collision/render extraction from committed state
```

No module manifest, proposal codec, declared-read plan, owner registry digest, reaction frame, decision receipt, runtime-facade method per door action, or certified replay proof is required.

### Slice 2: executable TypeScript comparison

Reimplement the same externally visible door behavior through the smallest trusted Game Project host:

- ordinary TypeScript branching and composition;
- one explicitly composed project service or behavior binding visible from the entity definition;
- one bounded view/event/message batch entering TypeScript;
- one atomic engine-capability command/state/schedule batch returning to Rust;
- versioned behavior/service state only if the door genuinely needs it;
- the same Rust collision, time, scheduling, persistence, and projection invariants;
- no callback or JavaScript closure in a save;
- no bridge call per capability read or command.

This is not intended to prove that TypeScript can mimic a door. It compares call-stack legibility, code locality, state ownership, amount of infrastructure, save/reopen complexity, diagnostics, and edit-to-run time against the Rust service implementation.

The comparison must record changed files and lines by ownership surface:

| Slice/variation | Project code/content | Rust kernel | Bridge | Persistence | Projection | Why |
|---|---:|---:|---:|---:|---:|---|

### Slice 3: reuse and genuine extension

Run three distinct change-amplification tests:

1. **Rust data-only variation:** one door remains latched while another closes after a configured delay or requires multiple activators. This should change definitions/configuration and tests only.
2. **Trusted project-code variation:** implement an encounter-gated exit using an existing typed enemy-defeat fact and existing engine capabilities. If the necessary world primitives exist, it should change Game Project TypeScript/content and focused tests without a Rust kernel, central protocol, or bridge case.
3. **New engine capability:** add one genuinely reusable world meaning that neither host already has. It should require one narrow Rust capability owner/command path and bindings, not an authored-language or runtime-host campaign.

This separates three questions that the previous version combined: whether data composition is cheap, whether ordinary game-specific programming is cheap, and whether extending reusable engine mechanics is cheap.

### Slice 4: real-time and multi-entity pressure

Two door-shaped examples are insufficient. Add one bounded behavior such as a moving/attacking enemy, projectile interaction, or small encounter controller that exercises several of:

- multiple related entities;
- spatial/capability queries;
- project- or service-local state;
- branching, cancellation, or target changes;
- batched commands;
- spawn/despawn;
- a named bounded real-time phase.

Measure bridge calls per tick, batch size, allocations, state copying, scaling with entity/behavior count, projection cost, and TypeScript edit-to-run time. Per-object tick callbacks are not the implementation model even if the TypeScript host participates in a named real-time phase.

### Product proof

The spike is incomplete until a player can exercise the comparison through the retained browser/Three/DOM shell. Proof should cover:

- project content loading into concrete definitions/components and explicit behavior bindings where used;
- normal resolved input;
- Rust-owned engine invariant enforcement and atomic mutation;
- collision and spatial consequences;
- visible projection;
- restart and one supported save/restore point, including project state/messages if present;
- diagnostics that identify the entity, responsible Rust or project service/behavior, command result, and committed events;
- the Rust data-only variation;
- the project-code encounter variation;
- the localized new-capability variation;
- the real-time characterization.

## Success criteria

The architecture earns further migration only if all of these hold:

1. An entity's behavior can be understood from its definition/components/relationships and one or two explicitly named Rust or project behavior owners.
2. Every mutable state family has one obvious owner and lifecycle.
3. The full input-to-visible-result path is traceable without consulting a contract taxonomy or ownership registry.
4. Components contain no scattered update logic or ambient subscriptions.
5. The frame/system order is visible in one composition file.
6. Typed events cause meaningful cross-domain consequences without string topics, mutable event objects, or hidden listener discovery.
7. The Rust door variation is data-only.
8. The trusted project-code encounter variation requires no Rust kernel, central protocol, or bridge case when its engine capabilities already exist.
9. A genuinely new reusable engine capability is localized to one narrow Rust owner/command path and bindings.
10. The Rust and TypeScript host comparison records change amplification, explainability, persistence cost, bridge cost, and edit-to-run time rather than choosing by aesthetics.
11. TypeScript behavior state, if used, is bounded/versioned, survives restart/save/reopen, and has no semantic mirror in Rust.
12. The real-time path is batched and characterized under a named workload rather than inferred from a tiny scene.
13. At least one substantial Asha feature service and the existing TS renderer shell are reused or transplanted, proving this is not a toy rewrite.
14. Ordinary gameplay, restart, and save/load work with certified replay disabled.
15. The successor has no dependency on Asha's high-level runtime/fabric crates.
16. The implementation deletes or avoids more structural code than it introduces for the walking features.

Useful measurements include changed-file surfaces for each variation, dependency counts of the runtime crate, bridge crossings and bytes per invocation/tick, allocations/state copying, number of runtime envelope types, edit-to-run time, and the shortest source path from an entity component or behavior binding to its responsible owner.

## Stop conditions

Stop or narrow the effort if:

- the spike becomes a general component framework, plugin platform, behavior DSL, editor schema system, or replay platform before the first mechanic works;
- importing one useful Asha feature requires importing Gameplay Fabric or the old `RuntimeSession` host;
- the second variation needs new engine/protocol/facade code;
- a new reusable engine capability still crosses many unrelated Rust crates;
- the explicit dispatcher grows into a generic ambient bus before two domains demonstrate the need;
- service boundaries recreate proposal/receipt/proof objects around ordinary in-process calls;
- executable TypeScript requires one bridge call per read/verb, repeated JSON round-trips, or repeated whole-world cloning;
- the same game-specific state is independently authoritative in Rust and TypeScript;
- object-bound behaviors acquire implicit lifecycle discovery, ambient subscription, or scattered per-object ticking;
- every project event or state field requires a new central Rust enum, protocol, or bridge case;
- the Rust and TypeScript hosts begin duplicating the same production role without separate named consumers and a retirement decision;
- save/load requires event-sourcing every mutation;
- browser product proof is deferred behind engine-only fixtures;
- maintaining compatibility with the old runtime starts dictating the new internal model.

A failed spike is still useful if it identifies the smallest Asha feature that cannot be transplanted without the old spine. That is much more actionable than another broad refactor wave.

## Risks and guardrails

### `GameRuntime` becomes another giant facade

Keep it private and orchestration-only. Domain behavior lives in services. Do not add one public method for every gameplay noun; expose a small input/session/inspection/save boundary.

### The service set becomes a locator

Never pass the whole service collection into components or arbitrary services. Compose calls explicitly and pass narrow dependencies.

### Entity views become backdoor authority

Make general entity views read-only. Keep mutation helpers scoped to the owning feature/service and avoid broad `get_any_mut` APIs.

### Events recreate hidden indirection

Begin with an explicit closed dispatcher and code-visible routes. Keep events immutable and post-commit. A typed dynamic bus is an earned optimization, not a baseline abstraction.

### Systems recreate ECS polling

Only create a system for genuine bounded batch/frame work. Discrete consequences use service calls or typed events. Central composition owns phase order.

### Services fragment into tiny architectural cells

Prefer cohesive feature modules and coarse crates. A service should represent a real behavior boundary, not wrap every function call.

### Derived state drifts

Derived indexes declare their source authority and rebuild lifecycle. Render projection and collision indexes reconcile centrally; gameplay code does not manually maintain multiple mirrors.

### Definition inheritance recreates restoration complexity

Default to explicit composition. If inheritance remains valuable, flatten it once during admission and test the materialized definition.

### Compatibility code becomes permanent

Put import/comparison adapters in an explicit `compat` area with deletion criteria. Never let them become the internal successor API.

### Replay requirements creep back into the baseline

Every assurance artifact must name its consumer and the failure class it prevents. If it is diagnostic-only, it must remain removable without changing gameplay semantics.

### Executable TypeScript recreates MonoBehaviour at another border

Do not equate ordinary TypeScript with one stateful callback object per entity. Prefer explicit project services/controllers and event batches. Entity attachment is used only when instance identity and lifecycle are actually valuable; named real-time phases replace implicit updates.

### Engine authority and Game Project authority blur together

Document which invariants Rust validates and which trusted decisions belong to project code. Rust rejects invalid world mutation; it does not reconstruct project intent merely to approve it. Project code never bypasses the kernel to mutate reusable capabilities. A stricter sandbox or certified host is a separate profile.

## Explicit non-goals

- Recreate Unity's `GameObject`, `MonoBehaviour`, prefab, scene serializer, or magic callback model.
- Adopt Bevy or another strict ECS framework for this spike.
- Put mutable behavior and polling in components.
- Build a universal gameplay IR or JSON behavior graph.
- Preserve Asha's public runtime surface one method at a time.
- Migrate every Asha crate or feature before product proof.
- Give TypeScript direct mutable access to Rust world state, renderer objects, browser-global authority, or unbounded callbacks.
- Assume executable TypeScript is the required or exclusive gameplay host before the comparison.
- Conflate stored definitions, runtime state, and projections.
- Remove typed public borders, strict external decoding, explicit time/randomness, or atomic rejection where product semantics require them.
- Guarantee certified replay for all games and all builds.
- Decide terminology by architectural law rather than readability.

## Initial decisions and deliberately deferred decisions

The spike begins with these defaults:

- use `Entity`, `EntityDefinition`, `Component`, `SessionState`, `Service`, `System`, and `Event`;
- concrete typed component tables composed at compile time;
- read-oriented entity views;
- direct typed Rust service methods for the familiar baseline;
- one typed atomic mutation boundary for reusable engine capabilities, callable in process or through a batched external host;
- immutable post-commit events and an explicit bounded dispatcher;
- central named frame phases;
- explicit snapshot persistence;
- TypeScript/Three/DOM as the shell, with trusted executable TypeScript game code treated as a bounded comparison rather than a settled layer;
- Asha leaf code copied/transplanted, not invoked through an old-runtime compatibility layer.

Only evidence from the walking slice should decide:

- whether some capabilities merit their own engine-level term;
- whether component-table repetition warrants generation;
- whether a dynamic typed event bus is actually needed;
- whether definition inheritance is worth retaining;
- whether ordinary trusted gameplay should live in Rust services, executable TypeScript project code, or two hosts with genuinely different named consumers;
- whether entity-attached behavior instances add useful locality beyond project services/controllers;
- whether project-owned state can remain inspectable, versioned, and cheap enough to justify the split-language host;
- which replay assurance profile has a real consumer;
- which Asha crates are clean enough to transplant versus cheaper to reimplement;
- whether `rusty-engine` should become the durable successor or remain a disposable spike.

## Recommended next action

The common kernel, Rust security-door baseline, and batched executable-TypeScript comparison now
run headlessly. Their evidence is recorded in `docs/experiment-results.md`; it is deliberately not a
game-code-host verdict. The next implementation is Slice 3's encounter-gated exit. It must change
only project TypeScript/content and focused tests while reusing the current generic event,
relationship, state, scheduling, command, persistence, and projection surfaces. A need for a new
central Rust encounter case is a falsification result, not an invitation to expand the host.

After that, add one genuinely reusable engine capability through one narrow Rust owner/command path,
then run the named real-time workload before browser/Studio investment. If those proofs pass, the
next architectural decision is not “rewrite Asha.” It is:

> Which complete vertical feature family can move next while the old and new runtimes remain separately launchable and never share authority?

## Source basis

This proposal is grounded in:

- Den `_global/ess-architecture-guide`;
- Den `asha/old-projects-retrospective-mapping`;
- Den `asha/expressive-typescript-gameplay-composition`;
- Den `asha/gameplay-implementation-fundamentals-proposal`;
- Den `asha/architecture-novelty-budget-critique`;
- Den `asha/external-object-owned-gameplay-runtime-spike` as an independent competing authority-host proposal;
- `/home/dev/ruleweaver/docs/agent/ARCHITECTURE.md`;
- `/home/dev/ruleweaver/docs/agent/SYSTEMS.md`;
- RuleWeaver design notes on state ownership, lifecycle, bus semantics, observability, rule commit boundaries, and architecture pressure points;
- `/home/stash/research/old-rpg/src/Entity/EntityController.cs`;
- `/home/stash/research/old-rpg/src/Data/Structures/ComponentArray.cs`;
- `/home/stash/research/old-rpg/src/Data/Definitions/DataDefinition.cs`;
- `/home/stash/research/old-rpg/src/Events/EntityEventHub.cs` and `RuleEvent.cs`;
- `/home/stash/research/old-rpg/src/Main/UnityBridge/UnityToEntityBridge.cs` and `EntityPhysicsCollider.cs`;
- `/home/stash/research/old-rpg/src/Physics/Unity/FakePhysicsObject.cs`;
- the current Asha workspace/crate manifests and representative `core-entity` and `gameplay-runtime-host` code at the donor snapshot above.
