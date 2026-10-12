// The star field in the sky (render-wgpu `stars.rs`), drawn over the
// panorama before the sun, the moon and the clouds, never fogged: each star
// of the catalogue a small soft point (`vs_star`, `fs_star`), and the band
// texture (the Milky Way) added over the whole sky (`fs_star_band`). The
// catalogue's frame turns into the world by `stars.orientation`.

#import rusty::types::PI
#import rusty::view::frame

struct StarParams {
    // The catalogue's frame to the world: a unit quaternion (x, y, z, w).
    orientation: vec4<f32>,
    // x: the limiting magnitude; y: the brightness; z: the band's strength;
    // w: the twinkle (0 to 1).
    look: vec4<f32>,
    // xy: the view's size in pixels.
    viewport: vec4<f32>,
};

@group(1) @binding(0) var<uniform> stars: StarParams;
@group(1) @binding(1) var band: texture_2d<f32>;
@group(1) @binding(2) var band_sampler: sampler;

// A star's footprint: a Gaussian this wide (standard deviation, pixels),
// drawn over a square this far either side of its centre. A star lands on
// about a pixel at any resolution, and as it moves between pixels its light
// shifts across them rather than flickering.
const STAR_SIGMA: f32 = 0.75;
const STAR_REACH: f32 = 2.5;
// Each magnitude dims a star by 10^(-0.4 × this): half the eye's contrast,
// so a sky to magnitude 6 stays readable on a screen.
const MAGNITUDE_CONTRAST: f32 = 0.5;
// Stars fade out over this many magnitudes either side of the limit.
const LIMIT_SOFTNESS: f32 = 0.25;
// How far a twinkle swings a star's light, and how fast (radians per
// second, plus up to the spread more).
const TWINKLE_SWING: f32 = 0.45;
const TWINKLE_RATE: f32 = 3.0;
const TWINKLE_SPREAD: f32 = 4.0;

// `v` turned by the unit quaternion `q`.
fn turned(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// A uniform value in [0, 1) for a star's index.
fn star_hash(index: u32, salt: u32) -> f32 {
    var v = index * 747796405u + salt * 2891336453u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    v = (v >> 22u) ^ v;
    return f32(v >> 8u) / 16777216.0;
}

struct StarOut {
    @builtin(position) clip: vec4<f32>,
    // The star's light, before its footprint.
    @location(0) light: vec3<f32>,
    // This corner's offset from the star's centre, pixels.
    @location(1) offset: vec2<f32>,
};

@vertex
fn vs_star(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) index: u32,
    @location(0) direction: vec3<f32>,
    @location(1) magnitude: f32,
    @location(2) color: vec3<f32>,
) -> StarOut {
    var out: StarOut;
    let limit = stars.look.x;
    let shown = 1.0 - smoothstep(limit - LIMIT_SOFTNESS, limit + LIMIT_SOFTNESS, magnitude);
    let rate = TWINKLE_RATE + TWINKLE_SPREAD * star_hash(index, 1u);
    let twinkle = 1.0 + stars.look.w * TWINKLE_SWING
        * sin(frame.time.x * rate + 2.0 * PI * star_hash(index, 2u));
    let light = color * stars.look.y * pow(10.0, -0.4 * MAGNITUDE_CONTRAST * magnitude) * shown * twinkle;
    // A direction projects as a point at infinity.
    var clip = frame.view_proj * vec4<f32>(turned(stars.orientation, direction), 0.0);
    if clip.w <= 0.0 || all(light <= vec3<f32>(0.0)) {
        // Behind the eye or unshown: nothing drawn.
        out.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        return out;
    }
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let offset = corners[vertex] * STAR_REACH;
    clip = vec4<f32>(
        clip.xy + offset * 2.0 / stars.viewport.xy * clip.w,
        0.5 * clip.w,
        clip.w,
    );
    out.clip = clip;
    out.light = light;
    // Clip y runs up, pixels down.
    out.offset = vec2<f32>(offset.x, -offset.y);
    return out;
}

@fragment
fn fs_star(in: StarOut) -> @location(0) vec4<f32> {
    let falloff = exp(-dot(in.offset, in.offset) / (2.0 * STAR_SIGMA * STAR_SIGMA));
    return vec4<f32>(in.light * falloff, 0.0);
}

struct BandOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_star_band(@builtin(vertex_index) index: u32) -> BandOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: BandOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.ndc = xy;
    return out;
}

// The band texture where the view looks, in the catalogue's frame (2:1
// equirectangular, as the sky panorama), added at its strength.
@fragment
fn fs_star_band(in: BandOut) -> @location(0) vec4<f32> {
    let far = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let direction = normalize(far.xyz / far.w - frame.camera.xyz);
    let inverse = vec4<f32>(-stars.orientation.xyz, stars.orientation.w);
    let local = turned(inverse, direction);
    let uv = vec2<f32>(
        atan2(local.z, local.x) / (2.0 * PI) + 0.5,
        0.5 - asin(clamp(local.y, -1.0, 1.0)) / PI,
    );
    let color = textureSampleLevel(band, band_sampler, uv, 0.0).rgb;
    return vec4<f32>(color * stars.look.z, 0.0);
}
