enable wgpu_ray_query;
enable wgpu_binding_array;
#define_import_path rusty::ray

// The sun's shadow traced as a ray against the ray scene (render-wgpu
// `ray_scene.rs`, `ray_shadows.rs`), in place of its cascades
// (RAY_SHADOWS). One ray toward the sun, offset along the normal; masked
// materials pass a candidate loop that reads the hit triangle's uvs from
// the scene's pooled geometry and its base texture's alpha, against its
// cutoff, as the shadow pass's `fs_shadow` does. Under a temporal stage the
// ray leans within the sun's disc by a blue noise that changes each frame,
// so the stage gathers a soft penumbra; otherwise it aims at the sun's
// centre.
//
// The mask pass (`ray_mask.wgsl`) traces one ray per pixel from the view's
// opaque depth; the world pass reads it (`ray_sun_shadow`) where its
// fragment is that surface, hidden behind it or beside it; only a blended
// part, which the mask does not hold, traces its own ray (RAY_TRACE).

#import rusty::surface::voxel_uv

// A part a ray may hit, by part id (the instance's custom data). Only a
// masked part's row is read.
struct RayPart {
    // x: the first of its range's indices in `ray_indices`; y: its mesh's
    // first vertex in `ray_vertices`; z: its base texture's slot
    // (`NO_TEXTURE`: none); w: 1 when its material is masked, 2 when a voxel
    // surface's mask, which tiles by `tile` and `sample_rect`.
    slots: vec4<u32>,
    // The base uv transform's rows (`rusty::surface::transform_uv`), then
    // the alpha cutoff (u.w) and the part's colour alpha (v.w), which
    // scales the vertices' and the texture's.
    uv_u: vec4<f32>,
    uv_v: vec4<f32>,
    tile: vec4<f32>,
    sample_rect: vec4<f32>,
};

struct RayParams {
    // x: the tangent of the cone the rays lean within (0: aimed at the
    // sun's centre); y: the frame's noise index; z: the normal offset at
    // the camera, metres; w: how much it grows a metre away.
    shape: vec4<f32>,
};

@group(4) @binding(0) var ray_scene: acceleration_structure;
@group(4) @binding(1) var<storage, read> ray_parts: array<RayPart>;
@group(4) @binding(2) var<storage, read> ray_vertices: array<f32>;
@group(4) @binding(3) var<storage, read> ray_indices: array<u32>;
@group(4) @binding(4) var ray_textures: binding_array<texture_2d<f32>, 256>;
// Each texture's own sampler, as its material binds it.
@group(4) @binding(5) var ray_samplers: binding_array<sampler, 256>;
@group(4) @binding(6) var<storage, read> ray_params: RayParams;
// Per pixel, the distance from the camera to the opaque surface the mask
// pass traced from, negative where the sun is hidden from it.
@group(4) @binding(7) var ray_mask: texture_2d<f32>;
// A 64×64 tile of two-channel blue noise, 8 bits a channel.
@group(4) @binding(8) var<storage, read> ray_blue_noise: array<u32>;

// The fragment's pixel, which the world pass's fragment stage sets.
var<private> ray_pixel: vec2<u32>;

// The instance mask bit of what casts shadows (`ray_scene::MASK_SHADOW`).
const RAY_MASK_SHADOW: u32 = 1u;
const NO_TEXTURE: u32 = 0xffffffffu;
// Floats in a pooled vertex, and where its uv and its colour's alpha are
// (`pipelines.rs`).
const VERTEX_FLOATS: u32 = 12u;
const UV_FLOAT: u32 = 6u;
const ALPHA_FLOAT: u32 = 11u;
const RAY_FAR: f32 = 10000.0;
const BLUE_NOISE_SIZE: u32 = 64u;

// Whether a candidate hit on a masked triangle is opaque where it was hit.
fn ray_hit_opaque(candidate: RayIntersection) -> bool {
    let part = ray_parts[candidate.instance_custom_data];
    if part.slots.w == 0u {
        return true;
    }
    let first = part.slots.x + candidate.primitive_index * 3u;
    let b = candidate.barycentrics;
    let weights = vec3<f32>(1.0 - b.x - b.y, b.x, b.y);
    var uv = vec2<f32>(0.0);
    var alpha = 0.0;
    for (var corner = 0u; corner < 3u; corner++) {
        let at = (part.slots.y + ray_indices[first + corner]) * VERTEX_FLOATS;
        uv += weights[corner] * vec2<f32>(ray_vertices[at + UV_FLOAT], ray_vertices[at + UV_FLOAT + 1u]);
        alpha += weights[corner] * ray_vertices[at + ALPHA_FLOAT];
    }
    alpha *= part.uv_v.w;
    if part.slots.z != NO_TEXTURE {
        let tiled = select(uv, voxel_uv(uv, part.tile, part.sample_rect), part.slots.w == 2u);
        let point = vec3<f32>(tiled, 1.0);
        let at = vec2<f32>(dot(part.uv_u.xyz, point), dot(part.uv_v.xyz, point));
        alpha *= textureSampleLevel(ray_textures[part.slots.z], ray_samplers[part.slots.z], at, 0.0).a;
    }
    return alpha >= part.uv_u.w;
}

// Two numbers in [0, 1) at `pixel`: its blue noise, turned each frame by a
// low-discrepancy step (the R2 sequence, 16 bits) so a temporal stage
// gathers every value while each frame stays blue.
fn ray_noise(pixel: vec2<u32>) -> vec2<f32> {
    let at = pixel % vec2<u32>(BLUE_NOISE_SIZE);
    let texel = ray_blue_noise[at.y * BLUE_NOISE_SIZE + at.x];
    let frame = u32(ray_params.shape.y);
    let turn = vec2<u32>((frame * 49471u) & 0xffffu, (frame * 37345u) & 0xffffu);
    return fract(vec2<f32>(f32(texel & 0xffu), f32((texel >> 8u) & 0xffu)) / 256.0 + vec2<f32>(turn) / 65536.0);
}

// How much of the sun reaches `position` (normal `normal`, `away` metres
// from the camera, at `pixel`) along `toward`.
fn ray_sun_visibility(pixel: vec2<u32>, position: vec3<f32>, normal: vec3<f32>, toward: vec3<f32>, away: f32) -> f32 {
    var direction = toward;
    let spread = ray_params.shape.x;
    if spread > 0.0 {
        // A point of the sun's disc, uniform over it.
        let side = normalize(select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(toward.x) > 0.9));
        let across = normalize(cross(toward, side));
        let up = cross(across, toward);
        let noise = ray_noise(pixel);
        let radius = spread * sqrt(noise.x);
        let angle = 6.2831853 * noise.y;
        direction = normalize(toward + (across * cos(angle) + up * sin(angle)) * radius);
    }
    let origin = position + normal * (ray_params.shape.z + ray_params.shape.w * away);
    var query: ray_query;
    rayQueryInitialize(
        &query,
        ray_scene,
        RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT, RAY_MASK_SHADOW, 0.0, RAY_FAR, origin, direction),
    );
    while rayQueryProceed(&query) {
        if ray_hit_opaque(rayQueryGetCandidateIntersection(&query)) {
            rayQueryConfirmIntersection(&query);
        }
    }
    return select(1.0, 0.0, rayQueryGetCommittedIntersection(&query).kind != RAY_QUERY_INTERSECTION_NONE);
}

// How far a fragment may be from the mask's surface and still be it:
// at the camera, and growing with distance (`ray_mask.wgsl` reads its
// depth at the pixel's centre, a fragment may lie elsewhere in the pixel).
const MASK_MATCH: f32 = 0.05;
const MASK_MATCH_PER_METRE: f32 = 0.02;

// How much of the sun reaches the world pass's fragment at `position`. A
// blended part is not in the mask, and shallow water lies within a match of
// the bed under it, so it always traces its own ray (RAY_TRACE). An opaque
// fragment takes the mask's ray where it is the mask's surface or behind it
// (drawn over later, or at an edge the mask's surface covers); in front of
// it, a neighbouring pixel's where one holds its surface (the mask reads a
// pixel's centre, a cut leaf's edge covers samples beside it), or else its
// own pixel's.
fn ray_sun_shadow(position: vec3<f32>, normal: vec3<f32>, toward: vec3<f32>, away: f32) -> f32 {
#ifdef RAY_TRACE
    return ray_sun_visibility(ray_pixel, position, normal, toward, away);
#else
    let tolerance = MASK_MATCH + MASK_MATCH_PER_METRE * away;
    let stored = textureLoad(ray_mask, ray_pixel, 0).x;
    if away > abs(stored) - tolerance {
        return select(1.0, 0.0, stored < 0.0);
    }
    for (var at = 0u; at < 9u; at++) {
        let beside = vec2<i32>(ray_pixel) + vec2<i32>(i32(at % 3u) - 1, i32(at / 3u) - 1);
        let near = textureLoad(ray_mask, beside, 0).x;
        if abs(abs(near) - away) < tolerance {
            return select(1.0, 0.0, near < 0.0);
        }
    }
    return select(1.0, 0.0, stored < 0.0);
#endif
}
