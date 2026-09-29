# Lane: audio

**Task:** #8789. **Start:** now. It consumes `PresentationWorld` audio ops and
does not depend on the wgpu backend.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

## What to do

Realize the audio projection ops in the Rust runtime, with one library choice
(kira, cpal/rodio, or similar). There are seven ops, covering voices, loops,
emitters, cues and cursors. The rules:
- cursors advance on Engine update facts, never wall clock;
- loops resume from the retained baseline;
- direct sounds and emitter bursts are not replayed on attach.

Also decide the browser dev mode's audio path: stream it beside frames, or keep
the WebAudio realizer until #8792 deletes the TS lane. Base the decision on
whether the playtest harness and recordings actually need browser audio, and
record the cost of the option you reject.

## Inputs

- **Projector:** `render-presentation/src/audio.rs` (`AudioProjector`). The
  current browser realization is `render/packages/renderer-host/src/audio-host.ts`;
  it is a reference only, frozen, and deleted later.
- **Rules:** `docs/architecture.md` and `docs/recorded-audio.md`. #8742
  (`25dfd514`) removed the Engine's clip count and byte budgets. Clips are
  admitted as encoded bytes (WAV, Ogg Vorbis/Opus, MP3, FLAC); see the
  container table in `docs/recorded-audio.md`.
- **Products:** Dagger (music cues) at `/home/dev/rusty-dagger`; Doom (weapon
  sounds) at `/home/dev/rusty-doom`.

## Files

- **Owns:** a new Rust audio realization crate, plus its wiring into the
  runtime process behind the desktop lifecycle.
- **Leave alone:**
  - `render-presentation/src/audio.rs`: read it; a change to the projector
    shape goes to Den first, because the main lane is editing other
    `render-presentation` descriptors;
  - `csharp-engine-services/src/audio.rs`: shared service code.

## Evidence

A Dagger music cue and a Doom weapon sound through the Rust path, and the
chosen browser path exercised once.
