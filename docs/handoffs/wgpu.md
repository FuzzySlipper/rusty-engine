# Lane: wgpu

**Tasks, in order:** #8783, #8815. **Start:** now.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Owner direction: desktop-first, TypeScript is UI only, enforced crate
boundaries over prose. Shared protocol: [README.md](README.md).

Round 1 of this lane landed #8796 (`6a88d6c7c`, in review). **Decision:
write new on raw wgpu 30**, from `rust/prototypes/wgpu-bootstrap/raw`. See Den
doc `rusty-engine/wgpu-bootstrap-decision`.

## Tasks

- **#8783: the `render-wgpu` crate.** An offscreen renderer over
  `PresentationWorld`, built as typed handle-keyed tables and a fixed pass
  pipeline. Read the task's updated description: it carries the decision, the
  parity checklist seed (hemisphere light, equirect sky, per-texture sampler
  modes, no default shadows or tone mapping) and the owner's "must not grow"
  list. It also needs:
  - a Chromium-free screenshot harness: move the prototype's capture script
    and loader in;
  - the wgpu row in `EXTERNAL_DEPENDENCY_OWNERS`
    (`scripts/dependency_boundary_check.py`);
  - software Vulkan (llvmpipe) for CI;
  - the baseline "files touched per visual capability, by language" measure.

  **Four other lanes wait on you:**
  - **Table layout.** Publish the resource-table layout on #8783 as a Den
    message as soon as it is settled. wgpu-scene and wgpu-view start from it.
  - **Readback.** Post on #8786 when readback is on main (streaming lane).
  - **Surface presentation.** Post on #8790 when it is on main (desktop
    lane).

  Landing the crate skeleton, tables and readback before the full first
  family is fine if it unblocks them sooner. Say so in the review request.
- **#8815: delete the prototype workspace** (`rust/prototypes/wgpu-bootstrap`)
  once `render-wgpu` renders the room-study fixture. Update the "Reproduce"
  section of `docs/evidence/wgpu-bootstrap-8796/`. It may land inside #8783.

## Files

- **Owns:** the `render-wgpu` crate root, device, tables and pass pipeline;
  the prototype workspace; the wgpu dependency-boundary row.
- **Leave alone:** `render-presentation` and `render-model` stay
  renderer-neutral. Read them; if a shape must change, raise it in Den first.

## Evidence

Fixture screenshots from the harness, and the captured Doom room study
rendered through `render-wgpu` beside the spike's Three reference, with
differences named.
