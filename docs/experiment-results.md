# Experiment results

Status: headless door comparison complete on 2026-07-23; broader successor hypothesis still open.

This milestone executes the same security-door behavior through two game-code centers over one
host-neutral Rust world kernel:

- named, directly called Rust services with a closed typed event queue;
- trusted executable TypeScript receiving one bounded invocation wave and returning one decision
  batch.

Both variants open a static collidable door, schedule a stable close operation, close at tick 3,
produce committed facts and render projection, support a latched data variation, and survive a
save/reopen point. Neither path uses Gameplay Fabric, replay frames, declared-read contracts,
owner-proof resolution, dynamic event subscription, component ticking, or whole-world cloning.

## Reproducible evidence

From a checkout with the Asha donor beside this repository:

```bash
pnpm install
pnpm run verify
cargo run -q -p game-host --bin headless-door
pnpm run measure:door
```

`pnpm run verify` currently proves:

- Rust formatting;
- 15 Rust integration tests across the kernel, Rust host, and project-code host;
- a loadable N-API addon;
- strict TypeScript compilation;
- 3 live Node-to-Rust behavior tests.

The tests include atomic rejection with no partial state, static-collider movement invariants,
stale revisions, strict snapshot decoding, stable scheduled messages, retry after a rejected
project decision, refusal to snapshot while TypeScript owns an outstanding invocation, wire-case
compatibility, and both door variations.

The Rust runner ends at tick 3 and world revision 2 with the door closed. The measured TypeScript
runner reports the same end state and, for that one open/wait/close scenario:

| Measurement | Observed value |
|---|---:|
| Gameplay bridge calls | 5 |
| Bytes into Rust | 981 |
| Bytes out of Rust | 3,023 |
| World command batches applied | 2 |
| World revisions committed | 2 |
| Engine facts | 4 |
| Project facts | 2 |

The five calls are one interaction invocation, one open decision, two explicit time advances, and
one close decision. Reads and commands within an invocation are batched; there is no call per
component access or per command. Creation, close, and reading the metrics are intentionally outside
that gameplay count.

Warm local test cycles on this machine were 0.07 seconds for `cargo test -q -p game-host` and 0.75
seconds for `pnpm run test:ts`. These are orientation measurements from an already-built checkout,
not portable benchmarks.

## Source footprint and change surfaces

These are physical line counts (`wc -l`), not complexity scores. They make the initial infrastructure
cost visible instead of crediting it to a hypothetical future.

| Ownership surface | Production source footprint | Notes |
|---|---:|---|
| Common world kernel | 4 Rust files / 764 lines | Entities/capabilities, bounded views, atomic commands, snapshots, projection. Shared by both hosts. |
| Direct Rust game host | 6 Rust files / 960 lines | Door/switch data, services, explicit event routing, scheduler, snapshot, runner. |
| External project-code host | 4 Rust files / 1,171 lines | Generic bindings, bounded invocations, state/message ownership, validation/application, snapshot. |
| N-API transport | 1 Rust file / 188 lines | Handle registry and JSON batch transport with byte/call counters. |
| Shared TypeScript boundary/runtime | 3 TypeScript files / 291 lines | Handwritten DTOs, addon loader, and ergonomic runtime wrapper. |
| TypeScript security-door behavior | 1 TypeScript file / 138 lines | Ordinary branching, state decoder, commands, facts, and schedule requests. |

The initial TypeScript route therefore costs materially more infrastructure than the direct Rust
route for one door. That is evidence against selecting it merely because ordinary TypeScript is
pleasant to write. Its promising result is different: once the host exists, the latched variation
is one project-state value and focused expectations, with no Rust kernel, bridge, persistence, or
projection case added.

| Slice/variation | Project code/content | Rust kernel | Bridge | Persistence | Projection | Why |
|---|---|---|---|---|---|---|
| Rust timed door | Rust door/switch model and named services | Shared unchanged | None | Typed Rust snapshot | Shared extraction | Familiar baseline and shortest direct call path. |
| Rust latched door | `auto_close_after_ticks = None` plus test | Unchanged | None | Unchanged | Unchanged | Configuration-only variation. |
| TypeScript timed door | One explicit `securityDoorController` behavior | Shared unchanged | Generic invocation/decision host required | Opaque versioned state plus stable message | Shared extraction | Tests whether project rules can stay outside Engine schemas. |
| TypeScript latched door | `autoCloseTicks = null` plus test | Unchanged | Unchanged | Unchanged | Unchanged | Project configuration-only variation. |

## Architectural findings

The smaller center has found its feet far enough to justify the next falsification slice:

- An entity is legible through its definition, concrete data, relationship, and one named owner.
- The Rust path from interaction to visible state fits in ordinary service/event code and does not
  serialize or replay local work.
- The TypeScript path keeps game-specific branching and state semantics out of Rust. Rust validates
  reusable world invariants once, atomically, without reconstructing the project's intent.
- Stable time messages and opaque project state survive restart without persisting callbacks or an
  event history.
- Projection is derived after commit; neither host mutates presentation state.
- The static-collider invariant is stronger than a naive command-at-a-time API: translation and
  collision participation must be judged over the final atomic batch.

The implementation also exposed costs that should not be smoothed over:

- The first live TypeScript test found a snake_case/camelCase mismatch inside tagged Rust enum
  fields. A focused Rust wire test now covers it, but the 185-line handwritten TypeScript DTO file
  is an obvious drift surface.
- JSON currently copies every invocation and decision. The measured door payload is acceptable for
  a discrete event, but says nothing about real-time or multi-entity pressure.
- The external project-code host is already larger than the direct Rust game host. It must earn that
  cost with a complex project-only change, not another door-shaped example.
- Only three stable Asha foundation crates are referenced. No substantial Asha feature service or
  browser shell has yet demonstrated transplantability.

## Provisional decision

Keep the common kernel and both hosts for the next comparison; do not choose a permanent dual-host
architecture. The direct Rust service route is the simpler default on current evidence. Executable
TypeScript remains credible specifically for project rules whose change amplification would
otherwise spread into Rust Engine contracts.

The next decisive experiment is the encounter-gated exit from Slice 3. It must reuse the existing
generic event/state/relationship boundary and world capabilities while changing only project
TypeScript/content and focused tests. If it requires a new central Rust event enum, bridge method,
or door/encounter-specific host case, the claimed project-code advantage has failed. After that,
one genuinely new reusable capability should test whether Rust-side extension stays narrow.
