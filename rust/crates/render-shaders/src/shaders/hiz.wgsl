// A world view's occluder depth reduced to a farthest-depth pyramid
// (render-wgpu `occlusion.rs`) for the scatter selection's occlusion test.
// Level 0 is the depth over the view's region, each pixel's farthest sample
// (the depth samples as the world pass does), padded with the far plane to
// a power of two; each level above holds the farthest of the four texels
// under it.

struct Seed {
    // The view's region in the depth texture: origin and size.
    origin: vec2<u32>,
    size: vec2<u32>,
};

@group(0) @binding(0) var<uniform> seed: Seed;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var level_zero: texture_storage_2d<r32float, write>;

@group(0) @binding(5) var depth_samples: texture_depth_multisampled_2d;

@group(0) @binding(3) var below: texture_2d<f32>;
@group(0) @binding(4) var above: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn cs_seed(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= textureDimensions(level_zero)) {
        return;
    }
    var value = 1.0;
    if all(id.xy < seed.size) {
        value = textureLoad(depth, seed.origin + id.xy, 0);
    }
    textureStore(level_zero, id.xy, vec4<f32>(value, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn cs_seed_samples(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= textureDimensions(level_zero)) {
        return;
    }
    var value = 1.0;
    if all(id.xy < seed.size) {
        value = 0.0;
        for (var sample = 0u; sample < textureNumSamples(depth_samples); sample++) {
            value = max(value, textureLoad(depth_samples, seed.origin + id.xy, i32(sample)));
        }
    }
    textureStore(level_zero, id.xy, vec4<f32>(value, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn cs_reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= textureDimensions(above)) {
        return;
    }
    // A level narrower than its square keeps its last texel along that side.
    let last = textureDimensions(below) - 1u;
    let at = min(id.xy * 2u, last);
    let next = min(id.xy * 2u + 1u, last);
    let value = max(
        max(textureLoad(below, at, 0).r, textureLoad(below, vec2<u32>(next.x, at.y), 0).r),
        max(textureLoad(below, vec2<u32>(at.x, next.y), 0).r, textureLoad(below, next, 0).r),
    );
    textureStore(above, id.xy, vec4<f32>(value, 0.0, 0.0, 0.0));
}
