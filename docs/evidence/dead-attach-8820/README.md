# Dead fixture Attach() methods (#8820)

#8799 removed the product `attach` callback; the host had not called it since
b777eb3e4. This change deletes the ten uncalled `public void Attach()` methods
left in Engine fixtures:

- **Empty bodies:** `csharp-audio-containers`, `csharp-video-playback`,
  `csharp-entity-store-debug`, `csharp-debug-command-catalog` (both products)
  and `csharp-debug-execution-context`.
- **`csharp-nativeaot-trial`** (`PublishPresentation()`): `Start` already calls
  `PublishPresentation()`.
- **`csharp-voxel-atlases` and `csharp-lighting-sky`** (`RefreshScene`):
  `ProjectScene`/`ProjectSceneDirectional` refresh the scene before returning
  (`voxel_scene_presentation.rs`), and `voxel-atlases` also refreshes explicitly
  in `Start`.
- **`csharp-world-streaming`** (`RefreshScene` + `Publish()`): the scene is
  published by `ProjectSceneDirectional` in `Start`, and `Update` calls
  `Publish()` every tick.

No publishing had to move; each body duplicated work that already runs.

## Evidence

- All nine fixture directories build (ten projects). The source-referenced ones
  build directly. The five package-referenced ones build against a scratch SDK
  packed from this tree (`pack-csharp-sdk.sh 0.1.0-dev.hygiene8820`).
- `DOTNET_ROOT=/home/agent/.dotnet scripts/test-runtime-pack.sh` passes: the
  moved runtime pack launched CoreCLR and NativeAOT bundles.

Downstream products clean up their own dead methods when they move to the #8799
pair.
