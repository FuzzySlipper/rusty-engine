# Lane: wgpu-scene

**Tasks, in order:** #8784, #8788. **Start:** once the wgpu lane posts
#8783's resource-table layout in Den (watch #8783). Until then, read the task
texts and the Three sources you replace. Don't write code against an
unpublished layout.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8784: instanced meshes, voxel scene surfaces, lighting, shadows and
  sky.** It replaces these files in `render/packages/renderer-three/src/`
  (reference only; they are frozen):
  - `uploaded-mesh-batching.ts`;
  - `voxel-surface-material.ts`;
  - `lighting.ts`;
  - `sky-blend.ts`;
  - `static-room.ts`.

  Voxel surfaces arrive as chunk payloads from `render-projection/src/voxel.rs`.
  Since #8797, only changed chunks are published; don't re-derive the world
  per frame.
- **#8788: ghost plates, animated meshes and the telemetry overlay.** It
  replaces:
  - `renderer-three/src/ghost-plate*.ts`;
  - `animated-mesh.ts`;
  - `mesh-inspection.ts`;
  - `renderer-host/src/ghost-plate-host.ts`.

  Ghost plates need captures, which are wgpu-view's; agree the capture
  interface with that lane on #8785.

## Files

- **Owns:** new modules in `render-wgpu` for these families.
- **Leave alone:** the crate root, device, shared tables and pass order (wgpu
  lane). Extend them through the published layout, and raise a table change in
  Den first. Camera and view modules belong to wgpu-view.

## Evidence

Doom E1M1 and Dagger captures through `render-wgpu` beside the Three captures,
with differences named, using the #8783 screenshot harness.
