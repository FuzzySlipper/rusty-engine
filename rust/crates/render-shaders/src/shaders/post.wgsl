// The finish pass's inputs from a view's HDR world (render-wgpu `post.rs`):
// bloom (light above a threshold, spread through a mip chain), auto
// exposure (the coverage-weighted log-average luminance, adapted over
// presentation time) and sun shafts (the sky around the sun, blurred along
// rays toward it). A pass reads only its region of the source, so views
// sharing a target never sample each other.

struct PostParams {
    // xy: the source's texel size; z: the bloom threshold; w: the soft knee
    // (shafts: zw the sun's place in the view's uv).
    texel: vec4<f32>,
    // Auto exposure: x speed (per second), y the least and z the most
    // exposure, w the presentation seconds since the last adaptation (a
    // negative value takes the target at once). Shafts: x how far a blur
    // pass marches toward the sun (a fraction of the way), y the view's
    // aspect ratio.
    adapt: vec4<f32>,
    // The source region the pass's uv spans: x, y, width, height in the
    // source's uv (the view's viewport in the world; the whole of a mip).
    region: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: PostParams;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;
// Auto exposure's last value (adaptation only).
@group(0) @binding(3) var previous: texture_2d<f32>;

// The luminance auto exposure aims to bring to middle grey.
const EXPOSURE_KEY: f32 = 0.18;

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: FullscreenOut;
    out.clip = vec4<f32>(xy, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

// The source uv a pass's uv stands for.
fn source_uv(uv: vec2<f32>) -> vec2<f32> {
    return params.region.xy + uv * params.region.zw;
}

// The source at `at`, kept half a texel inside the region, so a tap at its
// edge takes the edge texel and never one beyond.
fn tap(at: vec2<f32>) -> vec4<f32> {
    let half_texel = params.texel.xy * 0.5;
    let low = params.region.xy + half_texel;
    let high = params.region.xy + params.region.zw - half_texel;
    return textureSampleLevel(source, source_sampler, clamp(at, low, max(low, high)), 0.0);
}

// Four bilinear taps one texel out on each diagonal: a 4×4 box at half
// resolution.
fn box4(uv: vec2<f32>) -> vec4<f32> {
    let d = params.texel.xy;
    let at = source_uv(uv);
    return (tap(at + vec2<f32>(-d.x, -d.y))
        + tap(at + vec2<f32>(d.x, -d.y))
        + tap(at + vec2<f32>(-d.x, d.y))
        + tap(at + vec2<f32>(d.x, d.y))) * 0.25;
}

// The first bloom mip: the world's light above the threshold (a soft knee
// below it), downsampled.
@fragment
fn fs_bloom_prefilter(in: FullscreenOut) -> @location(0) vec4<f32> {
    let color = box4(in.uv).rgb;
    let brightness = max(color.r, max(color.g, color.b));
    let threshold = params.texel.z;
    let knee = max(params.texel.w, 1e-4);
    let soft = clamp(brightness - threshold + knee, 0.0, 2.0 * knee);
    let contribution = max(soft * soft / (4.0 * knee), brightness - threshold) / max(brightness, 1e-4);
    return vec4<f32>(color * max(contribution, 0.0), 1.0);
}

@fragment
fn fs_bloom_down(in: FullscreenOut) -> @location(0) vec4<f32> {
    return vec4<f32>(box4(in.uv).rgb, 1.0);
}

// A 3×3 tent from the smaller mip, added onto the larger.
@fragment
fn fs_bloom_up(in: FullscreenOut) -> @location(0) vec4<f32> {
    let d = params.texel.xy;
    var sum = textureSample(source, source_sampler, in.uv).rgb * 4.0;
    sum += (textureSample(source, source_sampler, in.uv + vec2<f32>(-d.x, 0.0)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(d.x, 0.0)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(0.0, -d.y)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(0.0, d.y)).rgb) * 2.0;
    sum += textureSample(source, source_sampler, in.uv + vec2<f32>(-d.x, -d.y)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(d.x, -d.y)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(-d.x, d.y)).rgb
        + textureSample(source, source_sampler, in.uv + vec2<f32>(d.x, d.y)).rgb;
    return vec4<f32>(sum / 16.0, 1.0);
}

// Log luminance weighted by coverage, and the coverage: their averages over
// the view give the world's log-average luminance.
@fragment
fn fs_luminance(in: FullscreenOut) -> @location(0) vec4<f32> {
    let color = tap(source_uv(in.uv));
    if color.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    let rgb = color.rgb / color.a;
    let luminance = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(log2(max(luminance, 1e-4)) * color.a, color.a, 0.0, 0.0);
}

// Halve a luminance mip: one bilinear tap between four texels.
@fragment
fn fs_luminance_down(in: FullscreenOut) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, in.uv);
}

// Move the exposure toward the one that brings the world's log-average
// luminance to middle grey, within the product's range, by its speed.
@fragment
fn fs_adapt() -> @location(0) vec4<f32> {
    let average = textureLoad(source, vec2<u32>(0u), 0);
    var target_exposure = 1.0;
    if average.g > 0.0 {
        target_exposure = EXPOSURE_KEY / exp2(average.r / average.g);
    }
    target_exposure = clamp(target_exposure, params.adapt.y, params.adapt.z);
    let seconds = params.adapt.w;
    if seconds < 0.0 {
        return vec4<f32>(target_exposure, 0.0, 0.0, 1.0);
    }
    let last = textureLoad(previous, vec2<u32>(0u), 0).r;
    let step = 1.0 - exp(-seconds * params.adapt.x);
    let exposure = exp2(mix(log2(max(last, 1e-6)), log2(target_exposure), step));
    return vec4<f32>(exposure, 0.0, 0.0, 1.0);
}

// Sun shafts are strongest at the sun and gone this far from it, in view
// heights.
const SHAFT_RADIUS: f32 = 0.35;
// Taps of each shaft blur pass (render-wgpu `post.rs` `SHAFT_TAPS`).
const SHAFT_TAPS: i32 = 12;

// 1 at the sun, falling to 0 a shaft radius away.
fn near_sun(uv: vec2<f32>) -> f32 {
    let offset = (uv - params.texel.zw) * vec2<f32>(params.adapt.y, 1.0);
    return max(1.0 - length(offset) / SHAFT_RADIUS, 0.0);
}

// The sky's share of each half-resolution pixel (what the world left
// uncovered), weighted toward the sun.
@fragment
fn fs_shafts_mask(in: FullscreenOut) -> @location(0) vec4<f32> {
    let sky = 1.0 - clamp(box4(in.uv).a, 0.0, 1.0);
    let near = near_sun(in.uv);
    return vec4<f32>(sky * near * near, 0.0, 0.0, 1.0);
}

// The mask blurred along the ray from the pixel toward the sun, nearer taps
// weighing more: light streams past whatever covers the sky.
@fragment
fn fs_shafts_blur(in: FullscreenOut) -> @location(0) vec4<f32> {
    let step = (params.texel.zw - in.uv) * params.adapt.x / f32(SHAFT_TAPS);
    var sum = 0.0;
    var weights = 0.0;
    for (var index = 0; index < SHAFT_TAPS; index = index + 1) {
        let weight = 1.0 - f32(index) / f32(SHAFT_TAPS);
        sum += tap(in.uv + step * f32(index)).r * weight;
        weights += weight;
    }
    return vec4<f32>(sum / weights, 0.0, 0.0, 1.0);
}
