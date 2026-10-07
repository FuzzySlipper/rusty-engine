// The opaque pass's depth, copied into a single-sample depth texture for
// the blend pass's water surfaces (render-wgpu `water.rs`): a fullscreen
// triangle writing each pixel's depth, sample 0 of a multisampled target.

@group(0) @binding(0) var source_depth: texture_depth_2d;
@group(0) @binding(1) var source_depth_multisampled: texture_depth_multisampled_2d;

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4<f32>(xy, 0.0, 1.0);
}

@fragment
fn fs_copy_depth(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(source_depth, vec2<i32>(position.xy), 0);
}

@fragment
fn fs_copy_depth_multisampled(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(source_depth_multisampled, vec2<i32>(position.xy), 0);
}
