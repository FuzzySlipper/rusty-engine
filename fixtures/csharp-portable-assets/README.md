# Portable asset package consumer

Build with an explicit immutable SDK feed and matching runtime:

```sh
RustyEngineFixtureSdkVersion=VERSION \
RestoreAdditionalProjectSources=FEED \
dotnet msbuild fixtures/csharp-portable-assets/CsharpPortableAssets.csproj \
  -restore -t:VerifyRustyEngineAot
```

Run the staged Product with the paired `rusty-product-host` using either loader.
Attach through the browser and query `portable.inspect` through live debug.
The retained viewport sprite alternates authored atlas frames using the Engine
playback clock; the descriptor's ordered durations are 0.2, 0.4, 0.2 seconds.
The fixture also compares loose/bundle typed facts, preserves an omitted
right-direction mapping, admits the model's named clip through Animation, and
reopens independent model member references three times.

The character GLB is the existing Kenney retro character fixture copied from
`fixtures/render/assets/kenney-retro-character`; its upstream attribution remains
there. The two content directories intentionally contain equivalent assets to
exercise both admission roots independently.
