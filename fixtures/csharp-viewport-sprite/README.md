# C# viewport sprite

Package-consumer fixture for the `Graphics` atlas-sprite viewport path. It
creates a two-frame atlas sprite, applies `SetSpriteViewport` with bottom-center
`Contain`, advances Engine-owned playback, and publishes it as a Viewmodel
appearance. The authored transform is intentionally nonidentity because
viewport placement owns the final screen geometry.

Use a current matching SDK/runtime pair:

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpViewportSprite.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpViewportSprite.csproj --runtime /path/to/pair/runtime-pack
```
