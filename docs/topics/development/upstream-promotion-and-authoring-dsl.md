# Upstream promotion and TypeScript authoring DSL

Rusty Engine deliberately centralizes reusable, host-neutral mechanisms. A
downstream product deliberately retains game vocabulary, authored
compositions, orchestration, consequences, and acceptance. This boundary is
decided from ownership and risk, not from a consumer-count threshold.

## Promote deliberately, not accidentally

Do not wait for two consumers, two named products, or an informal cross-repo
encounter before considering Engine ownership. Agent-owned downstream
repositories normally evolve in isolation; they do not discover shared needs
unless an architect, the user, or a deliberate survey connects them.

One credible downstream implementation, one concrete product need, or an
explicit architecture decision can justify upstream work when all of these are
true:

1. the proposed mechanism can be stated without importing the game's nouns,
   balance, content identities, or consequences;
2. it has a clear existing or proposed Engine owner and a bounded public API;
3. centralizing it removes parallel authority, prevents likely correctness
   drift, or establishes one canonical way to perform a risky operation;
4. downstream products can retain their own policy and orchestration without
   callbacks, service location, a generic method bridge, or an ambient event
   bus;
5. focused provider evidence can prove the mechanism independently; and
6. any relevant downstream check is selected explicitly for behavior that the
   provider cannot prove alone.

A cross-repository survey is useful evidence. It may reveal existing copies,
shrink an API, or identify a better owner. It is never a prerequisite for
promotion. Likewise, additional consumers may challenge and improve a public
surface later; they are not permission to create it in the first place.

The promotion decision belongs to explicit architecture judgment. An agent
working in a downstream repository should create or route an Engine task when
the smaller reusable owner is clear. It should not keep a local substitute
merely because no other downstream agent has independently reported the same
need.

## Bias toward canonical correctness mechanisms

Prefer an existing Engine mechanism even when it is more capable than the
downstream game strictly needs. Stats, tracks, attributed sources, effects,
inventory, items, equipment, damage, restoration, attempt resolution, typed
entity components, spatial admission, and fail-atomic mutation already encode
validation, revision, attribution, snapshot, and failure behavior that local
shortcuts often rediscover incorrectly.

This is not abstraction for its own sake. It is reuse of a paid correctness
cost. A small action game may use the same track and damage services as an RPG
so health is not mutated through unrelated ad hoc paths. A product still owns
what damage means, when it happens, and what death causes.

Promoting reusable authoring helpers follows the same rule. Engine may provide
neutral builders, generated wire contracts, or DSL facades for Engine-owned
mechanisms when that gives downstream authors one clear and validated route.
Game-specific operations and content vocabulary stay downstream.

## Four gameplay representations

An authored gameplay path has four inspectable forms:

```text
optional TypeScript authoring DSL
        |
        v
authored package / wire AST
        |
        v
canonical Rust gameplay definitions
        |
        v
live Rust state, attempts, receipts, and events
```

### TypeScript authoring DSL

The optional DSL is a build-time source language embedded in TypeScript. It
may use typed constructors, pure macros, imports, loops, and catalog
composition to author immutable declarations. It does not run in the product,
gather live facts, schedule work, or mutate authority.

Calling this source an authoring DSL is intentional. It is more expressive
than a passive configuration file, but it is not the canonical gameplay model
or a runtime scripting authority.

Keep three neighborhoods visible in a substantial DSL:

- **syntax** provides thin constructors for admitted serialized nodes;
- **macros** are pure TypeScript helpers that expand entirely into existing
  syntax; and
- **catalogs** mostly declare game content through syntax and macros.

The exact directories are downstream choices. The ownership distinction is
not.

### Authored package / wire AST

Materialization produces bounded immutable data. The package is an exchange
representation, not mutable runtime state. Engine may own a semantic-neutral
envelope or generated contracts while the downstream domain owns any
game-specific payload grammar.

### Canonical Rust gameplay definitions

Rust performs structural and semantic admission, resolves references and
units, applies quotas, and compiles the authored package into canonical owned
definitions. New serialized meaning begins here: define and test the Rust
semantics before exposing a TypeScript constructor or macro.

Direct Rust construction remains a first-class path. Novel mechanics do not
need to enter the DSL before their semantic shape has stabilized.

### Live Rust authority

Rust gathers facts, admits intents, supplies or verifies bounded random
samples, plans and commits effects, advances time, schedules work, persists
state, and publishes receipts. TypeScript source and wire data never become a
second gameplay authority.

## Daily placement rules

Use these defaults:

| Change | Default owner |
|---|---|
| New catalog instance or tuned values | TypeScript authoring DSL |
| Pure helper that lowers to existing nodes | TypeScript DSL macro |
| New serialized node, unit, target meaning, fact class, sample class, operation, or evaluator behavior | Rust semantics first, then an optional DSL facade |
| Live fact gathering, query, mutation, scheduling, AI decision, or persistence | Downstream Rust or the named neutral Engine service |
| Novel one-off mechanic without demonstrated authoring variation | Direct downstream Rust definition |
| Neutral mechanism with clear ownership or correctness value | Engine candidate; no consumer-count gate |

The shortest useful heuristic is:

> Authored variation goes in the DSL. New meaning begins in Rust. Live
> authority stays in Rust.

Adding a catalog row is ordinary. Adding a pure macro is local. Adding a new
serialized `kind` is an architectural change.

## What promotion must not become

Proactive upstream ownership does not authorize a universal game ontology,
one closed catalog of every game operation, a generic gameplay VM, an ambient
script runtime, a universal scheduler, or a plugin/service-locator bridge.

Engine can standardize mechanisms and authoring building blocks without
standardizing every game's vocabulary. Downstream games continue to own
attacks, spells, weapons, doors, target categories, AI goals, turn meaning,
death consequences, authored compositions, and product policy unless a
smaller neutral mechanism is explicitly promoted.
