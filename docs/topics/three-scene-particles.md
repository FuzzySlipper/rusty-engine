# Three scene particles

Status: implemented runtime and browser characterization for task 6926.

## Ownership

Particles are disposable presentation. Engine owns their bounded descriptor,
strict decoding, deterministic seeded simulation, renderer resource lifetime,
approximate local collision, Three realization, and diagnostics. A downstream
game owns why an effect happens and every authoritative consequence.

Particle contacts must never become damage, terrain edits, inventory drops,
projectile hits, or persistent debris. If an effect later becomes
gameplay-significant, promote it to an ordinary downstream entity and named
gameplay mechanism. That owner may publish a separate particle effect from its
authoritative result; it must not read particle-host positions or collision
results back into gameplay.

## Contract and compatibility

`ParticleEmitterDescriptor.visual` is a closed discriminated choice:

- `billboard` carries one content-identified sprite and its flipbook frame
  count;
- `cube` is an asset-free primitive for chunky debris.

An arbitrary mesh visual was deliberately not admitted. The voxel-debris use
case is served by instanced cubes, while a mesh variant would add mesh resource,
material, batching, and failure combinations without a measured consumer need.

Older serialized descriptors with a top-level `sprite` remain accepted and are
interpreted as `visual: { kind: "billboard", sprite }`. New Rust serialization
emits only `visual`. A descriptor may not provide both forms. Cube visuals
require zero flipbook rate. Rust source that constructed the old field directly
migrates explicitly from `sprite` to
`visual: ParticleVisual::Billboard { sprite }` and initializes `collision` to
`None`; the JSON compatibility path is intentionally broader than the rolling
source API.

The ordinary retained emitter lifecycle is unchanged: one-shot `emit` signals
remain idempotent, while retained handles support create, update, and destroy.
Sprite and cube emitters share seeded lifetime, velocity, acceleration, size,
color, budget, and visibility behavior.

## Approximate collision

Collision is optional and contains at most 16 planes or AABBs. Coordinates are
relative to each particle's spawn anchor. This makes a captured coarse floor,
wall, or nearby terrain box move-independent and prevents per-particle queries
against the live collision world.

The host uses a sphere proxy with a bounded radius even when the visual is a
cube. Segment sweeps test the complete movement interval, so accepted-scale
fast debris does not depend on endpoint overlap. A single advance resolves at
most four successive contacts. The descriptor bounds restitution and
tangential friction to `[0, 1]`, maximum impacts to `1..=32`, and sleep speed to
`0..=100`. Reaching the impact limit either sleeps or kills the particle.

Readouts expose collision tests and impacts. They are diagnostics, not contact
events. Planes must have normalized finite normals; AABBs must have finite,
strictly ordered extents. Invalid collision rejects at the Rust projector or
strict TypeScript border before host mutation.

## Three realization and bounds

`RendererSurface.createParticleSink()` returns a backend-neutral lifecycle
surface while privately attaching a Three group to the existing scene.
Billboards use dynamic `THREE.Points` buffers and one unlit atlas shader per
sprite identity. Cubes use dynamic `THREE.InstancedMesh` batches. The default
batch capacity is 256 and is bounded to 4096.

Creation, update, destruction, compaction, and disposal are explicit. Sink
creation failure rolls back a whole burst, leaving its signal retryable. The
sink reports active particles and batches, billboard/cube batch counts,
allocated slots, and its high-water mark. The host adds emitted, dropped,
collision, and retained-emitter counts. Surface disposal releases every sink,
geometry, material, and texture even if a caller omitted explicit sink cleanup.

The retained Rust defaults remain 64 active emitters, 1024 particles per
emitter, and 4096 reserved particles across a frame. These are presentation
budgets rather than a promise that every platform should render the maximum.

## Browser characterization

The real Chromium gate creates, updates for eight frames, renders, and tears
down DOM billboards, Three billboards, and instanced cubes at 64, 512, and 4096
particles. The run below used hosted headless Chromium with SwiftShader on
2026-08-12. Timings characterize this environment; the browser assertions own
bounded allocation, draw calls, and teardown rather than brittle time limits.

| Mode | Count | Create ms | Avg update + render ms | Draw-call delta | Slots | Teardown ms |
|---|---:|---:|---:|---:|---:|---:|
| DOM billboard | 64 | 1.20 | 2.95 | 0 | 64 | 0.40 |
| Three billboard | 64 | 0.90 | 1.86 | 1 | 256 | 0.50 |
| Instanced cube | 64 | 1.20 | 1.71 | 1 | 256 | 0.20 |
| DOM billboard | 512 | 7.90 | 12.06 | 0 | 512 | 1.00 |
| Three billboard | 512 | 1.40 | 1.39 | 2 | 512 | 0.20 |
| Instanced cube | 512 | 0.30 | 1.05 | 2 | 512 | 0.20 |
| DOM billboard | 4096 | 44.00 | 53.11 | 0 | 4096 | 7.40 |
| Three billboard | 4096 | 5.20 | 4.83 | 16 | 4096 | 1.50 |
| Instanced cube | 4096 | 6.30 | 3.85 | 16 | 4096 | 1.50 |

DOM particles incur no WebGL draw calls but scale as one element per particle.
The Three paths keep one draw call per 256-slot batch and avoid per-particle DOM
layout. All nine cases read back zero active particles, batches, and allocated
slots after teardown while preserving the exact high-water mark.

## Evidence

```bash
cargo test -p render-presentation --locked
pnpm --dir render run typecheck
pnpm --dir render --filter @rusty-engine/render-contracts test
pnpm --dir render --filter @rusty-engine/renderer-host test
pnpm --dir render --filter @rusty-engine/renderer-three test
PLAYWRIGHT_RENDER_PORT=4187 pnpm --dir render run test:browser
cd render && PLAYWRIGHT_RENDER_PORT=4187 pnpm exec playwright test \
  browser/renderer.browser.spec.ts --config playwright.config.ts --workers=1
```

The browser proof logs the complete `PARTICLE_PERF` JSON so future runs can be
compared without turning one machine's timing into a release threshold.
