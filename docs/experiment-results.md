# Experiment results

Status: Rust-owned headless gameplay direction selected on 2026-07-23; product and Asha feature
transplant proofs remain open.

## Current decision

The first comparison overemphasized which language should execute gameplay. The more important
question is whether ordinary gameplay has a direct, legible structural path in any language.

Active `main` now uses this split:

```text
TypeScript content composition
        -> strict stored project definitions
        -> one Rust admission step
        -> concrete entity/component state
        -> direct Rust services and typed committed events
        -> derived projection
        -> eventual TypeScript renderer/UI
```

Rust owns live session state, substantial gameplay logic, service composition, scheduling, and
persistence. TypeScript may use normal functions, loops, helpers, and type checking to generate
content, but it does not own runtime behavior instances, opaque gameplay state, callbacks, or a
second scheduler.

## What the language-host comparison established

The initial milestone implemented the same timed security door through direct Rust services and a
trusted executable TypeScript runtime over N-API. Its observed TypeScript scenario used five
gameplay bridge calls, 981 bytes into Rust, and 3,023 bytes out of Rust.

More importantly, enabling the short TypeScript behavior required:

| Removed runtime surface | Physical source footprint |
|---|---:|
| Generic Rust project-code host | 1,171 lines |
| N-API transport | 188 lines |
| Shared TypeScript boundary/runtime | 291 lines |
| TypeScript door behavior | 138 lines |

That route introduced opaque project state, invocation ownership, duplicated wire DTOs,
serialization, stable-message translation, bridge accounting, and another persistence lifecycle.
The first live test found casing drift inside a tagged Rust enum. These are structural obligations,
not shortcomings in TypeScript syntax.

The comparison was therefore useful negative evidence: moving logic across a language boundary
would relocate the same class of contract and lifecycle burden that the successor is meant to
remove. The complete implementation and its tests remain recoverable at Git tag
`external-ts-runtime-spike` (`9ed75581999291aa622713814d10832e597999d3`). Active `main` deletes
those runtime-host crates and packages.

## Active walking slices

### Security door

```text
Interact
  -> InteractionService
  -> SwitchActivated
  -> DoorService::open
  -> atomic transform/collision change
  -> DoorOpened
  -> optional stable CloseDoor schedule
```

The configured latched variation changes only `auto_close_after`. Save/reopen preserves a pending
close without persisting the diagnostic event journal.

### Encounter-gated exit

TypeScript composes explicit actor, encounter, exit, and enemy definitions. Rust admits those into
concrete `EnemyComponent`, `EncounterComponent`, `DoorComponent`, and relationship data before the
session starts.

```text
DefeatEnemy
  -> CombatService
  -> atomic collision/visibility change
  -> EnemyDefeated
  -> EncounterService
  -> EncounterCleared
  -> DoorService::open(exit)
  -> DoorOpened
```

There is no encounter polling, ambient subscription, dynamic topic, runtime script callback, or
generic rule resolution. `GameRuntime::drain_events` contains the finite route. After the first of
two enemies is defeated, the encounter remains active and the exit remains closed. The second
defeat produces exactly `EnemyDefeated`, `EncounterCleared`, and `DoorOpened` in order.

The one-enemy variation is a second generated project definition using the same TypeScript builder.
It changes no Rust runtime, service, event, persistence, or projection code and clears after the
first committed defeat fact.

Partial encounter progress survives save/reopen. Snapshots store concrete enemy and encounter
state, relationships, doors, and schedules; they do not store event history or replay frames.

## Reproducible evidence

From a checkout with the public Asha donor beside this repository:

```bash
pnpm install
pnpm run verify
cargo run -q -p game-host --bin headless-door
cargo run -q -p game-host --bin headless-encounter
```

The current verification gate proves:

- Rust formatting and strict TypeScript compilation;
- generated project content is byte-for-byte current with its TypeScript composition;
- 2 TypeScript content-composition tests;
- 14 Rust integration tests across the world kernel, security door, content admission, encounter
  routing, atomic rejection, projection, and save/reopen;
- strict rejection of unknown stored-content and snapshot fields.

## Active source footprint

These are physical line counts (`wc -l`), not complexity scores:

| Ownership surface | Production source footprint | Purpose |
|---|---:|---|
| Reusable Rust world kernel | 4 files / 732 lines | Entity/capability storage, atomic world mutation, snapshot, projection. |
| Rust game host and runners | 9 files / 1,684 lines | Concrete components, services, routing, content admission, scheduling, snapshots, two runners. |
| TypeScript content composition | 4 files / 126 lines | Typed definitions, encounter builder, reproducibility check, exports. |
| Generated project content | 2 files / 143 lines | Two-enemy proof and one-enemy variation. |

The Rust snapshot code is currently the largest single structural cost. It is explicit and easy to
trace, but future slices should test whether small typed codec helpers can reduce repetition without
introducing reflection, registries, or generic replay machinery.

## Findings

- The direct Rust service path solves the original discoverability problem without a language
  escape hatch.
- Typed events carry real cross-domain weight while remaining a short closed route.
- TypeScript still provides useful code-as-content ergonomics without participating in the live
  authority loop.
- The only retained command batch has a concrete consumer: a static door must change collision and
  translation atomically, while a defeated enemy changes collision and visibility atomically.
- Batched world reads and expected-revision machinery had no remaining in-process consumer and were
  deleted with the external host.
- The pivot removes substantially more runtime-host plumbing than the encounter slice adds.

## Remaining falsification work

This is not yet evidence for replacing Asha wholesale. The next proofs should focus on structural
reuse rather than language choice:

1. Transplant one substantial Asha spatial/collision or related feature below this runtime without
   importing Gameplay Fabric or the old runtime facade.
2. Add one genuinely new reusable engine capability and confirm its change remains localized.
3. Connect a retained TypeScript/Three/DOM shell through projection and resolved input.
4. Characterize one named real-time multi-entity workload without component-local ticking.
5. Revisit snapshot repetition only after another durable component family establishes the common
   shape.
