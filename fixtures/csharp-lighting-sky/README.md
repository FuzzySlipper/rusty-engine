# Lighting and sky mechanism fixture

Uses only packaged `Rusty.Engine` C# APIs. Build/stage with the explicit test
SDK version and its local feed, inherited by generated child builds:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  dotnet msbuild CsharpLightingSky.csproj -restore -t:VerifyRustyEngineAot
```

Run the staged bundle with the matching runtime pack. Debug commands and the
bounded proof contract are in [lighting and skies](../../docs/lighting-and-sky.md).
`generate-skies.py` regenerates the two small authored PNG fixture panoramas,
and `generate-particles.py` the torch's flame flipbook, ember and smoke sprites.
`content/tree.glb` is rusty-craftsurvive's normalized low-poly broadleaf tree
(`content/map-models/broadleaf.glb`, a Tripo generation recorded in that
repository's `content/map-models.sources.json`), the flat-shading pair's prop
(`lighting.facets`) and the tree the wind bends (`lighting.wind`).
`content/wave.wgsl` is the banner's product shader: a displace stage that
waves the card from its pole edge by the scene's wind.
The fixture owns the supplied clock value and source light descriptor; Engine
owns rendering, light sampling, scene persistence primitives and GPU resources.
