# Rusty Engine

Rusty Engine is a standalone, host-neutral provider for object-centric games.
It owns reusable entity, spatial, collision, navigation, voxel, asset,
authoring, persistence, diagnostics, offline-conversion, and retained-rendering
mechanisms. Downstream games own live gameplay state, substantial game logic,
product orchestration, storage policy, and game-specific presentation.

The architectural shorthand is:

> Objects carry typed facts. Named services own mechanisms. Downstream games
> own meaning and orchestration.

Components are mostly data. Engine code uses direct, explicit services rather
than ambient subscriptions, callback-driven components, service location, or a
universal gameplay runtime.

## Repository posture

This repository is the canonical Rusty Engine provider. Asha is historical
evidence and a donor source, not a compatibility target, runtime dependency, or
required mental model.

Downstream repositories sit beside this checkout and consume the complete
`../rusty-engine/rust/crates/rusty-engine` facade exactly as it stands. Engine
does not inspect or mutate a sibling demo checkout during ordinary work.

Repository docs describe committed architecture and implementation surfaces.
Den may hold current planning, review packets, and decisions when available,
but the repository is intentionally navigable without Den access. Start with
[the canonical design](docs/design.md) and the
[agent code atlas](docs/agent-code-atlas.md). The exact reviewed realtime,
builder, and Rusty D20 evidence for the completed gameplay-mechanics campaign
is indexed in the
[campaign closeout](docs/gameplay-mechanics-campaign-closeout.md).

## Provider boundary

```text
downstream game policy and orchestration
             |
             +--> entity-state / state-machine
             +--> gameplay-mechanics
             +--> gameplay-resolution
             +--> gameplay-rules <--> isolated rules/ authoring workspace
             +--> environment-authoring --> authored-scene
             +--> content-store / asset-catalog / asset-import
             +--> engine-spatial --> core-* / svc-*
             +--> voxel-asset / voxel-convert / voxel-object-runtime
             +--> render-model --> render-projection --> render-presentation
                                      |
                                      +--> retained JSON
                                      +--> isolated render/ workspace
                                      +--> isolated studio/ workspace

engine-inspector reads owner facts; runtime libraries do not depend on it. Ordinary mechanics
inspection is structural-only; products pass already-produced evaluation, activation, inventory,
or receipt evidence explicitly when they need an enriched diagnostic report.
```

`entity-state` owns reusable entity invariants and an instance-owned typed
component store. Stable authored type identities, explicit registration,
per-entity/per-component revision-guarded mutation, deterministic iteration,
bounded inspection, and
optional versioned codecs let downstream Rust add inert component families
without an Engine enum, global registry, or ECS scheduler.

`gameplay-mechanics` optionally registers inert stats, tracks, sources, effects,
inventory, unique-item, and equipment data in that same store, then exposes
direct attributed stat, track, damage, and item/equipment services without
owning product orchestration. The `engine-inspector` leaf can strictly reopen a
mechanics snapshot with its admitted catalog and project structural component
presence and stored facts without mutable access. Evaluated stats, effect
activations, joined inventory, and enriched compatibility details are projected
only from explicit caller-supplied owner evidence or receipts. Snapshot JSON
uses the explicit structural-v2 route; the old enriched JSON shape is available
only through its versioned, caller-evidence route.

`gameplay-rules` is an independent optional package boundary for rules-heavy
consumers. It admits opaque schema-1 JSON or direct Rust candidates into the
same immutable canonical representation, resolves exact package dependencies,
and carries provenance and bounded diagnostics. The isolated `rules/`
workspace generates its TypeScript contract from the Rust owner and offers
build-time canonical authoring without entering the ordinary provider graph.
Downstream games retain every payload meaning, compiler decision, publication
target, and runtime operation; mechanics-only games do not depend on it.

`gameplay-resolution` is an independent optional lifecycle kernel. It traverses
only sequence, conditional, bounded selection, and opaque downstream operation
nodes; freezes downstream interceptor order; stages downstream effects through
one fail-atomic transaction; and returns generic events and attributed traces.
Every intent, fact, policy, effect, event, and authoritative state owner remains
downstream. It is neither a universal gameplay vocabulary nor a scheduler.

`engine-spatial` composes one canonical voxel authority with derived collision,
navigation, mesh, motion, trigger, and edit mechanisms. Content and authoring
crates own strict durable formats, validation, plans, and explicit mutation
boundaries without imposing a universal product schema.

`render-model`, `render-projection`, and `render-presentation` own the complete
renderer-neutral retained border. The separately gated `render/` workspace owns
strict TypeScript decoding, retained projection, Three/WebGL resources, and
host integration. The separately gated `studio/` workspace is a first-party
authoring product over a closed external-project adapter.

Three/WebGL, DOM/WebAudio, Chromium, and Angular are current backend or product
choices, not ordinary Rust dependencies and not a commitment to web-game
delivery. Browser evidence proves browser-owned behavior; headless evidence
proves host-neutral mechanisms.

The first-party Studio runs from this Engine repository as one generic
adapterless host for trusted local development. Opening a project sends its
root and project-relative file through one `/api/studio-session/open`
transaction; the host reads that root's bounded `.rusty-studio.json`, starts
the declared adapter, and publishes the adapter identity plus canonical
project readout only after handshake and admission. The browser never parses
the project schema. The persistent `rusty-studio.service` is the recommended
ordinary interactive entrypoint where installed; downstream repositories
provide only project data, `.rusty-studio.json`, and their Rust adapter.

## Repository layout

Workspace inventory: **37 Cargo workspace crates, 2 public gameplay-rules
packages, 4 public renderer packages, and 1 Studio application plus 5 Studio
libraries.**

```text
rust/crates/
  core-*                    typed IDs, assets, math, time, coordinates, voxel values
  svc-*                     volume, spatial, collision, pathfinding, RNG, mesh
  entity-state              typed components, relationships, transforms, snapshots, atomic mutation
  state-machine             explicit definitions, instances, and transitions
  gameplay-mechanics        component-backed stats, tracks, sources, items, damage, restoration
  gameplay-resolution       bounded downstream-owned resolution lifecycle and receipts
  gameplay-rules            optional opaque package admission, provenance, diagnostics, resolution
  gameplay-standard         incubating metadata readouts beside exact gameplay-owner APIs
  developer-command         optional typed runtime developer-console binding contract
  developer-command-standard optional standard gameplay command descriptors and owner adapters
  engine-spatial            canonical voxel space and synchronized derived mechanisms
  content-store             manifests, source batches, prefabs, load/save plans, write sets
  asset-catalog             asset versions, locks, dependencies, materials, fallbacks
  asset-import              bounded offline mesh import and atomic publication
  authored-scene            versioned scene documents, admission, validation, edits
  environment-authoring     deterministic recipe planning and materialization
  voxel-*                   stored artifacts, annotations, conversion, object runtime
  render-*                  retained frame model, projection, presentation
  engine-inspector          read-only diagnostics and rusty-inspect CLI

render/
  packages/render-contracts strict TypeScript decoding of Rust retained frames
  packages/render-projection retained client projection
  packages/renderer-three   Three/WebGL backend and resource lifecycle
  packages/renderer-host    browser/headless/editor host composition
  browser/                  real Chromium/WebGL/WebAudio/DOM acceptance

rules/
  packages/gameplay-rules-contracts generated Rust-owned envelope contracts and strict decode
  packages/gameplay-rules-authoring semantic-neutral canonical build-time authoring

studio/
  apps/studio-app           first-party Angular authoring application
  libs/                     adapter client, editor shell, viewport, voxel editor, settings
  scripts/ and test/        host services and isolated/integration proof

content/                    checked generic conversion request and canonical artifact
fixtures/                   provider-owned deterministic fixtures and licensed sources
migration/                  machine-readable donor/equivalence accounting
docs/                       architecture, code maps, topics, migration evidence, reviews
scripts/                    provider, rules, renderer, Studio, isolation, and consistency gates
```

The [agent code atlas](docs/agent-code-atlas.md) maps these paths to ownership,
public surfaces, forbidden shortcuts, focused tests, and follow-up routes.

## Architecture boundaries

### Entity and service authority

Downstream Rust owns the live gameplay loop and calls named Engine services
directly. `entity-state` is the atomic boundary for entity components; it is
not a universal command route for collision, navigation, assets, presentation,
or other service-owned state. `gameplay-mechanics` owns only the reusable
catalog/component/service contracts documented in its
[code map](docs/code-map/gameplay-mechanics.md); downstream owns attacks, turns,
ticks, consequences, and complete persistence.

### Content and persistence

Reusable scene, asset, voxel, annotation, prefab, and persistence formats may
live here. Their codecs validate bounded data and preserve typed failures.
Consumers decide game meaning, admission timing, filesystem layout, and storage
policy.

### Rendering and hosts

Rust produces complete renderer-neutral retained frames. TypeScript decodes and
projects them; Three owns backend resources; hosts own DOM, audio, browser, and
editor integration. None of those projections becomes gameplay authority.

## Common commands

Run from the repository root.

### Ordinary Rust provider gate

```bash
./scripts/verify.sh
```

This checks Rust formatting, standalone and isolation rules, documentation
links, migration/equivalence accounting, locked workspace tests, and Clippy
with warnings denied. It deliberately installs no Node dependencies.

### Full repository gate

```bash
./scripts/verify-all.sh
```

This adds the isolated gameplay-rules, renderer, and Studio gates.

### Focused gates

```bash
./scripts/check-doc-links.sh
./scripts/audit-standalone.sh
./scripts/verify-rules.sh
./scripts/verify-render.sh
./scripts/verify-studio.sh
```

Complete Rust-facade consumption has a clean temporary-consumer proof:

```bash
./scripts/verify-rust-sdk-consumer.sh
```

The host-neutral FPS character controller has a facade example, reproducible
performance probe, and an explicitly selected downstream integration gate:

```bash
cargo run -p rusty-engine --example character_controller --locked
./scripts/measure-character-controller.sh
./scripts/verify-character-controller-consumer.sh /absolute/path/to/rusty-craftsurvive
```

The integration gate reads the selected adjacent consumer and runs its owned
verification without rewriting either checkout. Browser and playtest evidence
remain explicit product-owned checks.

The ordinary adjacent Engine checkout stays on clean, compile-clean `main` so
these path dependencies remain buildable during Engine development. Engine
tasks use separate persistent branch worktrees and promote coherent candidates
after their owning local checks pass; downstream projects then adopt them
automatically and main-branch CI/review can evaluate that exact revision.
Review findings are fixed forward through the task worktree. Candidate consumer
layouts are reserved for tasks that explicitly require downstream proof before
promotion.

Studio is an isolated first-party authoring product. Its ordinary verification
does not select, launch, or certify a downstream checkout.

### Focused Rust iteration

```bash
cargo test -p <crate-name> --locked
cargo clippy -p <crate-name> --all-targets --locked -- -D warnings
```

The public mechanics example is a small downstream-style executable:

```bash
cargo run -p gameplay-mechanics --example compositions
```

It shows immediate shooter damage, infrastructure damage/repair and an
explicitly scheduled effect, plus a d20-shaped preview/reaction/fresh-apply
flow. None of the three creates an Engine session, scheduler, browser host, or
rules interpreter.

## Offline voxel conversion

Regenerate the checked generic artifact with:

```bash
cargo run -q -p voxel-convert --bin voxel-convert -- \
  --request content/conversion/kenney-wall-a.request.json \
  --source fixtures/voxel-conversion/kenney-wall-a.glb \
  --output content/assets/kenney-wall-a.voxel.json
```

The format, limits, provenance, and failure behavior are documented in
[Stored voxel asset and offline conversion](docs/topics/voxel/voxel-asset-format.md).

## Key documents

| Document | Purpose |
|---|---|
| [Documentation index](docs/README.md) | Organized entry point for all repository docs |
| [Canonical design](docs/design.md) | Provider ownership, host/platform boundaries, dependency direction, promotion |
| [Agent code atlas](docs/agent-code-atlas.md) | Owner routing, primary paths, public surfaces, gates, and common mistakes |
| [Runtime developer console](docs/topics/development/runtime-developer-console.md) | End-to-end developer console / debug console flow, ownership, adapters, and focused checks |
| [Known limitations](docs/known-limitations.md) | Active provider limitations and explicitly scheduled consumer certification |
| [Rust source organization](docs/topics/development/rust-style.md) | Lightweight module and behavior-owner style |
| [Downstream Engine and Studio boundary](docs/topics/development/downstream-renderer-and-studio.md) | Sibling facade consumption, Engine-hosted Studio, and renderer ownership |
| [Downstream gameplay adoption](docs/topics/gameplay/downstream-adoption.md) | Disposition-first migration into authored packages, downstream Rust authority, and optional gameplay crates |
| [Rendering successor contract](docs/rendering-successor-contract.md) | Complete shared-rendering scope and adaptation boundary |
| [Rendering operations](docs/rendering-operations.md) | Provider verification, Rust host boundary, resources, and limitations |
| [Studio migration contract](docs/studio-migration-contract.md) | First-party authoring scope, parity, isolation, owner adoption |
| [Studio adapter protocol](docs/studio-adapter-protocol.md) | Closed external-project operations and optimistic guards |
| [Voxel asset format](docs/topics/voxel/voxel-asset-format.md) | Durable volume format and bounded converter border |
| [Voxel model conversion](docs/topics/voxel/voxel-model-conversion.md) | Object, animation, runtime, and Studio conversion workflow |
| [Migration cluster ledger](docs/migration/migration-cluster-ledger.md) | Durable transfer, replacement, exclusion, and reopening decisions |
| [Donor provenance](docs/migration/donor-provenance.md) | Exact donor revisions, adaptations, exclusions, and licenses |

## Notes for outside agents

- Read `AGENTS.md`, `docs/design.md`, and `docs/agent-code-atlas.md` before
  substantial work.
- Treat code, manifests, tests, and repository gates as implementation truth.
- Use Den for live task state when a Den task is supplied; do not require Den
  merely to understand the checked-out provider.
- Keep downstream game meaning downstream and host-specific behavior in its
  explicit backend or product owner.
- Do not hand-wave acceptance: run the gate for the surface actually changed,
  including a real browser or external consumer when that surface owns the
  behavior.
