# Rusty Engine agent guidance

## Project role

This repository is the standalone canonical Rusty Engine provider. It owns
reusable, host-neutral mechanisms for object-centric games. Downstream Rust
games own live authoritative gameplay state, substantial game logic, product
orchestration, content meaning, storage policy, and game-specific presentation.

Asha is historical evidence and a donor source, not a compatibility constraint,
runtime dependency, or architecture to recreate. The loading-bay product is
owned by the external `rusty-engine-demo` consumer. Ordinary Engine work must
not depend on, inspect, or mutate a sibling demo checkout.

## Source-of-truth posture

Use each source for the state it owns:

- The user's current scope and any supplied Den task own the work being done.
- [The canonical design](docs/design.md) owns durable architecture and
  ownership intent.
- Current code, manifests, tests, fixtures, and executable gates describe
  implemented behavior.
- [The agent code atlas](docs/agent-code-atlas.md) routes navigation; it does
  not overrule an owning contract or test.
- Historical migration evidence and prior-agent commentary explain decisions
  but do not define current work.

Repository docs are deliberately self-contained for agents without Den access.
Den owns current task state and review evidence when work is Den-managed; it is
not required to understand the committed provider. If design intent and
implemented behavior conflict, surface the drift instead of silently declaring
either one correct. Do not infer active work from migration milestones or old
review reports.

Keep [docs/design.md](docs/design.md) current when changing authority, execution
order, persistence, dependency direction, or presentation boundaries.

## Architecture contract

> Objects carry typed facts. Named services own mechanisms. Downstream games
> own meaning and orchestration.

- Keep the provider object-centric: capabilities, spatial authority,
  responsible services, and typed facts must be easy to trace.
- Components are mostly data. Do not add implicit update callbacks, ambient
  subscriptions, service location, renderer/I/O behavior, or hidden global
  registries to them.
- Downstream Rust calls direct named services. Engine does not own a universal
  game scheduler, gameplay AST, behavior graph, replay runtime, or ambient
  command/event bus.
- `entity-state` owns reusable entity invariants and atomic capability
  mutation. Its command batch is not a universal route for ordinary
  service-owned state.
- `engine-spatial` owns cohesive canonical voxel state when collision,
  navigation, mesh, motion, triggers, and edit history must remain synchronized.
- Durable formats validate bounded data; consumers retain game meaning,
  admission timing, filesystem layout, and persistence policy.
- Renderer and diagnostic projections observe owner facts. They never become a
  second gameplay authority.

## Repository structure

```text
rust/crates/
  core-*                    IDs, assets, math, time, coordinates, voxel values
  svc-*                     focused volume, spatial, collision, pathfinding, RNG, mesh
  entity-state              entity facts and atomic capability mutation
  state-machine             explicit definitions, instances, transitions
  engine-spatial            canonical voxel space and synchronized derivatives
  content-store             content manifests, batches, prefabs, load/save plans
  asset-catalog             versions, locks, dependencies, materials, fallbacks
  asset-import              bounded offline import and atomic publication
  authored-scene            versioned scene documents, admission, validation, edits
  environment-authoring     recipe planning, preview, materialization
  voxel-*                   durable formats, annotations, conversion, object runtime
  render-*                  retained model, projection, presentation
  engine-inspector          read-only diagnostics leaf and CLI

render/                     isolated retained TS projection, Three backend, hosts
studio/                     isolated Angular/Nx first-party authoring product
content/                    checked generic content inputs and artifacts
fixtures/                   deterministic provider and host evidence
migration/                  machine-readable donor/equivalence accounting
docs/                       design, atlas, topics, migration evidence, reviews
scripts/                    provider, renderer, Studio, and isolation gates
```

For path-level ownership, public surfaces, forbidden shortcuts, tests, and
follow-up routes, use [docs/agent-code-atlas.md](docs/agent-code-atlas.md).

## Ownership quick reference

| Area | Primary owner | Must not acquire |
|---|---|---|
| Entity facts and capability mutation | `entity-state`, `state-machine` | Generic service routing, callbacks, renderer/I/O |
| Canonical voxel world and derivatives | `engine-spatial`, `svc-*` | Game policy, browser input, duplicate authority |
| Content, assets, prefabs, scenes | `content-store`, `asset-*`, `authored-scene` | Product storage policy, implicit spawn behavior |
| Environment recipes | `environment-authoring` | Universal procgen framework, UI, scheduling |
| Voxel artifacts and conversion | `voxel-*` | URL/fetch runtime import, game-specific animation policy |
| Rust retained frames | `render-model`, `render-projection`, `render-presentation` | Three, DOM, WebAudio, gameplay authority |
| Renderer backend and hosts | `render/` | Gameplay authority, ordinary Rust dependencies |
| First-party authoring product | `studio/` | Demo/game policy, implicit sibling access |
| Inspection | `engine-inspector` | Mutation or runtime/library reverse dependencies |

## Host and platform boundary

Rusty Engine is host neutral, not a web-game engine.

- Ordinary Rust-provider work has no Node, TypeScript, browser, renderer, Studio,
  or demo dependency.
- Keep Three/WebGL, DOM/WebAudio, Chromium, Electron/Tauri, and product-shell
  concerns in explicit backend or host owners.
- Never add HTTP, URL/fetch, browser storage, DOM-event, WebGL, or
  Playwright-only seams to Rust or renderer-neutral packages merely to simplify
  browser tests.
- Browser evidence proves browser-owned behavior. Headless evidence proves
  host-neutral mechanisms. Neither substitutes for the other.
- Studio and renderer packages remain separately isolated workspaces with
  explicit gates.

## Donor and promotion rules

- Engine owns only reusable mechanisms proved by concrete consumers.
- Before selecting an Asha donor crate, consult the pinned portability report
  linked from
  [docs/migration/donor-provenance.md](docs/migration/donor-provenance.md),
  then re-audit the concrete dependency closure and current consumer.
- Internalize only a bounded, audited donor closure. Record every transfer,
  adaptation, replacement, and exclusion in donor provenance and the relevant
  machine-readable ledger.
- Preserve useful behavior for named consumers, not historical crate topology.
- The multi-consumer promotion rule does not permit silent loss of proven donor
  behavior during an owner-approved parity campaign.
- Do not add a universal gameplay AST, behavior graph, Gameplay Fabric
  compatibility layer, replay certification system, scheduler, Studio layer, or
  broad governance framework without concrete consumer evidence and an explicit
  architecture decision.
- Never persist callbacks or language closures.

## Local commands

Run from the repository root.

```bash
# Ordinary Rust provider gate
./scripts/verify.sh

# All isolated workspaces
./scripts/verify-all.sh

# Focused structural checks
./scripts/check-doc-links.sh
./scripts/audit-standalone.sh
./scripts/audit-render-isolation.sh
./scripts/audit-studio-isolation.sh

# Isolated products/backends
./scripts/verify-render.sh
./scripts/verify-studio.sh

# Focused Rust crate iteration
cargo test -p <crate-name> --locked
cargo clippy -p <crate-name> --all-targets --locked -- -D warnings
```

External integration is always selected explicitly:

```bash
./scripts/verify-render-consumer.sh <40-character-public-sha>
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

Check the owning code-map page before choosing a focused gate.

## Rust source style

Follow
[docs/topics/development/rust-style.md](docs/topics/development/rust-style.md).
These are navigation and ownership rules, not a new governance layer.

- Organize around one primary behavior owner or cohesive type family.
- Keep crate roots thin and public surfaces explicit.
- Prefer direct services, explicit state, explicit errors, and visible
  transaction boundaries.
- Use ownership vocabulary consistently: entity/component for data,
  service/system for behavior, projection/readout for derived observation.
- Keep crates coarse and independently meaningful.
- Avoid clever abstractions, framework-shaped machinery, hidden mutation,
  unexplained cloning, and generic wrappers that erase actionable error
  identity.
- File size is a review signal, not a CI rule.

## TypeScript and Studio style

- Keep package-root imports and explicit public exports.
- Do not deep-import another package's `src/` tree from a consumer.
- Keep strict decoding at the Rust-to-TypeScript border.
- Make renderer resource creation, replacement, stale-state handling, and
  disposal explicit.
- Keep UI state local and observational; it is not project or gameplay truth.
- Prefer clear named decisions and small typed functions over terse framework
  magic.

## Test and acceptance posture

- Run the narrowest relevant check first, then the gate that owns the changed
  surface.
- Cross-language contract changes require Rust validation, TypeScript decode
  coverage, and retained-frame fixture/golden updates.
- Persistence and mutation changes require failure-path and atomicity evidence,
  not only happy-path serialization.
- User-visible browser behavior requires the real browser gate.
- External-package or external-adapter claims require the exact consumer or
  integration gate.
- Synthetic tests are evidence for mechanisms, not proof that a downstream
  product works.
- Report exactly which commands ran and which relevant live checks were
  skipped.

## Shared-workspace discipline

- Treat a dirty worktree as normal and preserve unrelated changes.
- Keep edits inside the assigned directory or ownership surface when work is
  partitioned between agents.
- Do not reset, restore, delete, or reformat another agent's work.
- Re-read `git status` and the overlapping diff before and after edits.
- If a required change crosses the assigned boundary, hand it off or ask before
  expanding scope.

## Success criteria

Success is measured by mechanism locality, explainability, atomicity, focused
provider evidence, bounded dependencies, standalone operation, and honest
host/consumer proof. Real product behavior is proved in the downstream consumer
that owns it.
