# Rusty Engine agent guidance

## Den Guidance Bootstrap

- Project ID: `rusty-engine`
- Resolve live guidance with the Den MCP `get_agent_guidance` tool before
  substantial work.
- Treat the resolved Den guidance packet and its referenced Den documents as
  the source of truth.
- If Den is unreachable, stop and tell the user which Den tool or command
  failed and what you were about to do. Do not reconstruct Den state from local files.

## Project role

This repository is the standalone canonical Rusty Engine provider. It owns
reusable, host-neutral mechanisms for object-centric games. Downstream Rust
games own live authoritative gameplay state, substantial game logic, product
orchestration, content meaning, storage policy, and game-specific presentation.

Ordinary Engine work must not depend on, inspect, or mutate a sibling demo checkout.

## Source-of-truth posture

Use each source for the state it owns:

- The user's current scope and any supplied Den task own the work being done.
- [The canonical design](docs/design.md) owns durable architecture and
  ownership intent.
- Current code, manifests, tests, fixtures, and executable gates describe
  implemented behavior.
- [The agent code atlas](docs/agent-code-atlas.md) routes navigation; it does not overrule an owning contract or test.
- Historical migration evidence and prior-agent commentary explain decisions
  but do not define current work.

Repository docs are deliberately self-contained for agents without Den access.
Den owns current task state and review evidence when work is Den-managed; it is not required to understand the committed provider. If design intent and implemented behavior conflict, surface the drift instead of silently declaring either one correct. Do not infer active work from migration milestones or old review reports.

Keep [docs/design.md](docs/design.md) current when changing authority, execution order, persistence, dependency direction, or presentation boundaries.

## Architecture contract

> Objects carry typed facts. Named services own mechanisms. Downstream games
> own meaning and orchestration.

- Keep the provider object-centric: entity components, spatial authority,
  responsible services, and typed facts must be easy to trace.
- Components are mostly data. Do not add implicit update callbacks, ambient
  subscriptions, service location, renderer/I/O behavior, or hidden global
  registries to them.
- Downstream Rust calls direct named services. Engine does not own a universal
  game scheduler, gameplay AST, behavior graph, replay runtime, or ambient
  command/event bus.
- Do not use a consumer count as a promotion gate. One credible downstream
  proof, concrete need, or explicit architecture decision may justify a
  neutral Engine mechanism when centralization prevents parallel authority or
  correctness drift. Cross-repo surveys and later consumers are useful
  challenges, not prerequisites. See
  [upstream promotion and authoring DSL](docs/topics/development/upstream-promotion-and-authoring-dsl.md).
- `entity-state` owns reusable entity invariants and one instance-owned typed
  component store. Registration uses stable authored identities; mutation is
  guarded by the exact entity/component slot revision and does not expose
  unrestricted mutable component references. Its command batch is not a
  universal route for ordinary service-owned state.
- `gameplay-mechanics` owns optional reusable stats, tracks, attributed sources,
  effects, inventory/item/equipment data, damage, and restoration mechanisms.
  Downstream owns attacks, turns, ticks, effect timing, consequences, and
  complete saves.
- `gameplay-rules` optionally owns strict semantic-neutral package admission,
  exact dependencies, canonical bytes and fingerprints, provenance, and
  bounded diagnostics. Downstream owns the opaque payload schema, semantic
  compiler, publication, persistence policy, and execution.
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
  entity-state              entity facts, typed component storage, atomic mutation
  state-machine             explicit definitions, instances, transitions
  gameplay-mechanics        stats, tracks, sources, items, damage, restoration
  gameplay-rules            optional opaque packages, provenance, diagnostics, resolution
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
rules/                      isolated semantic-neutral TS rules authoring
studio/                     isolated Angular/Nx first-party authoring product
content/                    checked generic content inputs and artifacts
fixtures/                   deterministic provider and host evidence
migration/                  machine-readable donor/equivalence accounting
docs/                       design, atlas, topics, migration evidence, reviews
scripts/                    provider, rules, renderer, Studio, and isolation gates
```

For path-level ownership, public surfaces, forbidden shortcuts, tests, and
follow-up routes, use [docs/agent-code-atlas.md](docs/agent-code-atlas.md).

## Ownership quick reference

| Area | Primary owner | Must not acquire |
|---|---|---|
| Entity facts and component mutation | `entity-state`, `state-machine` | Generic service routing, callbacks, renderer/I/O |
| Reusable gameplay mechanics | `gameplay-mechanics` | Attacks, turns, ticks, game rules, complete saves |
| Optional rules package support | `gameplay-rules` | Domain semantics, runtime evaluation, registries, complete saves |
| Canonical voxel world and derivatives | `engine-spatial`, `svc-*` | Game policy, browser input, duplicate authority |
| Content, assets, prefabs, scenes | `content-store`, `asset-*`, `authored-scene` | Product storage policy, implicit spawn behavior |
| Environment recipes | `environment-authoring` | Universal procgen framework, UI, scheduling |
| Voxel artifacts and conversion | `voxel-*` | URL/fetch runtime import, game-specific animation policy |
| Rust retained frames | `render-model`, `render-projection`, `render-presentation` | Three, DOM, WebAudio, gameplay authority |
| Rules authoring | `rules/` | Domain semantics, runtime evaluation, browser/UI, ordinary Rust dependencies |
| Renderer backend and hosts | `render/` | Gameplay meaning or state, ordinary Rust dependencies |
| First-party authoring product | `studio/` | Demo/game policy, implicit sibling access |
| Inspection | `engine-inspector` | Mutation or runtime/library reverse dependencies |

## Host and platform boundary

Rusty Engine is host neutral, not a web-game engine.

- Ordinary Rust-provider work has no Node, TypeScript, browser, renderer, Studio, or demo dependency.
- Keep Three/WebGL, DOM/WebAudio, Chromium, Electron/Tauri, and product-shell concerns in explicit backend or host owners.
- Never add HTTP, URL/fetch, browser storage, DOM-event, WebGL, or
  Playwright-only seams to Rust or renderer-neutral packages merely to simplify browser tests.
- Browser evidence proves browser-owned behavior. Headless evidence proves
  host-neutral mechanisms. Neither substitutes for the other.
- Rules, Studio, and renderer packages remain separately isolated workspaces with explicit gates.

## Local commands

Run from the repository root.

```bash
# Ordinary Rust provider gate
./scripts/verify.sh

# All isolated workspaces
./scripts/verify-all.sh

# Focused structural checks
./scripts/check-doc-links.sh
python3 ./scripts/dependency_boundary_check.py
python3 ./scripts/code_map_freshness.py  # advisory; reports drift without failing
./scripts/audit-standalone.sh
./scripts/audit-render-isolation.sh
./scripts/audit-studio-isolation.sh

# Isolated products/backends
./scripts/verify-rules.sh
./scripts/verify-render.sh
./scripts/verify-studio.sh

# Focused Rust crate iteration
cargo test -p <crate-name> --locked
cargo clippy -p <crate-name> --all-targets --locked -- -D warnings
```

External integration is always selected explicitly and is never a dependency
freshness ceremony:

```bash
./scripts/verify-rust-sdk-consumer.sh
./scripts/verify-character-controller-consumer.sh /absolute/path/to/rusty-craftsurvive
```

The first command proves the complete local Rust facade in a clean temporary
consumer. Exact source and consumer commits for explicitly selected external
proof belong in Den task or review evidence; they are not committed downstream
dependency policy. Check the owning code-map page before choosing a focused gate.

## Engine task worktrees and stable adjacency

`/home/dev/rusty-engine` is the stable `main` integration checkout consumed by
adjacent downstream Cargo path dependencies. Keep that checkout free of
unfinished Engine edits and intentionally non-compiling intermediate states.

- Create a purpose-named branch and persistent worktree for substantial Engine
  implementation, normally under `/home/dev/worktrees/rusty-engine-<task>`.
- Develop, commit, and validate the exact task branch in that worktree. Do not
  redirect ordinary downstream dependency paths to it.
- Once the candidate is coherent, compile-clean, and has passed its owning local
  checks, fast-forward the stable checkout to the task branch and push `main`.
  Downstream consumers then adopt the completed candidate automatically through
  their unchanged adjacent path dependency, and main-branch CI can gate the
  exact revision used for review.
- Review may happen after that compile-clean promotion. Keep the Den task in
  review until it is accepted, and fix requested or unexpected behavior forward
  through the task worktree and another checked fast-forward. This workflow
  protects downstream build availability; it is not a release train or a
  promise that every promoted candidate is already behaviorally accepted.
- Run downstream product checks after promotion by default. Use a disposable
  candidate adjacency layout before promotion only when a task explicitly
  requires downstream evidence against the candidate Engine revision.
- If current `main` advanced while a task branch was in progress, integrate it
  into the task branch and rerun the owning checks before requesting promotion.
  Do not overwrite, reset, clean, or repurpose another task's worktree.

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
- Avoid clever abstractions, hidden mutation, unexplained cloning, and generic wrappers that erase actionable error identity.
- File size is a review signal, not a CI rule.

## TypeScript and Studio style

- Do not add plain JavaScript for application, product, or library code when
  TypeScript is available. Reserve `.mjs`/`.js` for tool-required configuration
  or small build/check plumbing, and do not let those files become owners of
  durable behavior.
- Keep durable semantics, state, and runtime behavior in their named owners.
  Use TypeScript for typed presentation, host integration, strict boundary
  code, and expressive authoring or content composition; it must not duplicate
  gameplay, project, or persistence state.
- Outside the named Engine-owned renderer packages and host adapters under
  `render/`, TypeScript and browser code may render DOM UI only. Downstream
  product TypeScript must never construct non-UI world geometry, materials,
  canvases, Three/WebGL objects, render loops, or substitute scene
  visualizations. If product work appears to require that, stop and report the
  missing renderer-neutral or Engine-owned presentation boundary; do not work
  around it in browser code.
- Treat optional gameplay TypeScript as a build-time authoring DSL: authored
  variation and pure macros may live there, new serialized meaning begins in
  the owning semantic definitions, and live fact gathering, mutation,
  scheduling, and persistence remain in named runtime services and state
  owners.
- Keep package-root imports and explicit public exports.
- Do not deep-import another package's `src/` tree from a consumer.
- Keep strict decoding at the Rust-to-TypeScript border.
- Make renderer resource creation, replacement, stale-state handling, and
  disposal explicit.
- Keep UI state local and observational; it is not project or gameplay truth.
- Prefer clear named decisions and small typed functions over terse clever magic.
- Treat each web application's HTML document, `main.ts`, bootstrap function,
  and initial loading shell as a thin composition root. They should do only the
  minimum needed to load dependencies, mount the application, and present
  bounded startup/loading/failure state.
- Do not accumulate feature logic, API clients, state models, sample content,
  generalized scaffolding, or large inline scripts in the bootstrap surface.
  Move such behavior into purpose-named external TypeScript modules or packages
  with an explicit owner, then import and compose those owners at startup. A
  starter app should remain visibly small enough that its wiring and authority
  boundaries can be understood at a glance.

## Architecture spikes and drift control

An architecture spike answers a named design or code-shape question. It is not
an unfinished implementation task and ordinary evidence defaults must not turn
it into one.

- The task's stated learning objective, exclusions, evidence budget, and stop
  conditions govern the spike. Do not restore ordinary hardening, compatibility,
  packaging, browser, or conformance expectations that the task deliberately
  deferred.
- Unsupported adjacent integration is a valid result. Stop and describe the
  exact missing boundary instead of creating mocks, placeholders, fake
  presentation, parallel authority, test-only product behavior, or a substitute
  implementation merely to make a proof pass.
- When the required capability belongs upstream or to another named owner,
  file one exact Den request if authorized, link it in the handoff, and stop.
  Do not vendor, copy, shim, simulate, or recreate that capability inside the
  assigned task merely to make the task appear complete. A completion packet
  that names the upstream request and why local work stopped is a valid result.
- Proof work must not add product behavior or an adjacent mechanism absent from
  the proposed architecture. If proof becomes larger than the mechanism being
  evaluated, stop and reassess the task.
- Prefer the smallest compile, load, or direct state observation that answers
  the spike's question. An honest inability to reach UI, rendering, packaging,
  or another owner is more useful than artificial end-to-end evidence.
- At meaningful subagent milestones, require a brief report naming the original
  goal advanced, necessary surfaces changed, proof/scaffolding added, any
  unsupported boundary or drift, any upstream request and stop decision, and
  the next task-faithful step. Use that exchange to redirect work early; do not
  add a permanent drift-agent or validation lane unless the task explicitly
  needs one.

## Task-faithful review

- Review the user's request, owning task, explicit exclusions, and directly
  applicable changed-surface contracts before judging the implementation.
- Do not introduce concerns, acceptance requirements, hardening, preferred
  approaches, or test obligations beyond the original task. General guidance
  may reveal a direct boundary violation; it must not enlarge the assignment
  into unrelated modernization.
- Passing tests are evidence, not the conclusion, but missing paperwork is not
  a product defect. Stale commit, branch, packet, gate, or review metadata must
  never cause changes requested when the reviewed artifact is unambiguous.
- Every blocking finding must identify a task-owned defect or acceptance gap,
  direct evidence, material consequence, and the minimum property needed for
  closure. Keep optional adjacent ideas non-blocking and separate.
- Do not request additional tests without naming the task-owned product need
  they would prove. Reuse credible existing evidence and stop when the original
  acceptance class is resolved.
- Treat every changes-requested verdict as real implementation cost. Review
  exists to improve the assigned engineering result, not to commission new work
  or satisfy procedure.

When available, use the personal `$task-faithful-review` skill for milestone
drift checks, final reviews, and rereviews.

## Test and acceptance posture

These are defaults for ordinary implementation tasks. Explicit architecture-
spike evidence budgets and exclusions override them.

- Run the narrowest relevant check first, then the gate that owns the changed surface.
- Cross-language contract changes require Rust validation, TypeScript decode
  coverage, and retained-frame fixture/golden updates.
- Persistence and mutation changes require failure-path and atomicity evidence, not only happy-path serialization.
- User-visible browser behavior requires the real browser gate only when that
  behavior is itself part of the requested deliverable. Never create browser
  presentation or product behavior solely to obtain browser evidence.
- Synthetic tests are evidence for mechanisms, not proof that a downstream
  product works.
- Do not add or recommend tests whose product or mechanism need cannot be
  stated concretely.
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

Success is measured by mechanism locality, explainability, atomicity, focused provider evidence, bounded dependencies, standalone operation, and honest host/consumer proof.
