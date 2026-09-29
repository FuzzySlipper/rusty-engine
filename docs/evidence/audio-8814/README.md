# Doom weapon and door sounds (#8814)

Campaign #8782, audio lane, 2026-09-29. Downstream only: Doom commit
[`c080e46`](https://github.com/FuzzySlipper/rusty-doom/commit/c080e46) on
`rusty-doom` main. No Engine code changed.

## Decision

Doom's `docs/source-provenance.md` said no WAD sound is read or shipped. The
owner chose to import the WAD sound lumps anyway, on the same footing as the
WAD-derived textures and sprites already shipped. The rejected options were a
CC0 pack, offline synthesis, or cancelling the task. The provenance document now
records the decision and the closure.

## Change (Doom)

- **Authoring.** `ts/packages/doom-e1m1-authoring/src/sounds.ts` and
  `sound-cli.ts` (`sounds:generate`, `sounds:check`) decode DMX lumps (format 3:
  11,025 Hz unsigned 8-bit mono with 16 padding samples at each end) into
  deterministic PCM WAVs under `content/doom-e1m1/sounds/`, with a manifest of
  WAD, lump and WAV hashes. The lumps are `DSPISTOL`, `DSSHOTGN`, `DSPUNCH`,
  `DSDOROPN` and `DSDORCLS` (the door sounds the canonical project names), from
  the shareware IWAD, SHA-256 `1d7d43be…`. `check-retained-content.mjs` verifies
  the WAV bytes and requires the textures' WAD hash.
- **Product.** `LoadingBayAudioPolicy` (`LoadingBayWorldServices.cs`) owns the
  clips and the Sfx bus state, with named constants for volume, spatial blend
  and attenuation. In the default room-study scene:
  - `LoadingBayRecipeGameplay.Shoot` emits the pistol or shotgun sound on each
    shot (the muzzle frame);
  - a fist emits `DSPUNCH` only when it lands, as in Doom;
  - `UseInteraction` emits `DSDOROPN` as a World3d sound at the door centre when
    a study door starts opening. Study doors never close, so `DSDORCLS` is
    shipped but not played.
  - The legacy voxel scene opens the clips but emits nothing yet.

## Evidence

- **Doom checks.** Semantic catalog `--check`, `dotnet build`, the lifecycle
  exercise, `check-retained-content` (54 textures, 5 sounds, 8 props), the
  canonical-project hash, the boundary and active-guidance audits, and
  `sounds:check` (byte-identical regeneration). Decoder tests: 2/2, one of them
  on DSPISTOL from the real WAD.
- **Doom on its own pinned pair** (`playtest-development-20260928k`, browser
  mode, via `docs/evidence/audio-8789/device-proof.sh` with `physical:` pointer
  input). Two primary-button presses in the room study produced two `emit` ops
  on the Sfx bus, signals `loading-bay.sfx.0` and `.1`, whose clip hash
  `dfbd4def…` is `DSPISTOL.wav` in the manifest, duration 0.51 s. See
  `doom-fire-audio-ops.txt` and `doom-fire-live-debug.txt`. The staged product
  admits all five clips.
- **Door sound not exercised in Doom.** The north-wing door is 37 m from spawn,
  the static route reports `NoPath`, and a waypoint driver (`doom-walk.py`,
  jumping at ledges) stalls near (0.9, −17.9). The door emit shares the `Emit`
  path with the weapon sounds, using a World3d emitter that the Engine fixture
  proves end to end (#8813).
- **Device path not exercised on Doom.** Doom's pinned pack predates
  `RUSTY_AUDIO_OUTPUT=device`, and Doom does not build against current Engine
  main (removed or changed API; see #8821). The same op shape plays on the
  device in #8789 (fixture one-shots) and #8813 (World3d).
