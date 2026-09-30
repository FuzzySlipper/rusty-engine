# Voxel collision and update continuity

Ordinary packaged C# fixture for Engine tasks 8684/8685. It retains a visible
100-cell floor/water structure, explicitly configures material 11 as passable,
and drops an Engine character onto the solid floor beneath three water layers.

Build with the **same pair's** SDK feed and runtime:

```sh
RustyEngineFixtureSdkVersion=VERSION RestoreAdditionalProjectSources=/absolute/sdk-feed \
  dotnet msbuild fixtures/csharp-voxel-collision/CsharpVoxelCollision.csproj \
  -restore -t:VerifyRustyEngineAot -p:Configuration=Release
```

Serve the staged Product with the pair's `rusty-product-host` using CoreCLR or
NativeAOT. Enable product live debug when staging and attach an Engine browser.

- `voxel.proof.inspect`: increasing update count, character grounded near
  Y=1.92, downward ray hits voxel (0,0,0), below water.
- `voxel.proof.edit 64 11`: one direct 64-water-cell transaction.
- `voxel.proof.edit 64 1`: replace those cells with stone.
- Repeat inspection after each transaction to verify subsequent updates.
  Counts 3, 4, 9 and 27 cover the smaller reported boundary.
- These edit commands deliberately omit a presentation/residency refresh.
  Canonical edits and character updates must work independently of a visual
  refresh; the original retained structure remains visible.
- `voxel.proof.reject-embedded` deliberately encloses a separate character in
  a solid fill and requires the named penetration diagnostic. Later ordinary
  fixture updates must continue after this handled rejection.
- Disposal publishes `voxel-proof/DISPOSED` and prints one disposal marker.
  The real Dagger product-host exercise separately proves music retirement.

The geometry and collision assertions use native service receipts. Screenshots
prove presentation remains available, not the hidden character's contact.
