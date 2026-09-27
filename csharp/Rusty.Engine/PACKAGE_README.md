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
