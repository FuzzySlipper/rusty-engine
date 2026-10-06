// The finish pass's inputs from a view's HDR world (render-wgpu `post.rs`):
// bloom (light above a threshold, spread through a mip chain) and auto
// exposure (the coverage-weighted log-average luminance, adapted over
// presentation time).

struct PostParams {
    // xy: the source's texel size; z: the bloom threshold; w: the soft knee.
    texel: vec4<f32>,
    // Auto exposure: x speed (per second), y the least and z the most
    // exposure, w the presentation seconds since the last adaptation (a
    // negative value takes the target at once).
    adapt: vec4<f32>,
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

// Four bilinear taps one texel out on each diagonal: a 4×4 box at half
// resolution.
fn box4(uv: vec2<f32>) -> vec4<f32> {
    let d = params.texel.xy;
    return (textureSample(source, source_sampler, uv + vec2<f32>(-d.x, -d.y))
        + textureSample(source, source_sampler, uv + vec2<f32>(d.x, -d.y))
        + textureSample(source, source_sampler, uv + vec2<f32>(-d.x, d.y))
        + textureSample(source, source_sampler, uv + vec2<f32>(d.x, d.y))) * 0.25;
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
    let color = textureSample(source, source_sampler, in.uv);
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
