# Particle texture ownership (#8851)

**Finding (confirmed on current source).**
- `effects.rs::particle_texture` appended a bind group per content hash to
  `particle_textures` and never removed it.
- Destroyed emitters, finished bursts and dead particles all left their
  texture bound. That kept the GPU texture alive until the whole `Renderer`
  was dropped.
- `forget_texture` only cleared sprite bindings.

**Ownership now.** A texture lives while something that can draw it exists.

- **Holders.** Each emitter holds the cache slot of its current visual. Each
  live particle holds the slot it spawned with, so after an emitter's visual
  changes, its earlier particles keep drawing the old texture.
- **Counting.** `Particles.texture_users` counts holders per slot at the
  points where they appear or leave:
  - an emit, create or visual-changing update;
  - particles spawning;
  - particles dying in `advance`;
  - an explicit destroy, which removes the emitter and its particles;
  - a finished burst leaving.

  A slot whose count reaches zero is queued in `released`.
- **Cache.** `particle_textures` is a slot table. After each presentation op
  and each `advance`, the effects cache drops the queued slots' bind groups
  and content-hash entries, and reuses the slots for later textures. An op
  the particle system refuses (duplicate or unknown handle) drops the slot it
  had just loaded. There is no per-frame sweep and no cache budget.
- **Retained texture release.** A sprite taken from a retained texture of the
  same id holds that texture's view through its bind group. Released or
  redefined textures therefore stay alive exactly while a particle user
  remains, then go with the slot.
- **Side effect.** Frame preparation uses each particle's slot directly. It
  no longer hashes the content-hash string of every particle in every view.

**Regression test.**
`tests/effects.rs` `particle_textures_are_released_when_their_last_emitter_and_particle_leave`
uses three distinct textures:

| Step | Live bindings |
|---|---|
| burst with A, fountain with B | 2 |
| fountain's visual changes to C | 3 (B's particles are still alive) |
| A's burst and B's particles age out; the fountain still renders with C | 1 |
| fountain destroyed; particles and emitters (0, 0); the image changes | 0 |
| a new burst with A reuses a slot, then ages out | 1, then 0 |

With slot dropping disabled (the old behaviour), the test fails at the third
step: `left: 3, right: 1`.

**Checks.** `cargo test -p render-wgpu` passes all 69 tests. That includes
the `particles` screenshot, whose slots are assigned in the same order as
before. Clippy `-D warnings` is clean.
