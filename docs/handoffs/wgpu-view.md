# Lane: wgpu-view

**Tasks, in order:** #8785, #8787. **Start:** once the wgpu lane posts
#8783's resource-table layout in Den (watch #8783). Until then, read the task
texts and the Three sources you replace. Don't write code against an
unpublished layout.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8785: camera composition, viewmodel camera, multi-view outputs and
  captures.**
  - It replaces these files in `renderer-three/src/` (reference only):
    - `view-composition.ts`;
    - `viewmodel-camera.ts`;
    - `camera-motion.ts`;
    - `camera-pose.ts`;
    - `render-output.ts`;
    - `browser-surface-render-pass.ts`.

    See also `render-contracts/src/view-composition.ts` and
    `render-output.ts`.
  - Consume the committed `CameraView` facts
    (`RuntimePublication::ViewComposition`); there is no second camera
    authority.
  - Define the capture interface that wgpu-scene's ghost plates (#8788) use,
    and post it on #8785.
- **#8787: billboards, sprites and particles.**
  - It replaces `renderer-three/src/sprite-material.ts` and
    `particle-sink.ts`.
  - The descriptors in `render-presentation` (`billboard.rs`, `particle.rs`)
    lost their policy caps in #8798. Realize any count and range the
    descriptors now admit.

## Files

- **Owns:** new modules in `render-wgpu` for these families.
- **Leave alone:** the crate root, device, shared tables and pass order (wgpu
  lane). Raise a table change in Den first. Mesh, voxel, lighting and ghost
  modules belong to wgpu-scene.

## Evidence

Captures through the #8783 harness beside the Three captures: a Doom viewmodel
over the world view, a multi-view output, and billboard, sprite and particle
fixtures.
