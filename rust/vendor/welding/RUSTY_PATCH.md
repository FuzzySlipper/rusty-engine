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
