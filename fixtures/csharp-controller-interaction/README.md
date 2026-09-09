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
its revision. Green is focused; blue is opened. Debug is optional for gameplay.

The read-only `interaction.query` command returns reticle candidates including
identity/revision, target point, distance, angle, visibility, unknown walking
route, availability reason, focus and successful-use count. Approach using
ordinary movement, cycle and use using ordinary buttons. Using a target checks
fresh facts again; the preceding frame's selection does not authorize a stale
or now-locked target. Opening a chest changes its incarnation and availability.

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
