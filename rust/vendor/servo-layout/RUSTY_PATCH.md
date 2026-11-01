# servo-layout patch

Source: crates.io `servo-layout` 0.7.0, https://github.com/servo/servo/tree/v0.7.0/components/layout
License: MPL-2.0. The desktop pack ships this modified source in
`share/third-party/servo-layout`.

Engine #10524 draws CSS masks (`mask-image` with `mask-mode`, `mask-origin`,
`mask-clip`, `mask-size`, `mask-position`, `mask-repeat` and
`mask-composite`). Servo 0.7.0 parsed them only behind
`layout.unimplemented` and drew none of them.

- `display_list/mask.rs` composes a masked element's mask layers into one
  alpha image over its border box, at device resolution (at most 4096 pixels
  a side), and clips the element's stacking context with it as a WebRender
  image mask. Layers are images (raster, `data:` and SVG), gradients and
  colours, tiled as backgrounds are; a layer that is `none`, loading or
  broken masks everything out. A mask image is kept while recent display
  lists use it, so an unchanged mask is composed and uploaded once.
- `display_list/background.rs`: the tile layout of a background layer is
  shared with mask layers (`layout_tiles`).
- `display_list/gradient.rs`: a gradient resolves to its shape and stops
  (`resolve`) before it is added to a display list, so a mask can sample it.
- `display_list/mod.rs`, `layout_impl.rs`: a stacking context with a mask is
  pushed with the mask's clip; the layout thread keeps the mask images.
- `style_ext.rs`: a mask image other than `none` makes a stacking context.

Not drawn: references to SVG `<mask>` elements, `mask-border`, and
`mask-clip`/`mask-origin` values other than the three boxes.

Remove this vendored copy once a Servo release draws masks.
