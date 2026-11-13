# Procedural mesh composition

Ordinary package-only C# fixture for generated mesh admission. C# builds a
two-material shockwave ring and publishes two appearances sharing one immutable
mesh. **Pulse shape** alternates its geometry; **Clear and recreate** releases
both appearances and their resource before admitting the next mesh. Counts and
shape are published through the ordinary UI projection. Reload the page to
check that the current shape and counters reconstruct without a product restart.

Use a current matching SDK/runtime pair:

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpMeshComposition.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpMeshComposition.csproj \
  --runtime /path/to/pair/runtime-pack --port 40177 --live-debug
```

`mesh.productdata true` shows three walls drawn by a product shader
(`content/product-data.wgsl`) that writes each fragment's product data
(`MeshResourceCreateRequest.ProductData`, `Surface.product_data`) as colour.
The left wall's data is its place on the wall, so it draws a red-green
gradient. The middle wall is the same mesh cut into twelve parts and draws the
same. The right wall has no data, reads zero and draws blue.

The generated SDK handles binding and staging. Geometry is visual-only and no
product renderer is involved. Runtime assertions check copied
resource counts and the required removal/disposal order. The content directory
holds the material showcase's maps and the product data walls' shader.
