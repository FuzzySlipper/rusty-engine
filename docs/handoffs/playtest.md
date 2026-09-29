# Lane: playtest

**Tasks, in order:** #8765, #8769. **Start:** now.
Campaign #8723. Shared protocol: [README.md](README.md).

## Tasks

- **#8765: visible downstream exercise. Pause, menu and remap keep the
  renderer mounted (changes requested).**

  Accepted evidence you don't need to repeat, in
  `docs/evidence/control-fence-8765/`:
  - retained video survives the intro fences;
  - no reconnect, refetch or context loss across the recorded transitions;
  - pause and resume hold and resume simulation.

  The reviewer asked for the missing half (Den message on #8765, 2026-09-28):
  - **Diagnose the input path first.** In the recorded run, Dagger's UI stayed
    in interface mode after attach and sent zero key facts. Establish one
    working gameplay action through an ordinary browser input path, in Dagger
    or the real-host Engine fixture.
  - **Held input clears.** Carry held input across a menu or control fence and
    show it clears.
  - **Remap works.** After a remap, the old key no longer triggers the action
    and the new one does.
  - **Resume works.** Input works again after resume.
  - **Held world.** Exercise the menu and observer camera while the world is
    held: world steps unchanged, observer usable.
  - Keep the canvas, context and resource observations around those
    transitions.
  - If the host is available, capture warnings with
    `scripts/capture-playtest-warning-delta.mjs` spanning the fences.
- **#8769: a fresh attachment restarts an active video from the beginning.**
  - `render-presentation/src/video.rs` `VideoProjector::snapshot` emits
    `Play { handle, clip }` with no position.
  - Since #8767 every reconnect is a fresh attachment, so this also hits
    network drops and slow subscribers.
  - Decide whether to resume at the Engine position (as audio does with
    `cursor_seconds`). Test completion-fact semantics across a reattach with
    the real `RendererVideoHost`: Dagger's three-clip opening must not
    double-advance or stall.
  - The TS video host is frozen and scheduled for deletion under #8792. Keep
    TS changes to the minimum seek, and put the cursor in the Rust baseline:
    wgpu video (#8791) reuses it.

## Files

- **Owns:** `docs/evidence/` for these tasks, and Playwright/playtest scripts
  under it.
  - For #8769 only: `render-presentation/src/video.rs` and the TS video host.
- **Rebuilt bundles:** if you change TypeScript, rebuild `render/artifacts`,
  unless #8752 (hygiene lane) has stopped tracking them.

## How to run

- Dagger is at `/home/dev/rusty-dagger`. Use a private clone for exercises
  (see the #8765 README).
- Run `rusty dev --engine-source <your worktree> --live-debug`, or use a
  locally built pair.
- The previous run's Playwright scripts are under
  `docs/evidence/control-fence-8765/`.
