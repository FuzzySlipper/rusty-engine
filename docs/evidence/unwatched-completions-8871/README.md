# #8871: completions no longer wait for a viewer

## Decision

Advance realization without drawing. In stream output the render loop draws
only while a viewer asks for frames, and animation completions, bounds
inspections and video ends were produced only while preparing a draw. So a
running product with no page never received them. Dagger's opening cinematic
stalled at its title.

Now, when the loop would idle past a change of Engine time, it calls
`Renderer::advance_undrawn`:
- **What it advances.** It poses animated instances (natural completions and
  posed-bounds inspections follow), ends a clip whose Engine time has passed
  its end, and propagates transforms. Then it submits, so the staged skinning
  writes do not pile up.
- **What it skips.** It decodes no video and renders nothing. A later draw
  catches the picture up.
- **Ghost plate readouts.** These were already a snapshot the runtime reads
  between calls, so they needed nothing.

Drawing when a viewer watches is unchanged. So are the frame route and the
facts a drawn frame reports. A frame drawn at a time marks that time
realized, so a watched runtime does no second pass.

Keeping the renderer drawing while unwatched would pay for rendering and
readback nobody sees. Documenting the dependency would keep an agent that
drives a product over HTTP stalled.

## Tests

`render-stream/tests/stream.rs`: a scene with no viewer plays `testsrc.webm`
(1.5 s), and the simulation runs 2 s. `take_video_facts` returns
`Completed` for the clip, and no frame was drawn (`last_drawn` is `None`).
Before this change nothing arrives. `cargo test -p render-wgpu -p render-stream
-p csharp-product-runtime` and stable clippy with `-D warnings` pass.

## Live run on Dagger

[drive-opening.sh](drive-opening.sh) uses no page. It:
1. reads the runtime binding from `engine.renderer.presentation`;
2. posts the title's Begin as its UI intent (`dagger.ui`,
   `dagger.ui.action.v1`, `{action: "begin"}`) to
   `/__rusty/product/runtime/input`;
3. polls `playtest.observe` until the mode is `Playing`.

- **This change** (runtime pack built from this tree, Dagger `36d6fac`):
  `Title` to `Playing` in 174 s, against 172.6 s of clips (45.4 + 5.6 + 121.6).
  `engine.renderer.presentation` still had no drawn frame at the end
  ([unwatched-run.txt](unwatched-run.txt)).
- **Pair `0.1.0-dev.57b98aa18c47`** (before this change), the same drive: still
  `Title` after 240 s ([baseline-run.txt](baseline-run.txt)). A page attached
  afterwards showed the next opening clip playing, so Begin had started the
  opening and it was waiting for a draw.

## Relation to other work

- #8769 made a clip start at its play op rather than its first draw, which
  lets an undrawn renderer know when a clip ends.
- `rusty dev --headless` stays for runs that want frames drawn or the product
  UI mounted. It is no longer needed for completions.
