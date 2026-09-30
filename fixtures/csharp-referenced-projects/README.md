# Referenced project watch proof

`Product` is an ordinary product that references `Library`, which references
`Transitive`. The SDK's default `RustyEngineWatchPaths` declares both
referenced project directories, so under `rusty dev` an edit in either
rebuilds the product and replaces the runtime. `referenced.revision` (live
debug) returns both projects' revision constants.

Build it with an explicit `RustyEngineFixtureSdkVersion` and
`RestoreAdditionalProjectSources` pointing at the matching SDK feed.
`scripts/test-csharp-sdk-package.sh` stages it and checks the declared watch
paths, with and without the Engine source override.
