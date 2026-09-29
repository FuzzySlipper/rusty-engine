# Lane: desktop

**Tasks, in order:** #8790, #8791. **Start:** once #8783's surface
presentation is on main (the wgpu lane posts on #8790). Decoder research for
#8791 can start earlier.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8790: the desktop shell.** A native window (winit) with a wgpu surface
  and the TypeScript UI overlay. The overlay choice is a decision based on
  measurements; the task lists the options.
  - **Audio.** Turn on device audio for the shell's runtime
    (`RUSTY_AUDIO_OUTPUT=device`; see the #8789 note on #8790 and
    `docs/recorded-audio.md#device-realization`).
  - **Frame header.** Agree it with the streaming lane (#8786).
  - **Families.** The task lists the world, camera and effect families as
    inputs. Start the shell on what #8783 renders, and adopt the families as
    they land.
- **#8791: video playback for the wgpu renderer.**
  - Decide the decoder, and add its row to `EXTERNAL_DEPENDENCY_OWNERS` in
    `scripts/dependency_boundary_check.py`.
  - Implementation is the last realization child. The task cites Opus on the
    device path (#8812) as the lesson on codec coverage.
  - Coordinate with the playtest lane's #8769 (video restart on reattach, in
    backlog): the video cursor belongs in the Rust baseline, and this task
    reuses it.

## Files

- **Owns:** the new desktop shell crate and the video realizer.
- **Leave alone:** `render-wgpu` internals (wgpu lanes); `render-audio`
  (audio lane).

## Evidence

The shell running Doom with the UI overlay and device audio; measurements for
the overlay decision; one video clip played through the chosen decoder.
