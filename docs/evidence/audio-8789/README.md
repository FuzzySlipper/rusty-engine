# Device audio realization and the browser dev audio path (#8789)

Campaign #8782. Worktree `lane/audio`, 2026-09-29.

## What changed

- **`rust/crates/render-audio`** (new). `AudioRealizer` plays the committed
  `PresentationOp::Audio` stream with kira 0.12 on cpal, decoding through
  symphonia. It handles all seven ops: Emit, Create, Restore, Update, Destroy,
  VoiceControl and BusControl.
  - It keeps no Engine clock. `Restore` starts a voice at the Engine cursor, and
    a looping cursor wraps into the clip.
  - A finished non-looping baseline voice is not restarted.
  - Pause/Resume/Retrigger act on the realized sound.
  - Natural completions and device diagnostics come back as `RealizedAudioFact`.
- **`csharp-product-runtime/src/audio_output.rs`** (new), wired into `lib.rs`.
  `RUSTY_AUDIO_OUTPUT=device` opens the default output device at load and closes
  it when the runtime drops.
  - Each committed call's audio ops play on the device and are removed from the
    published presentation. The frame's `operationCount` follows the removal;
    revisions are unchanged.
  - Snapshots (browser baselines) carry no audio.
  - On every binding change (Start, Restart, fault) the device resets and
    applies the committed baseline from the new `EngineServiceSet::audio_snapshot_frame`.
  - Before each update, completions and diagnostics go to the existing
    `ingest_audio_realization_feedback` store, so one-shot clip ownership is
    released as with browser feedback.
  - The device plays only while the runtime is Running and goes silent at
    Shutdown.
- **`EngineServiceSet::audio_clip_bytes`** shares admitted clip bytes by content
  hash without copying.
- **`scripts/dependency_boundary_check.py`** now has `EXTERNAL_DEPENDENCY_OWNERS`:
  only `render-audio` may depend on `kira` or `cpal`. The wgpu backend can add
  `wgpu` to the same table.
- **CI.** `verify.yml` and `pair.yml` install `libasound2-dev`. cpal links ALSA,
  so `rusty-product-host` now needs `libasound.so.2` at runtime on Linux.

## Rules and how they are met

| Rule | Where |
|---|---|
| Cursors advance on Engine update facts, never wall clock | The Engine cursor stays in `AudioProjector::advance_elapsed`. The device reads it only from `Restore`; device positions never flow back. Output is held while the runtime is not Running, so it does not run ahead of a frozen Engine cursor. |
| Loops resume from the retained baseline | `a_looping_baseline_voice_resumes_from_the_engine_cursor` (WAV and Vorbis: cursor 1.25 s in a 1 s loop plays from 0.25 s) |
| Direct sounds and emitter bursts are not replayed on attach | Baselines contain no Emit ops. `reset()` drops one-shot owners without reporting their completion (`reset_stops_voices_and_forgets_one_shot_owners`). A finished baseline voice is neither restarted nor completed twice. |
| Device selection and shutdown follow the runtime lifecycle | Opened in `load_admitted_with`, closed on drop, held while not Running, silenced on Shutdown (fixture lifecycle run below) |

## Evidence

The tools are in this directory:
- `device-proof.sh` starts a runtime layout with `RUSTY_AUDIO_OUTPUT=device` and
  `PULSE_SINK` set to a PulseAudio null sink, records that sink's monitor, and
  drives live-debug commands, product intents and lifecycle routes.
- `analyze-recording.py` prints RMS per 0.25 s window and the dominant
  frequency.

Measurements are in `recordings.txt`.

**Engine fixture: one-shots and loops for every container.**
`fixtures/csharp-audio-containers` ran on CoreCLR against an SDK packed from
this worktree; see `fixture-live-debug.txt`.
- `audio.proof.play` creates five looping voices and five one-shots.
- After 2 s, the Engine's own realization store holds, from the device:
  - four `NaturalCompletionOneShot` facts (WAV, Vorbis, MP3, FLAC);
  - `DecodeFailed` for the Opus voice and the Opus signal (#8812).
- Recording: silent before play; 440 Hz from play to `audio.proof.stop`, louder
  for the first second while the one-shots overlap the loops; silent after.
- The browser-bound SSE stream carried presentation frames with
  `operationCount: 0` and no audio ops.

**Engine fixture: lifecycle.** In `fixture-lifecycle-live-debug.txt`, play is
followed by `lifecycle/pause`, 2 s, `lifecycle/resume`, then stop. The recording
is silent for the paused windows and continues at the same level after resume.

**Dagger music cue.** Dagger (`/home/dev/rusty-dagger` 3097397) was copied
without build outputs, with content symlinked, and staged with this worktree's
SDK. There was no downstream source change. Inputs were Dagger's own UI intents:
`begin`, then `cinematic-skip` three times.
- Device mode: Dagger entered Playing and its music director started a voice.
  The recording has sound from 14.5 s to shutdown. Cross-correlated against the
  first 8 s of `content/worldrpg/media/music/clips/song_dungeon.ogg`, it matches
  at 14.23 s with r = 0.991. A different track (`song_02.ogg`) as a control gives
  r = 0.035.
- Browser mode (`AUDIO_OUTPUT=`, same inputs): the stream carries the op the
  device played (`dagger-browser-mode-music-op.json`): Create handle 1, clip
  `sha256:aa967a6f…` (Dagger's `music.dungeon`), ambient bus, volume 0.8,
  looping, `global2d`. Nothing reaches the device.

**Doom weapon sound: not available.** Doom (0a881b2) emits no sounds; it only
sets Sfx bus volume and mute. The fixture one-shots above stand in for a
weapon-shaped Emit. The product change is #8814.

**Chosen browser path exercised once.** `pnpm exec playwright test
browser/audio-containers.browser.spec.ts` passed 5/5: WAV, Vorbis, Opus, MP3 and
FLAC emit samples, loop and complete through `RendererAudioHost`.

**Checks run:**
- `cargo test -p render-audio` (11 passed);
- `cargo test -p csharp-product-runtime -p csharp-engine-services` (183 + 44 + 27 passed);
- clippy on the three crates (no new warnings; the baselines in `spatial.rs`
  (#8757) and `product-dev-host/src/model.rs` remain);
- `scripts/test_architecture_checks.py` and `dependency_boundary_check.py`;
- `scripts/verify-docs.sh`.

## Decision: the browser dev mode's audio path

**Keep the WebAudio realizer** (`render/packages/renderer-host/src/audio-host.ts`)
as the single retained TypeScript realization until #8792 deletes the TS
renderer lane. Do not stream audio beside frames.

The deciding facts:
- **The playtest harness does not consume browser audio.** crew-services
  playtests read screenshots and live-debug facts; agents cannot hear. The only
  browser-audio touchpoint is the `audio-feedback` request seen by warning
  capture.
- **Recordings do not use it either.** Human Wolf/Moonlight sessions stream the
  session's sound server, not a browser API. A runtime playing on the device in
  that session reaches them the same way.
- **Development runtimes run on the developer's machine.** `RUSTY_AUDIO_OUTPUT=device`
  already plays there. When #8792 deletes the TS lane, the browser dev mode can
  switch to the device realizer instead of streaming.

**Cost of the rejected option (streaming audio beside frames):**
- a runtime-side PCM or Opus encoder on the device realizer's mix;
- a second framed channel on the runtime socket, to coordinate with #8786's
  frame streaming;
- a browser AudioWorklet jitter buffer with clock-drift handling;
- A/V sync against streamed frames;
- about 20–60 ms of added latency;
- new TypeScript in a lane scheduled for deletion.

All of that serves a consumer that does not exist today.

## Accepted differences from the browser realization

- Opus does not decode on the device (#8812).
- The listener stays at the origin and entity-attached voices report
  `hostFailure` on both paths (#8813).
- Spatial falloff is kira's linear 1..`attenuation` rather than Web Audio's
  inverse model, and `spatial_blend` maps to kira's spatialization strength
  rather than a dry/wet split.
- Compressed-clip durations come from the decoder when the Engine admitted none.
- The device path is opt-in. The browser realization remains the default until
  the desktop shell (#8790) or #8792 turns it on.

## Observation, not investigated

Stopping a packaged CoreCLR host by closing its stdin took more than 20 s in
these runs, with and without device audio, before SIGTERM ended it. It is not
caused by this change.
