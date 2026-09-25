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
