// DLSS's inputs drawn from the stage's own (`dlss.rs`):
// - its depth (`fs_depth`): the world's depth, and the viewmodel's where it
//   drew, nearer than any world surface. Under a temporal stage the
//   viewmodel draws against a depth of its own
//   (`RenderPicture::viewmodel_depth`, far where it drew nothing), and DLSS
//   takes one depth;
// - its bias toward the current colour (`fs_bias`): the motion target's
//   reactive mask (render-wgpu `motion.rs`), which DLSS reads from a
//   texture of its own.

@group(0) @binding(0) var world_depth: texture_depth_2d;
@group(0) @binding(1) var viewmodel_depth: texture_depth_2d;
@group(0) @binding(2) var motion: texture_2d<f32>;

// The viewmodel's depth scaled into [0, VIEWMODEL_SPAN): before every world
// surface but one within a thousandth of the near plane's depth, and in its
// own order.
const VIEWMODEL_SPAN: f32 = 0.001;

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_depth(@builtin(position) at: vec4<f32>) -> @location(0) f32 {
    let texel = vec2<i32>(at.xy);
    let viewmodel = textureLoad(viewmodel_depth, texel, 0);
    let world = textureLoad(world_depth, texel, 0);
    return select(world, viewmodel * VIEWMODEL_SPAN, viewmodel < 1.0);
}

@fragment
fn fs_bias(@builtin(position) at: vec4<f32>) -> @location(0) f32 {
    return clamp(textureLoad(motion, vec2<i32>(at.xy), 0).z, 0.0, 1.0);
}
