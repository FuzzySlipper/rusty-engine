# Lane: streaming

**Task:** #8786. **Start:** once #8783's readback is on main (the wgpu lane
posts on #8786). The task does not wait for #8723 or #8777.
Campaign #8782; read Den doc `rusty-engine/desktop-renderer-campaign-2026-09`.
Shared protocol: [README.md](README.md).

## What to do

The streaming browser mode displays `render-wgpu` frames in the browser shell,
with the UI, input and playtest tooling unchanged.
- **Runtime side.** A frame stream from the runtime, which already serves
  HTTP/SSE itself (#8766). `runtime-input` stays unchanged.
- **Browser side.** A canvas in the browser shell that shows the frames. The
  likely home is `render/packages/product-browser-host`; confirm it. The DOM UI
  stays as it is.
- **Frame header.** Agree it with the desktop lane (#8790), which presents
  the same frames natively. Post it on #8786.

## Files

- **Owns:** the runtime's frame stream endpoint and the browser canvas path.
- **Leave alone:** `render-wgpu` internals (wgpu lanes). Consume readback
  through the API #8783 publishes.

## Evidence

A Doom playtest through crew-services in streaming mode, with measured latency
and bandwidth.
