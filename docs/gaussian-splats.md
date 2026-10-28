# Gaussian splat characters (spike)

`render-wgpu` can draw skinned 3D Gaussian splat characters next to the
ordinary scene. This is a spike, there to measure real integration and
runtime costs before content commits to splats. It has no retained-model,
service or C# path: a Rust host adds assets and sets posed instances on
the `Renderer` directly.

```rust
let decoded = asset_import::decode_vsplat(&bytes)?;          // VIGGCHAR v1
let asset = SplatAsset::new(&points, world_joints);          // render-wgpu
let id = renderer.add_splat_asset(&asset);
renderer.set_splat_instances(vec![SplatInstance {
    asset: id,
    transform,
    palette: posed_palette(&world_joints, &parents, &rotations),
}]);
```

`rust/crates/render-wgpu/tests/gaussian_splats.rs` is the example scene. It
draws a 48k-splat Viggle character, posed mid-stride, among boxes that hide
parts of it. It also has the cost measurement (`splat_costs`, ignored by
default; it needs a hardware adapter). Both tests read the characters
from the asset-pipeline checkout (`SPLAT_ASSETS`, default
`~/dev/asset-pipeline/tests/splats`) and pass with a note when it is absent:

```bash
cargo test -p render-wgpu --release --test gaussian_splats -- --include-ignored --nocapture
```

## How it draws

Each world view runs three steps after its blended parts:

1. **Skin and project** (`splat_skin.wgsl`, one compute dispatch for all
   instances). Per splat: linear-blend skinning of the centre over 4 bones
   from the instance's palette, then the view-space covariance through the
   blended matrix, `(B R S)(B R S)ᵀ`. Viggle's Unity package nlerps
   delta-quaternions instead; the blended matrix is the same cost and also
   carries stretch. Then EWA projection to a 2D conic. A splat that is
   culled (behind the camera, off screen, or transparent) writes nothing.
   Each visible one is appended through an atomic counter, with a
   quantised view-depth key, so the sort and draw only see visible splats.
2. **Sort** (`splat_sort.wgsl`). An LSD radix sort, 4 bits per pass, over the
   visible count read from the GPU: count, a single-workgroup scan, then a
   stable scatter. Every instance shares one sort, so characters
   interleave correctly. Keys are 16, 24 (the default) or 32 bits of depth
   between the instances' rest-bound depths.
3. **Draw** (`splat_draw.wgsl`). One indirect instanced draw of
   screen-space quads, sized to where the Gaussian falls under 1/255.
   Colour is premultiplied and blended back to front over the HDR world
   target. Depth is tested against the world's depth and not written.
   Splat colour is sRGB, taken to linear, and unlit.

`asset-import` owns the file decoding (`vsplat.rs`); `render-wgpu` only
takes decoded splats (`SplatPoint`), as the dependency boundary requires.
The three passes report `splat-skin`, `splat-sort` and `splat-draw` in
`Renderer::gpu_readout`.

## VIGGCHAR v1 facts the reader depends on

- Blocks are component-major. Position is u16 and scale and opacity are u8,
  each against per-block min/max with `(levels - 1)` steps. Scale is
  stored as log σ and opacity as a logit. Rotation is xyz in u8, with w
  rebuilt as non-negative. Colour is u8 sRGB.
- `boneIndex` goes through `palette` to global rig indices. `boneWeight`
  stores K−1 bytes; the last weight is what remains of 255.
- **`joints` is not always world positions.** On Viggle's 441-bone
  characters each row is the bone's translation in its parent's frame.
  World joints come from composing those with the SDK's
  `PARENTS_441`/`REST_ROTATIONS_441` (xyzw). Rigs bound by asset-pipeline's
  `make_skinned_vsplat.py`, such as the Tripo rat, store world positions.
  The file says nothing about which one it holds. A production import
  must carry the rig (names, parents, bind) beside the splats.

## Measured cost

RX 9070 XT (RADV, Vulkan), 1920×1080, 4× MSAA. The numbers are medians of
120 GPU-timestamped frames, each posed on the CPU and uploaded. The
character is Pinoc `rat.vsplat` (48k) and its `performance` tier (16k), on
the 441-bone rig. "far" is a 4-deep crowd about 250 px tall. "near" puts
the front row across the whole frame, which is the raster's worst case.
The empty scene's GPU frame is about 0.3 ms.

| Instances × splats | Drawn | Skin | Sort (24-bit) | Draw | Splats total | Pose CPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 × 48k far | 47.5k | 0.01 ms | 0.10 ms | 0.13 ms | 0.23 ms | 0.02 ms |
| 1 × 48k near | 47.5k | 0.01 | 0.10 | 0.53 | 0.63 | 0.02 |
| 8 × 48k far | 380k | 0.02 | 0.25 | 0.61 | 0.88 | 0.14 |
| 32 × 48k far | 1.52M | 0.10 | 1.03 | 1.87 | 2.99 | 0.55 |
| 32 × 48k near | 1.35M | 0.08 | 0.98 | 3.66 | 4.73 | 0.60 |
| 1 × 16k far | 15.6k | <0.01 | 0.11 | 0.05 | 0.17 | 0.02 |
| 8 × 16k far | 124k | 0.02 | 0.16 | 0.33 | 0.51 | 0.14 |
| 32 × 16k far | 498k | 0.05 | 0.47 | 1.03 | 1.56 | 0.55 |
| 32 × 16k near | 440k | 0.04 | 0.29 | 1.50 | 1.83 | 0.55 |

At 32 × 48k, the sort takes 0.70 ms with 16-bit keys, 1.03 ms with 24-bit
and 1.36 ms with 32-bit, about 0.17 ms per 4-bit pass.

What this says:

- **Skinning on the GPU is nearly free.** At about 0.1 ms for 1.5M splats it
  does not bound the crowd, unlike in the web viewer. Posing is now CPU
  palette work, about 17 µs per character for 441 bones.
- **The raster is the largest cost and follows screen coverage.** A
  splat-heavy close shot costs twice the far crowd. Overdraw of
  semi-transparent quads is what a production renderer has to manage.
- **The sort is second.** It is linear in visible splats and in key bits.
  For one character, 16-bit keys are enough.
- 32 animated 48k characters fit in about 3–5 ms of a 16.7 ms frame on
  this GPU. The web path managed 13–25 fps for the same crowd. A 16k tier
  for distant characters roughly halves the cost.

## What a production version needs

- **A retained path.** The asset is admitted as a resource. Instances
  become scene nodes. Poses come from `animation-pose`'s one evaluation
  (`AnimatedScene`), not a host palette, and the rig is stored with the
  asset (see the `joints` fact above). The service is exposed to C# like
  the other render services.
- **LOD tiers and culling.** Pick high/balanced/performance per instance
  by projected size, and drop whole instances outside the frustum before
  skinning. Splats under a pixel could fold into fewer, larger ones per
  tier.
- **A faster sort.** Options are a onesweep or subgroup radix sort, 16-bit
  keys per instance-depth band, or sorting at half rate when the view is
  still. Merging per-instance sorts is another option, because instances
  rarely overlap in depth.
- **Relighting.** Splats are unlit today. The cheapest Engine-side option
  shades each splat centre once in the skin pass from the clustered
  lights and the shadow cascades, using normals from a proxy mesh or
  estimated from the splat's shortest axis. Casting shadows goes through
  a proxy mesh.
- **Shader-animated effect splats.** A product WGSL modifier hook in the
  skin pass (position, scale, colour and opacity from time and splat
  index), through the Engine's material/shader mechanism, as the web
  prototype's Wraith tail does with a work-buffer modifier.
- **Frame integration.** Motion vectors and a reactive mask so TAAU/DLSS do
  not smear splats; ordering against other blended parts and soft
  particles; viewmodel/backdrop layers. A software adapter (llvmpipe)
  should refuse the pass cleanly or keep a small budget.

## Licences

The radix sort follows the structure of Brush's `brush-sort` (Apache-2.0),
rewritten in WGSL from its CubeCL. The decode formulas follow
`@viggle/splat-engine` 0.3.2's `decode.ts`, and the skinning follows its
Unity package's `SplatSkinning.compute` (both MIT). No code is copied
verbatim, and no character content or SDK tables are vendored: the tests
read them from the asset-pipeline checkout.
