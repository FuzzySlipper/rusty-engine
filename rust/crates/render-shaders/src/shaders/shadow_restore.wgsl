// A shadow layer's static casters' depth, restored into its atlas tile from
// the static cache (render-wgpu `shadows.rs`) before only its moving casters
// draw over it. The cache has the atlas's layout, so a tile's texel is the
// same texel of the same page there; the instance index names the page.

@group(0) @binding(0) var cache: texture_depth_2d_array;

struct RestoreOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) page: u32,
};

@vertex
fn vs_restore(@builtin(vertex_index) index: u32, @builtin(instance_index) page: u32) -> RestoreOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: RestoreOut;
    out.clip = vec4<f32>(xy, 0.0, 1.0);
    out.page = page;
    return out;
}

@fragment
fn fs_restore(in: RestoreOut) -> @builtin(frag_depth) f32 {
    return textureLoad(cache, vec2<i32>(in.clip.xy), i32(in.page), 0);
}
