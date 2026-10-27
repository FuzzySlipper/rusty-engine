# Release proof

`Game` is a window-output product whose release ships `ReleaseImport`, a
tool executable (`RustyEngineToolHost`), named by its `RustyEngineProductTool`
item. `rusty build --release <dir>` writes the release with the
`ReleaseFixture` launcher.

Without `release-data.rpak` the product shows an entry screen. Import opens
the folder picker (`Host.ChooseFolder`), runs `ReleaseImport <folder>
<installed content>` (`Host.RunTool`) and shows its output while it packs the
folder into `release-data.rpak` in the installed-content directory: beside
`product.rpak` in a release, `.runtime/installed/` under `rusty dev`. Cancel
stops the tool. Once it exits, the product opens the container, reads
`greeting.txt`, saves it, shows it and prints `RELEASE_FIXTURE_OPENED
<greeting>` (on Windows into `last-run.log` in
`%LOCALAPPDATA%\csharp-release`). The debug commands `release.import`,
`release.import_folder <folder>`, `release.cancel` and `release.observe` do
what the screen does.

A player can still run `tools/ReleaseImport/ReleaseImport <folder>` by hand;
it then installs beside `product.rpak`.

Build it with an explicit `RustyEngineFixtureSdkVersion` and
`RestoreAdditionalProjectSources` pointing at the matching SDK feed. The tool
copies the installed pair's test host library, so the SDK package smoke,
which has no installed pair, builds only `Game`.
