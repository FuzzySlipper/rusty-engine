// Ray-traced global illumination's light (`ray_gi.wgsl`) added to the opaque
// world's HDR picture between its pass's halves (render-wgpu `ray_gi.rs`):
// each lobe's light scaled by the surface's albedo for it. A pixel this frame
// did not trace (alpha 0) takes its neighbours'. The alpha stays the world
// pass's: the finish pass composites by it.

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
    let pixel = vec2<i32>(position.xy);
    var light = textureLoad(irradiance, pixel, 0);
    var reflection = textureLoad(specular, pixel, 0);
    // A pixel this frame did not trace takes the mean of the neighbours it did.
    if light.a == 0.0 {
        let last = vec2<i32>(textureDimensions(irradiance)) - 1;
        var steps = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
        for (var at = 0; at < 4; at++) {
            let beside = clamp(pixel + steps[at], vec2<i32>(0), last);
            light += textureLoad(irradiance, beside, 0);
            reflection += textureLoad(specular, beside, 0);
        }
        light /= max(light.a, 1.0);
        reflection /= max(reflection.a, 1.0);
    }
    let diffuse = textureLoad(diffuse_guide, pixel, 0).rgb * light.rgb;
    let reflected = textureLoad(specular_guide, pixel, 0).rgb * reflection.rgb;
    return vec4<f32>(diffuse + reflected, 0.0);
}
