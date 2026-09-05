# Procedural mesh composition

Ordinary package-only C# fixture for generated mesh admission. C# builds a
two-material shockwave ring and publishes two appearances sharing one immutable
mesh. **Pulse shape** alternates its geometry; **Clear and recreate** releases
both appearances and their resource before admitting the next mesh. Counts and
shape are published through the ordinary UI projection. Reload the browser to
check that the current shape and counters reconstruct without a product restart.

Use a current matching SDK/runtime pair (the mesh API first lands with #7787):

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpMeshComposition.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpMeshComposition.csproj \
  --runtime /path/to/pair/runtime-pack --port 40177 --live-debug
```

The generated SDK handles binding and staging. Geometry is visual-only and no
content files or product renderer are involved. Runtime assertions check copied
resource counts and the required removal/disposal order. The content directory
is intentionally empty.
