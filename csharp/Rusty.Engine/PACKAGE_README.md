# Rusty.Engine C# SDK

This package is the managed C# surface for Rusty Engine products. It contains
the generated Engine contracts and values, reusable managed helpers, and the
product generator used to create the CoreCLR and NativeAOT product boundary.

Reference one immutable `Rusty.Engine` package built from the same Engine
revision as the runtime pack that hosts the product. The normal development
path is the runtime pack's `rusty dev` command; NativeAOT is an explicit
fidelity and release path. Product code should use the public service APIs and
must not add handwritten P/Invoke, ABI declarations, or a second host.

Engine services and native handles are callback-confined: call them synchronously
from an Engine-invoked product callback that permits the operation. This includes
read-only calls and disposal. Do not call services from background tasks, timers,
finalizers or async continuations after the callback returns. The callback lane
is serialized but does not promise a permanent managed thread ID.
`SimulationScheduler` runs synchronously within admitted updates; it is not a
worker scheduler. Pure product computation can use copied data off-thread, with
bounded results admitted later from Update. See the repository's
`docs/world-streaming-contract.md` for the supported pattern and ownership rules.

The package carries its generated ABI identity in build metadata. Select a
matching runtime pack rather than attempting compatibility negotiation or
recreating a missing Engine capability in the product.

For world containers, doors and talk targets, use `Rusty.Engine.Interaction`:
`WorldInteraction` shares fresh target checks and ordinary product actions,
`InteractionFocus` handles sticky acquisition, and `AimAssist` supplies bounded
controller assistance. Register `Rusty.Engine.Debugging.InteractionDebugModule`
with the product's generated debug catalog. Agents can discover
`interaction.inspect` and copy `interaction.use <id> <revision>` instead of
repeatedly guessing screen coordinates; target-ID use preserves visibility,
reach and product rules. Verify the resulting UI separately.

The source guide is `docs/controller-interaction.md` in the Engine repository.

## Generated level artifacts

`Spatial.ReplaceContentArtifact` admits an Engine-format collision/navigation
artifact from a `ContentReference` into an existing `SpatialSession`. Engine
validation and preparation finish before static-mesh collision and planar
navigation are replaced together. Voxel content and residency remain intact.
A refused operation leaves the previous spatial state intact and throws
`EngineCallException` with service `Spatial`, operation `ReplaceContentArtifact`
and a named diagnostic (for example `CSHARP_SPATIAL_CONTENT_BOUNDS`).

Generation recipes, required connectivity, portal/socket pairing, gameplay
meaning and Procgen provenance checks belong to the generator/importer or
product. Convert the Procgen floor schema to the Engine spatial format before
calling this API; the Engine consumes precomputed navigation facts. Content
owns byte identity, and `Content.ResolveReference` can select an expected
digest. See `docs/csharp-sdk.md#generated-level-artifact-admission` and
`fixtures/csharp-spatial-artifact/valid.json` in the Engine repository for the format
and a complete call example.

## Collision navigation coordinates

`Spatial.ReplaceCollisionNavigation` samples a region on a world-aligned grid
whose origin is `(0, 0, 0)` in the current session frame. `WorldMin`/`WorldMax`
select the sampled region; never subtract them when constructing path cells.
For cell size `s`, cell coordinates are `floor(worldX / s)`,
`floor(supportHeight / s)`, `floor(worldZ / s)`, using mathematical floor for
negative coordinates too. The Y level uses the standing surface, not the solid
voxel below it or the character center. `ChunkSize` does not shift this grid.

For live foot positions, `Spatial.EvaluateNavigationStep` resolves the nearest
retained support in each endpoint's X/Z column within `min(s * 0.25, 0.1) + 0.001`
world units and returns a world-space `NextWaypoint`. This avoids guessing
levels or scanning cells. A reported walkable count includes every retained
support level; it does not imply that every point inside the publication box
is walkable. See `docs/csharp-sdk.md#collision-navigation-coordinates` and the
packaged `fixtures/csharp-navigation-mapping` example in the Engine repository.

## Particle bursts

`Presentation.EmitParticles` needs no prior signal or appearance registration.
Give each burst a non-empty `SignalId` label (repeats are distinct bursts),
explicitly set `Anchor.Kind` and `Visual`, and supply `MaxParticles`, positive
ordered lifetimes, and both size/color curves (2–8 keys, ages strictly
increasing from 0 to 1). `BurstCount` must not
exceed `MaxParticles` (at most 1024); `Seed` must fit 53 bits. For a billboard,
open an image with `Graphics.OpenResource`, pass its handle as `Sprite`, and set
`SpriteFrameCount = 1` for a static sprite. Animated sprites need a positive
frame rate. Cubes need no sprite and use zero flipbook rate.

Invalid descriptors raise `EngineCallException` with a named diagnostic.
A caught emission refusal preserves staged presentation; an exception escaping
the product callback still faults that callback. A valid optional burst may
return `Admitted`, `Clamped` or `Dropped` according to presentation capacity.
See the source SDK guide and `fixtures/csharp-particle-emission` for a complete
executable example, including collision.
