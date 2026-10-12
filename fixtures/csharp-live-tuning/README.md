# C# live tuning

Package-consumer fixture for live tuning: [live loose
content](../../docs/csharp-product-project.md#live-loose-content) and the
[content source](../../docs/csharp-product-project.md#content-source). A marker
cube stands on a floor; `content/tuning/marker.json` sets its size, height,
colour, spin and the floor's shade.

- The project declares `tuning` live, so under `rusty dev` an edit to that
  file reaches the running product as `ProductUpdate.ContentChanges` and the
  marker follows, with no runtime replacement. `content/notes.txt` is not
  declared: editing it restages the whole Product.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpLiveTuning.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev --live-debug \
  --project CsharpLiveTuning.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands, beside the Engine's `engine.content.source.*`:

- `livetuning.observe` is read-only. It reports the applied tuning, the
  reloads the product took, the last change, and a name for this product
  instance, which a runtime replacement would change.
- `livetuning.source.size SIZE` saves a marker size through the C#
  ContentSource service and applies it.
