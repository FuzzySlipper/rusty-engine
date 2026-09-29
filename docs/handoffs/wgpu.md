# Lane: wgpu

**Tasks, in order:** #8796, #8783.
**Start:** when Den doc `rusty-engine/wgpu-bootstrap-crate-survey` exists
(task #8795, in progress; it did not exist on 2026-09-29).
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Owner direction: desktop-first, TypeScript is UI only, enforced crate
boundaries over prose. Shared protocol: [README.md](README.md).

## Tasks

- **#8796: decide whether to bootstrap the renderer.** The candidates are:
  - (a) a pinned Bevy render cul-de-sac on our device (`RenderCreation::Manual`,
    headless, driven per tick);
  - (b) the best library-shaped renderer from the survey;
  - (c) raw wgpu.

  Render one Doom E1M1 room from a captured `PresentationWorld` to PNG with
  each candidate that isn't disqualified on paper. Record the measures the task
  lists. Write Den doc `rusty-engine/wgpu-bootstrap-decision`. Prototype crates
  stay behind the boundary; nothing goes to production.
- **#8783: the `render-wgpu` crate.** An offscreen renderer over
  `PresentationWorld`, built as typed handle-keyed tables and a fixed pass
  pipeline. It must not grow a system scheduler or a second retained world. It
  also needs:
  - a screenshot harness that runs without Chromium;
  - a dependency-boundary check so only this crate depends on wgpu;
  - the baseline "files touched per visual capability, by language" measure.

  **Publish the resource-table layout on #8783 early.** The family tasks
  (#8784, #8785, #8787, #8788) and #8786/#8790 start from it in their own
  lanes.

## Inputs from landed work

- **#8737** (`65ff4919`, in review) is the retained delta shape:
  - `RuntimeAppearanceProjector` (`render-projection/src/runtime_appearance.rs`)
    turns changed objects into `RenderFrameDiff` ops;
  - `PresentationWorld` (`render-presentation`) applies them;
  - products publish only changes through `Graphics.PublishChanges`.

  A pending review fix (animation controller retargeting) does not change that
  shape.
- **#8766 and #8767:** the runtime owns browser I/O, and there is no replay.
- **Doom:** `/home/dev/rusty-doom`. **Dagger:** `/home/dev/rusty-dagger`.

## Files

- **Owns:** the new `rust/crates/render-wgpu` crate, any prototype crates for
  #8796, and the dependency-boundary check.
- **Leave alone:** `render-presentation` and `render-model` stay
  renderer-neutral. Read them; if a shape must change, raise it in Den first.
  The main lane is editing `render-projection` and `render-presentation`
  descriptors.

## Evidence

Follow the acceptance for #8796 and #8783. CI needs an adapter (software Vulkan
or a GPU runner); record which.
