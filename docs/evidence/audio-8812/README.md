# Opus on the device audio path (#8812)

Campaign #8782, audio lane, 2026-09-29.

## Change

`render-audio/src/opus.rs` adds `OggOpusDecoder`, a kira streaming `Decoder`.
- symphonia's `OggReader` demuxes the stream, trims end padding and seeks.
- `opus-decoder` 0.1 decodes the packets.
- The decoder itself drops the `OpusHead` pre-skip. symphonia reports it as the
  track `delay` but leaves it in the packets, with timestamps starting at 0.
- A seek decodes 80 ms of pre-roll (RFC 7845 §4.6) before the target and
  discards it.

Clips that `render_model::AudioContainer::identify` reports as Opus stream
through this decoder; the other compressed containers keep kira's symphonia
decoder. Errors map into kira's `FromFileError`, so the voice handle type is
unchanged.

**Dependency cost.** One new crate, `opus-decoder` 0.1.1: pure Rust,
`forbid(unsafe_code)`, no FFI, MIT OR Apache-2.0, depending only on `thiserror`
(already in the tree). It claims all 12 RFC 8251 conformance vectors. It is
young (0.1), and that is the risk. The rejected alternative, libopus through
`opus` or `audiopus`, adds a C library to every runtime-pack build and host.
`scripts/dependency_boundary_check.py` now names `render-audio` as the only
owner of `opus-decoder` and `symphonia` (symphonia is now a direct dependency,
for the Ogg reader).

## Evidence

- **Accuracy against libopus.** The full decode of
  `fixtures/audio-containers/tone.opus` against ffmpeg's libopus decode:
  48,000 frames each, correlation 0.9999999988, max difference 3.05e-5 (one
  16-bit step). Resuming from a seek to 24,000 against the reference: correlation
  0.99993, max difference 0.0049 (pre-roll convergence).
- **Tests.** `opus::tests` checks the pre-skip and padding trim (48,000 frames)
  and seek continuity against the continuous decode (< 0.01).
  `one_shots_complete_for_every_decoded_container` now includes Opus, and
  `a_looping_baseline_voice_resumes_from_the_engine_cursor` includes an Opus loop
  restored at 1.25 s. The former `decodeFailed` test is removed. render-audio:
  14 passed.
- **Engine fixture on the device** (`RUSTY_AUDIO_OUTPUT=device`, null sink, via
  `docs/evidence/audio-8789/device-proof.sh`). `audio.proof.play` then
  `audio.proof.inspect` reports five `NaturalCompletionOneShot` facts, the Opus
  signal (3) among them, and no diagnostics. Five voices stay active. The
  recording has 440 Hz from play to stop.
- **Opus alone on the device.** A scratch program played one `tone.opus`
  one-shot through `AudioRealizer::open_default_device` into the null sink. The
  recording has 440 Hz audible for about 1 s, and the realizer reported
  `OneShotCompleted`.
