// The equirectangular sky behind the world pass, blending two panoramas;
// the sun's disc and halo (`Frame.sun`, `Frame.atmosphere`), added over the
// sky or clear colour when the atmosphere draws them; and the cloud layer
// over both (`fs_clouds`).

#import rusty::types::PI
#import rusty::view::frame
#import rusty::clouds::cloud_cover

struct SkyUniform {
    // x: blend amount toward the second panorama
    amount: vec4<f32>,
    // The cloud layer: x its coverage (0 to 1), y its altitude and z the
    // size of one cloud, in metres.
    clouds: vec4<f32>,
    // xy: the layer's drift over the ground (world x, z), in metres per
    // second.
    cloud_drift: vec4<f32>,
    // rgb: the tint of the light the clouds take.
    cloud_color: vec4<f32>,
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

// Where `direction` falls on the panoramas.
fn panorama_uv(direction: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(
        atan2(direction.z, direction.x) / (2.0 * PI) + 0.5,
        0.5 - asin(clamp(direction.y, -1.0, 1.0)) / PI,
    );
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let uv = panorama_uv(view_direction(in.ndc));
    let first = textureSample(sky_first, sky_first_sampler, uv).rgb;
    let second = textureSample(sky_second, sky_second_sampler, uv).rgb;
    let color = mix(first, second, sky.amount.x);
    // An overcast sky reads grey: behind a heavy cloud layer the panorama
    // loses its colour and some of its brightness, as much as the coverage.
    let grey = vec3<f32>(dot(color, vec3<f32>(0.299, 0.587, 0.114)));
    let overcast = sky.clouds.x * sky.clouds.x;
    return vec4<f32>(mix(color, grey * OVERCAST_SKY, overcast * OVERCAST_SHARE), 1.0);
}

// How grey and how dark the panorama goes under a full overcast.
const OVERCAST_SHARE: f32 = 0.6;
const OVERCAST_SKY: f32 = 0.8;

// The cloud layer, blended over the background (premultiplied alpha). The
// view ray meets a plane at the layer's altitude above the camera, so clouds
// shrink toward the horizon, where they fade into the panorama. They take
// the sun's colour, less where more cloud lies between them and the sun,
// and brighter at their thin edges toward it; and the sky's colour behind
// them, so a dusk sky warms them and a night sky darkens them.
@fragment
fn fs_clouds(in: SkyOut) -> @location(0) vec4<f32> {
    let direction = view_direction(in.ndc);
    let rise = max(direction.y, 0.02);
    let ground = frame.camera.xz + direction.xz * (sky.clouds.y / rise);
    let p = (ground - sky.cloud_drift.xy * frame.time.x) / sky.clouds.z;
    let detail = smoothstep(0.04, 0.4, direction.y);
    let cover = cloud_cover(p, detail, sky.clouds.x, 4);
    // Toward the sun across the layer, in clouds.
    let across = frame.sun.xz;
    let toward = across / max(length(across), 1e-3) * 0.35;
    let shadowed = cloud_cover(p + toward * 0.75, detail, sky.clouds.x, 4);
    let sun_up = smoothstep(-0.05, 0.15, frame.sun.y) * frame.sun.w;
    let sun = frame.sun_color.rgb * min(frame.sun_color.w, 1.5) * sun_up;
    let edge = 1.0 + 1.5 * pow(max(dot(direction, frame.sun.xyz), 0.0), 8.0) * (1.0 - cover);
    let direct = sun * mix(1.0, 0.55, shadowed) * edge;
    // The sky a little above the cloud, read at its coarsest detail.
    let uv = panorama_uv(normalize(vec3<f32>(direction.x, rise + 0.2, direction.z)));
    let behind = mix(textureSampleLevel(sky_first, sky_first_sampler, uv, 0.0).rgb,
        textureSampleLevel(sky_second, sky_second_sampler, uv, 0.0).rgb, sky.amount.x);
    // The sky lights them too, but mostly as grey: their colour is the sun's.
    let grey = dot(behind, vec3<f32>(0.299, 0.587, 0.114));
    let ambient = mix(vec3<f32>(grey), behind, 0.5) * 0.5;
    let color = sky.cloud_color.rgb * (direct * 0.9 + ambient);
    let alpha = cover * smoothstep(0.0, 0.25, direction.y);
    return vec4<f32>(color * alpha, alpha);
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
