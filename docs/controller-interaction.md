# Controller and interaction composition

`Rusty.Engine.Input` is an optional managed composition over ordinary admitted
`ProductInputEvent` batches. It works without debug facilities or crew-services.
Products retain their input configuration, gameplay state and actions.

## FPS baseline

Create a product-held `FpsInput` with a selected `FpsInputConfig`. `Standard`
provides WASD/left-stick movement, mouse/right-stick look, Space/A jump,
Control/B crouch, Shift/left-stick-click sprint, and E/X use. These are opt-in
bindings; constructing the helper does not change host input mappings.

Call `Consume(update.Input, simulationSeconds)` once per admitted update, then
`IntegrateLook(yourLookState, frame)`. Pass `frame.Movement`, jump/crouch facts
and the returned yaw to the normal `Spatial.ProposeCharacterStep` path. Apply
speed/sprint and interaction policy in the product. When the update admits
multiple fixed steps, step the character once for each admitted step; apply
one-shot presses once. This adds no clock or simulation loop.

Pointer sensitivity is radians per pointer unit. Stick sensitivity is radians
per simulation second at full deflection. They are composed separately, so
changing mouse sensitivity does not change controller turn speed. Positive raw
browser Y means down; the standard configuration inverts it into Engine's
positive-up pitch. Products can select inversion and independent rates.

The pieces are independently useful:

- `PhysicalInputState`: held state, per-batch pressed/released edges, summed
  pointer displacement, latest stick axes and analog button/trigger values.
  Mapped/direct events are ignored to avoid applying physical input twice.
  Consume every update, including empty batches. Clear neutralizes held state
  and exposes releases while discarding pending presses and mouse movement.
- `AnalogInput`: radial inner/outer deadzone and exponent, scalar trigger
  remapping, and bounded keyboard/analog movement composition. Partial stick
  magnitude remains partial; a diagonal does not increase maximum speed.
- `FpsInput`: the baseline composition, with its `Physical` state available for
  product-specific actions and cycling. A new configuration may reuse that
  state; clear it when the product disables its gameplay input context.

Host physical sampling, controller disconnect, focus and context lifecycle
remain upstream host responsibilities. Device injection, remote sessions,
timing, cancellation, screenshots and recovery remain crew-services concerns.

## Shared interaction focus

`Rusty.Engine.Interaction.InteractionFocus` is product-held selection state,
not a registry. Supply current `InteractionCandidate` values at each update:
stable ID/incarnation revision, label, world target point, interaction reach,
availability, visibility and ranking priority. Increment the revision when an
ID represents a different target incarnation. Removing a target removes its
focus. Products decide which objects qualify and what using them means.

`InteractionQuery` supplies a world-space ray, acquisition/release half-angles
in radians, distances in world units and ranking weights. An optional `DistanceOrigin` keeps
player reach separate from an offset orthographic cursor ray origin. The caller supplies
a bounded candidate collection; the helper evaluates it synchronously and
returns candidates within release bounds. Release bounds must enclose
acquisition bounds. Higher product priority wins; ties use weighted angle and
distance, then stable ID. An eligible selected target persists inside release
bounds even if a neighbour becomes slightly closer to the reticle. Pass an
explicit -1/+1 cycle direction to select another eligible acquisition target.

`Observe` is read-only and uses the same calculations as human focus. It
returns target identity, world point, distance, angular error, selection and
rejection facts. Out-of-reach candidates can guide an approach without becoming
usable. Unknown visibility is distinct from visible or occluded. A line of
sight is never reported as proof of a walking route: route evidence defaults
to `Unknown`. Locked/unavailable/invalid targets cannot be focused or used.

Immediately before the ordinary use action, call `Revalidate` with the target
identity and **fresh** candidate facts. Only `Ready` permits the product action.
An old observation is not authorization: moved, blocked, removed, locked or
reincarnated targets must be checked again. Selection alone performs no look,
movement or gameplay effect. This slice does not enable look assistance.

`InteractionVisibilityQuery.Cast` composes existing `Spatial.CastRay` over the
product's ordinary session and entity colliders. Ignore the selected target's
own collider when testing its center. The helper does not retain another
spatial world. Products may instead supply their own visibility facts, including
existing `Perception.QueryVisibility` results. Missing observations stay unknown.

## Reticle, free cursor and agent queries

A first-person reticle uses the product's eye origin and normal look direction.
A free cursor supplies its own ray; it does not become a mouse-look delta.
`CameraQueries.Project` and `Ray` provide stateless perspective/orthographic
math over the existing camera descriptor and an explicitly supplied viewport
aspect. Coordinates are viewport-local, normalized, bottom-left based. This is
not live renderer/viewport readback; callers must know the selected view's aspect
rather than guessing it or treating a screenshot as a presentation revision.

Expose optional query methods through the existing generated
`DebugCommand`/`IDebugCommandModuleSource` facilities. No additional adapter or
query registry is required. Include selection mode and assistance flags in the
product's response so semantic assistance is distinguishable from unaided
visual discovery. Queries do not activate objects, navigate or mutate inventory.
Screenshot freshness remains separate work (#7816).

The [ordinary C# proving scene](../fixtures/csharp-controller-interaction/README.md)
uses these helpers for movement, competing chests, a wall and product-owned
open/locked state. The optional query reports exactly the scene's ordinary focus
facts. Its default gameplay needs neither an agent nor a debug connection.

## Existing mechanisms reused

The host already admits four standard axes, controller digital edges and analog
button values through the generated safe input surface. Existing `Look`,
`Spatial.ProposeCharacterStep`, `Spatial.CastRay`, camera descriptors, appearance
snapshots and compiled debug commands provide the underlying mechanisms. The
new managed helpers fill composition gaps; they add no native handles, ABI
protocol, renderer or browser gameplay state. Menu navigation, remapping UI,
haptics, multiple-player device assignment and optional look assistance are
separate extensions, not hidden behavior of this baseline.

For repeatable product-owned inspection poses and renderer submission facts,
see [viewpoints and presentation observations](presentation-capture.md).
