# Content store removal and single-plan import (#8763)

## Consumers

Survey #8762, plus a grep of the downstream C# repositories: dagger,
craftsurvive, doom, space, crawler, rifles, underworld, procgen, d20, dungeon,
roguelike, template and asset-pipeline.

- **The only store writer** is `rusty-procgen`
  `RustyProcgenProduct.PublishAndReadBack`. It publishes an authored catalog
  to the store, reads the snapshot back and reopens it: a round trip that
  proves the store, not a consumer that needs it. The repository is dormant;
  its last commit (Sep 7) is "Move active procedural experimentation to
  CraftSurvive".
- **Test doubles** only throw from `IEngineContext.ContentStore`: doom
  `LoadingBaySession`, space `EngineTriples`, crawler `FakeEngineContext`, and
  the Engine's own examples and fixtures.
- **Not store use.** `AuthoredContent.AdmitCatalog*`, `ReadCatalog` and
  `Resolve*` (craftsurvive, doom, rifles) are in-memory admission and are
  unchanged.
- **Engine-internal.** The authored catalog, prefab and scene store paths
  published one JSON body each, and every one has a `…FromContent` sibling.
  The inspector's `content` command read a store manifest; #8745 has since
  deleted the inspector.

No consumer needs a generation, snapshot, transaction or multi-file
publication, so none was kept.

## Removed

- **Crates:** `content-store-host` (generation writer, pointer file, snapshot
  reads) and `content-store`'s write set, batch admission, manifest,
  load/save plan and owner body codecs.
- **ABI:**
  - the `content_store` module and `NativeContentStoreApi`;
  - the six authored-content functions `Publish{Catalog,PrefabRegistry,Scene}ToStore`,
    `Reopen{Catalog,PrefabRegistry}FromStore` and `PrepareSceneFromStore`,
    with their request and receipt types.
- **Services and C#:**
  - `RuntimeContentStoreBridge` and the authored-content store bridge code;
  - the generated `IContentStoreService` / `IEngineContext.ContentStore`.
- **Plumbing:**
  - `--content-store-root` in `rusty-product-host`, the supervisor and
    `rusty dev`, plus `rusty dev`'s `.runtime/content-store` root;
  - `EngineServiceSet::new`'s `content_store_root` argument.
- **Inspector:** `rusty-inspect content` read a store manifest. #8745 deleted
  `rusty-inspect` entirely in parallel, so this change no longer touches it.
- **Asset import:** the second source conversion in `rusty-asset-import`.
  - The CLI used to plan once only to learn the import manifest's path, then
    convert the source again with the prior manifest.
  - It now plans once and calls `ImportPlan::against_prior`, which reclassifies
    the reimport and fixes the report line.
  - A new test checks that this matches planning with the prior manifest.

## Kept, and why

- **Prefab registries** move into `authored-scene`, beside the scene model:
  the model, validation, codec and resolution, with their three tests.
  `authored_content` admits and resolves prefabs from content.
- **Directory publication** in `asset-import` stays: it stages the output
  directory, swaps it in, and updates the source sidecar alongside.
  - Its product is the offline import output, which later build or dev
    staging reads.
  - The survey found no reader that sees a partial state. Removing it would
    risk leaving a half-written directory after a failed write, and nothing
    about this task needed that.

## Evidence

**`rusty-asset-import write`** on `fixtures/collision/static-ramp.mesh.json`,
into an absolute output directory (relative outputs are refused):

| Step | Reimport plan | Publication |
|---|---|---|
| first write | `structuralReload` (first import) | 3 files, `replacedPrevious=false` |
| unchanged rewrite | `noop` | replaced |
| material colour edited | `visualUpdate` | replaced |
| a position moved | `structuralReload` | replaced |
| malformed source `{ broken` | admission failed, no candidate | output hashes unchanged |
| source deleted | read error | previous output left in place |

**Old and new live references.** No remaining consumer of this path holds one.
Bundle content keeps its old and new references across a development reload;
see [#8743](../source-root-reload-8743/README.md).

**Checks**
- **Rust:** `cargo test --workspace --exclude renderer-webview-host` passes
  (1,146 tests).
- **Clippy:** nothing in the touched crates.
- **SDK:** `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes.
- **C# builds:** the NativeAOT trial, the Entities and Application examples,
  and `csharp-debug-execution-context` build.
  - `fixtures/csharp-json-persistence` already failed before this change
    (missing `Video` and `RenderOutput`), and nothing builds it (#8809).
- **Not run:** `scripts/test-runtime-pack.sh` cannot pass. The NativeAOT trial
  publish emits a CoreCLR app with no `.so`; that also happens on plain
  `origin/main` (#8808).

## Migration

- **Test doubles:** remove the `IContentStoreService ContentStore` member.
- **`rusty-procgen`:** drop the `PublishAndReadBack` store round trip, or
  write the catalog JSON with ordinary file IO if the workbench still wants a
  file.
- **Authored content:** use `AdmitCatalogFromContent`,
  `AdmitPrefabRegistryFromContent` and `PrepareSceneFromContent` instead of
  the store variants.
- **Host launches:** stop passing `--content-store-root`. Old
  `.runtime/content-store` directories can be deleted.
- **Rust callers:** import the prefab types from `authored_scene` instead of
  `content_store`, and drop the `content_store_root` argument from
  `EngineServiceSet::new`.
