# Voxel stamp

Ordinary packaged C# product. A dual-contoured block of rock (4 × 2 × 4 chunks
of 16³ one-metre voxels, solid up to 20 m) is shaped with implicit field
stamps (#9505): the product composes a field with `ImplicitRecipe` and calls
`Voxel.StampImplicit`, which applies the node's surface to the session's
densities as a density brush of that shape.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-voxel-stamp/CsharpVoxelStamp.csproj --live-debug
```

Live debug commands:

- `stamp.caves`: subtract a smooth union of three capsules, a tunnel system
  entering the block's front face.
- `stamp.boulder`: add a sphere roughened by spectral waves, in moss, on top
  of the block.
- `stamp.inspect`: the chunk count and the last stamp's receipt (changed
  voxels, solidity changes, rebuilt chunks, meshing and call time).
- `stamp.camera <x> <y> <z> <targetX> <targetY> <targetZ>`: move the camera.
