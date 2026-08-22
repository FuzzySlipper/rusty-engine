# Standard Gameplay Substrate

Status: idea for implementation discussion

Related proposal: [Runtime developer console and command port](runtime-developer-console.md)

## Summary

Rusty Engine should provide a discoverable, opinionated **standard gameplay substrate** above its neutral mechanism crates.

The standard layer would make the common safe path easy:

- component-backed stats and tracks;
- attributed sources and effects;
- damage and restoration through named services;
- inventory and equipment through canonical owners;
- a bounded action authoring grammar;
- Rust semantic admission and runtime execution;
- standard inspection, preview, receipts, and developer-console capabilities;
- typed downstream extension points for game-specific facts and operations.

This is not a universal game model and not a generic gameplay VM. The goal is to centralize common invariants and tooling while leaving game vocabulary, orchestration, scheduling, persistence, AI meaning, and unusual mechanics downstream.

The working policy is:

> Upstream early. Stabilize late. Standardize invariants. Extend vocabulary. Keep one authority.

## Why the previous promotion rule fails under agent-driven development

A rule such as “wait for two consumers” assumes consumers can discover one another and compare implementations. Independent repository agents do not normally do that. They optimize locally and rarely perform cross-repository architectural surveys.

This creates a deadlock:

```text
A mechanism must be upstream to be visible and reusable by other agents.

But it must be independently rediscovered by other agents before it is
allowed upstream.
```

The practical outcomes are predictable:

- multiple downstream stat, health, action, or inventory dialects;
- duplicated bugs and validators;
- different inspection and developer-tool surfaces;
- later extraction after product-specific assumptions have hardened;
- a permanent human coordination tax for noticing and reconciling drift.

A second product remains useful evidence, but it should refine an upstream abstraction rather than grant permission for the abstraction to exist.

## Upstream ownership and API stability are separate decisions

Moving a mechanism into Rusty Engine should not automatically declare it permanent.

Use maturity levels instead:

| Status | Meaning |
|---|---|
| `incubating` | Engine-owned and preferred over reinvention, but allowed to change substantially |
| `standard` | Recommended default for downstream products |
| `stable` | Compatibility-sensitive contract with deliberate migration policy |
| `specialized` | Engine-owned implementation for a narrower use, opt-in rather than default |
| `product-extension` | Explicitly downstream-owned vocabulary or policy |

This preserves a place for shared refinement without carving every early design into basalt.

## Proposed architectural strata

```text
┌──────────────────────────────────────────────────────────────┐
│ Product gameplay                                             │
│ game-specific formulas, target meanings, AI, death, quests,  │
│ scheduling, save policy, presentation consequences           │
└──────────────────────────────▲───────────────────────────────┘
                               │ typed extensions and bindings
┌──────────────────────────────┴───────────────────────────────┐
│ Standard gameplay substrate                                  │
│ common expressions, actions, mechanics operations,           │
│ admission, inspection, preview, console capabilities,        │
│ presets, generated authoring/client contracts                │
└──────────────────────────────▲───────────────────────────────┘
                               │ composes
┌──────────────────────────────┴───────────────────────────────┐
│ Neutral Engine mechanisms                                    │
│ entity-state, gameplay-mechanics, gameplay-rules,             │
│ gameplay-resolution, spatial and service crates              │
└──────────────────────────────────────────────────────────────┘
```

The neutral mechanism crates should remain austere and independently useful. The standard layer composes them into a strong default instead of forcing each downstream repository to design its own gameplay architecture from loose atoms.

A possible initial layout is:

```text
rust/crates/gameplay-standard/
rules/packages/gameplay-standard-contracts/
rules/packages/gameplay-standard-authoring/
```

The exact crate and package split can evolve. The important dependency direction is:

```text
gameplay-standard
  -> gameplay-rules
  -> gameplay-resolution
  -> gameplay-mechanics
  -> entity-state
```

Neutral mechanism crates must not depend on the standard layer.

## Default path, extension path, and low-level path

Downstream documentation should present three deliberate routes.

### 1. Standard path

Use the standard gameplay substrate for ordinary:

- stats;
- mutable bounded resources;
- modifiers and effects;
- damage and restoration;
- inventory and equipment;
- authored actions;
- inspection and developer commands.

This should be the first page and easiest route.

### 2. Extension path

Add typed product-specific:

- fact readers;
- target roles;
- expressions;
- operations;
- action metadata;
- console modules;
- orchestration around standard attempts.

Extensions compose with the standard core without becoming opaque runtime JSON or arbitrary callback hooks.

### 3. Low-level path

Use the neutral Engine crates directly for a mechanically unusual product whose needs do not fit the standard substrate.

This remains supported, but it should be a conscious architectural choice rather than default homework for every downstream agent.

## Opinionated defaults

The standard stack should answer routine ownership questions consistently:

- Derived numeric values use stats.
- Bounded mutable resources use tracks.
- Modifiers use attributed sources and effects.
- Damage uses the canonical damage service.
- Healing and repair use canonical track restoration.
- Inventory and equipment use their canonical owners.
- Authored actions use the standard action model where it fits.
- Live mutation uses named Rust services.
- Runtime debugging uses typed commands and receipts.
- Product-specific behavior extends these paths rather than shadowing them.

A downstream product may reject a default when its mechanics genuinely differ. The rejection should be explicit and documented.

## Overbuilt internals are acceptable; overbuilt entry paths are not

A simple action game may not require every feature of the mechanics stack, but using its track and damage invariants can still avoid entire bug classes:

- scattered direct health writes;
- UI and gameplay disagreeing about bounds;
- effects bypassing damage policy;
- save state reconstructing invalid values;
- debug tools mutating fields outside normal owners.

The implementation cost has already been paid upstream. The remaining concern is adoption cost.

The standard layer therefore needs concise presets and builders over the same canonical state, not a second simplified implementation.

Illustrative Rust shape:

```rust
let gameplay = StandardGameplay::builder("demo.v1")
    .track("health", fixed_range(0, 100))
    .damage_kind("physical")
    .build()?;

gameplay.spawn()
    .with_track("health", 100)
    .attach(&mut state, actor)?;
```

The result should still be the ordinary admitted mechanics catalog, ordinary registered components, and ordinary service calls.

## Extract capabilities from Rusty Dagger, not files

Rusty Dagger contains a useful first implementation, but its source files mix generic gameplay concepts with Daggerfall-specific vocabulary. Promotion should happen by **capability column** rather than wholesale file movement.

A capability column includes:

```text
TypeScript authoring builder
        |
        v
wire definition
        |
        v
Rust admission and canonical definition
        |
        v
runtime service or evaluator
        |
        v
receipt and inspector projection
        |
        v
standard console query, preview, and command
        |
        v
tests, fixtures, and generated documentation
```

### Strong upstream candidates

| Current concept | Likely standard status |
|---|---|
| constants, add, subtract, multiply, min, max | Standard scalar expression |
| explicit floor and truncating division | Standard deterministic arithmetic |
| stat reads | Standard mechanics expression |
| current and maximum track reads | Standard mechanics expression |
| bounded named rolls | Standard runtime input expression |
| comparison predicates | Standard predicate |
| sequence, conditional, operation | Standard program structure |
| spend and restore track | Standard mechanics operation |
| damage request | Standard mechanics operation |
| apply and remove effect | Standard mechanics operation |
| common inventory changes | Standard mechanics operation where action use is clear |

### Likely Dagger extensions

| Current concept | Likely downstream status |
|---|---|
| equipped weapon skill | Dagger extension, or later neutral equipment-query capability |
| equipped weapon damage roll | Dagger extension initially |
| struck body-part armor | Dagger extension |
| classic body-part table | Dagger data and semantics |
| minimum weapon material | Dagger policy |
| mobile IDs and classic actor fields | Dagger definitions |
| classic loot categories and repeated-roll behavior | Dagger definitions and runtime policy |
| career and Daggerfall progression rules | Dagger definitions and semantics |

The extraction test is not whether a type currently appears in a generic-looking file. It is whether the capability can be stated and tested without product vocabulary.

## Closed standard core with typed extension slots

The standard vocabulary should be closed and versioned. Downstreams extend it through explicit typed leaves.

Illustrative Rust shape:

```rust
pub enum StandardExpr<CustomFact> {
    Constant(MechanicsScalar),
    Stat {
        subject: SubjectRole,
        stat: StatId,
    },
    TrackCurrent {
        subject: SubjectRole,
        track: TrackId,
    },
    TrackMaximum {
        subject: SubjectRole,
        track: TrackId,
    },
    Roll(RollRef),
    Add(Vec<Self>),
    Sub(Box<Self>, Box<Self>),
    Multiply(Vec<Self>),
    Minimum(Vec<Self>),
    Maximum(Vec<Self>),
    Custom(CustomFact),
}

pub enum StandardOperation<CustomOperation> {
    SpendTrack { /* standard fields */ },
    RestoreTrack { /* standard fields */ },
    Damage { /* standard fields */ },
    ApplyEffect { /* standard fields */ },
    RemoveEffect { /* standard fields */ },
    Custom(CustomOperation),
}
```

Engine recursively admits and evaluates standard nodes. Downstream Rust compiles and evaluates typed custom leaves.

Do not use:

- arbitrary method-name bridges;
- reflection registries;
- service locators;
- untyped JSON at runtime;
- a universal operation enum containing every product verb;
- persisted callbacks or closures.

## Generalize subject roles without standardizing target ontology

Do not upstream a fixed `actor | target` union as the universal subject model.

Use declared roles such as:

```text
actor
primaryTarget
source
secondaryTarget
heldItem
```

An authored action declares the roles it requires. Downstream admission binds those role IDs to concrete types.

A standard mechanics operation can require that a role resolve to an entity. A custom operation may accept a door, position, volume, route, or another product-owned target type.

This provides shared expression and action machinery without pretending all games share one target ontology.

## Replace generic evidence with typed inputs

The standard layer should distinguish four input classes:

| Input | Meaning |
|---|---|
| `parameter` | Immutable action or catalog input |
| `fact` | Deterministically gathered from authoritative state |
| `roll` | Supplied nondeterministic value with declared bounds |
| `choice` | Explicit player, AI, or external decision |

Compilation should derive each action’s requirements, for example:

```text
requires actor stat: accuracy
requires primary target track: health
requires roll: melee.hit.d100 [1, 100]
requires parameter: base_damage
```

This improves:

- semantic admission;
- console autocomplete;
- diagnostic errors;
- deterministic test scenarios;
- replay experiments;
- agent understanding of which owner supplies each value.

A generic string evidence bag should not become the standard ABI.

## Authoring contract and Rust authority

The standard authoring flow remains staged:

```text
TypeScript authoring source
        |
        v
normalized standard package artifact
        |
        v
Rust structural and semantic admission
        |
        v
canonical standard definitions plus typed product extensions
        |
        v
Rust runtime state and service-owned mutation
```

Rules:

- TypeScript source is an authoring language, not runtime authority.
- The wire contract should be generated from Rust-owned DTOs where practical.
- Handwritten TypeScript builders and macros sit above generated wire types.
- Direct Rust construction and checked artifact admission must converge on the same canonical definitions.
- New standard serialized node kinds begin in Rust.
- Product macros may compose existing nodes without Engine changes.

## Standard developer console as the executable map

The related [runtime developer console](runtime-developer-console.md) should expose the standard substrate directly:

```js
await inspect.track(entity, "health");
await preview.action({ actor, action: "melee", primaryTarget: target, rolls });
await debug.track.restore(entity, "health", 20);
await debug.effect.apply(entity, "poisoned");
await debug.inventory.grant(entity, "potion", 3);
```

Every command routes through canonical Engine or product services. The console does not patch fields directly.

The console, generated client, command descriptors, API documentation, and agent-facing help should derive from the same capability declarations. This gives isolated agents a discoverable shared surface without relying on cross-repository social contact.

## Proposed promotion criteria

Promote a mechanism upstream when:

1. it can be stated without product-specific fiction or vocabulary;
2. centralized ownership eliminates a meaningful class of correctness, drift, inspection, or tooling problems;
3. it has a clear authority owner, bounded contract, and testable invariants;
4. downstream policy can bind or extend it without Engine knowing the product; and
5. incomplete generality can be represented honestly through an incubating maturity status.

A second consumer strengthens or challenges the design. It is not mandatory permission.

## Suggested implementation sequence

### Phase 1: establish the incubating standard layer

- Add an Engine-owned `gameplay-standard` home.
- Document maturity status and dependency direction.
- Define the standard package identity and versioning policy.
- Keep neutral crates unchanged.

### Phase 2: extract generic program and scalar expressions

- Add standard expression, predicate, subject-role, and program DTOs.
- Generate TypeScript contract types from Rust-owned definitions.
- Add handwritten ergonomic TypeScript builders.
- Adapt Rusty Dagger to compile its current generic subset into standard canonical nodes.

### Phase 3: add standard mechanics operations

Start with:

- spend track;
- restore track;
- damage;
- apply effect;
- remove effect.

Add inventory operations only after their action-level semantics are clear.

### Phase 4: introduce typed product extensions

- Define Dagger-specific fact and operation enums.
- Compile authored extension payloads into typed Rust values.
- Remove duplicate generic evaluator and compiler branches from Dagger after parity.

### Phase 5: provide presets and downstream templates

- Add a minimal action-game profile.
- Add examples using a shooter, a non-character simulation object, and a rules-heavy actor.
- Make the standard path the primary downstream documentation route.

### Phase 6: integrate the standard console

- Generate standard query, preview, and command descriptors.
- Mount them in the upstream console shell.
- Let downstream products add namespaced extension modules.

### Phase 7: use Rusty Engine Demo as a pressure test

Move the demo onto the standard profile deliberately. Its role is ergonomic and conformance pressure, not permission for the abstraction to exist.

## Acceptance criteria

The proposal is successful when:

- a new downstream action game can adopt canonical health, damage, and action paths without designing a private gameplay grammar;
- Rusty Dagger uses standard generic nodes while retaining typed Dagger-specific extensions;
- direct Rust and TypeScript-authored package paths converge on the same canonical definitions;
- no parallel stat, track, inventory, effect, or action state is introduced by the standard layer;
- standard preview and apply traverse the same semantic path;
- standard console commands call named owners and return typed receipts;
- downstream opt-out remains possible without forking neutral mechanism crates;
- incubating APIs can evolve without pretending to be stable contracts.

## Non-goals and constitutional limits

The standard substrate does not own:

- universal attacks, weapons, doors, factions, death, turns, AI goals, or quests;
- the product loop or scheduler;
- complete save and migration policy;
- presentation consequences;
- arbitrary script callbacks;
- mutable JavaScript gameplay objects;
- a universal gameplay VM.

The action layer handles bounded attempts. Product runtimes still own when attempts occur and what committed results mean for the larger game.

## Open implementation questions

1. Should `gameplay-standard` begin as one crate or separate contract, compile, and runtime crates?
2. Which existing Dagger expressions are generic enough for the first extraction without redesign?
3. Should subject roles be interned IDs, a declared package table, or a small standard enum plus extension IDs?
4. How should custom extension payloads be represented on the wire while becoming fully typed during Rust admission?
5. Which mechanics operations belong in the first standard action set?
6. What compatibility promise should `incubating` make to downstream repositories?
7. Should generated TypeScript types come directly from Rust DTO metadata or from an intermediate schema artifact?
8. Which standard presets minimize adoption ceremony without hiding authority or validation?
