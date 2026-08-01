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

## Gameplay mechanics campaign breadth stops at three compositions

- **Status:** deliberate-boundary
- **Affected surface:** `gameplay-mechanics`, optional `gameplay-rules`, and
  their downstream adoption evidence
- **Limitation:** GM7 reconciles one realtime product, one bounded headless
  infrastructure falsification fixture, and one real rules-heavy D20 product.
  It does not certify a shipped builder, a second rules-heavy domain, or a
  universal content/mod platform.
- **Impact:** The initial provider campaign has no known acceptance gap. New
  shared semantics still require a concrete consumer and must be promoted at
  their narrow owner rather than inferred from these three compositions.
- **Detection:** See the exact revision and acceptance mapping in the
  [gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md),
  then run the focused mechanics or rules gate for the surface being changed.
- **Follow-up:** Downstream products own content and UI expansion. Create an
  Engine task only when another concrete consumer exposes a reusable mechanism
  gap.
- **Last reviewed:** 2026-07-29 / codex

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
  handles, and content-addressed `packedStreamsLeV1`/`packedStreamsLeV2`
  resources, but deliberately
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
