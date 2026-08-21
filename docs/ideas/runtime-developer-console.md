# Runtime Developer Console and Command Port

Status: idea for implementation discussion

Related proposal: [Standard gameplay substrate](standard-gameplay-substrate.md)

## Summary

Rusty Engine should provide an upstream pull-down developer console and a coherent command-port architecture that downstream products can mount and extend.

The key decision is:

> JavaScript may be the console’s interactive control and composition language. Rust remains the only publisher of authoritative runtime state.

This captures most of the useful runtime-VM ergonomics:

- interactive queries;
- local variables, loops, and assertions;
- reusable test snippets;
- previewing canonical gameplay paths;
- explicit developer mutation commands;
- live authoring overlays;
- checkpoint, restore, and scratch-session experiments;
- a common client for humans, agents, CI, and headless tools.

It avoids turning the JavaScript heap into a second game runtime with hidden state, timers, callbacks, mutable world objects, or save-critical closures.

The console should be a **REPL-shaped airlock**, not a second authority.

## Separate the three VM desires

A runtime scripting VM often appears attractive because it bundles three distinct capabilities:

1. **Interactive language** for inspecting and poking the game.
2. **Hot-swappable definitions** for tuning and experimentation.
3. **Runtime behavior ownership** where scripts hold state, receive callbacks, schedule work, and mutate the world.

The console clearly needs the first. It likely benefits from much of the second. The third is a separate architectural commitment and should not enter accidentally through developer tooling.

A console project should therefore solve interaction and hot experimentation first, while preserving explicit Rust authority.

## Split authority is not the presence of two languages

Rust and JavaScript can coexist without split authority.

Split authority occurs when both can independently publish canonical health, inventory, catalog, schedule, or world state.

JavaScript may safely:

- query immutable snapshots and readouts;
- retain console-local scratch variables;
- search and filter entities;
- compose and sequence requests;
- loop over test cases;
- supply explicit bounded roll evidence;
- preview actions;
- decide what it would like to attempt;
- submit privileged developer commands.

Rust must remain the sole owner that:

- resolves IDs against current state;
- validates preconditions and revisions;
- gathers canonical facts;
- stages effects;
- commits mutations;
- advances clocks and schedules;
- installs admitted catalogs;
- persists authoritative state;
- returns typed receipts and diagnostics.

JavaScript holds the clipboard. Rust keeps the stamp.

## Proposed architecture

```text
pull-down browser console
Node scenario runner
agent or CLI client
        |
        v
JavaScript control language
local variables, loops, assertions, formatting
        |
        v
frozen generated capability client
inspect / preview / play / debug / session / author / fault
        |
        v
closed product-specific typed protocol
opaque IDs, expected revisions, command envelopes
        |
        v
Rust runtime command queue
accepted only at explicit safe points
        |
        v
normal Rust owners and services
resolution / mechanics / world / scheduler / persistence
        |
        v
structured reply
snapshot / receipt / trace / revision / diagnostic
```

The VM never receives:

- a mutable `DaggerRuntime` or equivalent;
- `&mut` component access;
- a service locator;
- a callback into arbitrary Rust;
- a direct pointer or proxy to a live entity;
- a writable object graph representing canonical state.

## Opaque handles may feel live without being authoritative

The console API can provide convenient handles:

```js
const rat = await inspect.nearest({ tag: "rat" });
console.log(await rat.inspect());

const result = await play.attempt("melee", {
  actor: player.id,
  primaryTarget: rat.id,
});

console.log(result.trace);
```

The handle is only a frozen client object over an opaque identity:

```js
Object.freeze({
  id: "encounter:rat:17",
  inspect: () => inspect.entity("encounter:rat:17"),
});
```

This must not work as an authority path:

```js
rat.health = 0;
```

Administrative mutation remains explicit:

```js
await debug.track.set({
  entity: rat.id,
  track: "health",
  value: 0,
  expectedRevision: rat.trackRevision,
});
```

Rust resolves the entity, validates the expected revision and bounds, calls the owning service, commits at a safe point, and returns a receipt.

## Visible capability lanes

Do not expose one omnipotent `world` object. Give the console visibly different capability roots.

| Surface | Purpose | Mutation |
|---|---|---|
| `inspect` | Catalogs, entities, components, facts, traces, receipts, spatial queries | None |
| `preview` | Run canonical gather, check, plan, and staging paths without commit | None |
| `play` | Submit ordinary player or AI style intents | Through normal gameplay authority |
| `debug` | Grant, teleport, spawn, restore, reset, or force named developer conditions | Through explicit debug owners |
| `session` | Pause, step, checkpoint, restore, reset, or create scratch sessions | Runtime owner controls lifecycle |
| `author` | Submit candidate packages or overlays for Rust admission | Catalog activation only after admission |
| `fault` | Deliberate invalid fixtures and failure injection | Disposable test sessions only |

Example:

```js
const player = await inspect.player();
const target = await inspect.aimedEntity();

const candidate = await preview.action({
  actor: player.id,
  action: "melee-attack",
  primaryTarget: target.id,
  rolls: {
    "melee-attack.d100": 37,
  },
});

console.table(candidate.effects);

await play.action(candidate.intent);
await debug.inventory.grant(player.id, "healing-potion", 5);
await session.step({ ticks: 30 });
```

The visible lane names teach users and agents which operations merely observe, which use ordinary product authority, and which are privileged developer actions.

## Generic transport, closed semantic command sets

A small number of generic endpoints is useful:

```text
POST /api/dev/v1/query
POST /api/dev/v1/preview
POST /api/dev/v1/command
POST /api/dev/v1/catalog
POST /api/dev/v1/session
GET  /api/dev/v1/describe
```

The transport may be generic. The semantic command set must remain closed, typed, product-aware, and explicitly executed.

Illustrative Rust shape:

```rust
pub enum DevCommand {
    Standard(StandardDevCommand),
    Session(SessionDevCommand),
    Product(ProductDevCommand),
    Fault(FaultDevCommand),
}

pub enum StandardDevCommand {
    Track(TrackDevCommand),
    Effect(EffectDevCommand),
    Inventory(InventoryDevCommand),
    Equipment(EquipmentDevCommand),
    Action(ActionDevCommand),
}
```

Each command variant must reach a concrete Rust match arm and named owner.

Do not introduce:

```text
call("TrackService", "set", [...])
invoke("whatever.path.agent.invented")
world.patch("entities.12.health", 999)
resolveService("inventory").mutate(...)
```

These are reflection, method-name bridges, and service location wearing console makeup.

## Capability descriptors and generated clients

The Rust command definitions should generate or otherwise produce descriptors containing:

```text
name
argument schema
result schema
capability namespace
read / preview / play / debug / author / fault classification
required build profile or permission
required runtime safe point
atomicity guarantee
expected revision fields
help text and examples
```

Descriptors support:

- console autocomplete;
- generated TypeScript clients;
- CLI help;
- agent discovery;
- structured forms for common commands;
- capability manifests;
- API documentation.

Descriptors aid discoverability. They do not dynamically locate arbitrary services.

A common generated package could be used by all clients:

```text
@rusty-engine/dev-client
```

Downstream products can add generated extension clients, for example:

```text
@rusty-dagger/dev-client
```

## Command envelopes, revisions, and provenance

Mutating commands should look like submitted operations rather than ambient function calls.

Illustrative TypeScript envelope:

```ts
export type DevCommandEnvelope<T> = Readonly<{
  commandId: string;
  sessionId: string;
  expectedWorldRevision?: number;
  expectedCatalogEpoch?: number;
  command: T;
}>;
```

Illustrative result:

```ts
export type DevCommandResult<R> = Readonly<{
  status: "applied" | "rejected" | "faulted";
  beforeRevision: number;
  afterRevision: number;
  catalogEpoch: number;
  receipt?: R;
  diagnostics: readonly Diagnostic[];
}>;
```

This provides:

- visible stale-handle rejection;
- deterministic command history;
- clear provenance for world changes;
- agent-friendly structured results;
- exportable reproduction scripts;
- one answer to “what changed this state?”;
- no command executing halfway through another runtime operation.

## Execute commands only at runtime safe points

The runtime thread or owner should drain developer commands at explicit safe points, normally:

- between ticks;
- while paused;
- before or after a bounded attempt;
- at a product-defined session boundary.

JavaScript receives Promises. It does not synchronously reenter Rust during arbitrary gameplay code.

Named operations may be atomic where their owners support it. A console script containing several commands is not automatically one universal transaction.

Avoid adding a generic heterogeneous world transaction merely because the REPL makes semicolon-separated commands convenient.

For multi-step experiments that require isolation, use a checkpoint or scratch runtime instead.

## Preview must use the real path

`preview.action` should run the same canonical path as apply:

```text
admit intent
  -> gather facts
  -> check
  -> plan
  -> intercept
  -> stage
  -> abort instead of commit
```

The console must not reimplement combat formulas, stat evaluation, damage, inventory policy, or target checks in JavaScript.

A preview reply should expose useful read-only evidence:

- admitted intent;
- gathered facts;
- required and supplied inputs;
- predicate decisions;
- planned operations and effects;
- mechanics previews;
- expected revisions;
- rejection or fault phase;
- trace details.

## Browser REPL first; embedded JS engine later only if earned

A browser-hosted product already has a JavaScript runtime. The first console should use it rather than embedding QuickJS, Boa, V8, or another engine into Rust solely for developer interaction.

Suggested shape:

```text
pull-down console component
        |
        v
dedicated Web Worker or sandboxed iframe
        |
        v
frozen generated Dev API over MessagePort
        |
        v
browser service and typed Rust protocol
```

The worker owns:

- REPL evaluation;
- command history;
- local variables;
- loops and assertions;
- formatting;
- autocomplete;
- saved snippets;
- scenario recording.

It should not receive unrestricted access to the browser application or raw runtime internals.

A Web Worker is sufficient for a trusted developer console. Untrusted mod scripting is a separate security and compatibility project.

## Reuse the same client for headless scenarios and agents

The generated client should work outside the browser through a transport adapter.

Consumers include:

- pull-down console;
- Node scenario runner;
- CLI;
- CI smoke scenarios;
- coding agents;
- balancing experiments;
- automated regression reproduction.

An interactive console session should be exportable as a durable scenario script. A late-night debugging ritual can then become a regression test rather than evaporating with the process.

## Live authoring through admitted catalog overlays

Runtime VM appeal often comes from editing a definition and seeing the game change immediately. This does not require JavaScript definitions to become canonical.

Use an overlay pipeline:

```text
TypeScript or console-created candidate overlay
        |
        v
bounded package artifact
        |
        v
Rust decode and semantic compilation
        |
        v
candidate canonical catalog
        |
        v
safe-point activation
        |
        v
new Rust-owned catalog epoch
```

Example:

```js
const overlay = await author.overlay({
  baseFingerprint: await author.catalogFingerprint(),
});

overlay.replaceAction(
  "melee-attack",
  actions.melee({ cooldownSeconds: 0.15 }),
);

const candidate = await author.preview(overlay);
console.table(candidate.diagnostics);

await author.activate({
  candidateFingerprint: candidate.fingerprint,
  expectedCatalogEpoch: candidate.baseEpoch,
});
```

The JavaScript overlay is only a candidate value. Rust admits it, compiles it, validates compatibility, and activates the canonical result.

Track at least:

- base package fingerprint;
- overlay fingerprint;
- active catalog epoch;
- origin such as file watcher, console session, or agent;
- compatibility classification;
- rollback target.

### Catalog change classes

| Class | Activation policy |
|---|---|
| `tuning-safe` | May activate while a session runs |
| `rebind-safe` | Rebuild dependent definitions or restart an encounter before activation |
| `session-incompatible` | Requires reset or a new session because IDs, state shape, or persistence assumptions changed |

Console overlays should be session-local unless explicitly exported to source. The durable writable authority remains source files or committed package artifacts.

## Checkpoints and disposable scratch sessions

Single-action preview is insufficient for questions such as:

- “fight this encounter 500 times”;
- “grant these items, advance ten minutes, then attack”;
- “apply this effect to every enemy and inspect the result”;
- “compare two progression curves.”

Provide checkpoint and scratch-session support:

```js
const sim = await session.fork({
  from: "current",
  seed: 81273,
});

await sim.debug.inventory.grant(sim.player.id, "ebony-dagger", 1);
await sim.session.step({ seconds: 10 });

const report = await sim.scenario.repeat(500, async run => {
  await run.reset();
  return run.play.encounter("gallery-orcs");
});

console.table(report.summary);
await sim.dispose();
```

A scratch session is a separate runtime instance restored from an explicit snapshot or fixture.

Rules:

- Do not merge arbitrary scratch state back into the live runtime.
- Do not present a scratch session as a universal atomic transaction.
- Submit deliberate commands or admitted overlays to change the main runtime.
- Fault injection is confined to scratch or explicitly disposable sessions.

A simpler first implementation may support checkpoint, mutate, and restore before true concurrent forks.

## Keep developer console and runtime behavior scripting separate

Even if both eventually execute JavaScript, they need different capabilities and lifecycle contracts.

### Developer console capability

A trusted developer console may receive:

```text
inspect
preview
play
debug
session
author
fault, in test builds
```

### Runtime behavior capability

A shipping behavior script should receive only something like:

```text
immutable facts
explicit bounded behavior state
emit intent
request schedule
structured diagnostic log
```

Do not give behavior scripts grant, teleport, catalog activation, checkpoint, restore, or fault privileges merely because the binding code exists.

Likewise, do not turn build-time authoring modules into runtime behavior modules automatically.

Their contracts are distinct:

```text
authoring module
  pure construction of immutable candidate definitions

behavior module
  bounded runtime invocation with explicit facts, state, outputs, and budget

console module
  trusted developer orchestration over capability APIs
```

They may share generated IDs, DTOs, and builders. They should not share authority or lifecycle by accident.

## Safe future shape for a runtime behavior VM

Complex gameplay may eventually justify runtime scripting. The safer model is fact-to-intent policy, not mutable world objects.

```text
Rust gathers explicit behavior facts
        |
        v
JavaScript policy function
facts + explicit script state
        |
        v
next explicit state + proposed intents + schedule requests
        |
        v
Rust validates, schedules, resolves, and commits
```

Example:

```ts
export function decide(
  facts: GoblinBehaviorFacts,
  state: GoblinBehaviorState,
): GoblinBehaviorDecision {
  if (facts.healthPercent < 25 && !state.hasFled) {
    return {
      state: { ...state, hasFled: true },
      intents: [{ kind: "flee", destination: facts.escapePoint }],
      schedules: [],
    };
  }

  return {
    state,
    intents: [
      { kind: "attemptAction", action: "stab", target: facts.enemy },
    ],
    schedules: [],
  };
}
```

Rust owns and persists `GoblinBehaviorState` as bounded explicit data. The VM heap must not secretly own:

- durable globals;
- quest closures;
- pending timers;
- registered callbacks;
- coroutine stacks;
- references to live entities;
- save-critical Promise chains.

Scripts propose intents and schedule requests. Rust owns consequences, clocks, queues, persistence, and mutation.

This future VM is a separate project with separate acceptance criteria. The developer console should not require it.

## Upstream and downstream ownership

The standard console should be upstream because it provides the shared interaction grammar for common Engine capabilities.

Suggested ownership:

```text
Rusty Engine standard console
  command and query DTOs
  capability descriptors
  common executors
  inspection projections
  generated TypeScript client
  default pull-down console shell

Downstream product
  mounts enabled capabilities
  supplies the runtime-safe command queue
  binds product sessions and target roles
  adds namespaced product commands
  controls developer and shipping availability
```

A possible layout:

```text
rust/crates/developer-command/
rust/crates/gameplay-standard/src/dev/
rules/packages/dev-client/
studio/packages/developer-console/
```

The exact placement should follow existing workspace boundaries. The architectural requirements are more important than these names.

Downstream extensions should mount explicit modules:

```text
standard.mechanics
standard.actions
standard.authoring
product.session
dagger.encounters
dagger.classic-formulas
```

This is explicit capability assembly, not an ambient service locator.

## Build profiles and safety

At minimum, distinguish:

| Profile | Available surfaces |
|---|---|
| production | ordinary product actions only, no console transport |
| developer | inspect, preview, play, debug, session, author |
| test | developer capabilities plus fault injection and scratch-session controls |

Additional constraints:

- bind developer transports to explicit configured interfaces;
- do not expose privileged commands by default in shipping builds;
- include capability manifests in bootstrap replies;
- reject unavailable commands structurally rather than silently ignoring them;
- log command provenance and results;
- bound request sizes, recursion, output records, and execution budgets;
- require explicit permissions for filesystem-writing authoring operations.

## Suggested implementation sequence

### Phase 1: formalize the command port

- Define `DevQuery`, `DevPreview`, `DevCommand`, `CatalogCommand`, and `SessionCommand` families.
- Use closed typed enums and strict DTOs.
- Execute commands on the owning runtime thread at safe points.
- Return structured receipts and revisions.

### Phase 2: generate descriptors and the TypeScript client

- Generate argument and result types.
- Generate capability metadata and autocomplete help.
- Add a transport-independent client core.
- Add browser and Node transport adapters.

### Phase 3: build the pull-down browser console

- Run evaluation in a dedicated worker.
- Expose only the frozen generated capability proxy.
- Add history, autocomplete, formatting, saved snippets, and scenario export.
- Keep ordinary product UI independent of console state.

### Phase 4: add standard gameplay capabilities

Start with:

- entity and mechanics inspection;
- track read, set, spend, and restore through canonical services;
- effect application and removal;
- inventory grant and consume;
- action preview and ordinary intent submission;
- reset and step.

### Phase 5: add catalog overlays and epochs

- Admit candidate standard packages in Rust.
- Classify compatibility.
- Activate at safe points.
- Record provenance and rollback targets.
- Export successful experiments to source rather than persisting hidden VM state.

### Phase 6: add checkpoints and scratch sessions

- Implement checkpoint and restore first.
- Add disposable forks when multi-run simulation and balancing need them.
- Keep fault injection isolated to test sessions.

### Phase 7: add downstream extensions

- Mount Rusty Dagger encounter and classic-formula modules.
- Prove that product-specific commands compose without changing generic Engine enums.
- Reuse the same client from the browser, Node, agents, and CI.

## Acceptance criteria

The proposal is successful when:

- a pull-down console can inspect and manipulate a running downstream game without direct mutable state access;
- all mutations reach named Rust owners and return typed receipts;
- stale handles and revisions reject visibly;
- commands execute only at runtime safe points;
- action preview and apply use the same canonical semantic path;
- the console can run loops and assertions without making its heap authoritative;
- live definition changes pass through Rust admission and catalog epochs;
- console history can export a durable scenario;
- the same generated client works for browser, Node, agents, and CI;
- downstream modules can add product commands without reflection or service location;
- production builds can exclude the privileged transport and console surface;
- no runtime behavior VM is required to deliver the console.

## Non-goals

This proposal does not provide:

- untrusted mod sandboxing;
- arbitrary remote code execution in Rust;
- mutable JavaScript entity proxies;
- universal field patching;
- a generic service invocation bridge;
- a universal heterogeneous world transaction;
- save persistence of the JavaScript heap;
- hidden timers or callbacks;
- a complete runtime gameplay VM.

## Open implementation questions

1. Which existing host should own the upstream pull-down console shell?
2. Should the first protocol use HTTP, WebSocket, a Studio adapter channel, or a transport-neutral core with several adapters?
3. What is the smallest common safe-point queue contract that can remain product-neutral?
4. Should capability descriptors be generated from Rust types directly or from an intermediate schema artifact?
5. How should read-only entity handles expose revisions without encouraging users to treat snapshots as live objects?
6. Which standard commands belong in Engine and which remain product modules?
7. How should catalog compatibility classes be computed and overridden?
8. What snapshot boundary is sufficient for checkpoint and restore before full scratch-runtime forks?
9. Which command history and scenario format should be durable?
10. How should agent access be authenticated and scoped on a developer LAN?
