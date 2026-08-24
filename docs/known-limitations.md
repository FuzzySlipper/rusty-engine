# Known limitations

This is the offline provider record for active limitations. Intentional
architecture boundaries are stated separately so agents do not “fix” them by
introducing a second authority. Product-specific limitations belong to their
downstream repositories.

## Runtime voxel-sprite enhancement experiment

- **Status:** experimental
- **Affected surface:** `renderer-three`, `renderer-host`, and the bundled
  application host
- **Limitation:** One disposable application-host attachment can replace a
  retained visual with a triggered single-view color/depth/normal/coverage
  capture, or consume four admitted prepared textures with matching dimensions
  and color-space metadata. The five comparison modes use view-space normals,
  RGBA8 linear depth, approximate normal-oriented splats, and CPU submission
  observations; they do not provide GPU timing, multi-view reconstruction,
  sorted transparent splats, collision, or gameplay state. The retained-only
  held-animation bank experiment can capture bounded exact/cadenced
  pose×direction frames from an admitted embedded or external-pack clip using
  an independent clone and then select resident textures without recapture.
  It remains an experimental CPU-submission-only facility: no scheduler,
  gameplay cadence meaning, offline cache, pooling, GPU completion timing, or
  crowd budget is supplied.
  Representation orientation can remain camera-facing, hold the admitted
  capture basis, or blend between those orientations, with either captured
  elevation or a world-upright azimuth. This exposes local parallax from the
  existing displaced geometry. Relief is rebased to the captured subject's
  projected bounds, expanded by bounded surface contrast, and centered on the
  card plane. Amplitude therefore changes thickness without translating a
  near-uniform layer through the capture camera clip range; optional
  quantization divides that centered shape. The connected-card comparison now
  performs a bounded 4-to-32-step fragment parallax-occlusion lookup with
  bounded UV travel; zero steps retains the vertex-displacement fallback.
  Angle-conditioned consumers may align neighboring captures onto one held
  card and assign opaque, complementary-dither, or alpha transition weights.
  These are experimental presentation controls, not multi-view reconstruction.
  Retained-only `ghost-plate` mode freezes an isolated clone at capture time;
  skinned vertices are baked into private static geometry so later plate-space
  normalization cannot invalidate the captured bind transforms. It then
  projects captured color and coverage onto that same pose, and compresses its
  source-camera depth while preserving the exact source-view projection.
  Perspective captures scale ray x/y with compressed depth; orthographic
  captures compress z only. Plate-locked and source-projective mappings are
  available. A reference-counted renderer layer lease suppresses the canonical
  hierarchy from main color without changing its visibility, materials,
  animation, or transform, and restores the layer on final disposal. Ghost
  mode rejects prepared frames because their current metadata lacks the exact
  projection matrix. It requires one complete retained visual hierarchy and
  holds one pose until explicit recapture. Whole-mesh remains the default;
  optional strict source-shell rejection compares the fragment's capture-view
  depth with the captured linear depth, while repaired-shell mode searches only
  the four adjacent texels before rejecting. Tolerance is expressed in capture
  view units and automatically includes half of the RGBA8 depth quantization
  step. Per-fragment rejection and repair ratios remain explicitly unavailable
  because the experiment performs no GPU texture readback or statistics pass.
  Actor-relative azimuth banks support 1, 4, 8, or 16 exact-pose captures with
  hysteresis, hard cuts, and plate-coordinate ordered-dither handoffs. Every
  resident sector retains its capture textures and private frozen geometry;
  only the previous/current pair may render during a handoff. Elevation
  sectors, regional depth, animation cadence, stylization, offline capture,
  capture pooling, and GPU timing remain unavailable.
  It does not yet add viewer-biased splat axes or custom-material world fog/tint.
  Splat density is independent from the base/depth-parallax sample grid and
  depth quantization; alpha and additive modes disable depth writes but do not
  sort instances within the instanced draw. The explicit 4,096-square capture
  ceiling retains 256 MiB across four RGBA8 outputs and uses another temporary
  64 MiB 32-bit hardware depth texture before driver overhead, so it is a
  manual stress option rather than a suitable default.
  Runtime color is the rendered source, not material albedo. Replacing complete
  application content disposes the attachment and makes its public port
  explicitly stale.
- **Detection:** Inspect `createVoxelSpriteExperiment().readout()` and run
  `./scripts/verify-render.sh`; unit coverage exercises prepared ownership,
  retained-source isolation, fail-atomic source replacement, failed-recapture
  fallback, parent-safe capture-held/blended preparation, frozen multipart
  ghost poses, canonical-source isolation, exact CPU projection invariants,
  failed ghost recapture preservation, and disposal. A Chromium proof uses
  both the retained-capture and admitted-prepared-texture producers through the
  bundled application-host package root; a real-WebGL ghost proof compiles the
  mesh-basic skin/morph/fog path and observes a changed off-axis image without
  a GL error. The reviewed CraftSurvive task 7006
  product run and the
  [campaign evaluation](topics/runtime-voxel-sprite-evaluation.md) found no
  enhancement mode ready for promotion.
- **Follow-up:** Treat the API as an iteration instrument. Judge the held-basis
  result in a real consumer before adding viewer-biased splats, fog/tint, or
  depth-banded splat layers. Do not add offline caching, keyed-animation
  policy, or another public contract before runtime cost or product evidence
  requires it.
- **Last reviewed:** 2026-08-18 / codex

## Authored sky background

- **Status:** implemented-bounded
- **Affected surface:** `render-model`, retained TypeScript projection,
  `renderer-three`, application host, and native webview host
- **Limitation:** One surface can select one content-addressed 2:1 sRGB PNG as
  an equirectangular camera-relative background. It follows camera rotation but
  not translation and contributes no depth, handles, picks, collision,
  environment lighting, reflections, exposure, or post-processing. Cubemaps,
  HDR formats, layered skies, and animated weather are not included.
- **Detection:** Run `./scripts/verify-render.sh`; strict cross-language tests
  cover admission and nullable replacement, backend tests cover refresh and
  disposal, and the Chromium framebuffer proof observes translation stability,
  rotation change, and clear-color restoration. Rusty Engine Demo exact
  revision `3eb9e05830fd82ba861d8ae9f8c1d578ec90d297` supplies the canonical E1M1
  consumer proof: downstream Rust publishes the content-addressed original
  `SKY1` PNG and emits public `DefineTexture` then `SetSkyBackground` operations
  without renderer reach-through or geometry fallback.
- **Follow-up:** Extend representation or lighting only with a concrete
  consumer that cannot use the single-panorama contract.
- **Last reviewed:** 2026-08-14 / codex

## Multi-view render-target composition

- **Status:** implemented-bounded
- **Affected surface:** `render-contracts`, `renderer-three`, and `renderer-host`
- **Limitation:** One surface can render bounded perspective or orthographic views into the primary
  surface or RGBA8 sRGB named targets and can present those targets back onto the primary surface.
  Supported target policy is optional depth24 plus nearest or linear sampling. There is no public
  CPU readback, target-to-target feedback, cubemap, arbitrary target material, post-processing
  graph, cross-backend target interchange, or gameplay minimap/discovery policy.
- **Detection:** Inspect `RendererSurface.viewCompositionReadout()` and run
  `./scripts/verify-render.sh`; contract and backend tests cover quotas, immutable atomic
  replacement, stale revisions, allocation failure, and disposal, while the real Chromium proof
  inspects distinct primary and offscreen pixels at desktop and narrow sizes and observes explicit
  stale-to-current target synchronization after scene and camera changes. Rusty Roguelike exact
  revision `098b6d6c468711b4c149583996ac5147c9f58941`, pinning Engine
  `8673aaa6d0b811195b3904f34d7729c0d6e92530`, supplies the public consumer proof without acquiring
  discovery or visibility authority.
- **Follow-up:** Add another target format or consumption mode only with a concrete consumer and a
  bounded renderer-neutral contract.
- **Last reviewed:** 2026-08-02 / codex

## Browser lighting and shadows

- **Status:** implemented-bounded
- **Affected surface:** retained light descriptors, `renderer-three`, `renderer-host`, and the
  optional Application Host renderer lighting policy
- **Limitation:** The public browser surface can preserve or disable the neutral world/viewmodel rigs
  and can enable a bounded Three shadow map for retained directional, point, and spot requests.
  `mountRustyApplication` forwards this policy only when a downstream explicitly supplies
  `renderer.lighting`; omission retains the compatible neutral-rig, shadows-disabled default.
  Ambient shadow requests remain unsupported. This is not an environment-lighting system,
  post-processing pipeline, automatic light-density policy, or cross-backend shadow guarantee.
- **Detection:** Inspect `RendererSurface.lightingReadout()` and run `./scripts/verify-render.sh`;
  the real browser proof covers default compatibility, independent rig selection, all retained light
  kinds, active/unsupported shadow status, and lifecycle disposal. Rusty Roguelike exact commit
  `e88856aca2b07212e79ca8a9a8cdc904cb49bd61` supplies the public consumer proof for Rust-owned
  authored torch facts, retained-count equality, and visible localized warm falloff.
- **Follow-up:** Add another backend or richer light/shadow resources only with a concrete consumer
  and a bounded renderer-neutral contract.
- **Last reviewed:** 2026-08-24 / codex

## Non-kinematic rigid bodies

- **Status:** provider-implemented-awaiting-consumer
- **Affected surface:** `entity-state`, `svc-collision`, and `engine-spatial`
- **Limitation:**
The #6531 implementation is intentionally bounded to dynamic spheres, cuboids,
and local +Y capsules with positive mass and solver-derived inertia. Dynamic
triangle meshes and arbitrary inertia tensors are not admitted. Discrete and
continuous collision each have explicit per-step motion ceilings; exceeding a
ceiling rejects the step rather than silently accepting tunneling. Repeatability
is certified for one exact Engine/backend build and target, not across backend
upgrades or different floating-point environments. See
[Rigid-body dynamics](topics/rigid-body-dynamics.md).
- **Follow-up:** Close #6531 with exact provider and downstream consumer proof.

## Textured voxel atlas filtering and corpus breadth

- **Status:** accepted-temporary
- **Affected surface:** `asset-catalog`, `svc-mesh`, retained render
  projection, `renderer-three`, and the explicit Studio voxel-consumer
  integration
- **Limitation:** The initial runtime surface contract admits non-interlaced
  RGBA8 PNG albedo, nearest or linear filtering, and one-pixel linear atlas
  gutters without mipmaps. The checked art corpus is one deterministic 16x8
  directional atlas plus color-only, repeat, and atlas-region materials. It
  does not measure GPU time or driver VRAM.
- **Impact:** Large faces and strips retain the greedy partition and the exact
  consumer proves orientation, repetition, replacement, reopen, and disposal,
  but this is not evidence for mipmapped atlas gutters, compressed GPU-native
  formats, normal/emissive texture maps, or broad production art styles.
- **Detection:** See the exact hashes, geometry/resource counts, and browser
  proof in the
  [textured voxel campaign closeout](textured-voxel-campaign-closeout.md), then
  run `./scripts/verify-render.sh` and the explicit Studio voxel-consumer gate.
- **Follow-up:** Add a new format, mip policy, or specialization only when a
  concrete consumer supplies a bounded corpus and demonstrates a reusable
  mechanism gap.
- **Last reviewed:** 2026-08-01 / codex

## Reconstructed voxel surfaces are experimental and color-only

- **Status:** accepted-temporary
- **Affected surface:** `svc-mesh`, `voxel-object-runtime`, voxel-object render
  projection, shared renderer resources, and the explicit Studio comparison
  harness
- **Limitation:** Optional `marchingCubes` and `dualContouring` presentation
  use one binary center-sampled field with no smoothing control. Their
  deterministic material attribution is not a multi-material interface solve,
  and they have no stable repeat/atlas UV projection. Textured reconstructed
  candidates are rejected; only default greedy cubes retain runtime surface
  textures. Marching's closed-loop interior rule is certified for the checked
  binary fixtures, not arbitrary continuous scalar fields.
- **Impact:** Both reconstructions can soften recognizable silhouettes, but
  they chamfer intentional hard voxel steps and cost more triangles/build time.
  They must not be selected as collision, navigation, persistence, or gameplay
  truth. Callers should not present them as a universally superior smooth mode.
- **Detection:** Read
  [reconstructed voxel surfaces](topics/voxel/reconstructed-surfaces.md), run
  `cargo test -p svc-mesh -p voxel-object-runtime -p render-projection --locked`,
  and generate the checked Chromium contact sheet with
  `./scripts/verify-voxel-surface-comparison.sh /home/dev/rusty-engine-voxels`.
- **Follow-up:** Add smoothing, a stronger continuous-field topology policy,
  or stable reconstructed-surface UVs only with a concrete consumer, bounded
  fixtures, and separately reviewed ownership.
- **Introduced by:** task 6599
- **Last reviewed:** 2026-08-09 / codex

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
- **Detection:** The explicit Engine-owned voxel integration proof validates the selected checkout's
  reports, stable collision, missing/corrupt rejection, and real Chromium playback. Exact source and
  consumer identities in that proof are review evidence recorded in Den, not a downstream source
  pin or freshness requirement.
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
  shared semantics require an explicit neutral ownership decision and must be
  promoted at their narrow owner rather than inferred from these three
  compositions.
- **Detection:** See the exact revision and acceptance mapping in the
  [gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md),
  then run the focused mechanics or rules gate for the surface being changed.
- **Follow-up:** Downstream products own content and UI expansion. Create an
  Engine task when a concrete need, implementation, or architecture survey
  exposes a neutral reusable mechanism whose centralization prevents parallel
  authority or correctness drift; no consumer-count threshold applies.
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
  handles, and content-addressed `packedStreamsLeV1`/`packedStreamsLeV2`/
  `packedStreamsLeV3`
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
