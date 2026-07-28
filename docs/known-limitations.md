# Known limitations

This is the offline provider record for active limitations. Intentional
architecture boundaries are stated separately so agents do not “fix” them by
introducing a second authority. Product-specific limitations belong to their
downstream repositories.

## Gameplay mechanics awaits Rusty D20 GM7 reconciliation

- **Status:** accepted-temporary
- **Affected surface:** the initial `gameplay-mechanics` provider and GM7
  closeout
- **Limitation:** GM0-GM6, the reviewed external realtime demo, and the bounded
  infrastructure fixture prove the provider plus two composition shapes. The
  rules-heavy Rusty D20 consumer has not yet completed the final
  three-composition reconciliation.
- **Impact:** The provider APIs, realtime product path, and infrastructure-shaped
  direct composition are independently usable, but the campaign has not yet
  shown whether the rules-heavy composition reveals another reusable gap or
  only downstream policy.
- **Detection:** `cargo test -p gameplay-mechanics --test gm7_builder --locked`,
  `cargo test -p gameplay-mechanics -p engine-inspector --locked`, and
  `cargo run -p gameplay-mechanics --example compositions` prove the
  host-neutral provider shapes. They deliberately do not launch or inspect a
  sibling product.
- **Follow-up:** Rusty Engine task #6291 reconciles the reviewed shooter,
  infrastructure, and Rusty D20 evidence.
- **Last reviewed:** 2026-07-28 / codex

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
