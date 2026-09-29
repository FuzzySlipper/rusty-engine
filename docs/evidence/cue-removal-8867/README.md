# #8867: the animation cue API is removed

## Why

`IAnimation.ReplaceCueDefinitions` declared markers on animated clips. The
TypeScript animation host turned each marker crossing into an audio or
particle signal and reported it back as a `Cue` realization fact. #8792
deleted that host, and `render-wgpu` never realized cues (#8788). Since then
the definitions were copied, published and applied by nothing, and nothing
produced a `Cue` fact.

The removal-first default applies: the API has no consumer anywhere.
`git grep` finds no use of `ReplaceCueDefinitions`,
`AnimationCueDefinition`, `AnimationCueSignalDomain`, a `Cue` fact kind or its
fields in this repository (including `scripts/fixtures/`, which the
release-pair check compiles). It finds none in any product either:
- rusty-craftsurvive, rusty-crawler, rusty-d20 and rusty-dagger;
- rusty-doom, rusty-dungeon, rusty-rifles and rusty-roguelike;
- rusty-space, rusty-template and rusty-underworld.

Doom's `LoadingBayE1M1AnimationCue` is its own struct. A product that
wants sound or particles on a clip marker can play them from its update, where
it already reads the clip's playback cursor.

## What went

- **ABI (`csharp-engine-abi`).** Removed:
  - `NativeAnimationCueSignalDomain`, `NativeAnimationCueDefinition` and
    `NativeAnimationCueDefinitionReplaceRequest`;
  - the `replace_cue_definitions` entry in the animation table;
  - the `Cue` realization fact kind (the other kinds keep their numbers);
  - the fact's `cue_id`, `signal_domain`, `signal_id` and `marker_millis`
    fields.

  The product ABI fingerprint moves from `6297d14a…` to `2ccdb0e6…`.
- **Services (`csharp-engine-services`).** Removed:
  - the bridge function, and the copied cue snapshot the appearance state
    retained;
  - its baseline snapshot and the `AnimationCueDefinitions` call output;
  - the `Cue` fact mapping, and the unit test of the snapshot's
    replacement semantics.
- **Publication (`runtime-publication`).** Removed
  `RuntimeAnimationCueDefinition`, `RuntimeAnimationCueSignalDomain`, the
  `AnimationCueDefinitions` publication, its two errors and its test.
- **Hosts.** `product-dev-host` loses its re-exports and the drop arm.
  `csharp-product-runtime` loses the cue-to-publication mapping and its test.
- **Docs.** `architecture.md` and `csharp-lifecycle.md` no longer list cues
  among publications or unreplayed history.

## Generated API diff

[generated-api.diff](generated-api.diff) diffs the generated public C# from
`scripts/generate-csharp-native-bindings.sh` at `2df233500` against this
change. It shows three changes and nothing else:
- `ReplaceCueDefinitions` is gone from the animation service contract;
- `AnimationCueSignalDomain`, `AnimationCueDefinition` and
  `AnimationCueDefinitionReplaceRequest` are gone;
- `AnimationRealizationFactKind.Cue` is gone, and `AnimationRealizationFact`
  loses `CueId`, `SignalDomain`, `SignalId` and `MarkerMillis`.

## Checks

- **Rust.**
  - `cargo fmt --check` and `cargo clippy --workspace --all-targets -D
    warnings` pass.
  - Tests pass for csharp-engine-abi, csharp-engine-services (184),
    runtime-publication (4), product-dev-host (38 unit, 26 loopback) and
    csharp-product-runtime (44 lib, 27 host).
- **C# SDK.** `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes:
  it packs the SDK on the new ABI, builds a packaged consumer, and runs the
  CoreCLR lifecycle and loopback exercise.

## Migration

Products call nothing that was removed, so they need no source changes. The
fingerprint change means each product rebuilds on a pair from this change
(`rusty update`, then build), as with any ABI change. `0.1.0-dev.e9511434c81a`
is the first such pair.

Doom (`788c784`) and Dagger (`c4248c9`) moved to `0.1.0-dev.b67d90d5b601`, the
current pair, with `rusty update` alone. `rusty dev` rebuilt both against the
new SDK and ran them streamed. `parity-capture.mjs` ran unchanged and recorded
no page errors, and the frames match the #8792 captures.
