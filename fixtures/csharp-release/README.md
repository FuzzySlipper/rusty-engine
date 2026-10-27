# Release proof

`Game` is a window-output product whose release ships `ReleaseImport`, a
tool executable (`RustyEngineToolHost`), named by its `RustyEngineProductTool`
item. `rusty build --release <dir>` writes the release with the
`ReleaseFixture` launcher.

`tools/ReleaseImport/ReleaseImport <folder>` packs the folder into
`release-data.rpak` beside `product.rpak`. With a `greeting.txt` in the folder,
the launcher then opens the window, the product reads the greeting from the
container, saves it, prints `RELEASE_FIXTURE_OPENED <greeting>` (on Windows
into `last-run.log` in `%LOCALAPPDATA%\csharp-release`) and quits.

Build it with an explicit `RustyEngineFixtureSdkVersion` and
`RestoreAdditionalProjectSources` pointing at the matching SDK feed. The tool
copies the installed pair's test host library, so the SDK package smoke,
which has no installed pair, builds only `Game`.
