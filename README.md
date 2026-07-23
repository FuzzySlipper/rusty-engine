# Rusty Engine

Rusty Engine is an external architecture lab for a smaller, object-centric successor to Asha's
gameplay runtime spine.

The active hypothesis is deliberately conventional:

- Rust owns live entities, component state, gameplay services, scheduling, persistence, and typed
  committed events.
- TypeScript is code-as-content and tooling: it composes project definitions with normal language
  features, then emits strict data admitted by Rust before a session begins.
- Presentation eventually returns to the existing TypeScript/Three/DOM shell through derived
  projection; it does not own gameplay truth.

The architecture charter is [asha-object-centric-successor-spike.md](./asha-object-centric-successor-spike.md).

## Implemented walking spike

The runtime now proves four connected paths:

1. A switch-controlled security door with an optional configured close delay.
2. An encounter-gated exit where committed enemy-defeat facts clear an authored encounter and open
   its related door.
3. A reusable `KinematicCapability` and centrally scheduled `KinematicMotionSystem` over Asha's
   voxel authority and Parry collision projection.
4. A retained browser/Three/DOM product shell where resolved actions enter Rust and typed facts plus
   projection deltas return to Asha's real Three renderer.

Components remain data. `InteractionService`, `CombatService`, `EncounterService`, and
`DoorService` own behavior; `GameRuntime` contains the short explicit event route. World capability
changes are applied atomically only where collision/render invariants require it. Snapshots preserve
durable state and scheduled intents without replaying an event history.

The project definitions under `content/generated/` are reproducibly composed by
`ts/packages/project-content`. Changing the encounter from two enemies to one changes only authored
content and expectations, not Rust gameplay code. The same code-as-content package builds a named
256-body voxel-wall workload without adding a TypeScript runtime authority.

The loading-bay browser lets a player defeat both enemies and run a bounded spatial phase. Its
Chromium smoke proves the resulting `EnemyDefeated -> EncounterCleared -> DoorOpened` chain and a
visible kinematic probe stopped by authored voxel collision.

## Donor checkout

Mature Asha foundation crates are referenced rather than copied. The repository expects this sibling
layout:

```text
parent/
  asha-engine/
  rusty-engine/
```

The referenced Asha snapshot and paths are recorded in
[docs/donor-provenance.md](./docs/donor-provenance.md).

## Verification

```bash
pnpm install
pnpm run verify
```

To update generated project content after changing its TypeScript composition:

```bash
pnpm run generate:content
```

Run the two headless paths directly with:

```bash
cargo run -q -p game-host --bin headless-door
cargo run -q -p game-host --bin headless-encounter
```

Run the product shell locally with:

```bash
pnpm run build:shell
cargo run -q -p game-host --bin browser-host
```

Then open `http://127.0.0.1:37881`. Run the measured real-time matrix with:

```bash
cargo run --release -q -p game-host --bin motion-workload -- --matrix
```

The measured evidence and current limitations are in
[docs/experiment-results.md](./docs/experiment-results.md).

## Archived language-host comparison

The initial experiment also implemented equivalent trusted TypeScript runtime behavior through a
batched N-API host. It usefully demonstrated that changing language did not remove the structural
costs of a second authority lifecycle. That implementation is preserved at the Git tag
`external-ts-runtime-spike`; it is intentionally absent from active `main`.
