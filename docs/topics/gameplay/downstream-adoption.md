# Downstream gameplay adoption

This guide is the practical starting point for a downstream game that wants to
use Rusty Engine's gameplay capabilities. It covers new games and existing
repositories that are being simplified or migrated.

The central rule is:

> Decide which product and gameplay concepts survive before moving any code.
> Delete obsolete surfaces first; author and migrate only retained gameplay.

Rusty Engine provides optional mechanisms. It does not prescribe one game
model, one rules language, or one directory full of inherited sample code.
Downstream Rust owns game meaning, live authority, orchestration, and
persistence. Optional build-time TypeScript makes retained definitions easier
to compose; it does not execute gameplay.

This document composes the canonical boundaries in
[design](../../design.md), the
[gameplay mechanics](../../code-map/gameplay-mechanics.md),
[gameplay rules](../../code-map/gameplay-rules.md), and
[gameplay resolution](../../code-map/gameplay-resolution.md) code maps, and the
[gameplay rules contract](../../gameplay-rules-contract.md). Those documents
remain authoritative for their individual Engine surfaces.

## Start with the product, not the old code

An existing repository often contains experiments, clean rooms, abandoned
demos, duplicate schemas, proof-only adapters, and tests for features the
product no longer needs. Framework adoption is not a reason to preserve them.

Before designing a package or moving a module, write a short disposition table:

| Existing surface | Product decision | Gameplay disposition | Reason |
|---|---|---|---|
| Current playable product path | keep | migrate retained definitions and authority | It is the product |
| Old demo or test room | delete | do not port | It no longer serves the product |
| Shared gameplay mechanism | keep | use an Engine capability if it fits | Avoid parallel machinery |
| Game-specific rule or action | keep | author downstream and execute in downstream Rust | It carries game meaning |
| UI value editor duplicating gameplay fields | delete or make read-only | source definitions from Rust readout | It must not remain a second authority |
| Import, renderer, collision, or host code | keep only if the product needs it | do not move into gameplay authoring | It has a different owner |

Remove surfaces marked `delete` and their exclusive tests, scripts, manifests,
and documentation before doing a broad gameplay move. If deletion and migration
must overlap to keep the checkout buildable, keep one explicit checklist and do
not improve code already marked for deletion.

For each retained concept, classify its destination:

1. **Authored definition:** immutable configuration or content composed at
   build time. It may belong in downstream TypeScript.
2. **Downstream Rust semantics:** decoding, semantic compilation, policy,
   formulas, target meaning, facts, scheduling, and orchestration.
3. **Reusable Engine mechanism:** stats, tracks, effects, items, damage,
   resolution lifecycle, entity state, spatial queries, and other named
   services used through the Rust facade.
4. **Runtime authority:** current world and actor state, clocks, queues, save
   policy, and mutations. These remain in Rust owners.
5. **Presentation:** HUD, animation selection, rendering, and explanation UI.
   These observe gameplay; they do not define it.

Do not classify a constant as authored gameplay merely because it is a
constant. A collision tolerance may belong to a spatial owner; an animation
frame rate may belong to presentation; an attack cost probably belongs to an
authored action.

## Choose only the capabilities the game needs

The gameplay crates are siblings. A downstream may use any combination.

| Need | Capability | Engine owns | Downstream still owns |
|---|---|---|---|
| Component-backed numeric and item state | `rusty_engine::gameplay_mechanics` | Stats, tracks, attributed sources, effects, damage/restoration, inventory, unique items, equipment, validation, and receipts | Identifier meanings, attacks, timing, consequences, saves, and orchestration |
| Package build-time definitions with provenance | `rusty_engine::gameplay_rules` | Strict bounded opaque envelopes, exact dependencies, canonical bytes, fingerprints, provenance, and diagnostics | Payload schema, semantic compiler, publication, persistence, and execution |
| One observable fail-atomic attempt | `rusty_engine::gameplay_resolution` | Admit/gather/check/plan/intercept/stage/commit lifecycle, bounded structural traversal, correlation, evidence, receipts, and traces | Intent, facts, predicates, operations, effects, events, transactions, targets, scheduling, and meaning |
| Typed object facts and component storage | `rusty_engine::entity_state` | Entity invariants, typed component registration, exact-slot mutation, relationships, and snapshots | Entity roles, spawn policy, gameplay services, and complete save policy |

Common choices are:

- A simple action game may use mechanics directly and ignore rules packages.
- A data-heavy game may use rules plus mechanics without using the standard
  resolution lifecycle.
- A game with inspectable actions may use mechanics and resolution with direct
  Rust definitions and no TypeScript.
- A rules-heavy game may use all three.

`gameplay_rules` is not an interpreter above `gameplay_mechanics`.
`gameplay_resolution` does not call either sibling. Downstream Rust composes
them.

## Recommended downstream layout

For a new repository, use one visibly bounded gameplay home with unambiguous
Rust and authoring subtrees:

```text
gameplay/
  Cargo.toml                 # downstream Rust gameplay crate
  src/
    lib.rs
    authored.rs              # decoded candidate DTOs only
    catalog.rs               # canonical game definitions
    compile.rs               # semantic compiler and diagnostics
    mechanics.rs             # MechanicsCatalog and component binding
    resolution.rs            # intents, facts, policy, transactions, receipts
    decision.rs              # optional AI/player decision-to-intent boundary
  authoring/                 # optional build-time TypeScript workspace
    package.json
    tsconfig.json
    src/
      grammar/               # game-owned types and builders
      catalogs/              # everyday content/rule editing surface
      packages/              # package composition roots
    scripts/
      materialize.mjs        # small deterministic build plumbing
    dist/                    # ignored compiler output
data/
  gameplay/                  # committed canonical package artifacts
product/                     # host, runtime loop, persistence, rendering, UI
```

The exact folder names are not an Engine API. The important distinctions are:

- `gameplay/src` above is Rust authority;
- `gameplay/authoring/src` is optional build-time TypeScript;
- `data/gameplay` contains immutable exchanged artifacts; and
- product UI TypeScript lives elsewhere.

Existing repositories do not need a cosmetic move solely to match this tree.
Rusty Dagger predates this recommendation: its TypeScript is rooted at
`gameplay/src`, while its Rust authority is in `crates/dagger-rpg`. That is
valid because the two owners are still explicit. New repositories should avoid
using the same unqualified `gameplay/src` name for different languages.

Do not put gameplay catalogs under an Angular, HUD, browser-shell, renderer, or
Studio application. Do not make the product shell the semantic compiler.

## Preserve four inspectable representations

A rules-heavy authored path has four distinct forms:

```text
TypeScript authoring source
        |
        v
normalized package artifact
        |
        v
canonical downstream Rust definitions
        |
        v
live state, attempts, receipts, and events
```

### 1. TypeScript authoring source

Downstream builders, functions, loops, and catalogs provide authoring
ergonomics. This source is not canonical and never runs in the product.

### 2. Normalized package artifact

Materialization emits immutable JSON with a downstream-owned payload inside a
`gameplay-rules` envelope. The artifact is committed when the product needs it
at runtime and checked for drift in development.

The schema-1 shared envelope accepts JavaScript-safe integers, not fractional
JSON numbers. Encode exact ratios, fixed-point quantities, milliseconds, or
domain decimal strings explicitly and validate them in downstream Rust.

### 3. Canonical downstream Rust definitions

Engine envelope admission proves only that the package is bounded and
structurally valid. Downstream Rust decodes the opaque payload, rejects unknown
or inconsistent meanings, resolves references, applies quotas, compiles
programs, and constructs any `MechanicsCatalog`. Only this canonical result may
enter live gameplay.

### 4. Runtime state and operations

Ordinary Rust components and named services own current state. Resolution
policies gather facts and stage typed effects. Capability owners commit state.
Receipts, semantic events, and traces are read-only evidence, not another state
store.

Never skip directly from TypeScript objects to mutable runtime state. Never
make the normalized JSON payload the live object model merely to avoid writing
the semantic compiler.

## Author a small downstream grammar

The authoring grammar should contain only shapes earned by the game. Catalogs
may use the grammar but may not invent new node kinds privately.

An illustrative action-game grammar might look like this:

```ts
export type Subject = "actor" | "intentTarget";

export type Expression =
  | Readonly<{ kind: "constant"; value: number }>
  | Readonly<{ kind: "stat"; subject: Subject; stat: string }>
  | Readonly<{ kind: "evidence"; id: string }>;

export type Predicate = Readonly<{
  kind: "lessThanOrEqual";
  left: Expression;
  right: Expression;
}>;

export type Operation =
  | Readonly<{ kind: "spendTrack"; track: string; amount: Expression }>
  | Readonly<{
      kind: "damage";
      target: Readonly<{ kind: "intentTarget" }>;
      amount: Expression;
    }>;

export type Program =
  | Readonly<{ kind: "sequence"; steps: readonly Program[] }>
  | Readonly<{
      kind: "when";
      predicate: Predicate;
      thenProgram: Program;
      otherwiseProgram?: Program;
    }>
  | Readonly<{ kind: "operation"; operation: Operation }>;

export type ActionDefinition = Readonly<{
  id: string;
  tags: readonly string[];
  program: Program;
}>;
```

Catalog source should read primarily as data:

```ts
export const actions = [
  action(
    "arc-bolt",
    ["spell", "ranged"],
    sequence(
      operation(spendTrack("mana", constant(4))),
      when(
        lessThanOrEqual(evidence("arc-bolt.hit.d100"), stat("actor", "focus")),
        operation(damage(intentTarget(), evidence("arc-bolt.damage"))),
      ),
    ),
  ),
] as const satisfies readonly ActionDefinition[];
```

One package composition root combines catalogs and source records:

```ts
export const corePackage = composePackage({
  packageId: "core",
  version: 1,
  sources: {
    stats: "gameplay/authoring/src/catalogs/stats.ts",
    actions: "gameplay/authoring/src/catalogs/actions.ts",
  },
  payload: {
    schemaVersion: 1,
    stats,
    actions,
    actors,
    items,
  },
});
```

`composePackage` and the grammar above are downstream code. Engine owns only
the semantic-neutral envelope contract. A new game operation therefore changes
the downstream TypeScript grammar and downstream Rust compiler together; it
does not require an Engine enum edit.

Materialization should:

1. import package composition roots;
2. normalize them deterministically;
3. emit canonical envelope bytes;
4. write only declared paths under `data/gameplay`; and
5. support a drift command that rebuilds and compares committed artifacts.

The materializer is build plumbing. Do not let an `.mjs` script acquire
semantic validation that belongs in Rust.

## Compile packages in downstream Rust

Keep authored DTOs separate from canonical definitions. Reject unknown fields
and put explicit bounds on every downstream collection and recursion depth.

The rough flow is:

```rust
pub fn compile_gameplay_package(bytes: &[u8]) -> Result<GameCatalog, GameError> {
    let package = rusty_engine::gameplay_rules::decode_canonical_rule_package(bytes)?;
    require_expected_domain_and_package(package.identity())?;

    let authored: AuthoredGameplayPayload =
        serde_json::from_value(package.payload().clone())?;
    require_schema(authored.schema_version)?;
    enforce_downstream_quotas(&authored)?;

    let stats = compile_stats(&authored.stats)?;
    let mechanics = compile_mechanics_catalog(&stats, &authored.items)?;
    let actions = compile_actions(authored.actions, &stats, &authored.items)?;
    let actors = compile_actors(authored.actors, &stats, &actions)?;

    Ok(GameCatalog {
        package_fingerprint: package.fingerprint().to_string(),
        stats,
        mechanics,
        actions,
        actors,
    })
}
```

Compilation is where the game proves facts the envelope cannot know:

- every action, stat, item, and actor identity is valid and unique;
- references resolve to the expected downstream kind;
- expression and program shapes obey downstream limits;
- an operation is legal for its target/reference shape;
- authored values fit downstream ranges and units; and
- the result can bind to required Engine mechanisms.

Return source-correlated diagnostics when possible. Do not install a partial
catalog after an error.

## Bind neutral concepts to gameplay mechanics

Use `gameplay-mechanics` where its model fits without translating game meaning
upstream. The downstream compiler maps authored game vocabulary into neutral
definitions:

```rust
fn compile_mechanics_catalog(
    authored: &AuthoredStats,
) -> Result<MechanicsCatalog, GameError> {
    let stats = authored
        .stats
        .iter()
        .map(compile_stat_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let tracks = authored
        .tracks
        .iter()
        .map(compile_track_definition)
        .collect::<Result<Vec<_>, _>>()?;

    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: downstream_catalog_version()?,
        stats,
        tracks,
        sources: compile_sources(authored)?,
        damage_kinds: compile_damage_kinds(authored)?,
        effects: compile_effects(authored)?,
        capacity_metrics: compile_capacity_metrics(authored)?,
        items: compile_items(authored)?,
        equipment_slots: compile_equipment_slots(authored)?,
    })
    .map_err(GameError::MechanicsCatalog)
}
```

The game decides that `health` is a track, `focus` is a stat, `fire` is a
damage kind, or `mass` is an inventory capacity. Engine does not attach those
meanings to the IDs.

Live actors attach only the components they need. Game services call
`StatService`, `TrackService`, `DamageService`, `EffectService`,
`InventoryService`, `EquipmentService`, and `ItemService` directly.

Do not retain a second `GameActorState { health, mana, inventory }` beside
mechanics-backed state after migration. Keep a downstream component only when
the neutral mechanism genuinely does not fit, and record why.

Mechanics services own their advertised operation atomicity. They do not own a
complete action, combat session, turn, tick, or save.

## Resolve attempts without standardizing game meaning

Use `gameplay-resolution` for a bounded attempt whose admission, facts,
planning, staged effects, one commit, and explanation should share a lifecycle.
Do not route every service call or simulation tick through it.

Targets remain concrete downstream values:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetRef {
    Actor(ActorId),
    Door(DoorId),
    Position(WorldPosition),
}

pub struct ActionIntent {
    pub actor: ActorId,
    pub action: ActionId,
    pub target: TargetRef,
    pub origin: IntentOrigin,
}

pub enum IntentOrigin {
    Player,
    Ai { controller: EntityId },
    Scripted,
}
```

The reference identifies a target. The policy gathers immutable facts needed
by this attempt:

```rust
pub struct ActionFacts {
    pub actor: ActorFacts,
    pub target: TargetFacts,
    pub distance: Distance,
    pub line_of_sight: bool,
    pub action: ActionDefinition,
}
```

Do not pass an arbitrary mutable target object into the resolver. Spatial,
inventory, mechanics, or game-owned services resolve typed identities and
produce facts. This keeps data access and mutation visible.

The downstream policy supplies every generic parameter:

```rust
impl ResolutionPolicy for GamePolicy<'_> {
    type RawIntent = ActionIntent;
    type Intent = AdmittedActionIntent;
    type Facts = ActionFacts;
    type Predicate = GamePredicate;
    type Operation = GameOperation;
    type Effect = GameEffect;
    type Event = GameEvent;
    type Evidence = GameEvidence;
    type Interceptor = GameInterceptor;
    type TraceDetail = GameTraceDetail;
    type Rejection = ActionRejection;
    type Fault = ActionFault;
    type Suspension = PendingChoice;

    // admit, gather, check, plan, evaluate_predicate, plan_operation,
    // interceptors, and before_commit remain game-owned methods.
}
```

Authored actions compile into Engine's small structural
`Program<GamePredicate, GameOperation>` grammar: `Sequence`, `When`, and opaque
`Operation`. Selectors are part of downstream operations or facts; the second
consumer deliberately removed an Engine-owned `ForEach`/selector abstraction.
See the
[two-beat overfitting report](../../reviews/gameplay-resolution-two-beat-overfitting.md).

Operations plan typed effects rather than mutating state:

```rust
pub enum GameEffect {
    SpendTrack {
        actor: ActorId,
        track: TrackId,
        amount: MechanicsScalar,
    },
    DamageActor {
        target: ActorId,
        request: DamageRequest,
    },
    OpenDoor {
        door: DoorId,
    },
}
```

The downstream `ResolutionTransaction` stages these effects without touching
authority. Its `commit` validates and publishes the complete candidate once or
returns an error without mutation. The implementation may use a downstream
candidate-state clone, narrow prepared replacements, or another explicit
fail-atomic strategy appropriate to its state owners. Preview follows the same
policy and staging path, then aborts.

Do not call a mutating mechanics service against authoritative state from
`stage`. Do not use the resolver as permission to hand a policy unrestricted
mutable world access.

## Make player and AI converge on intents

AI should decide what to attempt, not privately resolve gameplay consequences.

```text
world and perception services
        |
        v
game-owned decision facts
        |
        v
selected typed intent
        |
        v
the same admission / resolution / mechanics path used by player intents
```

Perception, pathfinding, collision, line of sight, timing, and scheduling remain
in their owning Rust systems. Authored AI data may describe goals, candidate
actions, requirements, weights, or thresholds. Downstream Rust admits and
evaluates that data.

An AI output may be `MoveTo`, `Face`, `Wait`, `Flee`, or
`AttemptAction(ActionIntent)`. It must not contain resolved damage, health
deltas, inventory mutation, or an already-applied event. A patrol or behavior
module may own locomotion decisions while still routing an attack attempt into
the shared action policy.

Do not introduce a universal GOAP or behavior-tree schema merely to satisfy
this separation. First make the local fact-to-intent boundary explicit and
traceable. Promote a smaller decision mechanism only after mechanically
different consumers reveal the same need.

## Keep scheduling, saves, and presentation downstream

The gameplay crates do not own:

- the product loop or fixed tick;
- turns, phases, cooldown advancement, effect expiration, or queued work;
- perception, collision, navigation, hit geometry, or target discovery;
- input mapping or AI scheduling;
- complete save and migration policy;
- animation playback, HUD, renderer state, or sound; or
- an ambient event bus.

The product runtime chooses when to resolve an intent and supplies explicit
evidence such as random draws or external decisions. It consumes typed semantic
events after a committed result and routes them to presentation or subsequent
game-owned consequences.

Persist authoritative components and downstream scheduling state through their
own codecs. Receipts and traces are normally diagnostic records, not a promised
replay format.

## Explain without creating another authority

A useful read-only explorer can relate:

- TypeScript source and package provenance;
- normalized package payload;
- canonical Rust definition;
- admitted actor/component state;
- raw and admitted intent;
- gathered facts and supplied evidence;
- predicate decisions and interceptor changes;
- planned and committed effects;
- mechanics receipts and semantic events; and
- rejection/fault identity and phase trace.

The UI should render these records from Rust-owned readouts. It should not
duplicate the authoring grammar, evaluate expressions, or mutate individual
runtime fields. Authoring happens in TypeScript source; runtime debugging uses
explicit product commands or fixtures, not a generic value editor.

## Remove the old paths after the new path is live

A migration is incomplete when the new catalog exists but callers still prefer
old constants or schemas.

For each migrated concept:

1. switch production reads to the canonical Rust definition;
2. switch live values to the chosen component/capability owner;
3. switch player, AI, and diagnostics to the same action path;
4. remove superseded Rust defaults and parallel state;
5. remove duplicate project/JSON fields or make one explicitly derived;
6. remove writable UI forms and mirrored validators;
7. update save/project migration deliberately; and
8. retain a read-only explanation route.

Temporary compatibility must have one named owner, end condition, and test. A
fallback that silently makes the old hardcode authoritative is not migration.

## Applying this to a demo cleanup

A demonstration repository should read like a small product plus a clear
gameplay owner, not like an archive of every experiment used to build it.

Use this order:

1. name the one product/demo that survives;
2. inventory which modules, projects, assets, scripts, tests, and apps that
   product actually reaches;
3. delete retired demos, clean rooms, proof-only UI, and their exclusive
   verification machinery;
4. move the retained Rust gameplay modules into the gameplay owner;
5. adopt Engine mechanics/resolution where they clearly replace retained local
   machinery;
6. add TypeScript authoring only for retained definitions that benefit from
   expressive composition;
7. keep shell, project admission, persistence, import, rendering, and host code
   outside the gameplay crate; and
8. prove the surviving product end to end.

Do not port an old inventory, weapon, encounter, or AI subsystem merely because
the migration task listed it before the product was narrowed. Conversely, do
not use cleanup as a reason to leave retained gameplay in a monolithic shell.

## When to extend Engine

Keep a missing concept downstream when it names or assumes the game. Examples
include target categories, attacks, spells, weapons, doors, hostile/friendly
meaning, AI goals, turns, and death consequences.

Consider Engine work only when:

1. the missing behavior can be stated without downstream vocabulary;
2. it is a mechanism rather than a product policy;
3. the current Engine owner cannot express it without a parallel authority;
4. two mechanically different consumers exercise it; and
5. the second consumer is allowed to shrink or reject the proposed abstraction.

Do not add a method-name bridge, reflection registry, service locator, universal
operation enum, or generic script VM as a shortcut.

## Verification checklist

Choose checks proportionally, but establish evidence at every changed border.

### Authoring and packages

- TypeScript typecheck passes.
- Materialization is deterministic and its drift check passes.
- Engine envelope decoding rejects malformed, oversized, or non-canonical
  artifacts as intended.
- Downstream semantic compilation rejects unknown kinds, unresolved
  references, invalid units, excess depth/nodes, and duplicate identities.
- Direct Rust construction or checked artifact admission requires no runtime
  Node process.

### Mechanics and state

- Catalog admission and component registration are tested.
- Preview/apply and failure non-mutation are tested at named service borders.
- Snapshot/project migration is explicit and can reopen retained gameplay.
- No parallel stat, track, inventory, equipment, or effect store remains unless
  documented as a deliberate downstream owner.

### Resolution and decisions

- Player and AI origins reach the same policy for the same action.
- Randomness and external choices arrive as explicit evidence.
- Preview and apply traverse the same path.
- Rejection, quota failure, staging failure, and commit failure do not mutate
  authority.
- Receipts expose facts, effects, events, and trace details useful for
  explanation.

### Product

- Focused headless tests pass first.
- The downstream repository's ordinary Rust and TypeScript gates pass.
- A real supported product path exercises representative retained gameplay.
- Deleted surfaces have no dangling scripts, manifests, docs, or CI jobs.
- Checks skipped as irrelevant are named with reasons.

## Migration completion checklist

A downstream conversion is complete when all applicable answers are `yes`:

- Was the surviving product scope decided before migration?
- Were obsolete surfaces deleted rather than ported?
- Is every retained gameplay definition owned in exactly one writable place?
- Is TypeScript build-time authoring only?
- Does Rust structurally and semantically admit the package?
- Are canonical definitions distinct from authored DTOs and live state?
- Are neutral stats/tracks/effects/items/damage bound to
  `gameplay-mechanics` where it fits?
- Do bounded attempts use `gameplay-resolution` without making it the game
  loop?
- Are target references, facts, operations, effects, events, and AI meanings
  downstream-owned?
- Do player and AI produce intents into shared resolution paths?
- Are runtime mutation, scheduling, persistence, and presentation still owned
  by explicit Rust/product capabilities?
- Were superseded constants, schemas, state stores, and writable UI contracts
  removed?
- Can a read-only diagnostic explain what happened and why?
- Does the surviving product still work through its real controls?

## Worked evidence and further reading

Rusty Dagger is the clearest current worked example, particularly
`gameplay/src/{authoring,catalogs,packages}`,
`crates/dagger-rpg/src/resolution`, and
`docs/gameplay-resolution.md` in that repository. It demonstrates the model;
its RPG vocabulary and exact directory history are not a template API.

The realtime Doom hitscan consumer proved that the resolution seam must not
assume RPG selectors or target collections. The resulting changes and residual
risks are recorded in the
[two-beat overfitting report](../../reviews/gameplay-resolution-two-beat-overfitting.md).

Use these Engine-owned references for exact APIs and limits:

- [Gameplay mechanics](../../code-map/gameplay-mechanics.md)
- [Gameplay rules](../../code-map/gameplay-rules.md)
- [Gameplay resolution](../../code-map/gameplay-resolution.md)
- [Gameplay rules contract](../../gameplay-rules-contract.md)
- [Gameplay mechanics campaign closeout](../../gameplay-mechanics-campaign-closeout.md)
- [Inspection and diagnostics](../../inspection-and-diagnostics.md)
