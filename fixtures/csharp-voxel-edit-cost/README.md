# Voxel edit cost

Ordinary packaged C# product. The dual-contoured landscape of
[`csharp-voxel-lod`](../csharp-voxel-lod/README.md), 16 × 3 × 16 chunks of 16³
one-metre voxels drawn with level of detail at 64 m, is edited through
`Voxel.ApplyEdits` and redrawn through `VoxelScenePresentation.RefreshScene`.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-voxel-edit-cost/CsharpVoxelEditCost.csproj --live-debug
```

Live debug commands:

- `edits.inspect`: chunk count, chunks drawn coarse, and the admission and
  first projection times.
- `edits.run [repeats]`: runs each case `repeats` times (9 by default), each
  run setting the cells to the other material, and reports the median and
  worst `ApplyEdits` and `RefreshScene` milliseconds. The last run's
  breakdown follows: rebuilt chunks, mesh, collider and settle microseconds
  from the edit receipt, and from the presentation readout whether the
  refresh compared every chunk, the chunks it compared, the meshes it sent,
  the chunks whose level of detail it chose and its projection microseconds.
- `stream.start [updates]`: streams for that many updates (600 by default).
  Each update admits the next 8 chunks of a column ahead of the landscape and
  evicts 8 a column behind in one deferred `ApplyResidency`, then calls
  `RefreshScene`, so about 768 chunks stay resident; the camera moves a column
  with each column admitted.
- `stream.report`: the stream's `ApplyResidency`, `RefreshScene` and combined
  milliseconds (median, 95th percentile, worst) and the median of each phase
  the receipt and readout report: settle, write and rebuild, projection and
  scatter, chunks compared and meshes sent.

The cases, near the camera:

- 1 cell, inside one chunk;
- 50 cells inside one chunk;
- 256 cells, a 16 × 16 patch filling one chunk's columns;
- 256 cells, 16 in each of a 4 × 4 block of chunks;
- two one-cell edits in different chunks, then one refresh.

Times are the product's stopwatch around each call. Compare runs on the same
machine and pair, and note the machine's load.
