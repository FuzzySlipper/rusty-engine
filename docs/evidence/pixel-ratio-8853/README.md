# CSS-pixel presentation at the output's pixel ratio (#8853)

render-wgpu had no device pixel ratio. Labels (#8827), pixel-sized sprites
(#8787) and particle points were authored in CSS pixels but drawn as target
pixels. On a denser target (a HiDPI desktop window, or a stream viewer asking
for device pixels) they came out at 1/ratio of their size, while the
Chromium-drawn UI beside them kept its CSS size. Three rendered at
`pixelRatio` and so kept them at CSS size.

## Change

- **`Renderer::set_pixel_ratio(ratio)`** records the target pixels per CSS
  pixel. The host sets it, never the product. It applies to:
  - **Labels.** They rasterize at the ratio, so they stay crisp rather than
    being magnified.
    - The label canvas takes a scale. Callers still draw in CSS pixels; glyph
      size, rectangles, radii, the 1px borders and icons are mapped to target
      pixels. At ratio 1 every existing label screenshot is unchanged.
    - Layout multiplies the policy's CSS lengths by the ratio: indicator width,
      row estimate, safe area, stack step and placement hysteresis.
    - A ratio change re-rasterizes every active label from the fonts and icons
      its creation already loaded.
  - **Pixel-sized sprites.** `size × ratio` target pixels. Viewport-placed
    sprites are fractions of the viewport and did not need a change.
  - **Particles.** The 24-pixels-per-unit point size is multiplied by the
    ratio, as Three multiplied point size by `pixelRatio`.
- **Desktop shell.** It sets the window's `scale_factor()`, the same factor
  it gives Chromium for the page, before each draw.
- **Streaming.**
  - The page states its CSS width with each frame request
    (`&cssWidth=`, added to `streamed-frame-surface.ts`).
  - The frame route records `width / cssWidth` as the viewer's ratio
    (`ProductDevFrameStream::wanted_pixel_ratio`), and `render-stream` sets it
    before each draw.
  - The stream surface still renders at `pixelRatio ?? 1`, so today the ratio
    is 1 unless the application sets a renderer `pixelRatio`.

## Evidence

**Fixtures at ratio 2 beside ratio 1.**
- **`labels_keep_their_css_size_at_device_pixel_ratio_two`.**
  - A structured indicator with a meter and a text label are drawn into a 2×
    target at ratio 2. The drawn bounds are twice the ratio-1 bounds, within
    3 px (`screenshots/labels-ratio-2.png`, crisp text).
  - Raising the ratio after the labels exist rasterizes them to exactly the
    image created at ratio 2.
- **`pixel_sized_sprites_keep_their_css_size_at_device_pixel_ratio_two`.** A
  24-pixel sprite covers twice the columns and rows in a 2× target at ratio 2.
- **Frame route.** A frame request with `cssWidth` parses to its ratio; a
  `cssWidth` without a size, or of 0, is refused.

**Desktop window.**
- Doom's legacy-voxel scene (its `doom-exit` indicator is a render-wgpu
  structured label) ran on X11 in the private headless KWin, with winit's
  scale factor forced through `WINIT_X11_SCALE_FACTOR` (`dpr_run.sh`).
- The same physical window size was captured three times:
  - **`window-scale-1.jpg`.** Scale 1: the indicator and the Chromium HUD at
    their CSS size.
  - **`window-scale-2-before.jpg`.** Scale 2 on the `fbc78ee4c` desktop pack:
    Chromium doubles the HUD, and the indicator stays at half its CSS size.
  - **`window-scale-2-after.jpg`.** Scale 2 with this change: the indicator
    doubles with the HUD and stays crisp.

**Checks.**
- `cargo test` for render-wgpu (all label and effects screenshots unchanged
  at ratio 1), render-stream, product-dev-host, csharp-product-runtime and
  desktop-shell.
- Workspace clippy, stable clippy, desktop-shell clippy with `web-overlay`,
  and fmt.
- product-browser-host: 106 tests. Docs verification.
