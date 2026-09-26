# Voxel material atlas proof

Ordinary packaged C# product. Two separately identified authored atlases select
red and blue regions of the same small PNG; the Engine owns all geometry and
rendering. The PNG is a procedural 4×2 RGBA image (two red columns and two blue
columns), authored for this fixture.

Stage with the exact SDK version and matching feed/runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  dotnet msbuild fixtures/csharp-voxel-atlases/CsharpVoxelAtlases.csproj \
  -restore -t:VerifyRustyEngineAot
```

Run the staged Product with the matching `rusty-product-host` and either loader.
Enable live debug on the owned staged manifest, attach a browser, then call:

- `atlases.inspect`: two rendered materials, selected mode and caught rejections.
- `atlases.mode 0`: solid / opaque second material.
- `atlases.mode 1`: decorative / opaque second material.
- `atlases.mode 2`: decorative / blend second material (opaque texture texels).
- `atlases.palette`: remove/reintroduce the second voxel without rebinding;
  reject missing and duplicate base bindings, catch their typed diagnostics,
  and successfully refresh the unchanged palette afterward.

Startup binds both materials before the second voxel exists. It then adds that
voxel, refreshes, and catches two intentional invalid updates. Red and blue cubes
should remain visible across all three modes and repeated palette exercises.
The Blend case tests the reported material configuration, not transparent-water
sorting or transmission. This is Engine fixture evidence, not CraftSurvive
product certification.
