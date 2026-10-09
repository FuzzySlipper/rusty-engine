// The equirectangular sky behind the world pass, blending two panoramas;
// the sun's disc and halo (`Frame.sun`, `Frame.atmosphere`), added over the
// sky or clear colour when the atmosphere draws them; and the cloud layer
// over both, flat (`fs_clouds`) or raymarched (`fs_clouds_march`, drawn by
// `fs_clouds_composite`).

#import rusty::types::PI
#import rusty::view::frame
#import rusty::clouds::{cloud_cover, cloud_coverage, cloud_density, cloud_detail, cloud_kind_detail, CLOUD_OCTAVES}

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
// view ray meets a plane at the layer's altitude (absolute, as the ground's
// shade `cloud_light` and the volumetric slab measure it), so clouds shrink
// toward the horizon, where they fade into the panorama. A camera at or
// above the altitude sees no flat layer overhead. They take
// the sun's colour, less where more cloud lies between them and the sun,
// and brighter at their thin edges toward it; and the sky's colour behind
// them, so a dusk sky warms them and a night sky darkens them.
@fragment
fn fs_clouds(in: SkyOut) -> @location(0) vec4<f32> {
    let direction = view_direction(in.ndc);
    let above = sky.clouds.y - frame.camera.y;
    if above <= 0.0 {
        return vec4<f32>(0.0);
    }
    let rise = max(direction.y, 0.02);
    let ground = frame.camera.xz + direction.xz * (above / rise);
    let p = (ground - sky.cloud_drift.xy * frame.time.x) / sky.clouds.z;
    // The layer's coverage raised by the regions over this point.
    let coverage = cloud_coverage(ground);
    let detail = cloud_kind_detail(cloud_detail(direction.y), coverage.w);
    let cover = cloud_cover(p, detail, coverage.x, CLOUD_OCTAVES);
    // Toward the sun across the layer, in clouds.
    let across = frame.sun.xz;
    let toward = across / max(length(across), 1e-3) * 0.35;
    let shadowed = cloud_cover(p + toward * 0.75, detail, coverage.x, CLOUD_OCTAVES);
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
    // A storm region's clouds are darker underneath.
    let color = sky.cloud_color.rgb * (direct * 0.9 + ambient) * (1.0 - 0.6 * coverage.y);
    let alpha = cover * smoothstep(0.0, 0.25, direction.y);
    return vec4<f32>(color * alpha, alpha);
}

// Extinction per metre of the volumetric clouds at full density.
const CLOUD_EXTINCTION: f32 = 0.004;
// Light scattered many times inside a cloud reaches deeper than Beer's law
// alone lets it: a second, weaker, longer-reaching term (after Wrenninge).
const CLOUD_MULTIPLE_SCATTER: f32 = 0.35;
const CLOUD_MULTIPLE_REACH: f32 = 0.2;
// Steps of the march toward the sun, for each sample's own shadow.
const CLOUD_LIGHT_STEPS: i32 = 2;
// How far the clouds reach before they fade into the panorama, metres.
const CLOUD_REACH: f32 = 30000.0;

// The cloud layer raymarched through a slab from its altitude up by its
// thickness, at reduced resolution (render-wgpu `cloud_march.rs`) and
// filtered over frames: `fs_clouds_march` marches each pixel of a target
// half the view's size, its step offset moving each frame, and blends it
// with that pixel's cloud as the view's previous frame saw it (reprojected
// by the camera's move and the clouds' drift); `fs_clouds_composite` draws
// that over the background, premultiplied as the flat layer.

struct CloudSample {
    // Premultiplied colour and opacity.
    color: vec4<f32>,
    // The distance along the ray of the cloud it sees, metres.
    distance: f32,
};

struct CloudMarchParams {
    // The view's previous frame: view-projection and eye.
    prev_view_proj: mat4x4<f32>,
    // xyz: the previous eye; w: seconds since the previous frame (the
    // clouds drifted that long).
    prev_eye: vec4<f32>,
    // x: the history's weight (0: none); y: this frame's step phase; zw:
    // the reduced target's size, pixels.
    temporal: vec4<f32>,
};

@group(2) @binding(0) var<uniform> cloud_march: CloudMarchParams;
@group(2) @binding(1) var cloud_history: texture_2d<f32>;
@group(2) @binding(2) var cloud_sampler: sampler;
@group(2) @binding(3) var cloud_current: texture_2d<f32>;

// One ray through the cloud slab (`Frame.clouds`): each sample's density (`rusty::clouds::cloud_density`) is lit by
// the sun through the cloud between it and the sun (Beer's law, with a
// forward-scattering lobe and a dark-edge powder term) and by the sky behind
// it, and dims what lies past it. Toward the horizon the clouds fade into
// the panorama, as the flat layer's do. Steps per ray: `Frame.cloud_drift.w`.
fn march_clouds(direction: vec3<f32>, pixel: vec2<f32>, phase: f32) -> CloudSample {
    var none: CloudSample;
    if direction.y <= 0.01 {
        return none;
    }
    let base = frame.clouds.y;
    let top = base + frame.clouds.w;
    let camera = frame.camera.xyz;
    let start = max((base - camera.y) / direction.y, 0.0);
    let end = min(max((top - camera.y) / direction.y, 0.0), CLOUD_REACH);
    if end <= start {
        return none;
    }
    let steps = max(i32(frame.cloud_drift.w), 1);
    let step = (end - start) / f32(steps);
    // A per-pixel offset breaks the steps' banding into fine noise; it
    // moves each frame (`phase`), so the temporal history gathers them.
    let jitter = fract(fract(sin(dot(pixel, vec2<f32>(12.9898, 78.233))) * 43758.5453) + phase);
    let sun_up = smoothstep(-0.05, 0.15, frame.sun.y) * frame.sun.w;
    let sun = frame.sun_color.rgb * min(frame.sun_color.w, 1.5) * sun_up;
    let toward_sun = normalize(frame.sun.xyz + vec3<f32>(0.0, 1e-3, 0.0));
    let cosine = dot(direction, toward_sun);
    // Forward scattering toward the sun, with some back scatter.
    let g = 0.6;
    let forward = (1.0 - g * g) / pow(1.0 + g * g - 2.0 * g * cosine, 1.5);
    let lobe = mix(1.0, min(forward, 4.0), 0.25);
    let light_step = frame.clouds.w * 0.18;
    // The sky a little above the horizon, at its coarsest, lights them too.
    let uv = panorama_uv(normalize(vec3<f32>(direction.x, max(direction.y, 0.05) + 0.2, direction.z)));
    let behind = mix(textureSampleLevel(sky_first, sky_first_sampler, uv, 0.0).rgb,
        textureSampleLevel(sky_second, sky_second_sampler, uv, 0.0).rgb, sky.amount.x);
    let grey = dot(behind, vec3<f32>(0.299, 0.587, 0.114));
    let ambient = mix(vec3<f32>(grey), behind, 0.5) * 1.05;
    var transmittance = 1.0;
    var color = vec3<f32>(0.0);
    var weighted = 0.0;
    for (var index = 0; index < steps; index = index + 1) {
        let t = start + (f32(index) + jitter) * step;
        let position = camera + direction * t;
        let density = cloud_density(position, 2);
        if density <= 0.002 {
            continue;
        }
        var optical = 0.0;
        for (var k = 1; k <= CLOUD_LIGHT_STEPS; k = k + 1) {
            optical += cloud_density(position + toward_sun * light_step * f32(k), 1) * light_step;
        }
        let through = exp(-CLOUD_EXTINCTION * optical)
            + CLOUD_MULTIPLE_SCATTER * exp(-CLOUD_EXTINCTION * CLOUD_MULTIPLE_REACH * optical);
        let extinction = density * CLOUD_EXTINCTION;
        let powder = 1.0 - exp(-2.0 * extinction * step);
        let here = cloud_coverage(position.xz);
        let height = clamp((position.y - base) / max(here.z, 1.0), 0.0, 1.0);
        let darkness = here.y;
        let lit = (sun * through * lobe * mix(0.7, 1.0, powder) * 0.45
            + ambient * mix(0.6, 0.9, height)) * (1.0 - 0.6 * darkness);
        let passes = exp(-extinction * step);
        color += transmittance * lit * (1.0 - passes);
        weighted += transmittance * (1.0 - passes) * t;
        transmittance *= passes;
        if transmittance < 0.02 {
            break;
        }
    }
    let fade = smoothstep(0.02, 0.3, direction.y) * (1.0 - smoothstep(CLOUD_REACH * 0.5, CLOUD_REACH, start));
    let opacity = 1.0 - transmittance;
    var sample: CloudSample;
    sample.color = vec4<f32>(sky.cloud_color.rgb * color * fade, opacity * fade);
    // Where the cloud the ray sees stands, for reprojection: its
    // opacity-weighted distance, or the slab's middle through clear air.
    sample.distance = select((start + end) * 0.5, weighted / opacity, opacity > 1e-3);
    return sample;
}

// One pixel of the reduced clouds, blended with its history.
@fragment
fn fs_clouds_march(in: SkyOut) -> @location(0) vec4<f32> {
    let direction = view_direction(in.ndc);
    let current = march_clouds(direction, in.clip.xy, cloud_march.temporal.y);
    let weight = cloud_march.temporal.x;
    if weight <= 0.0 {
        return current.color;
    }
    // The cloud this pixel sees, where it stood a frame ago (it drifted),
    // as the previous view saw it.
    let point = frame.camera.xyz + direction * current.distance;
    let drifted = point - vec3<f32>(frame.cloud_drift.x, 0.0, frame.cloud_drift.y) * cloud_march.prev_eye.w;
    let clip = cloud_march.prev_view_proj * vec4<f32>(drifted, 1.0);
    if clip.w <= 0.0 {
        return current.color;
    }
    let uv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return current.color;
    }
    let previous = textureSampleLevel(cloud_history, cloud_sampler, uv, 0.0);
    // A pixel whose cloud moved across the target trusts its history less.
    let size = cloud_march.temporal.zw;
    let moved = length((uv - in.clip.xy / size) * size);
    return mix(previous, current.color, clamp(weight + moved * 0.5, weight, 1.0));
}

// The reduced clouds over the view's background, upscaled.
@fragment
fn fs_clouds_composite(in: SkyOut) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.ndc.x * 0.5 + 0.5, 0.5 - in.ndc.y * 0.5);
    return textureSampleLevel(cloud_current, cloud_sampler, uv, 0.0);
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
