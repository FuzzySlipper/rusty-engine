// An image effect (`CameraView.SetImageEffect`, lighting.flash): a white
// flash of strength `effect_parameter(0).x` over the whole view, and a
// shimmer of the picture by `effect_parameter(0).y` that ripples with time.
#import rusty::image::{ImagePixel, effect_parameter, picture_at}

fn image_effect(pixel: ImagePixel) -> vec4<f32> {
    let flash = effect_parameter(0u).x;
    let shimmer = effect_parameter(0u).y;
    let wave = vec2<f32>(sin(pixel.uv.y * 60.0 + pixel.time * 3.0), cos(pixel.uv.x * 50.0 + pixel.time * 2.0));
    let color = picture_at(pixel.uv + wave * shimmer * 0.004);
    return vec4<f32>(mix(color.rgb, vec3<f32>(1.0), flash), color.a);
}
