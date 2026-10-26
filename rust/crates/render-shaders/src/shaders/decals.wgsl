#define_import_path rusty::decals

// Projected decals (render-wgpu `decals.rs`, `CameraView.SetDecal`): marks
// laid over the opaque world inside a box each, projected down the box's -y.
// A world-space grid of `DECAL_CELL_METRES` cells over the ground (x, z),
// hashed into `DECAL_BUCKETS` buckets, lists the decals whose box reaches
// each cell, so a fragment tests only its bucket's.
//
// One buffer of words: a header (the decal count, the rows' first word, the
// lists' first word, flags: 1 a normal atlas, 2 an emission atlas), the buckets' first list
// entries (`DECAL_BUCKETS` + 1 words: bucket b's entries are lists[first[b]]
// up to lists[first[b + 1]]), the lists of decal indices, then each decal's
// row of `DECAL_ROW_WORDS` words:
// - 0 to 2: the world to the box's own space, a row per axis (xyz, w), the
//   box spanning -0.5 to 0.5 along each;
// - 3: its region of the atlas (u0, v0, u1, v1);
// - 4: its linear tint (rgb) and opacity (a);
// - 5: x the roughness it lays (0: the surface's), y how strongly the normal
//   atlas bends the surface, z flags (1 the angle falloff, 2 over
//   vegetation, 4 over animated meshes), w the presentation seconds its fade
//   starts;
// - 6: xyz the box's +y in the world (unit), w the seconds its fade ends (0:
//   it stays);
// - 7: xyz the linear light it emits (0: none), w unused.

#import rusty::types::Surface
#import rusty::view::frame

@group(3) @binding(0) var<storage, read> decal_words: array<u32>;
@group(3) @binding(1) var decal_atlas: texture_2d<f32>;
@group(3) @binding(2) var decal_sampler: sampler;
@group(3) @binding(3) var decal_normals: texture_2d<f32>;
@group(3) @binding(4) var decal_emission: texture_2d<f32>;

// `decals.rs` BUCKETS, CELL_METRES and ROW_WORDS.
const DECAL_BUCKETS: u32 = 16384u;
const DECAL_CELL_METRES: f32 = 2.0;
const DECAL_ROW_WORDS: u32 = 32u;
const DECAL_HEADER_WORDS: u32 = 4u;
const DECAL_ANGLE_FALLOFF: u32 = 1u;
const DECAL_ON_VEGETATION: u32 = 2u;
const DECAL_ON_ANIMATED: u32 = 4u;
// Header flags.
const DECAL_NORMAL_ATLAS: u32 = 1u;
const DECAL_EMISSION_ATLAS: u32 = 2u;
// The angle falloff: whole on faces within about 60 degrees of the box's
// +y, gone from about 80.
const DECAL_FACING_WHOLE: f32 = 0.5;
const DECAL_FACING_NONE: f32 = 0.17;

// The bucket of the grid cell holding `position` (`decals.rs` `bucket`).
fn decal_bucket(position: vec3<f32>) -> u32 {
    let cell = vec2<i32>(floor(position.xz / DECAL_CELL_METRES));
    let hash = (bitcast<u32>(cell.x) * 73856093u) ^ (bitcast<u32>(cell.y) * 19349663u);
    return hash & (DECAL_BUCKETS - 1u);
}

fn decal_vec4(word: u32) -> vec4<f32> {
    return vec4<f32>(
        bitcast<f32>(decal_words[word]),
        bitcast<f32>(decal_words[word + 1u]),
        bitcast<f32>(decal_words[word + 2u]),
        bitcast<f32>(decal_words[word + 3u]),
    );
}

// The decals over `surface`, in the order they were placed: each blends its
// atlas texel times its tint over the base colour by the texel's alpha times
// its opacity, fade and angle falloff, and may lay its roughness and bend the
// normal. A decal that emits adds its emission times the emission atlas's
// texel (the colour atlas's without one) by the same cover, to the light the
// surface emits after it is lit. `vegetation` and `animated` say what the surface belongs to: a
// decal lays over those only when it asks to. The atlas is sampled with the
// gradients of the box's projection, so its mip follows the screen.
fn decaled(surface: Surface, vegetation: bool, animated: bool) -> Surface {
    let position = surface.world_position;
    let dx = dpdx(position);
    let dy = dpdy(position);
    let count = decal_words[0];
    if count == 0u {
        return surface;
    }
    let rows = decal_words[1];
    let lists = decal_words[2];
    let normals = (decal_words[3] & DECAL_NORMAL_ATLAS) != 0u;
    let emission_atlas = (decal_words[3] & DECAL_EMISSION_ATLAS) != 0u;
    let bucket = decal_bucket(position);
    let first = decal_words[DECAL_HEADER_WORDS + bucket];
    let last = decal_words[DECAL_HEADER_WORDS + bucket + 1u];
    var result = surface;
    for (var entry = first; entry < last; entry = entry + 1u) {
        let row = rows + decal_words[lists + entry] * DECAL_ROW_WORDS;
        let detail = decal_vec4(row + 20u);
        let flags = u32(detail.z);
        if (vegetation && (flags & DECAL_ON_VEGETATION) == 0u) || (animated && (flags & DECAL_ON_ANIMATED) == 0u) {
            continue;
        }
        let x = decal_vec4(row);
        let y = decal_vec4(row + 4u);
        let z = decal_vec4(row + 8u);
        let local = vec3<f32>(dot(x.xyz, position) + x.w, dot(y.xyz, position) + y.w, dot(z.xyz, position) + z.w);
        if any(abs(local) > vec3<f32>(0.5)) {
            continue;
        }
        let tint = decal_vec4(row + 16u);
        let axis = decal_vec4(row + 24u);
        var opacity = tint.a;
        if axis.w > 0.0 {
            opacity *= 1.0 - smoothstep(detail.w, axis.w, frame.time.x);
        }
        if (flags & DECAL_ANGLE_FALLOFF) != 0u {
            opacity *= smoothstep(DECAL_FACING_NONE, DECAL_FACING_WHOLE, dot(result.normal, axis.xyz));
        }
        if opacity <= 0.0 {
            continue;
        }
        let rect = decal_vec4(row + 12u);
        let span = rect.zw - rect.xy;
        let uv = rect.xy + (local.xz + 0.5) * span;
        let uv_dx = vec2<f32>(dot(x.xyz, dx), dot(z.xyz, dx)) * span;
        let uv_dy = vec2<f32>(dot(x.xyz, dy), dot(z.xyz, dy)) * span;
        let texel = textureSampleGrad(decal_atlas, decal_sampler, uv, uv_dx, uv_dy);
        let cover = texel.a * opacity;
        result.base = vec4<f32>(mix(result.base.rgb, texel.rgb * tint.rgb, cover), result.base.a);
        if detail.x > 0.0 {
            result.roughness = mix(result.roughness, detail.x, cover);
        }
        if normals && detail.y > 0.0 {
            // The normal atlas's x along the box's +x, its y up the region
            // (toward the box's -z).
            let bend = textureSampleGrad(decal_normals, decal_sampler, uv, uv_dx, uv_dy).xy * 2.0 - 1.0;
            let along = normalize(x.xyz) * bend.x - normalize(z.xyz) * bend.y;
            result.normal = normalize(result.normal + along * detail.y * cover);
        }
        let emission = decal_vec4(row + 28u).xyz;
        if any(emission > vec3<f32>(0.0)) {
            var glow = texel.rgb;
            if emission_atlas {
                glow = textureSampleGrad(decal_emission, decal_sampler, uv, uv_dx, uv_dy).rgb;
            }
            result.emission += glow * emission * cover;
        }
    }
    return result;
}
