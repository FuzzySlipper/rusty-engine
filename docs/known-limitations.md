# Known limitations

This is the offline provider record for active limitations. Intentional
architecture boundaries are stated separately so agents do not “fix” them by
introducing a second authority. Product-specific limitations belong to their
downstream repositories.

## Animated voxel corpus breadth and GPU accounting

- **Status:** accepted-temporary
- **Affected surface:** `voxel-convert`, `voxel-object-runtime`, retained voxel-object rendering,
  and the explicit Studio voxel-consumer integration
- **Limitation:** The checked quality/performance corpus is one CC0 humanoid with one material and
  three clips sampled at a stylized 6 Hz. Schema 1 stores complete poses. The renderer exposes
  successful frame acknowledgement but not GPU timer queries or driver-reported VRAM.
- **Impact:** The 96 × 144 × 96 quality target is 12,758,243 canonical bytes, has an estimated
  34,541,208-byte unique CPU mesh payload, and took roughly 2.60 seconds to admit and mesh in one
  unoptimized local observation. It is not an interactive conversion default. End-to-end Chromium
  acknowledgement must not be reported as isolated GPU time, and CPU payload estimates must not be
  reported as exact VRAM.
- **Detection:** `./scripts/verify-studio-voxel-integration.sh
  /absolute/path/to/the-exact-rusty-engine-voxels-checkout` validates the pinned reports, stable
  collision, missing/corrupt rejection, and real Chromium playback. The source pin is
  `studio/voxel-consumer-source.json`.
- **Follow-up:** Add materially different licensed corpora and simultaneous-instance measurements
  before choosing delta/reference storage, another cadence default, or renderer GPU instrumentation.
- **Last reviewed:** 2026-07-27 / codex

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
- **Limitation:** Engine represents bounded inline data, explicit shared-buffer
  handles, and content-addressed `packedStreamsLeV1` resources, but deliberately
  does not auto-promote every mesh or own a generic cross-process cache broker.
- **Impact:** A consumer chooses the packed projector when bulk transport is
  justified and owns publication paths, cache lifetime, and host resolution.
  Current Studio hosts eagerly preload at most 1,024 resources and 256 MiB.
- **Detection:** `render-model`/`render-projection` resource tests, renderer
  lifecycle tests, and the exact `rusty-engine-voxels` Chromium integration
  cover the explicit paths.
- **Follow-up:** Add measured lazy admission or a narrower host broker only if
  a real corpus exceeds the current eager boundary.
- **Last reviewed:** 2026-07-28 / codex

## Deliberate mechanics boundaries, not defects

The mechanics crate intentionally does not provide a complete game save,
catalog migration engine, heterogeneous cross-service transaction, effect
clock, attack/turn/reaction owner, item behavior registry, event bus, replay
log, renderer, or Studio schema. Downstream owners compose those concerns
around direct named services. A future operation that cannot preserve valid
intermediate states must justify a narrow generic seam at its true owner rather
than reviving `MechanicsState`, a universal command AST, or a complete-world
clone.
