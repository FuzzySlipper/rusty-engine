# welding patch

Source: crates.io `welding` 0.15.0, https://github.com/merely-made/wgpu-weld
License: MPL-2.0 (each source file carries the notice). The crate's own
`Cargo.lock` is left out.

Engine #10123 carries a page's image cursor to the host. 0.15.0 drops CEF's
custom cursor info and reports `CT_CUSTOM` as `CursorShape::Custom("cef-other")`,
so the desktop window showed the default arrow for a CSS `cursor: url(...)`.

- `src/surface.rs` adds `CursorShape::Image(CursorImage)`: straight RGBA
  pixels, size, hotspot and image scale, compared and hashed by content.
  `src/lib.rs` exports `CursorImage`.
- `src/cursor.rs` adds `from_cef_change`, which copies the custom cursor's
  premultiplied BGRA buffer into a `CursorImage` during the callback.
- The Linux, Windows and macOS `on_cursor_change` handlers call it.

`cursor: none` already arrives as `CursorShape::Custom("none")`. Remove this
vendored copy once a welding release carries custom cursor pixels.

Engine #10261 draws the Windows desktop window with Vulkan and builds no DX12
backend. 0.15.0 imports Chromium's Windows frames into D3D12 only.

- `src/native_frame/d3d11.rs` (was `dx12.rs`): the D3D11 copier finds its
  adapter by the Vulkan device's LUID and copies into a
  `MISC_SHARED | MISC_SHARED_NTHANDLE` texture with no keyed mutex; its
  flush-and-wait orders the copy. `import_d3d11` imports the NT handle with
  wgpu-hal's `texture_from_d3d11_shared_handle`, wraps it with
  `create_texture_from_hal`, and acquires it from `VK_QUEUE_FAMILY_EXTERNAL`
  in a barrier recorded through `CommandEncoder::as_hal_mut`. The D3D12
  import, its cache-flush wait and `copy_dx12_callback_frame` are gone;
  `copy_dx12_callback_frame_to_owned` is `copy_d3d11_callback_frame_to_owned`.
- `src/windows_cef/mod.rs`: page and popup frames go through
  `WgpuTextureImporter::import`.
- `src/native_frame/mod.rs`: `InteropBackend::Dx12` and
  `ImportError::D3d12OpenShared` are gone.
- `Cargo.toml`: wgpu 30 without default features (no `dx12`), `cef` without
  `accelerated_osr` (its wgpu import is unused), `grafting` only off Windows,
  `ash` and the windows crate's `Win32_Security` on Windows.

The frame type keeps its upstream name, `Dx12SharedTexture`. Upstream, this
fits as an `InteropBackend::Vulkan` arm beside DX12, with the generic import
in grafting.

With no keyed mutex, the copier's flush-and-wait is the only thing ordering
the copy before the import, so it waits for the event query's data to read
TRUE. The windows crate's `GetData` returns `Ok` for S_FALSE (not ready) as
well; 0.15.0 returned on the first poll (`d3d11_event_reached`).

Engine #9947 builds the desktop window's device with DLSS's Vulkan device
extensions beside the DMA-BUF ones. 0.15.0's `build_dmabuf_capable_device`
fixes its device-creation callback.

- `src/native_frame/vulkan_dmabuf.rs` adds `build_dmabuf_capable_device_with`,
  which opens the device itself with Graft's DMA-BUF extension list
  (`DMABUF_DEVICE_EXTENSIONS`, a copy of Graft's private list) and then lets
  the host's `extend` add to the callback's arguments. `src/lib.rs` exports
  it on Linux with `wgpu-30`.

Engine #10364 draws the desktop window's UI with Servo as well as Chromium.
Servo renders with GL; its frames must reach the window's Vulkan device
without a copy, and the FFI that needs belongs here with welding's other
native interop rather than in the Engine's unsafe-free crates.

- `src/gl_target/` (feature `gl-target`, Linux) adds `GlSharedTarget`: an EGL
  context on a GBM device of the wgpu adapter's render node (matched by PCI
  bus, else vendor and device), drawing into a ring of linear ABGR8888 GBM
  buffers bound as GL textures through `EGL_EXT_image_dma_buf_import`. Each
  buffer exports as `NativeFrame::DmaBufImage` for `WgpuTextureImporter`.
  libgbm is loaded at run time (`libloading` 0.8, renamed `gbm-loader`).
- `Cargo.toml`: the `gl-target` feature and its optional `gleam`, `glow` and
  `khronos-egl` (dynamic) dependencies.

Upstream has no GL producer; this fits beside its CEF producers as a
generic "GL renderer into wgpu" source.


## Tab keeps focus in the page

`src/{linux,windows,macos}_cef/cef_backed.rs` give `WeldClient` a focus
handler. Off-screen there is no browser chrome to Tab into, so when focus
would leave the page past its last (or first) element (`OnTakeFocus`), the
browser takes it back (`SetFocus(false)`, then `SetFocus(true)`); otherwise
`document.hasFocus()` stays false until the next click.
