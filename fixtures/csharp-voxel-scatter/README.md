# Voxel scatter

Ordinary packaged C# product. A dual-contoured hill of 8 × 2 × 8 chunks of 16³
one-metre voxels, ground over rock, grows grass clumps and low-poly bushes
through `VoxelScenePresentation.SetScatter` (#9546): the product names the
meshes, materials, slots, densities and reach; the Engine places the copies on
the reconstructed ground around the camera, draws each chunk's patch as one
instanced draw, shrinks them away at the edge of their reach and places them
again where the ground is dug. The grass is a masked, double-sided card
material with wind; the bushes are flat shaded and cast shadows.
`generate-grass.py` writes `content/grass.png`.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-voxel-scatter/CsharpVoxelScatter.csproj --live-debug
```

Live debug commands:

- `scatter.inspect`: chunks, scatter patches, copies, chunks left bare by a
  budget, and the time the last scatter request took.
- `scatter.grass <per m²>` and `scatter.bushes <per m²>`: set a density; `0`
  removes that scatter.
- `scatter.radius <metres>`: how far from the camera things grow.
- `scatter.wind <strength>`: the scene wind (`0` stills it).
- `scatter.dig <x> <y> <z> <radius>`: subtract a sphere from the ground; the
  chunks it touches are placed again.
- `scatter.exclude <minX> <minY> <minZ> <maxX> <maxY> <maxZ>`: keep everything
  from growing in a box (`SetScatterExclusions`), as under a built floor;
  `scatter.clearExclusions` removes every box.
- `scatter.camera <x> <y> <z> <targetX> <targetY> <targetZ>`: move the camera;
  patches follow on the next update.
