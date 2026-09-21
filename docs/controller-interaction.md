# World interaction and controller aim assistance

Use `Rusty.Engine.Interaction` for world containers, doors, talk/use targets and
assisted controller aiming. These are shared Engine SDK components, available
without an agent or debug connection. Product code supplies current targets and
ordinary gameplay actions; Engine supplies selection, hysteresis, fresh use
checks, visibility composition, aiming math and discoverable debug commands.

## Agent green path: stop hunting for a tiny click target

When testing an object that opens a container/UI, first inspect the product's
live-debug catalog. If it includes `interaction.inspect`:

```sh
rusty-live-debug --origin http://127.0.0.1:PORT --command "interaction.help"
rusty-live-debug --origin http://127.0.0.1:PORT --command "interaction.inspect"
# Copy the exact useCommand from the desired candidate, for example:
rusty-live-debug --origin http://127.0.0.1:PORT --command "interaction.use 11 1"
```

`inspect` is read-only. It lists labels, identity/revision, world points,
distance/reach, visibility, availability, focus, route evidence, rejection
reasons and an exact `useCommand`. It includes off-reticle candidates, so looking
a few pixels away does not make an object undiscoverable. The product supplies
a relevant local collection; `truncated` and `totalCandidates` report the
module's output limit (64 by default, configurable up to 256).

`use` is an **explicit assisted action**, not a query. It removes only the need
to place a reticle/pointer precisely. The command reads fresh product facts,
checks identity/revision, query distance, interaction reach, visibility and
availability, then calls the same product handler as ordinary use. It does not
walk, teleport, turn the camera, open locked objects or reach through walls.
The receipt says `assistance: "target-id"`, `performed`, `reason`, and `message`.
Transport success alone does not mean the object was used.

If `OutOfReach`, approach with ordinary controls; if `Occluded`, find a visible
approach; if `StaleTarget`, inspect again. `Locked`/`Unavailable` are product
facts. `route: Unknown` is not a pathfinding result. After use, inspect/capture
the actual UI and perform the intended UI test. When testing picking accuracy
itself, use physical pointer input; target-ID use demonstrates the shared action
and downstream UI, not that a particular screen coordinate was clickable.

If the commands are absent, adopt the component below. Do not invent another
agent-only world registry, browser gameplay hook or repeated screenshot-click
loop. This feature is opt-in; it cannot discover product objects that were never
supplied as candidates.

## Product green path: one component, one ordinary action

Implement `IWorldInteractionScene`, construct one `WorldInteraction`, and use
it for both human focus/use and the optional debug module:

```csharp
sealed class Containers : IWorldInteractionScene
{
    public InteractionSceneSnapshot ReadInteraction()
    {
        // Build current candidate facts from your ordinary product state.
        // Use InteractionVisibilityQuery.Cast over the existing Spatial session.
        return new(CurrentQuery(), CurrentCandidates(), CurrentStamp(), "open container");
    }

    public InteractionActionResult UseInteraction(InteractionTarget target)
    {
        // The SAME ordinary action: applies current product rules, opens UI,
        // publishes through Engine Ui. No special debug inventory mutation.
        return OpenContainer(target);
    }
}

var interactions = new WorldInteraction(containers);
var interactionDebug = new InteractionDebugModule(interactions);
// In the product's IDebugCommandModuleSource.RegisterDebugCommands:
registrar.Register(interactionDebug);
// During ordinary admitted input/update:
interactions.Update(cycleDirection); // -1, 0, +1
if (usePressed) interactions.UseFocused();
```

The illustrative `Current*` and `OpenContainer` methods are product methods,
not additional Engine APIs. Namespaces are `Rusty.Engine.Interaction` and
`Rusty.Engine.Debugging`. See the runnable
[container fixture](../fixtures/csharp-controller-interaction/README.md).

`ReadInteraction` returns a call-local `InteractionSceneSnapshot(Query,
Candidates, Stamp, Action)`. Call these operations on the ordinary serialized
product update/debug boundary. Keep the supplied memory valid for that call;
there is no cached second world. The action handler retains action-specific
rules and reports whether it performed the action. `targetedUseEnabled: false`
disables target-ID actions while keeping observation and ordinary focus usable.

## Sticky reticle and cursor acquisition

`InteractionFocus` is also independently usable. Supply current
`InteractionCandidate` values: stable ID/incarnation revision, label, world
point, reach distance, availability, visibility and optional ranking priority.
Increment the revision when an ID represents a different incarnation. Removed,
locked, unavailable and occluded targets cannot be used.

`InteractionQuery` contains ray origin/direction, acquisition/release half-angles
in radians, distances in world units, and ranking weights. Release bounds enclose
acquisition bounds. Higher priority wins, then weighted angular/distance score,
then stable ID. A selected eligible target persists within release bounds even
when another target becomes slightly closer. Explicit cycling chooses another
eligible acquisition target. Selection does not perform an action.

`Observe` returns candidates within release bounds; `Inspect` includes all
supplied candidates for discovery. Both are read-only. `Revalidate` rechecks
ordinary focus with fresh facts. `RevalidateTarget` is explicit target-ID
assistance: it ignores angular acquisition but retains maximum query distance,
reach, identity, availability and visibility. `WorldInteraction` composes these
checks with the ordinary handler so callers need not duplicate that sequence.

A reticle uses eye origin and normal look direction. A free cursor uses
`CameraQueries.Ray` with the product camera and explicit viewport aspect;
coordinates are viewport-local, normalized and bottom-left based. Use
`InteractionQuery.DistanceOrigin` to measure reach from the player rather than
an offset orthographic cursor ray. Cursor selection is not mouse-look input.

`InteractionVisibilityQuery.Cast` uses full retained `Spatial.CastRay`, including
static meshes, plus supplied entity colliders. Ignore the target's own collider
when testing its center. Shared Perception visibility also uses retained world
occlusion, including static meshes; observer and target entity colliders remain
excluded from their own visibility test.

## Controller aiming and shot magnetism

`AimAssist` reuses `InteractionFocus` for sticky target acquisition. Supply the
same fresh visibility/availability facts, with product-selected weapon range
as candidate reach. Product code chooses hostile targets, aim points, activation
policy and tuning; human and agent controller input take the same path.

Call `Update(candidates, query, lookDeltaRadians, simulationSeconds, config,
active)` before applying controller look. Deltas are already time-integrated:
X is yaw-right, Y is pitch-up. The query direction is the look before those
deltas. The result reports focus, adjusted deltas, correction and slowdown.
`AimAssistConfig` selects slowdown angle/minimum scale, maximum tracking radians
per second, shot cone and maximum shot correction radians. Tracking uses admitted
simulation time; deliberate input away from a target receives no slowdown or
tracking. When inactive, focus clears. Apply the returned delta through normal
`Look.IntegrateClamped`; do not add a second simulation or input loop.

At fire time, `CorrectShot` revalidates the retained target with fresh facts and
returns a direction corrected only inside the shot cone and angular limit.
**Cast that direction through ordinary collision.** An assisted direction is
not a guaranteed hit and cannot authorize damage through cover. For hitscan this
is a redirected ray; it does not implement curved/homing projectile simulation.
Disabling tracking or shot correction with zero tuning is supported independently.

## FPS input baseline

`Rusty.Engine.Input.FpsInput` composes persistent `PhysicalInputState`, radial
deadzones, pointer/stick look and configurable bindings. `Standard` provides
WASD/left-stick movement, mouse/right-stick look, Space/A jump, Control/B crouch,
Shift/left-stick-click sprint and E/X use. Products select fire/cycling bindings.

Call `Consume(update.Input, simulationSeconds)` every admitted update, including
empty batches. Pointer sensitivity is radians per pointer unit; controller look
is radians per simulation second at full deflection. `IntegrateLook` composes
them separately. Positive raw browser Y is down; the standard config inverts
look Y to positive-up pitch. Apply one-shot presses once, and character movement
once per admitted fixed step. Clear input/focus when disabling gameplay.

Host sampling, disconnect/focus/context cleanup stay in Engine. Remote device
injection, sessions, holds, cancellation and screenshots stay in crew-services.
The managed helpers retain no native pointers, renderer or spatial world, need
no ABI extension, and are shipped in the ordinary matched SDK/runtime pair.
