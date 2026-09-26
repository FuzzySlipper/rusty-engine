# Lighting and sky mechanism fixture

Uses only packaged `Rusty.Engine` C# APIs. Build/stage with the explicit test
SDK version and its local feed, inherited by generated child builds:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  dotnet msbuild CsharpLightingSky.csproj -restore -t:VerifyRustyEngineAot
```

Run the staged bundle with the matching runtime pack. Debug commands and the
bounded proof contract are in [lighting and skies](../../docs/lighting-and-sky.md).
`generate-skies.py` regenerates the two small authored PNG fixture panoramas.
The fixture owns the supplied clock value and source light descriptor; Engine
owns rendering, light sampling, scene persistence primitives and GPU resources.
