#define_import_path rusty::clouds

// The cloud layer's shape, shared by the sky's cloud pass (`sky.wgsl`
// `fs_clouds`) and the light it casts on the ground (`cloud_light`): the
// same noise over the same drifting plane, so a cloud overhead and its
// shadow below agree.

#import rusty::view::{frame, cloud_regions}

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

// How much cloud the sky holds over world (x, z) `xz`, and how dark its
// undersides are: the layer's coverage, raised by each cloud region (full
// within two thirds of its radius, fading over the rest), drifting with the
// region.
fn cloud_coverage(xz: vec2<f32>) -> vec2<f32> {
    var coverage = frame.clouds.x;
    var darkness = 0.0;
    let regions = u32(frame.cloud_drift.z);
    for (var index = 0u; index < regions; index = index + 1u) {
        let region = cloud_regions[index];
        let center = region.center_radius.xy + region.drift_darkness.xy * frame.time.x;
        let radius = region.center_radius.z;
        let inside = 1.0 - smoothstep(radius * 0.66, radius, distance(xz, center));
        let amount = region.center_radius.w * inside;
        coverage = max(coverage, amount);
        darkness = max(darkness, region.drift_darkness.z * inside);
    }
    return vec2<f32>(min(coverage, 1.0), darkness);
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
    if frame.clouds.y <= 0.0 || toward.y <= 0.0 {
        return 1.0;
    }
    let plane = position.xz + toward.xz * ((frame.clouds.y - position.y) / max(toward.y, 0.05));
    let coverage = cloud_coverage(plane);
    if coverage.x <= 0.0 {
        return 1.0;
    }
    var cover: f32;
    if frame.cloud_drift.w > 0.0 {
        // Volumetric clouds: the density a third of the way up the slab,
        // where the ray toward the light crosses it, so a cloud and its
        // shadow agree.
        let middle = frame.clouds.y + frame.clouds.w * 0.33;
        let crossing = position.xz + toward.xz * ((middle - position.y) / max(toward.y, 0.05));
        cover = smoothstep(0.0, 0.35, cloud_density(vec3<f32>(crossing.x, middle, crossing.y), 2));
    } else {
        let p = (plane - frame.cloud_drift.xy * frame.time.x) / frame.clouds.z;
        cover = cloud_cover(p, CLOUD_SHADOW_DETAIL, coverage.x, CLOUD_SHADOW_OCTAVES);
    }
    // A storm's dark cloud lets less through.
    return mix(1.0, CLOUD_SHADE * (1.0 - 0.6 * coverage.y), cover);
}

// Smooth value noise in 3D, in [0, 1]: the volumetric clouds' billows.
fn cloud_noise_3d(p: vec3<f32>) -> f32 {
    let cell = vec3<i32>(floor(p));
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let layer = cell.z * 7919;
    let below = mix(
        mix(cloud_hash(cell.xy + vec2<i32>(layer, 0)), cloud_hash(cell.xy + vec2<i32>(layer + 1, 0)), w.x),
        mix(cloud_hash(cell.xy + vec2<i32>(layer, 1)), cloud_hash(cell.xy + vec2<i32>(layer + 1, 1)), w.x),
        w.y,
    );
    let above_layer = layer + 7919;
    let above = mix(
        mix(cloud_hash(cell.xy + vec2<i32>(above_layer, 0)), cloud_hash(cell.xy + vec2<i32>(above_layer + 1, 0)), w.x),
        mix(cloud_hash(cell.xy + vec2<i32>(above_layer, 1)), cloud_hash(cell.xy + vec2<i32>(above_layer + 1, 1)), w.x),
        w.y,
    );
    return mix(below, above, w.z);
}

// Fractal 3D noise of the volumetric clouds, in [0, 1]: `octaves` octaves
// (at most three), each half the size and half the weight of the last.
fn cloud_fbm_3d(p: vec3<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var total = 0.0;
    var amplitude = 0.55;
    var at = p;
    for (var octave = 0; octave < octaves; octave++) {
        sum += cloud_noise_3d(at) * amplitude;
        total += amplitude;
        at = at * 2.07 + vec3<f32>(13.7, 5.3, 29.1);
        amplitude *= 0.5;
    }
    return sum / total;
}

// The volumetric clouds' density at `position` (0 to 1): 3D noise at the
// cloud size, wider than tall, kept where it rises above the sky's coverage
// there (the layer's, raised by regions), so more coverage fills more of the
// slab; shaped by height into flat bottoms that thin toward the top. Zero
// outside the slab from the layer's altitude up by its thickness.
fn cloud_density(position: vec3<f32>, octaves: i32) -> f32 {
    let height = (position.y - frame.clouds.y) / max(frame.clouds.w, 1.0);
    if height < 0.0 || height > 1.0 {
        return 0.0;
    }
    let coverage = cloud_coverage(position.xz).x;
    if coverage <= 0.0 {
        return 0.0;
    }
    let drifted = position.xz - frame.cloud_drift.xy * frame.time.x;
    let p = vec3<f32>(drifted.x, (position.y - frame.clouds.y) * 2.5, drifted.y) / frame.clouds.z;
    let shape = cloud_fbm_3d(p, octaves);
    let profile = smoothstep(0.0, 0.1, height) * (1.0 - smoothstep(0.55, 1.0, height));
    // The noise sits mostly between 0.3 and 0.7: no coverage keeps none of
    // it, full coverage nearly all.
    let threshold = mix(0.72, 0.33, coverage);
    return clamp((shape - threshold) / (1.0 - threshold) * 3.0, 0.0, 1.0) * profile;
}
