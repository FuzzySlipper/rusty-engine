# C# architectural room

An ordinary packaged C# product that authors a reusable room with a recessed
floor walkway, stepped ceiling, three windows, a movable door, and a connected
passage. The product uses `RoomRecipes.Shell` for the architectural shell and
keeps the portal declarations returned by that call. It then captures the
extracted pieces into an opt-in Engine audit, feeds every declared room join to
`ReadExpectedJoin`, and checks the declared portals with `ReadEnclosure`.

The audit is deliberately opt-in: run the `architecture.audit` debug command
to extract disposable audit meshes and print its `allComplete` report. The
product owns its sampling budget and acceptance policy, while the Engine
performs mesh extraction and analysis.
The door is product state (`E` / controller `X`); input, camera, retained mesh
resources, and rendering are Engine-owned. The DOM module only labels
the controls.

Use a current matching SDK/runtime pair:

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpArchitecturalRoom.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpArchitecturalRoom.csproj \
  --runtime /path/to/pair/runtime-pack --port 40178 --live-debug
```
