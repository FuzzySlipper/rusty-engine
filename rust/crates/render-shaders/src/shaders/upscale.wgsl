// The temporal upscale stage, TAAU (render-wgpu `upscale.rs`): each output
// pixel gathers the jittered frames the primary views drew at render
// resolution into a history kept at output resolution.
//
// Per output pixel, `fs_resolve`:
// - reconstructs this frame's colour from the 3×3 render pixels around it,
//   each weighted by how near its jittered sample stands to the pixel's
//   centre, and takes their mean and spread;
// - follows the motion back to where the pixel stood in the previous frame:
//   the motion of the nearest surface among those 3×3 (dilated by depth, so
//   a moving edge carries its own motion over the background it uncovers;
//   the viewmodel, drawn against a depth of its own, is nearest of all);
// - samples the history there (a sharp bicubic), and clips it towards the
//   mean to the neighbourhood's colour box in YCoCg, so what the history
//   holds that this frame no longer shows does not linger;
// - blends the two by how much history the pixel has gathered and how well
//   this frame sampled it, keeping fewer frames while it moves and fewest
//   where it lands between the history's texels (each such resampling
//   softens it, which swaying grass and leaves pile up frame after frame),
//   taking at least the reactive mask's share of
//   this frame (`rusty::motion`: water's ripples and reflections, labels
//   and overlay lines), and half where a pixel has no motion of its own
//   (`rusty::types::NO_MOTION`). Such pixels keep that little history: they
//   cannot ghost far. Where the mask says the pixel was drawn without the
//   jitter (labels, overlay lines), this frame's colour is the picture
//   where it stands.
//
// Colour works in the finished picture's linear values (it was already tone
// mapped by the finish pass), averaged as a multisample resolve averages
// them. The history's alpha is how many frames' worth of samples it holds.
// `fs_sharpen` then lightly sharpens the history into the output, limited
// by each pixel's neighbours so it never rings.

#import rusty::types::{NO_MOTION_AT_LEAST, REACTIVE_JITTERED}

struct Params {
    // xy: the render resolution; zw: the output resolution, in pixels.
    sizes: vec4<f32>,
    // xy: this frame's jitter in render pixels, x right and y down; z: 1
    // when the history no longer holds; w: the sharpening, 0 to 1.
    frame: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_depth: texture_depth_2d;
@group(0) @binding(3) var motion: texture_2d<f32>;
@group(0) @binding(4) var history: texture_2d<f32>;
@group(0) @binding(5) var history_sampler: sampler;
// The viewmodel passes' own depth (far where they drew nothing): the world's
// depth survives them in `picture_depth`.
@group(0) @binding(6) var viewmodel_depth: texture_depth_2d;

// The most frames' worth of samples the history holds: its share of a still
// pixel is at least 1 - 1/16.
const HISTORY_FRAMES: f32 = 16.0;
// While the pixel moves, fewer: each frame's resampling of the history
// softens it a little.
const MOVING_HISTORY_FRAMES: f32 = 8.0;
// Output pixels of motion at which the history holds its moving frames.
const MOVING_PIXELS: f32 = 4.0;
// While it moves and lands halfway between the history's texels, fewer
// still.
const BETWEEN_HISTORY_FRAMES: f32 = 4.0;
// Output pixels of motion from which landing between texels counts: below
// it the pixel holds still, and its history lands on its texel.
const LANDING_PIXELS: f32 = 0.05;
// The history's bicubic (Keys): sharper than Catmull-Rom's -0.5, so the
// resampling each moving frame softens the history less; the clip box
// bounds its overshoot.
const HISTORY_CUBIC: f32 = -0.75;
// This frame's least share of a pixel with no motion of its own.
const NO_MOTION_SHARE: f32 = 0.5;
// How far the clip box reaches beyond the neighbourhood's mean, in its
// standard deviations.
const BOX_DEVIATIONS: f32 = 1.25;

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: FullscreenOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

fn ycocg(rgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * rgb.r + 0.5 * rgb.g + 0.25 * rgb.b,
        0.5 * rgb.r - 0.5 * rgb.b,
        -0.25 * rgb.r + 0.5 * rgb.g - 0.25 * rgb.b,
    );
}

fn rgb_of(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

// The Keys cubic's weight at `distance` texels (below 2) from the sample.
fn cubic(distance: vec2<f32>) -> vec2<f32> {
    let a = HISTORY_CUBIC;
    let d = distance;
    let near = ((a + 2.0) * d - (a + 3.0)) * d * d + 1.0;
    let far = ((a * d - 5.0 * a) * d + 8.0 * a) * d - 4.0 * a;
    return select(far, near, d <= vec2<f32>(1.0));
}

// The history at `uv`, bicubic through five bilinear taps (the four corner
// taps' weights are small and left out).
fn history_at(uv: vec2<f32>) -> vec4<f32> {
    let size = params.sizes.zw;
    let position = uv * size;
    let centre = floor(position - 0.5) + 0.5;
    let f = position - centre;
    let w0 = cubic(1.0 + f);
    let w1 = cubic(f);
    let w2 = cubic(1.0 - f);
    let w3 = cubic(2.0 - f);
    let w12 = w1 + w2;
    let at12 = (centre + w2 / w12) / size;
    let at0 = (centre - 1.0) / size;
    let at3 = (centre + 2.0) / size;
    let sum = textureSampleLevel(history, history_sampler, vec2<f32>(at12.x, at0.y), 0.0) * (w12.x * w0.y)
        + textureSampleLevel(history, history_sampler, vec2<f32>(at0.x, at12.y), 0.0) * (w0.x * w12.y)
        + textureSampleLevel(history, history_sampler, at12, 0.0) * (w12.x * w12.y)
        + textureSampleLevel(history, history_sampler, vec2<f32>(at3.x, at12.y), 0.0) * (w3.x * w12.y)
        + textureSampleLevel(history, history_sampler, vec2<f32>(at12.x, at3.y), 0.0) * (w12.x * w3.y);
    let total = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return max(sum / total, vec4<f32>(0.0));
}

// `history` clipped towards `mean` into the box `mean ± extent`.
fn clipped(history: vec3<f32>, mean: vec3<f32>, extent: vec3<f32>) -> vec3<f32> {
    let offset = history - mean;
    let reach = abs(offset) / max(extent, vec3<f32>(1e-4));
    let furthest = max(reach.x, max(reach.y, reach.z));
    if furthest > 1.0 {
        return mean + offset / furthest;
    }
    return history;
}

fn no_motion(value: vec2<f32>) -> bool {
    return max(abs(value.x), abs(value.y)) >= NO_MOTION_AT_LEAST;
}

// The new history at an output pixel: its colour and its frames.
fn resolve(in: FullscreenOut) -> vec4<f32> {
    let render_size = params.sizes.xy;
    let output_size = params.sizes.zw;
    let scale = output_size / render_size;
    let jitter = params.frame.xy;
    // Below render scale 1 the kernel widens with the render pixels, so
    // this frame's colour stays smooth between its sparser samples.
    let widening = max(scale.x, 1.0);
    let last = vec2<i32>(render_size) - 1;
    // The output pixel's centre in render pixels; render pixel i's sample
    // stands at i + 0.5 - jitter there.
    let centre = in.uv * render_size;
    let nearest = vec2<i32>(floor(centre + jitter));

    // This frame: the colour, its neighbourhood's moments, how near its
    // nearest sample stands, and the nearest surface's motion.
    var sum = vec3<f32>(0.0);
    var weights = 0.0;
    var first = vec3<f32>(0.0);
    var second = vec3<f32>(0.0);
    var low = vec3<f32>(1e9);
    var high = vec3<f32>(-1e9);
    var closest = 1e9;
    var depth = 2.0;
    var moved = vec2<f32>(0.0);
    var moved_length = -1.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let texel = clamp(nearest + vec2<i32>(x, y), vec2<i32>(0), last);
            let color = ycocg(textureLoad(picture, texel, 0).rgb);
            first += color;
            second += color * color;
            low = min(low, color);
            high = max(high, color);
            // The sample's distance in output pixels: this frame weighs
            // most where its samples fall.
            let offset = (vec2<f32>(texel) + 0.5 - jitter - centre) * scale;
            let distance = dot(offset, offset);
            closest = min(closest, distance);
            let weight = exp(-4.58 * distance / widening);
            sum += color * weight;
            weights += weight;
            // Nearest depth first, a viewmodel nearer than any world;
            // among equal depths (the sky), the longest motion.
            let viewmodel = textureLoad(viewmodel_depth, texel, 0);
            let at = select(textureLoad(picture_depth, texel, 0), viewmodel - 1.0, viewmodel < 1.0);
            let value = textureLoad(motion, texel, 0).xy;
            let length = select(dot(value, value), -1.0, no_motion(value));
            if at < depth - 1e-6 || (at <= depth + 1e-6 && length > moved_length) {
                depth = at;
                moved = value;
                moved_length = length;
            }
        }
    }
    let mean = first / 9.0;
    let deviation = sqrt(max(second / 9.0 - mean * mean, vec3<f32>(0.0)));
    // Where the weights vanish (no sample within reach), the nearest's
    // colour carries.
    let nearest_texel = clamp(nearest, vec2<i32>(0), last);
    var current = ycocg(textureLoad(picture, nearest_texel, 0).rgb);
    if weights > 1e-4 {
        current = sum / weights;
    }
    // How well this frame samples the pixel: 1 for a sample on its centre.
    let sampled = exp(-2.29 * closest);

    // The pixel's own motion says whether it has any, and its reactive mask
    // how much of this frame it takes; the dilated motion says where it
    // came from.
    let own = textureLoad(motion, nearest_texel, 0);
    let reactive = select(clamp(own.z, 0.0, 1.0), NO_MOTION_SHARE, no_motion(own.xy));
    // Above `REACTIVE_JITTERED` the mask says the pixel was drawn without
    // the jitter (labels, overlay lines): this frame's colour is the
    // picture where it stands, which the jittered reconstruction would
    // soften and make shimmer.
    let unjittered = clamp((reactive - REACTIVE_JITTERED) / (1.0 - REACTIVE_JITTERED), 0.0, 1.0);
    if unjittered > 0.0 {
        let standing = ycocg(textureSampleLevel(picture, history_sampler, in.uv, 0.0).rgb);
        current = mix(current, standing, unjittered);
    }
    if no_motion(moved) {
        moved = vec2<f32>(0.0);
    }
    let previous_uv = in.uv - moved / render_size;
    let moved_pixels = length(moved * scale);

    var frames = 0.0;
    var held = current;
    let inside = all(previous_uv >= vec2<f32>(0.0)) && all(previous_uv <= vec2<f32>(1.0));
    if params.frame.z < 0.5 && inside {
        let previous = history_at(previous_uv);
        let extent = deviation * BOX_DEVIATIONS;
        let box_low = max(mean - extent, low);
        let box_high = min(mean + extent, high);
        held = clipped(ycocg(previous.rgb), 0.5 * (box_low + box_high), 0.5 * (box_high - box_low));
        frames = previous.a;
    }
    let moving = mix(HISTORY_FRAMES, MOVING_HISTORY_FRAMES, clamp(moved_pixels / MOVING_PIXELS, 0.0, 1.0));
    // How far between the history's texels the pixel lands: 1 halfway.
    let landed = fract(previous_uv * output_size - 0.5);
    let between = 4.0 * max(landed.x * (1.0 - landed.x), landed.y * (1.0 - landed.y));
    let most = mix(moving, BETWEEN_HISTORY_FRAMES, between * clamp(moved_pixels / LANDING_PIXELS, 0.0, 1.0));
    var gathered = min(frames + sampled, most);
    var share = 1.0;
    if frames > 0.0 {
        share = sampled / max(gathered, 1e-4);
    }
    if reactive > 0.0 {
        share = max(share, reactive);
        gathered = min(gathered, 1.0 / reactive);
    }
    let color = rgb_of(mix(held, current, share));
    return vec4<f32>(max(color, vec3<f32>(0.0)), gathered);
}

// The history, which `fs_sharpen` brings to the output.
@fragment
fn fs_resolve(in: FullscreenOut) -> @location(0) vec4<f32> {
    return resolve(in);
}

// The history into the output, sharpened by `params.frame.w` with a lobe
// its four neighbours bound: never past their darkest or brightest.
@fragment
fn fs_sharpen(in: FullscreenOut) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.clip.xy);
    let last = vec2<i32>(params.sizes.zw) - 1;
    let c = textureLoad(history, pixel, 0).rgb;
    let n = textureLoad(history, clamp(pixel + vec2<i32>(0, -1), vec2<i32>(0), last), 0).rgb;
    let s = textureLoad(history, clamp(pixel + vec2<i32>(0, 1), vec2<i32>(0), last), 0).rgb;
    let w = textureLoad(history, clamp(pixel + vec2<i32>(-1, 0), vec2<i32>(0), last), 0).rgb;
    let e = textureLoad(history, clamp(pixel + vec2<i32>(1, 0), vec2<i32>(0), last), 0).rgb;
    let low = min(c, min(min(n, s), min(w, e)));
    let high = max(c, max(max(n, s), max(w, e)));
    // The most negative lobe that keeps every channel within [low, high]
    // reached from either side.
    let from_low = low / max(4.0 * high, vec3<f32>(1e-4));
    let from_high = (1.0 - high) / min(4.0 * low - 4.0, vec3<f32>(-1e-4));
    let limit = max(-from_low, from_high);
    let lobe = clamp(max(limit.r, max(limit.g, limit.b)), -0.1875, 0.0) * params.frame.w;
    let sharpened = (c + lobe * (n + s + w + e)) / (1.0 + 4.0 * lobe);
    return vec4<f32>(clamp(sharpened, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
