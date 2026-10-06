# Voxel level of detail

Ordinary packaged C# product. A dual-contoured landscape of 16 × 3 × 16 chunks
of 16³ one-metre voxels, admitted in one residency transaction, is drawn with
`VoxelScenePresentation.SetLevelOfDetail`: chunks farther than the coarse
distance (64 m at start) from the camera are drawn from the Engine's coarse
meshes. Collision keeps every chunk at full resolution.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-voxel-lod/CsharpVoxelLod.csproj --live-debug
```

Live debug commands:

- `lod.inspect`: chunk count, chunks drawn coarse and the coarse distance.
- `lod.distance <metres>`: set the coarse distance; `0` draws every chunk at
  full resolution.
- `lod.camera <x> <y> <z> <targetX> <targetY> <targetZ>`: move the camera; the
  Engine redraws chunks that cross the distance on the next update.

Compare `engine.renderer.snapshot` captures at the same camera with
`lod.distance 0` and `lod.distance 64` for seam quality, and render each with
`rusty-scene-render` for frame time.
