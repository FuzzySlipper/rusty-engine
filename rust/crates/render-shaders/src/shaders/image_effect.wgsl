// A product image effect over a view's finished picture (render-wgpu
// `image_effect.rs`): a full-screen triangle that hands each pixel of the
// output to the product's `image_effect`, with the picture (drawn first
// into an image of its own) and the world's depth there.

#import rusty::image::{ImagePixel, image_params, picture_at}
#import rusty::product::image_effect

@group(0) @binding(3) var picture_depth: texture_depth_2d;
@group(0) @binding(4) var picture_depth_multisampled: texture_depth_multisampled_2d;

struct ImageOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_image(@builtin(vertex_index) index: u32) -> ImageOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: ImageOut;
    out.clip = vec4<f32>(xy, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

// The pixel at `uv`, with the depth `depth`.
fn pixel(uv: vec2<f32>, depth: f32) -> ImagePixel {
    var pixel: ImagePixel;
    pixel.uv = uv;
    pixel.color = picture_at(uv);
    pixel.depth = depth;
    pixel.time = image_params.picture.z;
    return pixel;
}

// The picture's depth texel under an output pixel at `uv` (the picture may
// be smaller than the output under a render scale).
fn depth_texel(uv: vec2<f32>, size: vec2<u32>) -> vec2<i32> {
    return vec2<i32>(min(vec2<u32>(uv * vec2<f32>(size)), size - vec2<u32>(1u)));
}

@fragment
fn fs_image(in: ImageOut) -> @location(0) vec4<f32> {
    let texel = depth_texel(in.uv, textureDimensions(picture_depth));
    return image_effect(pixel(in.uv, textureLoad(picture_depth, texel, 0)));
}

@fragment
fn fs_image_multisampled(in: ImageOut) -> @location(0) vec4<f32> {
    let texel = depth_texel(in.uv, textureDimensions(picture_depth_multisampled));
    return image_effect(pixel(in.uv, textureLoad(picture_depth_multisampled, texel, 0)));
}
