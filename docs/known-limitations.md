# Known limitations

This is the offline provider record for active limitations. Intentional
architecture boundaries are stated separately so agents do not “fix” them by
introducing a second authority. Product-specific limitations belong to their
downstream repositories.

## Gameplay mechanics awaits reference-consumer certification

- **Status:** accepted-temporary
- **Affected surface:** the initial `gameplay-mechanics` provider revision
- **Limitation:** GM0-GM5 prove the public provider, strict reconstruction,
  inspection, quotas, and direct example compositions in Rusty Engine. The
  external reference demo has not yet removed its private mechanics authority
  and certified the exact provider revision in its real product path.
- **Impact:** The provider APIs are independently usable, but Rusty Engine
  alone cannot claim the reference product's save migration, controls,
  presentation, or duplicate-authority removal.
- **Detection:** `cargo test -p gameplay-mechanics -p engine-inspector --locked`
  and `cargo run -p gameplay-mechanics --example compositions` prove the
  provider boundary. They deliberately do not launch or inspect a sibling demo.
- **Follow-up:** `rusty-engine-demo` task #6290 performs the exact-revision
  migration; Rusty Engine task #6291 reconciles the shooter, infrastructure,
  and d20-shaped compositions with that external evidence.
- **Last reviewed:** 2026-07-27 / codex

## Planar navigation footprint and multi-agent planning

- **Status:** accepted-temporary
- **Affected surface:** `svc-pathfinding` planar projection and direct movement
- **Limitation:** The default planar projection validates one X/Z cell plus
  configured vertical clearance. It does not expand walkability by arbitrary
  body radius or coordinate several moving agents. The separate volumetric
  query is not the loading-bay planar path.
- **Impact:** Consumers compose body collision after a planar proposal and own
  scheduling/avoidance among moving entities.
- **Detection:** `cargo test -p svc-pathfinding
  projected_direct_nav_movement_is_deterministic` and
  `volumetric_agent_volume_requires_empty_occupied_cells`.
- **Follow-up:** Add a cohesive clearance-aware planar projection or crowd
  service only when a concrete consumer requires it.
- **Last reviewed:** 2026-07-24 / codex

## Automatic mesh payload promotion and transport caching

- **Status:** accepted-temporary
- **Affected surface:** `svc-mesh`, `engine-spatial`, `render-model`, and the
  shared renderer border
- **Limitation:** Engine represents bounded inline mesh data and explicit shared
  buffer handles, but does not provide a generic content-addressed broker that
  automatically promotes and caches mesh results across processes.
- **Impact:** A consumer deliberately uses bounded inline data or supplies a
  host-owned shared-buffer provider with explicit lifetime.
- **Detection:** The engine-spatial live-remesh test and renderer-three
  inline/shared lifecycle tests cover the current explicit paths.
- **Follow-up:** Add a narrow host-owned broker only when measured transport
  traffic justifies it.
- **Last reviewed:** 2026-07-24 / codex

## Deliberate mechanics boundaries, not defects

The mechanics crate intentionally does not provide a complete game save,
catalog migration engine, heterogeneous cross-service transaction, effect
clock, attack/turn/reaction owner, item behavior registry, event bus, replay
log, renderer, or Studio schema. Downstream owners compose those concerns
around direct named services. A future operation that cannot preserve valid
intermediate states must justify a narrow generic seam at its true owner rather
than reviving `MechanicsState`, a universal command AST, or a complete-world
clone.
