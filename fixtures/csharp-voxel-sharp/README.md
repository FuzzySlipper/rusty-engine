# Voxel sharp cuts

Ordinary packaged C# product. A dual-contoured block of rock (2 × 2 × 2 chunks
of 16³ one-metre voxels, solid up to 14 m) at Sharp placement with flat facets
(35-degree crease) is cut with the Engine's density brushes (#9504): a box out
of its near corner and a sphere of rock on top. Brushes record the exact
normal of their shape at every crossing they cut, so Sharp placement puts the
cut's edges where they are. The product uses only brushes and surface
characters, so the same build runs on a runtime from before #9504 for a
same-camera comparison.

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-voxel-sharp/CsharpVoxelSharp.csproj --live-debug
```

Live debug commands: `sharp.cut`, `sharp.knob`, `sharp.inspect` and
`sharp.camera <x> <y> <z> <targetX> <targetY> <targetZ>`.
