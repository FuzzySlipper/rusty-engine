#define_import_path rusty::clouds

// The cloud layer's shape, shared by the sky's cloud pass (`sky.wgsl`
// `fs_clouds`) and the light it casts on the ground (`cloud_light`): the
// same noise over the same drifting plane, so a cloud overhead and its
// shadow below agree.

#import rusty::view::frame

// A uniform value in [0, 1) for a cell of the cloud plane (pcg2d).
fn cloud_hash(cell: vec2<i32>) -> f32 {
    var v = vec2<u32>(bitcast<u32>(cell.x), bitcast<u32>(cell.y)) * 1664525u + 1013904223u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v ^= v >> vec2<u32>(16u);
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v ^= v >> vec2<u32>(16u);
    return f32(v.y >> 8u) / 16777216.0;
}

// Smooth value noise over the cloud plane, in [0, 1].
fn cloud_noise(p: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor(p));
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = cloud_hash(cell);
    let b = cloud_hash(cell + vec2<i32>(1, 0));
    let c = cloud_hash(cell + vec2<i32>(0, 1));
    let d = cloud_hash(cell + vec2<i32>(1, 1));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

// How much cloud is at `p` (in clouds) for a layer of this `coverage`:
// `octaves` octaves of noise (at most four), cut at the coverage with a soft
// edge. Toward the horizon (`detail` toward 0) a pixel spans many clouds, so
// the fine octaves fade and the edge softens rather than shimmer.
fn cloud_cover(p: vec2<f32>, detail: f32, coverage: f32, octaves: i32) -> f32 {
    var sum = 0.0;
    var total = 0.0;
    var amplitude = 0.5;
    var at = p;
    for (var octave = 0; octave < octaves; octave++) {
        let weight = amplitude * mix(1.0, detail, f32(octave) / 3.0);
        sum += cloud_noise(at) * weight;
        total += weight;
        at = at * 2.03 + vec2<f32>(17.1, 9.4);
        amplitude *= 0.5;
    }
    let shape = sum / total;
    let threshold = 1.0 - coverage;
    let soft = mix(0.45, 0.15, detail);
    return smoothstep(threshold - 0.02, threshold + soft, shape) * step(0.0001, coverage);
}

// What share of a directional light reaches `position` through the cloud
// layer (`Frame.clouds`): the layer is met where the ray toward the light
// (`toward`, unit) crosses its altitude, and a cloud there lets through only
// `CLOUD_SHADE` of the light. Under a full overcast every point is shaded;
// under a broken sky the shade drifts across the ground with the clouds. A
// light at or below the horizon is not shaded here.
const CLOUD_SHADE: f32 = 0.2;
// The shadow's noise: fewer octaves than the sky draws (a shadow's edge is
// soft anyway), at a fixed middle detail.
const CLOUD_SHADOW_OCTAVES: i32 = 2;
const CLOUD_SHADOW_DETAIL: f32 = 0.6;
fn cloud_light(position: vec3<f32>, toward: vec3<f32>) -> f32 {
    let coverage = frame.clouds.x;
    if coverage <= 0.0 || toward.y <= 0.0 {
        return 1.0;
    }
    let plane = position.xz + toward.xz * ((frame.clouds.y - position.y) / max(toward.y, 0.05));
    let p = (plane - frame.cloud_drift.xy * frame.time.x) / frame.clouds.z;
    let cover = cloud_cover(p, CLOUD_SHADOW_DETAIL, coverage, CLOUD_SHADOW_OCTAVES);
    return mix(1.0, CLOUD_SHADE, cover);
}
