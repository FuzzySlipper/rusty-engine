# Controller interaction proving product

An ordinary safe C# package consumer. Engine supplies input shaping, FPS look,
character collision, interaction selection, raycasts, camera rays and rendering.
This product owns a floor, two competing chests, an occluded chest, their
availability/open state and the choice of controls.

```sh
export RustyEngineFixtureSdkVersion=<matching-pair-version>
dotnet restore CsharpControllerInteraction.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpControllerInteraction.csproj --runtime /path/to/pair/runtime-pack \
  --live-debug
```

Normal controls: WASD/left stick movement, mouse/right stick look, Space/A jump,
Control/B crouch, E/X use, Q/RB cycle. K toggles the left chest's lock and changes
its revision. E/X opens a reachable visible chest through the shared interaction
handler and publishes its product-owned title and contents in the container panel.
Escape closes that panel through the declared `container.close` keyboard mapping.
Green is focused; blue is opened. Debug is optional for gameplay.
The Pause/Resume button at the bottom left pauses and resumes the Engine
runtime through `context.lifecycle`. Its label shows the state the Engine reports.
Each item in the container panel has a Take button that claims
`container.take`. The product moves the item to its taken list and publishes the
panel again, whether running (`Update`) or paused (`HandlePausedIntents`);
`inventory.read` reports the taken items and how many were taken while paused.

M switches to a free cursor, as a map or strategy screen would
(`context.ui.setCursorMode('unlocked')`): pointer lock is released and clicks no
longer take it. A click on the world then carries its cursor position, and the
product casts `CameraQueries.Ray` through it to pick the chest under the cursor;
`interaction.cursor.last` reports that pick. M again returns to mouselook.

The read-only `interaction.query` command returns reticle candidates including
identity/revision, target point, distance, angle, visibility, unknown walking
route, availability reason, focus and successful-use count. Approach using
ordinary movement, cycle and use using ordinary buttons. Using a target checks
fresh facts again; the preceding frame's selection does not authorize a stale
or now-locked target. Opening a chest changes its incarnation and availability.

For a discoverable assisted path, run `interaction.help`, then
`interaction.inspect`. The inspection response gives exact `interaction.use <id>
<revision>` commands. Target-ID use avoids pixel hunting only: it freshly checks
the candidate identity, reach, visibility and locked/open state, then invokes the
same product handler as E/X. A successful response shows the same container panel;
Escape closes only the panel and does not change the chest's open state.

`interaction.cursor x y aspect` asks the same mechanism about a free cursor
without changing look or focus. Supply viewport-local normalized coordinates
(bottom-left origin) and the selected viewport's width/height ratio. A caller
must know these facts; the command does not pretend to read a live host viewport.
Both responses label semantic targeting enabled and look assistance disabled.
These queries do not navigate, aim, open a chest or mutate inventory.

Focused mechanism checks:

```sh
dotnet run --project ../../csharp/Rusty.Engine.Input.Example
dotnet run --project ../../csharp/Rusty.Engine.Interaction.Example
dotnet run --project ../../csharp/Rusty.Engine.CameraQueries.Example
```

Repeatable inspection uses `viewpoint.visit entrance`, `viewpoint.visit near`,
and `viewpoint.visit side`. These explicitly move the player and are labelled
viewpoint-assisted evidence. Query `engine.renderer.presentation` to inspect
submitted camera/view/viewport and canonical publication frontiers. See
[the capture contract](../../docs/presentation-capture.md) for pending states
and the distinction between renderer submission and remote screenshot identity.
