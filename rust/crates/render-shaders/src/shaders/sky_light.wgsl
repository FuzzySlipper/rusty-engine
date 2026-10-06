// The sky's light (render-wgpu `sky_light.rs`): from the background (a sky
// panorama, two blended, or the clear colour) a cube whose mips hold its
// radiance prefiltered for rising roughness (GGX importance sampling with
// filtered lookups, N = V = R), and nine spherical-harmonics coefficients of
// its irradiance, already convolved with the cosine lobe.

#import rusty::types::PI

struct SkyLightParams {
    // x: 1 for panoramas, 0 for the clear colour; y: the blend amount toward
    // the second panorama; z: this level's roughness; w: samples per texel.
    source: vec4<f32>,
    // rgb: the clear colour (linear).
    color: vec4<f32>,
    // x: this level's size in texels.
    level: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: SkyLightParams;
@group(0) @binding(1) var first: texture_2d<f32>;
@group(0) @binding(2) var second: texture_2d<f32>;
@group(0) @binding(3) var panorama_sampler: sampler;
// One mip level of the cube, its six faces as layers.
@group(0) @binding(4) var level: texture_storage_2d_array<rgba16float, write>;
// The irradiance coefficients (rgb each).
@group(0) @binding(5) var<storage, read_write> irradiance: array<vec4<f32>, 9>;

// Where a direction falls on the panorama: the mapping `sky.wgsl` draws with.
fn panorama_uv(direction: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(
        atan2(direction.z, direction.x) / (2.0 * PI) + 0.5,
        0.5 - asin(clamp(direction.y, -1.0, 1.0)) / PI,
    );
}

// The background's linear radiance along `direction`, from panorama mip `lod`.
fn radiance(direction: vec3<f32>, lod: f32) -> vec3<f32> {
    if params.source.x < 0.5 {
        return params.color.rgb;
    }
    let uv = panorama_uv(direction);
    let near = textureSampleLevel(first, panorama_sampler, uv, lod).rgb;
    let far = textureSampleLevel(second, panorama_sampler, uv, lod).rgb;
    return mix(near, far, params.source.y);
}

// The direction through texel `uv` of cube face `face` (+X, -X, +Y, -Y, +Z,
// -Z), as the cube is sampled.
fn cube_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let s = uv.x * 2.0 - 1.0;
    let t = uv.y * 2.0 - 1.0;
    switch face {
        case 0u: { return vec3<f32>(1.0, -t, -s); }
        case 1u: { return vec3<f32>(-1.0, -t, s); }
        case 2u: { return vec3<f32>(s, 1.0, t); }
        case 3u: { return vec3<f32>(s, -1.0, -t); }
        case 4u: { return vec3<f32>(s, -t, 1.0); }
        default: { return vec3<f32>(-s, -t, -1.0); }
    }
}

fn hammersley(index: u32, count: u32) -> vec2<f32> {
    return vec2<f32>(f32(index) / f32(count), f32(reverseBits(index)) * 2.3283064365386963e-10);
}

// A half vector about `normal` drawn from the GGX distribution of `alpha`.
fn ggx_half(xi: vec2<f32>, normal: vec3<f32>, alpha: f32) -> vec3<f32> {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (alpha * alpha - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    var up = vec3<f32>(1.0, 0.0, 0.0);
    if abs(normal.z) < 0.999 {
        up = vec3<f32>(0.0, 0.0, 1.0);
    }
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);
    return normalize(tangent * cos(phi) * sin_theta + bitangent * sin(phi) * sin_theta + normal * cos_theta);
}

fn ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

@compute @workgroup_size(8, 8, 1)
fn cs_prefilter(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = u32(params.level.x);
    if id.x >= size || id.y >= size {
        return;
    }
    let normal = normalize(cube_direction(id.z, (vec2<f32>(id.xy) + 0.5) / f32(size)));
    let roughness = params.source.z;
    var color = vec3<f32>(0.0);
    if roughness <= 0.0 {
        color = radiance(normal, 0.0);
    } else {
        let alpha = roughness * roughness;
        let samples = u32(params.source.w);
        // Each sample reads a panorama mip whose texels cover about its
        // share of the lobe (filtered importance sampling).
        let dimensions = vec2<f32>(textureDimensions(first));
        let texel_angle = 4.0 * PI / (dimensions.x * dimensions.y);
        var weight = 0.0;
        for (var index = 0u; index < samples; index = index + 1u) {
            let halfway = ggx_half(hammersley(index, samples), normal, alpha);
            let light = normalize(2.0 * dot(normal, halfway) * halfway - normal);
            let n_dot_l = dot(normal, light);
            if n_dot_l > 0.0 {
                let pdf = ggx(max(dot(normal, halfway), 0.0), alpha) * 0.25 + 1e-4;
                let sample_angle = 1.0 / (f32(samples) * pdf);
                let lod = max(0.5 * log2(sample_angle / texel_angle) + 1.0, 0.0);
                color += radiance(light, lod) * n_dot_l;
                weight += n_dot_l;
            }
        }
        color = color / max(weight, 1e-4);
    }
    textureStore(level, vec2<i32>(id.xy), i32(id.z), vec4<f32>(color, 1.0));
}

// The real spherical harmonics up to band 2 at `d`.
fn harmonics(d: vec3<f32>) -> array<f32, 9> {
    return array<f32, 9>(
        0.282095,
        0.488603 * d.y,
        0.488603 * d.z,
        0.488603 * d.x,
        1.092548 * d.x * d.y,
        1.092548 * d.y * d.z,
        0.315392 * (3.0 * d.z * d.z - 1.0),
        1.092548 * d.x * d.z,
        0.546274 * (d.x * d.x - d.y * d.y),
    );
}

const SH_COLUMNS: u32 = 64u;
const SH_ROWS: u32 = 32u;
const SH_THREADS: u32 = 256u;
var<workgroup> partial: array<vec3<f32>, 256>;

// Project the background onto the harmonics over a 64×32 grid of
// directions weighted by solid angle, then convolve with the cosine lobe
// (band 0 by π, band 1 by 2π/3, band 2 by π/4), so irradiance along a
// normal is the coefficients' sum with the harmonics there.
@compute @workgroup_size(256, 1, 1)
fn cs_irradiance(@builtin(local_invocation_index) thread: u32) {
    var sums: array<vec3<f32>, 9>;
    let lod = max(log2(f32(textureDimensions(first).x) / f32(SH_COLUMNS)), 0.0);
    let per_thread = SH_COLUMNS * SH_ROWS / SH_THREADS;
    for (var step = 0u; step < per_thread; step = step + 1u) {
        let cell = thread * per_thread + step;
        let uv = (vec2<f32>(f32(cell % SH_COLUMNS), f32(cell / SH_COLUMNS)) + 0.5)
            / vec2<f32>(f32(SH_COLUMNS), f32(SH_ROWS));
        let phi = (uv.x - 0.5) * 2.0 * PI;
        let theta = (0.5 - uv.y) * PI;
        let d = vec3<f32>(cos(theta) * cos(phi), sin(theta), cos(theta) * sin(phi));
        let solid_angle = (2.0 * PI / f32(SH_COLUMNS)) * (PI / f32(SH_ROWS)) * cos(theta);
        let light = radiance(d, lod) * solid_angle;
        var y = harmonics(d);
        for (var k = 0u; k < 9u; k = k + 1u) {
            sums[k] += light * y[k];
        }
    }
    var bands = array<f32, 9>(
        PI,
        2.0 * PI / 3.0,
        2.0 * PI / 3.0,
        2.0 * PI / 3.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
    );
    for (var k = 0u; k < 9u; k = k + 1u) {
        partial[thread] = sums[k];
        workgroupBarrier();
        for (var stride = SH_THREADS / 2u; stride > 0u; stride = stride / 2u) {
            if thread < stride {
                partial[thread] += partial[thread + stride];
            }
            workgroupBarrier();
        }
        if thread == 0u {
            irradiance[k] = vec4<f32>(partial[0] * bands[k], 0.0);
        }
        workgroupBarrier();
    }
}
