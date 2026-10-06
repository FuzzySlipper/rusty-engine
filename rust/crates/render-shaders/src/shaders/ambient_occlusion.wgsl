// Screen-space ambient occlusion over a view's half-resolution depth
// (`render-wgpu/src/ambient_occlusion.rs`): a normal-oriented disc of depth
// samples (Alchemy/SAO style) with normals reconstructed from depth, then a
// depth-aware blur. The occlusion scales the ambient and hemisphere light of
// the world pass through `Surface.occlusion`.
//
// Two paths compute the same occlusion from the same inputs so they can be
// timed against each other: `cs_occlusion` loads a workgroup's depth tile
// into shared memory and samples from it; `fs_occlusion` samples the depth
// texture directly from a full-screen triangle. `fs_blur` serves both, once
// along each axis (`BLUR_AXIS`).

#import rusty::types::PI

struct AoParams {
    // View space from NDC (depth 0..1), and back.
    inv_projection: mat4x4<f32>,
    projection: mat4x4<f32>,
    // The view's region of the AO texture: xy origin, zw size (texels).
    region: vec4<u32>,
    // x: sample radius (world units); y: bias (cosine); z: intensity;
    // w: the largest sample radius in texels (`cs_occlusion`'s tile allows
    // `TILE_APRON`).
    tuning: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: AoParams;
@group(0) @binding(1) var depth_map: texture_depth_2d;
// `cs_occlusion` writes here; `fs_blur` reads it.
@group(0) @binding(2) var occlusion_out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var occlusion_in: texture_2d<f32>;

const SAMPLES: u32 = 12u;
const SPIRAL_TURNS: f32 = 7.0;
const WORKGROUP: u32 = 16u;
// Texels of depth each side of the workgroup's footprint in shared memory.
const TILE_APRON: u32 = 8u;
const TILE: u32 = WORKGROUP + 2u * TILE_APRON;
const BLUR_RADIUS: i32 = 2;
// The axis one blur pass smooths along: 0 for x, 1 for y.
override BLUR_AXIS: u32 = 0u;

// The last texel of the region on each axis.
fn region_max() -> vec2<i32> {
    return vec2<i32>(params.region.xy + params.region.zw) - vec2<i32>(1);
}

fn clamp_texel(texel: vec2<i32>) -> vec2<i32> {
    return clamp(texel, vec2<i32>(params.region.xy), region_max());
}

fn load_depth(texel: vec2<i32>) -> f32 {
    return textureLoad(depth_map, clamp_texel(texel), 0);
}

// View-space position of a texel centre of the region at `depth`.
fn view_position(texel: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = (texel + 0.5 - vec2<f32>(params.region.xy)) / vec2<f32>(params.region.zw);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let view = params.inv_projection * vec4<f32>(ndc, depth, 1.0);
    return view.xyz / view.w;
}

// View-space depth (negative z) at `depth`, without the x and y terms a
// symmetric projection's inverse leaves at zero.
fn view_z(depth: f32) -> f32 {
    let inverse = params.inv_projection;
    return (inverse[2][2] * depth + inverse[3][2]) / (inverse[2][3] * depth + inverse[3][3]);
}

// The disc radius in texels for a surface at view depth `-z`: the world
// radius projected (fixed for an orthographic projection), bounded by the
// tile.
fn disc_radius(z: f32) -> f32 {
    let half_height = 0.5 * f32(params.region.w);
    var texels = params.tuning.x * params.projection[1][1] * half_height;
    if params.projection[3][3] == 0.0 {
        texels = texels / max(-z, 1e-3);
    }
    return clamp(texels, 1.0, params.tuning.w);
}

// Per-texel rotation of the sample spiral (interleaved gradient noise).
fn spiral_phase(texel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(texel, vec2<f32>(0.06711056, 0.00583715)))) * 2.0 * PI;
}

// The view-space normal at `centre` from its depth neighbours, taking the
// shallower difference on each axis so a depth edge does not tilt it.
fn reconstruct_normal(
    centre: vec3<f32>,
    left: vec3<f32>,
    right: vec3<f32>,
    up: vec3<f32>,
    down: vec3<f32>,
) -> vec3<f32> {
    let to_right = right - centre;
    let to_left = centre - left;
    let dx = select(to_left, to_right, abs(to_right.z) < abs(to_left.z));
    let to_down = down - centre;
    let to_up = centre - up;
    let dy = select(to_up, to_down, abs(to_down.z) < abs(to_up.z));
    var normal = normalize(cross(dy, dx));
    if normal.z < 0.0 {
        normal = -normal;
    }
    return normal;
}

// Occlusion of `centre` by the sample at `sample`: the cosine of the sample
// above the tangent plane past the bias, rescaled so a sample straight above
// still counts fully, falling off with distance.
fn sample_occlusion(centre: vec3<f32>, normal: vec3<f32>, sample: vec3<f32>) -> f32 {
    let v = sample - centre;
    let vv = dot(v, v);
    let radius = params.tuning.x;
    let falloff = max(1.0 - vv / (radius * radius), 0.0);
    let cosine = dot(v, normal) / (sqrt(vv) + 1e-4);
    let bias = params.tuning.y;
    return falloff * max(cosine - bias, 0.0) / (1.0 - bias);
}

fn finish_occlusion(sum: f32) -> f32 {
    return clamp(1.0 - params.tuning.z * sum / f32(SAMPLES), 0.0, 1.0);
}

// The texel offset of sample `index` on a spiral of `radius` texels.
fn spiral_offset(index: u32, phase: f32, radius: f32) -> vec2<f32> {
    let alpha = (f32(index) + 0.5) / f32(SAMPLES);
    let angle = alpha * SPIRAL_TURNS * 2.0 * PI + phase;
    return vec2<f32>(cos(angle), sin(angle)) * alpha * radius;
}

// ---- Compute path: a shared depth tile per workgroup ----

var<workgroup> tile: array<f32, 1024>;

fn tile_index(local: vec2<i32>) -> u32 {
    return u32(local.y) * TILE + u32(local.x);
}

// Depth at a texel from the tile; `origin` is the tile's first texel.
fn tile_depth(texel: vec2<i32>, origin: vec2<i32>) -> f32 {
    let local = clamp(texel - origin, vec2<i32>(0), vec2<i32>(i32(TILE) - 1));
    return tile[tile_index(local)];
}

@compute @workgroup_size(16, 16)
fn cs_occlusion(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let origin = vec2<i32>(params.region.xy + group.xy * WORKGROUP) - vec2<i32>(i32(TILE_APRON));
    // 32×32 depths, four per invocation of the 16×16 workgroup.
    let first = local.y * WORKGROUP + local.x;
    for (var slot = first; slot < TILE * TILE; slot += WORKGROUP * WORKGROUP) {
        let local_texel = vec2<i32>(i32(slot % TILE), i32(slot / TILE));
        tile[slot] = load_depth(origin + local_texel);
    }
    workgroupBarrier();

    let texel = vec2<i32>(params.region.xy + global.xy);
    if any(global.xy >= params.region.zw) {
        return;
    }
    let depth = tile_depth(texel, origin);
    if depth >= 1.0 {
        textureStore(occlusion_out, texel, vec4<f32>(1.0));
        return;
    }
    let position = vec2<f32>(texel);
    let centre = view_position(position, depth);
    let normal = reconstruct_normal(
        centre,
        view_position(position - vec2<f32>(1.0, 0.0), tile_depth(texel - vec2<i32>(1, 0), origin)),
        view_position(position + vec2<f32>(1.0, 0.0), tile_depth(texel + vec2<i32>(1, 0), origin)),
        view_position(position - vec2<f32>(0.0, 1.0), tile_depth(texel - vec2<i32>(0, 1), origin)),
        view_position(position + vec2<f32>(0.0, 1.0), tile_depth(texel + vec2<i32>(0, 1), origin)),
    );
    let radius = disc_radius(centre.z);
    let phase = spiral_phase(position);
    var sum = 0.0;
    for (var index = 0u; index < SAMPLES; index++) {
        let at = position + spiral_offset(index, phase, radius);
        let sample_texel = clamp_texel(vec2<i32>(round(at)));
        let sample_depth = tile_depth(sample_texel, origin);
        sum += sample_occlusion(centre, normal, view_position(vec2<f32>(sample_texel), sample_depth));
    }
    textureStore(occlusion_out, texel, vec4<f32>(finish_occlusion(sum)));
}

// ---- Raster path: the same from a full-screen triangle ----

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4<f32>(xy, 0.0, 1.0);
}

@fragment
fn fs_occlusion(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(clip.xy);
    let depth = load_depth(texel);
    if depth >= 1.0 {
        return vec4<f32>(1.0);
    }
    let position = vec2<f32>(texel);
    let centre = view_position(position, depth);
    let normal = reconstruct_normal(
        centre,
        view_position(position - vec2<f32>(1.0, 0.0), load_depth(texel - vec2<i32>(1, 0))),
        view_position(position + vec2<f32>(1.0, 0.0), load_depth(texel + vec2<i32>(1, 0))),
        view_position(position - vec2<f32>(0.0, 1.0), load_depth(texel - vec2<i32>(0, 1))),
        view_position(position + vec2<f32>(0.0, 1.0), load_depth(texel + vec2<i32>(0, 1))),
    );
    let radius = disc_radius(centre.z);
    let phase = spiral_phase(position);
    var sum = 0.0;
    for (var index = 0u; index < SAMPLES; index++) {
        let at = position + spiral_offset(index, phase, radius);
        let sample_texel = clamp_texel(vec2<i32>(round(at)));
        sum += sample_occlusion(centre, normal, view_position(vec2<f32>(sample_texel), load_depth(sample_texel)));
    }
    return vec4<f32>(finish_occlusion(sum));
}

// ---- Blur: 5 taps along one axis, weighted by view depth difference ----

@fragment
fn fs_blur(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(clip.xy);
    let depth = load_depth(texel);
    if depth >= 1.0 {
        return vec4<f32>(1.0);
    }
    let z = view_z(depth);
    // Depth differences within this fraction of the depth count as one
    // surface.
    let tolerance = max(abs(z) * 0.05, 0.02);
    let step = select(vec2<i32>(1, 0), vec2<i32>(0, 1), BLUR_AXIS == 1u);
    var sum = 0.0;
    var weights = 0.0;
    for (var tap = -BLUR_RADIUS; tap <= BLUR_RADIUS; tap++) {
        let at = clamp_texel(texel + step * tap);
        let sample_depth = load_depth(at);
        let spatial = exp(-f32(tap * tap) / 4.0);
        let weight = spatial
            * select(0.0, exp(-abs(view_z(sample_depth) - z) / tolerance), sample_depth < 1.0);
        sum += textureLoad(occlusion_in, at, 0).r * weight;
        weights += weight;
    }
    return vec4<f32>(select(1.0, sum / weights, weights > 0.0));
}
