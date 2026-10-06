// The equirectangular sky behind the world pass, blending two panoramas;
// and the sun's disc and halo (`Frame.sun`, `Frame.atmosphere`), added over
// the sky or clear colour when the atmosphere draws them.

#import rusty::types::PI
#import rusty::view::frame

struct SkyUniform {
    // x: blend amount toward the second panorama
    amount: vec4<f32>,
};

@group(1) @binding(0) var<uniform> sky: SkyUniform;
@group(1) @binding(1) var sky_first: texture_2d<f32>;
@group(1) @binding(2) var sky_second: texture_2d<f32>;
@group(1) @binding(3) var sky_first_sampler: sampler;
@group(1) @binding(4) var sky_second_sampler: sampler;

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.ndc = xy;
    return out;
}

// The view ray through a point of the screen.
fn view_direction(ndc: vec2<f32>) -> vec3<f32> {
    let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    return normalize(far.xyz / far.w - frame.camera.xyz);
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let direction = view_direction(in.ndc);
    let uv = vec2<f32>(
        atan2(direction.z, direction.x) / (2.0 * PI) + 0.5,
        0.5 - asin(clamp(direction.y, -1.0, 1.0)) / PI,
    );
    let first = textureSample(sky_first, sky_first_sampler, uv).rgb;
    let second = textureSample(sky_second, sky_second_sampler, uv).rgb;
    return vec4<f32>(mix(first, second, sky.amount.x), 1.0);
}

// Added onto the background (one-one blending).
@fragment
fn fs_sun(in: SkyOut) -> @location(0) vec4<f32> {
    return vec4<f32>(sun_light(view_direction(in.ndc)), 0.0);
}

// The sun seen along `direction`: a disc of the set angular radius with an
// antialiased rim, and a halo a few degrees wide, in the sun's colour
// dimmed by an intensity below 1. The background is never finished, so
// these are display values.
fn sun_light(direction: vec3<f32>) -> vec3<f32> {
    let cosine = dot(direction, frame.sun.xyz);
    let angle = acos(clamp(cosine, -1.0, 1.0));
    let rim = max(fwidth(angle), 1e-5);
    let radius = frame.atmosphere.w;
    if frame.sun.w <= 0.0 {
        return vec3<f32>(0.0);
    }
    var light = 0.0;
    if radius > 0.0 {
        light = 1.0 - smoothstep(radius - rim, radius + rim, angle);
    }
    light += frame.haze.w * pow(max(cosine, 0.0), 128.0);
    return frame.sun_color.rgb * min(frame.sun_color.w, 1.0) * light;
}
