# Level build

Ordinary packaged C# product. It builds a generated level of 289² stone
blocks, 1,002,252 faceted triangles (3,006,756 vertices, no shared vertices),
and shows it in 32 m parts. Two modes:

- `inline`: in one update, build the streams, `CreateMeshResource`,
  `PartitionMesh`, take every part and create its appearance.
- `deferred`: build the streams with `context.Jobs.Run` on the worker lane;
  in the update that takes them, create the mesh with `Deferred`, partition it
  with `Deferred` and dispose the source; once `ReadMeshPartition` is `Ready`,
  take the parts (deferred) and create their appearances; once
  `ReadResourcePreparation().CompletedBatch` reaches that call's batch, show
  them.

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
  `LEVEL_BUILD` line, also returned by `build.report`: triangles, the
  milliseconds to build the streams, of the `CreateMeshResource` and
  `PartitionMesh` calls, until the cut was ready, of taking the parts, and
  until the level was ready; then the steps admitted and dropped, and the
  update callback's and the interval between updates' p50, p99 and maximum,
  with the intervals over two fixed steps (`hitches`).
- `build.cancel`: cancels a running build in whatever phase (disposing its
  job or partition) and reports the Graphics counts, which return to what
  they were before it.
- `build.report`: the last report and the Graphics counts.

A restart cancels a running build: the job with the lifecycle generation,
the partition by the product's `Restart`.
