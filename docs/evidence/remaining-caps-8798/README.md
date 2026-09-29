# Remaining policy caps and duplicate checks (#8798)

This finishes the cap removal #8742 started. It covers the presentation,
content and codec limits #8742 left, plus the open-time bundle re-hashing
merged from #8759. Limits that protect a real representation, backend or
resource bound stay, with the reason given below.

## Removed

| Area | Removed |
|---|---|
| Billboards (`render-presentation/src/billboard.rs`) | 256-byte text, 128-byte key, 8-argument, 4-meter and 8-status-cue caps. Also the 32-segment and ±1e12 meter-value caps, and the width, spacing, radius, safe-area, reference-distance and distance-scale maxima. Values must still be finite, non-negative or positive as their meaning needs, and ordered. |
| Particles (`particle.rs`) | 8-key curve cap, 10,000/s rate, 0.01–60 s lifetime and 120 fps flipbook maxima. The TS particle host interpolates curves of any length. |
| Sprites (`csharp-engine-services/src/appearance.rs`) | 4,096 atlas frames, 4,096 playback entries and 4,096 markers. |
| Animation cues | The 128-definition cap, in the services bridge, `runtime-publication`, the dev host and `local-transport.ts`. Also the second and third re-validation of the same text in `runtime-publication` and the dev host's output constructor. |
| Authored content (`authored_content.rs`) | Catalog entry/dependency, payload-row, entity-definition, prefab, prefab-row and scene-row caps, and the 4,096-byte text cap. |
| Persistence (`persistence.rs`) | The 256 MiB payload cap on write and read. |
| Trigger snapshots (`engine-spatial/src/trigger_codec.rs`) | The 16 MiB encode/decode cap. |
| Entities (`entity-state`, `authored-scene`, C# `EngineComponentTypes`) | `MAX_ABS_TRANSLATION` (1e6), `MAX_ABS_VELOCITY` (1e4) and the 60 s character-timer maximum, in Rust and in the C# SDK mirror. Translation, velocity and timers must be finite; timers must be non-negative. |
| Bundles (`csharp-engine-services/src/content/bundles.rs`, #8759) | The SHA-256 of every file on every bundle open. |

## Bundle identity (#8759)

Staging writes each bundle file's SHA-256 into the manifest from the same
bytes. `ProductContentBundles::load` now takes a file's identity from the
manifest instead of re-hashing. A file edited after staging keeps its staged
identity until the next restage, which gives it a new one. The byte-length
check stays: it costs nothing, since the bytes are already read, and it
refuses a file cut short by an interrupted restage.

Removed work: SHA-256 over every byte of the bundle on each open, about
0.18 s per 256 MiB on this host (`sha256sum` with hardware SHA; the read alone
is 0.016 s). The bundle test now shows both cases:
- a same-length edit after staging opens under the staged identity;
- a length change is refused.

## Kept, and why

- **Persistence read: the declared payload length must fit the file.** This
  replaces the 256 MiB cap. A corrupt save header must not size an allocation
  larger than the file itself.
  `a_corrupt_length_is_refused_without_sizing_an_allocation_from_it` covers it.
- **96-byte animation cue text.** Cue ids, clips and signal ids cross the ABI
  inline in `NativeAnimationFeedbackText { bytes: [u8; 96] }`. The services
  bridge, which admits cues, keeps the check. The TS decoder keeps its copy
  until #8792 deletes the browser realization paths.
- **Feedback fact counts (128)** in the dev-host models and the transport.
  They match the browser hosts' 128-entry retention rings, so a producer
  cannot exceed them. They are bounded buffers, not refusals of product input.
- **Sprite playback transitions per advance (16,384).** One advance records
  each marker crossing in turn. Removing the cap needs arithmetic cycle
  skipping that keeps marker multiplicity; that is a behavior change, not a
  cap removal.
- **TS transport decoders** for binding, runtime readout, input results and
  cue definitions. None runs per frame: the per-tick outputs (frame,
  presentation, UI projection) are casts since #8742. Per the task, they stay
  until the browser host paths are deleted under #8792.
- **The TS billboard host's structured/layout invariant.** It is a cheap
  per-create check, left for #8792.
- **Diagnostic retention rings** (`MAX_*_DIAGNOSTICS`,
  `MAX_ANIMATION_REALIZATION_FACTS`). They are bounded logs, not input caps.

Moved out:
- The static-mesh `expected_geometry_hash` and trigger `expected_revision`
  guards went to #8754 (spatial lane); they live in its files.
- The C# `EntityStoreDebugModule` 4 KiB result truncation is a paged debug
  tool; see #8744.

## Evidence

Each removed cap has a test of ordinary use past it:

| Test | Crosses |
|---|---|
| `structured_billboards.rs` `structured_composition_accepts_any_counts_and_enforces_identity_rules` | 6 meters, 10 cues, 1 KiB text |
| `domain_projectors.rs` `particle_descriptors_take_long_curves_high_rates_and_long_lifetimes` | 13-key curve, 20,000/s, 120 s lifetime, 240 fps |
| `appearance.rs` `sprite_atlas_admits_more_frames_than_the_former_cap` | 5,000 atlas frames |
| `character_motion.rs` `far_fast_entities_and_long_timers_are_ordinary_values` | translation 2e6, speed 2e4, 90 s timer, snapshot round trip |
| `persistence.rs` `a_corrupt_length_is_refused_without_sizing_an_allocation_from_it` | the kept read check |
| `bundles.rs` `discovery_is_metadata_only_and_reference_ownership_survives_bundle_close` | trusted identity and the length check |

Tests whose only purpose was a removed cap were rewritten into those. That
covers the billboard meter/cue/text counts and the meter segment and magnitude
cases.

### Checks

- **Workspace:** Rust tests pass. Clippy adds nothing beyond the pre-existing
  #8757 lints.
- **TypeScript:** the `product-browser-host` tests pass (105), and the
  `render/artifacts` bundles are rebuilt.
- **C#:** the SDK builds; `scripts/test-csharp-sdk-package.sh --coreclr-smoke`
  and `scripts/test-runtime-pack.sh` pass.

## Migration

- None required for products. Every change widens what is accepted.
- `RuntimePublicationError::TooManyAnimationCueDefinitions`,
  `TransformInvalid::TranslationOutOfRange`, and the exported
  `MAX_ABS_TRANSLATION`, `MAX_ABS_VELOCITY` and `MAX_CHARACTER_TIMER_SECONDS`
  are gone. Rust callers that matched or read them drop those references.
