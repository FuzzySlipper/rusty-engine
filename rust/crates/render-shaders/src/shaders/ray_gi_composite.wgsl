// Ray-traced global illumination's light (`ray_gi.wgsl`) added to the opaque
// world's HDR picture between its pass's halves (render-wgpu `ray_gi.rs`):
// each lobe's light scaled by the surface's albedo for it. The alpha stays
// the world pass's: the finish pass composites by it.

@group(0) @binding(0) var diffuse_guide: texture_2d<f32>;
@group(0) @binding(1) var specular_guide: texture_2d<f32>;
@group(0) @binding(2) var irradiance: texture_2d<f32>;
@group(0) @binding(3) var specular: texture_2d<f32>;

@vertex
fn vs_composite(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_composite(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(position.xy);
    let diffuse = textureLoad(diffuse_guide, pixel, 0).rgb * textureLoad(irradiance, pixel, 0).rgb;
    let reflected = textureLoad(specular_guide, pixel, 0).rgb * textureLoad(specular, pixel, 0).rgb;
    return vec4<f32>(diffuse + reflected, 0.0);
}
