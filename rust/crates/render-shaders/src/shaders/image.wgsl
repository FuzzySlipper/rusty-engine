#define_import_path rusty::image

// What a product image effect (`fn image_effect(pixel: ImagePixel) ->
// vec4<f32>`, `CameraView.SetImageEffect`) reads: the view's finished
// picture, which it may sample anywhere (to blur, refract or shift it), its
// own parameters, its two textures, and the presentation time. The effect's
// result replaces the pixel (render-wgpu `image_effect.rs`).

struct ImageEffectParams {
    // The product's own values (`ImageEffectRequest.Parameter0..3`).
    parameters: array<vec4<f32>, 4>,
    // xy: the picture's size in pixels; z: the presentation time in
    // seconds (it holds while the simulation does).
    picture: vec4<f32>,
};

@group(0) @binding(0) var<uniform> image_params: ImageEffectParams;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(5) var effect_map_a: texture_2d<f32>;
@group(0) @binding(6) var effect_sampler_a: sampler;
@group(0) @binding(7) var effect_map_b: texture_2d<f32>;
@group(0) @binding(8) var effect_sampler_b: sampler;

// One pixel of the finished picture, as the effect sees it.
struct ImagePixel {
    // Where the pixel is: 0 to 1 across and down the picture.
    uv: vec2<f32>,
    // The finished colour there (display values, alpha its coverage).
    color: vec4<f32>,
    // The world's depth there, 0 at the near plane to 1 at the far plane
    // and over the sky.
    depth: f32,
    // The presentation time in seconds.
    time: f32,
};

// The finished picture at `uv` (0 to 1), filtered.
fn picture_at(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(picture, picture_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
}

// The product's parameter row `index` (0 to 3).
fn effect_parameter(index: u32) -> vec4<f32> {
    return image_params.parameters[min(index, 3u)];
}

// The product's textures at `uv` (white when unset).
fn effect_texture_a(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(effect_map_a, effect_sampler_a, uv, 0.0);
}

fn effect_texture_b(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(effect_map_b, effect_sampler_b, uv, 0.0);
}

// The picture's size in pixels.
fn picture_size() -> vec2<f32> {
    return image_params.picture.xy;
}
