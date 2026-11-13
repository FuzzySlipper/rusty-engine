# Level build

Ordinary packaged C# product. It builds a generated level of 289² stone
blocks, 1,002,252 faceted triangles (3,006,756 vertices, no shared vertices),
and shows it in 32 m parts. Two modes:

- `inline`: in one update, build the streams, `CreateMeshResource`,
  `PartitionMesh`, take every part and create its appearance.
- `deferred`: build the streams with `context.Jobs.Run` on the worker lane,
  in bands of 4 rows (73 meshes). Then, each update, create bands with
  `Deferred`, partition each with `Deferred` and dispose its source, until 2 ms
  of the update are spent. Once every band's `ReadMeshPartition` is `Ready`,
  take all the parts (913, deferred) and create their appearances. Once
  `ReadResourcePreparation().CompletedBatch` reaches that call's batch, show
  them. A band's streams are copied in the call that creates it, so the bands
  keep that copy, about 70 ms for the whole level, from landing in one
  update.

Until a build is ready the previous level, or nothing, is shown. A block turns
every update so a frozen update shows.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-level-build/CsharpLevelBuild.csproj --live-debug
```

Live debug commands:

- `build.start [inline|deferred] [blocks-per-side]`: starts a build (289 by
  default). When it is shown and 30 more updates have run, the host log gets a
  `LEVEL_BUILD` line, also returned by `build.report`: triangles, meshes,
  the milliseconds to build the streams, of the `CreateMeshResource` calls
  (summed, and the slowest) and of the `PartitionMesh` calls, the parts, until the cut was ready, of taking the parts, and
  until the level was ready; then the steps admitted and dropped, and the
  update callback's and the interval between updates' p50, p99 and maximum,
  with the intervals over two fixed steps (`hitches`).
- `build.cancel`: cancels a running build in whatever phase (disposing its
  job or partition) and reports the Graphics counts, which return to what
  they were before it.
- `build.report`: the last report and the Graphics counts.

A restart cancels a running build: the job with the lifecycle generation,
the partition by the product's `Restart`.

## Navigation

`nav.start [inline|deferred|deferred-edit]` derives a collision-derived
navigation grid of 321² = 103,041 one-metre cells over the level: the blocks
and 16 m of floor around them, for a 0.3 m body that steps 0.5 m. The first run builds the
blocks on the worker lane and admits them and the floor as static collision,
deferred (73 band assets and the floor), into a session of its own. Each run
then publishes a small grid synchronously (40 m square over the level's
corner), and in the next update makes the whole grid's call, in it
(`inline`) or with `Deferred` (`deferred`). Until `ReadCollisionNavigation`
says the grid is installed, each update checks that the small grid still
answers: its revision and hash, and the four route queries asked of it before.

Thirty updates after the grid is installed, the host log gets a
`LEVEL_NAVIGATION` line, also returned by `nav.report`: the grid's cells,
walkable cells, components, edge tests and derivation milliseconds, the call's
milliseconds, the updates it was pending and how many of those the small grid
answered, and the update callback's p50, p99 and maximum from the update that
made the call. Then the same grid is derived synchronously as a second grid and
compared: `matchesSynchronous` is the hash, cells, components and the four
route queries all agreeing (edge tests differ when the inline call reuses the
small grid's columns). That comparison's update comes after the measured ones.

`nav.start deferred-edit` also adds a 1 m box to the collision synchronously
in the first update the grid is pending, while the worker holds the scene, and
once the grid is installed removes it and adds it again, reporting both calls'
milliseconds (`editWhileDerivingMs`, `editAfterMs`); those calls count in the
measured updates.
