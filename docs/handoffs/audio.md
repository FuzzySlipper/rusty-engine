# Lane: audio

**Tasks, in order:** #8812, #8813, #8814. **Start:** now.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

Round 1 of this lane landed #8789 (`9dfa0f426`, in review):
- the `render-audio` crate (kira/cpal/symphonia);
- `RUSTY_AUDIO_OUTPUT=device` in `csharp-product-runtime/src/audio_output.rs`;
- the decision record in `docs/evidence/audio-8789/README.md`.

These are its follow-ups.

## Tasks

- **#8812: decode Opus on the device path.**
  - Today it reports `decodeFailed`.
  - Files: `render-audio`, the `EXTERNAL_DEPENDENCY_OWNERS` table in
    `scripts/dependency_boundary_check.py` if you add a decoder crate,
    `docs/recorded-audio.md`.
  - Fixture: `fixtures/audio-containers/tone.opus`.
- **#8813: give audio realization a listener pose and entity emitter
  positions.**
  - `audio_output.rs` passes `NoEntityPositions` and never calls
    `set_listener` (`render-audio/src/lib.rs`).
  - **Listener.** `render-presentation` has no camera state. The committed
    camera is `CameraView` (`csharp-engine-services/src/camera_view.rs`). It
    reaches the runtime as `RuntimePublication::ViewComposition`, pushed in
    `csharp-product-runtime/src/lib.rs`. Take the listener pose from there.
  - **Entity positions.** Take them from the retained presentation world,
    which `render-audio` already reads.
  - No new product API, and no wgpu dependency. Don't wire the browser
    `RendererAudioHost`; #8792 deletes it.
- **#8814: Doom emits weapon sounds through Audio.**
  - Downstream only: `/home/agent/dev/rusty-doom`, where
    `csharp/LoadingBay.Game/LoadingBayWorldServices.cs` holds
    `LoadingBayAudioPolicy`, plus content imports.
  - Use Doom's current pair. No Engine change.

## Files

- **Owns:** see the README table.
- **Leave alone:** `render-presentation/src/audio.rs` (the projector). A shape
  change goes to Den first.

## Evidence

Null-sink monitor recordings, as in #8789: an Opus clip, a positioned emitter
panning with the listener, and a Doom weapon sound.
