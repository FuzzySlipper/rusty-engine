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
