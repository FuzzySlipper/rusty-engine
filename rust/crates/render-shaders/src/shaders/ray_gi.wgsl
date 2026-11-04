// Adapted from Bevy's bevy_solari (https://github.com/bevyengine/bevy, MIT OR
// Apache-2.0), commit 84e2d2cc5: `presample_light_tiles`, `world_cache_query`,
// `world_cache_compact`, `world_cache_update`, `initial_path`, `no_restir` and
// `resolve_dlss_rr_textures` in `realtime/`, `sampling` and `brdf` in `scene/`.
//
// Ray-traced global illumination (render-wgpu `ray_gi.rs`): the light a world
// view's opaque surfaces receive from everything but what the forward pass
// lit them with directly. Each frame the light tiles presample the light
// rows and emissive parts, the world cache (a hash grid of irradiance over
// the surfaces the paths reach) decays, compacts and updates, and each pixel
// of a checkerboard traces one path from the view's depth and guides, ending
// in the cache where a wide lobe allows it. A pixel's first lobe is chosen from its guides
// and its light kept apart: `ray_gi_composite.wgsl` scales the diffuse by the
// diffuse albedo and the specular by the specular albedo.
//
// A path also writes what Ray Reconstruction reads: on a smooth surface the
// motion of what it reflects, and on a mirror the surface the mirror chain
// ends on, in place of the mirror (primary surface replacement). The
// composite keeps the world's own guides, and `resolve_guides` merges depth
// and motion once the world's blended parts have drawn.
//
// The ambient and hemisphere rows are what a ray meets when it leaves the
// scene, and a leaf does not stop them: a ray that hits a masked part still
// meets them if nothing solid stands beyond it (`rows_past_leaves`). They stand
// for light a canopy scatters down, which its few layers of dark leaves
// would not carry as a path; a cave keeps them out, plants and all.

#import rusty::lighting::{brdf_ggx, distance_attenuation, environment_brdf}
#import rusty::motion::{previous_pixel_from, previous_pixel_toward}
#import rusty::ray::{ray_clear, ray_closest, ray_indices, ray_params, ray_past_leaves, ray_parts, ray_samplers, ray_textures, ray_vertices, ray_voxel_uv, RayHit, NO_TEXTURE, PART_MASKED, VERTEX_FLOATS}
#import rusty::view::{frame, lights, sky_specular, sky_sampler}

const PI: f32 = 3.14159265;
const PI_2: f32 = 6.28318531;

struct GiParams {
    // The world from the view's clip space, and back.
    inverse_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // xyz: the camera; w unused.
    camera: vec4<f32>,
    // The viewport's origin and size, pixels.
    viewport: vec4<u32>,
    // x: the frame's random seed; y: the sources; z: 1 to start the cache's
    // history again; w: how many sources are emissive parts.
    state: vec4<u32>,
    // The ambient rows' radiance, and the hemisphere rows' above and below,
    // which a ray that leaves the scene meets.
    ambient: vec4<f32>,
    hemisphere_sky: vec4<f32>,
    hemisphere_ground: vec4<f32>,
};

// A light a tile may sample.
struct GiSource {
    // x: the light row's index, or an emissive part's id; y: the part's
    // triangles, 0 for a light row.
    kind: vec4<u32>,
    // An emissive part's world transform, by rows.
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    row_z: vec4<f32>,
};

// A point on (or direction to) a light, chosen apart from any shading point.
struct GiLight {
    // xyz: the point, or the direction toward it; w: `LIGHT_*`.
    place: vec4<f32>,
    // rgb: what it gives, as its kind counts it; a: one over the chance of
    // this sample (in area measure for a triangle).
    radiance: vec4<f32>,
    // xyz: a triangle's normal or a spot's axis; w: a point's range.
    axis: vec4<f32>,
    // x, y: a spot's cosines; z: a point's decay exponent.
    falloff: vec4<f32>,
};

struct GeometryData {
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
};

// `ray_gi.rs` CACHE_BYTES counts these.
struct WorldCache {
    checksums: array<atomic<u32>, WORLD_CACHE_SIZE>,
    life: array<atomic<u32>, WORLD_CACHE_SIZE>,
    radiance: array<vec4<f32>, WORLD_CACHE_SIZE>,
    geometry_data: array<GeometryData, WORLD_CACHE_SIZE>,
    luminance_deltas: array<f32, WORLD_CACHE_SIZE>,
    active_cells_new_radiance: array<vec3<f32>, WORLD_CACHE_SIZE>,
    a: array<u32, WORLD_CACHE_SIZE>,
    b: array<u32, 1024>,
    active_cell_indices: array<u32, WORLD_CACHE_SIZE>,
    active_cells_count: u32,
};

@group(1) @binding(0) var depth: texture_depth_2d;
@group(1) @binding(1) var diffuse_guide: texture_2d<f32>;
@group(1) @binding(2) var specular_guide: texture_2d<f32>;
@group(1) @binding(3) var normal_guide: texture_2d<f32>;
@group(1) @binding(4) var irradiance_out: texture_storage_2d<rgba16float, write>;
@group(1) @binding(5) var specular_out: texture_storage_2d<rgba16float, write>;
// x, y: the motion of what the pixel reflects; z: the depth a mirror's
// replacing surface stands at; w: `GUIDE_*`.
@group(1) @binding(6) var guide_record_out: texture_storage_2d<rgba32float, write>;
@group(1) @binding(7) var<uniform> params: GiParams;
// What Ray Reconstruction reads for the world's guides.
@group(1) @binding(8) var diffuse_out: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(9) var specular_guide_out: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(10) var normal_out: texture_storage_2d<rgba16float, write>;
@group(1) @binding(11) var<storage, read> previous_models: array<mat4x4<f32>>;
@group(2) @binding(0) var<storage, read_write> world_cache: WorldCache;
@group(2) @binding(1) var<storage, read_write> light_tiles: array<GiLight>;
@group(2) @binding(2) var<storage, read> sources: array<GiSource>;
@group(3) @binding(0) var<storage, read_write> active_cells_dispatch: vec3<u32>;
// `resolve_guides`, which binds nothing else.
@group(3) @binding(1) var resolve_depth: texture_depth_2d;
@group(3) @binding(2) var resolve_motion: texture_2d<f32>;
@group(3) @binding(3) var resolve_record: texture_2d<f32>;
@group(3) @binding(4) var resolve_normal: texture_2d<f32>;
@group(3) @binding(5) var motion_out: texture_storage_2d<rgba16float, write>;
@group(3) @binding(6) var depth_out: texture_storage_2d<r32float, write>;
@group(3) @binding(7) var specular_motion_out: texture_storage_2d<rgba16float, write>;
@group(3) @binding(8) var<uniform> resolve_params: GiParams;

const WORLD_CACHE_SIZE: u32 = 1048576u;
const LIGHT_TILES: u32 = 128u;
const TILE_SAMPLES: u32 = 1024u;

const LIGHT_DIRECTION: u32 = 0u;
const LIGHT_POINT: u32 = 1u;
const LIGHT_SPOT: u32 = 2u;
const LIGHT_TRIANGLE: u32 = 3u;

const RAY_T_MAX: f32 = 100000.0;
// The lobe width of the smoothest surface the world pass draws
// (`world.wgsl` holds roughness to 0.0525): what counts as a mirror.
const MIRROR_ROUGHNESS_THRESHOLD: f32 = 0.0028;
const MAX_BOUNCES: u32 = 3u;
const PRIMARY_DI_SAMPLES: u32 = 8u;
const SECONDARY_DI_SAMPLES: u32 = 4u;
const CACHE_TERMINATION_MIN_SOLID_ANGLE: f32 = PI;

// What a pixel's guide record holds.
const GUIDE_NONE: f32 = 0.0;
const GUIDE_SPECULAR_MOTION: f32 = 1.0;
const GUIDE_REPLACED: f32 = 2.0;
// A surface is replaced outright only when nearly all its reflectance leaves
// through the delta lobe.
const FULL_REPLACEMENT_FRACTION: f32 = 0.9;
// How far a reflector's virtual image may shift between neighbouring pixels,
// as a share of the virtual distance, before it counts as too curved.
const MAX_VIRTUAL_DEPTH_JITTER: f32 = 0.002;
// What a flat surface's normals measure between neighbours in half floats.
const NORMAL_QUANTIZATION_FLOOR: f32 = 0.002;
// The lobe width below which a surface's reflection has motion of its own.
const SPECULAR_MOTION_ALPHA: f32 = 0.0625;

// Frames a cell lives for without being queried, and the attempts to find a
// cell past a hash collision.
const WORLD_CACHE_CELL_LIFETIME: u32 = 10u;
const WORLD_CACHE_MAX_SEARCH_STEPS: u32 = 3u;
const WORLD_CACHE_EMPTY_CELL: u32 = 0u;
const WORLD_CACHE_MAX_TEMPORAL_SAMPLES: f32 = 32.0;
const WORLD_CACHE_DIRECT_LIGHT_SAMPLES: u32 = 32u;
const WORLD_CACHE_MAX_GI_RAY_DISTANCE: f32 = 50.0;
const WORLD_CACHE_CELL_UPDATES_SOFT_TARGET: f32 = 40000.0;
const WORLD_CACHE_BASE_CELL_SIZE: f32 = 0.15;
const WORLD_CACHE_LOD_SCALE: f32 = 15.0;

// The floats of a pooled vertex's normal and colour (`pipelines.rs`).
const NORMAL_FLOAT: u32 = 3u;
const UV_FLOAT: u32 = 6u;
const COLOR_FLOAT: u32 = 8u;

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn rand_u(state: ptr<function, u32>) -> u32 {
    *state = *state * 747796405u + 2891336453u;
    let word = ((*state >> ((*state >> 28u) + 4u)) ^ *state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_f(state: ptr<function, u32>) -> f32 {
    return f32(rand_u(state)) * bitcast<f32>(0x2f800004u);
}

fn rand_vec2f(state: ptr<function, u32>) -> vec2<f32> {
    let x = rand_f(state);
    return vec2<f32>(x, rand_f(state));
}

fn rand_range_u(n: u32, state: ptr<function, u32>) -> u32 {
    return rand_u(state) % n;
}

// A basis around `z`: tangent, bitangent, `z`.
fn orthonormalize(z: vec3<f32>) -> mat3x3<f32> {
    let sign = select(-1.0, 1.0, z.z >= 0.0);
    let a = -1.0 / (sign + z.z);
    let b = z.x * z.y * a;
    return mat3x3<f32>(
        vec3<f32>(1.0 + sign * z.x * z.x * a, sign * b, -sign * z.x),
        vec3<f32>(b, sign + z.y * z.y * a, -z.y),
        z,
    );
}

fn sample_cosine_hemisphere(normal: vec3<f32>, rng: ptr<function, u32>) -> vec3<f32> {
    let cos_theta = 1.0 - 2.0 * rand_f(rng);
    let phi = PI_2 * rand_f(rng);
    let sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    return normalize(normal + vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta));
}

fn power_heuristic(f: f32, g: f32) -> f32 {
    let f2 = f * f;
    if f2 == 0.0 {
        return 0.0;
    }
    return 1.0 / (1.0 + g * g / f2);
}

// A surface a ray reached, as far as light is concerned.
struct GiMaterial {
    base_color: vec3<f32>,
    metallic: f32,
    // Perceptual, and its square (the lobe's width).
    roughness: f32,
    alpha: f32,
};

struct GiHit {
    position: vec3<f32>,
    normal: vec3<f32>,
    geometric_normal: vec3<f32>,
    emission: vec3<f32>,
    material: GiMaterial,
    masked: bool,
    triangle_area: f32,
    // The triangles of its part when it is a source, else 0.
    triangles: f32,
};

fn pooled_vec3(vertex: u32, float: u32) -> vec3<f32> {
    let at = vertex * VERTEX_FLOATS + float;
    return vec3<f32>(ray_vertices[at], ray_vertices[at + 1u], ray_vertices[at + 2u]);
}

// What a ray from `origin` along `direction` hit, facing it. Its albedo is
// the base texture at the mesh's uv, or the material's mean past the slots.
fn resolve_hit(hit: RayHit, origin: vec3<f32>, direction: vec3<f32>) -> GiHit {
    let part = ray_parts[hit.part];
    let first = part.slots.x + hit.primitive * 3u;
    let a = part.slots.y + ray_indices[first];
    let b = part.slots.y + ray_indices[first + 1u];
    let c = part.slots.y + ray_indices[first + 2u];
    let weights = vec3<f32>(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics);
    let to_world = mat3x3<f32>(hit.object_to_world[0], hit.object_to_world[1], hit.object_to_world[2]);
    let from_world = mat3x3<f32>(hit.world_to_object[0], hit.world_to_object[1], hit.world_to_object[2]);
    let pa = pooled_vec3(a, 0u);
    let edges = cross(to_world * (pooled_vec3(b, 0u) - pa), to_world * (pooled_vec3(c, 0u) - pa));
    let area = length(edges);
    var geometric = edges / max(area, 1e-12);
    geometric = select(geometric, -geometric, dot(geometric, direction) > 0.0);
    let local_normal = weights.x * pooled_vec3(a, NORMAL_FLOAT)
        + weights.y * pooled_vec3(b, NORMAL_FLOAT)
        + weights.z * pooled_vec3(c, NORMAL_FLOAT);
    var normal = normalize(local_normal * from_world);
    normal = select(normal, -normal, dot(normal, direction) > 0.0);
    let color = weights.x * pooled_vec3(a, COLOR_FLOAT)
        + weights.y * pooled_vec3(b, COLOR_FLOAT)
        + weights.z * pooled_vec3(c, COLOR_FLOAT);
    var base = part.surface.rgb * color;
    if part.slots.z != NO_TEXTURE {
        let va = a * VERTEX_FLOATS + UV_FLOAT;
        let vb = b * VERTEX_FLOATS + UV_FLOAT;
        let vc = c * VERTEX_FLOATS + UV_FLOAT;
        let uv = weights.x * vec2<f32>(ray_vertices[va], ray_vertices[va + 1u])
            + weights.y * vec2<f32>(ray_vertices[vb], ray_vertices[vb + 1u])
            + weights.z * vec2<f32>(ray_vertices[vc], ray_vertices[vc + 1u]);
        let at = ray_voxel_uv(part, uv);
        base *= textureSampleLevel(ray_textures[part.slots.z], ray_samplers[part.slots.z], at, 0.0).rgb;
    }
    let roughness = part.surface.w;
    return GiHit(
        origin + direction * hit.t,
        normal,
        geometric,
        part.emission.rgb,
        GiMaterial(base, part.emission.w, roughness, roughness * roughness),
        (part.slots.w & PART_MASKED) != 0u,
        0.5 * area,
        f32(part.source.x),
    );
}

// Whether `place` (a point, or a direction when `LIGHT_DIRECTION`) is in
// sight of `origin`.
fn trace_visibility(origin: vec3<f32>, place: vec4<f32>) -> f32 {
    var direction = place.xyz;
    var reach = RAY_T_MAX;
    if u32(place.w) != LIGHT_DIRECTION {
        let ray = place.xyz - origin;
        let away = length(ray);
        direction = ray / max(away, 1e-6);
        // Short of the light: an emissive part must not hide itself.
        reach = away - ray_offset(0.0);
    }
    if reach <= 0.0 {
        return 0.0;
    }
    return select(0.0, 1.0, ray_clear(origin, direction, reach));
}

// A ray's start off a surface `away` metres from the camera.
fn ray_offset(away: f32) -> f32 {
    return ray_params.shape.z + ray_params.shape.w * away;
}

// What the ambient and hemisphere rows give a ray along `direction`.
fn rows_radiance(direction: vec3<f32>) -> vec3<f32> {
    return params.ambient.rgb
        + mix(params.hemisphere_ground.rgb, params.hemisphere_sky.rgb, 0.5 * direction.y + 0.5);
}

// What the rows give a ray from `origin` that a leaf stopped.
fn rows_past_leaves(origin: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    let rows = rows_radiance(direction);
    if all(rows == vec3<f32>(0.0)) || !ray_past_leaves(origin, direction) {
        return vec3<f32>(0.0);
    }
    return rows;
}

// What a ray that leaves the scene meets. `roughness` picks the sky's
// prefiltered level: a wide lobe would only gather the sharper levels' noise.
fn sky_radiance(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    var radiance = rows_radiance(direction);
    if frame.sky_light.x > 0.0 {
        radiance += frame.sky_light.x
            * textureSampleLevel(sky_specular, sky_sampler, direction, roughness * frame.sky_light.y).rgb;
    }
    return radiance;
}

// One sample of a source, the light's chance of being picked left to the
// caller. Light rows follow `rusty::lighting::add_light`.
fn sample_source(source: GiSource, rng: ptr<function, u32>) -> GiLight {
    if source.kind.y > 0u {
        return sample_triangle(source, rng);
    }
    let light = lights[source.kind.x];
    let kind = u32(light.color_kind.w);
    let color = vec4<f32>(light.color_kind.rgb, 1.0);
    if kind == 2u {
        let toward = -normalize(light.direction_decay.xyz);
        let basis = orthonormalize(toward);
        let random = rand_vec2f(rng);
        let radius = ray_params.shape.x * sqrt(random.x);
        let angle = PI_2 * random.y;
        let direction = normalize(toward + (basis[0] * cos(angle) + basis[1] * sin(angle)) * radius);
        return GiLight(vec4<f32>(direction, f32(LIGHT_DIRECTION)), color, vec4<f32>(0.0), vec4<f32>(0.0));
    }
    let spot = select(LIGHT_POINT, LIGHT_SPOT, kind == 4u);
    return GiLight(
        vec4<f32>(light.position_range.xyz, f32(spot)),
        color,
        vec4<f32>(normalize(light.direction_decay.xyz), light.position_range.w),
        vec4<f32>(light.extra.x, light.extra.y, light.direction_decay.w, 0.0),
    );
}

// A point on one of an emissive part's triangles, uniform over them.
fn sample_triangle(source: GiSource, rng: ptr<function, u32>) -> GiLight {
    let part = ray_parts[source.kind.x];
    let first = part.slots.x + rand_range_u(source.kind.y, rng) * 3u;
    var corners: array<vec3<f32>, 3>;
    for (var corner = 0u; corner < 3u; corner++) {
        let local = vec4<f32>(pooled_vec3(part.slots.y + ray_indices[first + corner], 0u), 1.0);
        corners[corner] = vec3<f32>(dot(source.row_x, local), dot(source.row_y, local), dot(source.row_z, local));
    }
    var random = rand_vec2f(rng);
    if random.x + random.y > 1.0 {
        random = 1.0 - random;
    }
    let along = corners[1] - corners[0];
    let across = corners[2] - corners[0];
    let edges = cross(along, across);
    let area = length(edges);
    return GiLight(
        vec4<f32>(corners[0] + along * random.x + across * random.y, f32(LIGHT_TRIANGLE)),
        vec4<f32>(part.emission.rgb, f32(source.kind.y) * 0.5 * area),
        vec4<f32>(edges / max(area, 1e-12), 0.0),
        vec4<f32>(0.0),
    );
}

struct LightContribution {
    radiance: vec3<f32>,
    inverse_pdf: f32,
    // In solid angle, for rays that can also hit the light.
    inverse_solid_angle_pdf: f32,
    wi: vec3<f32>,
    brdf_rays_can_hit: bool,
};

// What `sample` gives a surface at `origin`. A light row's colour is the
// irradiance it gives, as the forward pass counts it; a triangle's the
// radiance it leaves with.
fn light_contribution(sample: GiLight, origin: vec3<f32>) -> LightContribution {
    let kind = u32(sample.place.w);
    let inverse_pdf = sample.radiance.a;
    if kind == LIGHT_DIRECTION {
        return LightContribution(sample.radiance.rgb, inverse_pdf, 0.0, sample.place.xyz, false);
    }
    let ray = sample.place.xyz - origin;
    let away = length(ray);
    let wi = ray / max(away, 1e-6);
    if kind == LIGHT_TRIANGLE {
        let cos_light = abs(dot(wi, sample.axis.xyz));
        let squared = max(away * away, 1e-6);
        return LightContribution(
            sample.radiance.rgb * cos_light / squared,
            inverse_pdf,
            inverse_pdf * cos_light / squared,
            wi,
            true,
        );
    }
    var attenuation = distance_attenuation(away, sample.axis.w, sample.falloff.z);
    if kind == LIGHT_SPOT {
        attenuation *= smoothstep(sample.falloff.x, sample.falloff.y, dot(-wi, sample.axis.xyz));
    }
    return LightContribution(sample.radiance.rgb * attenuation, inverse_pdf, 0.0, wi, false);
}

fn empty_light() -> GiLight {
    return GiLight(vec4<f32>(0.0, 1.0, 0.0, f32(LIGHT_DIRECTION)), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
}

@compute @workgroup_size(1024, 1, 1)
fn presample_light_tiles(
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_index) sample_index: u32,
) {
    let tile = workgroup_id.x;
    var rng = (tile * 0x9E3779B9u) + sample_index + params.state.x;
    let at = tile * TILE_SAMPLES + sample_index;
    let count = params.state.y;
    if count == 0u {
        light_tiles[at] = empty_light();
        return;
    }
    var sample = sample_source(sources[rand_range_u(count, &rng)], &rng);
    sample.radiance.a *= f32(count);
    light_tiles[at] = sample;
}

fn get_cell_size(world_position: vec3<f32>, rng: ptr<function, u32>) -> f32 {
    let camera_distance = distance(params.camera.xyz, world_position) / WORLD_CACHE_LOD_SCALE;
    let lod_f = log2(1.0 + camera_distance);
    let lod_fract = fract(lod_f);
    let lod = floor(lod_f) + select(0.0, 1.0, rand_f(rng) < lod_fract * lod_fract * lod_fract);
    return WORLD_CACHE_BASE_CELL_SIZE * exp2(lod);
}

fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn iqint_hash(input: u32) -> u32 {
    let n = (input << 13u) ^ input;
    return n * (n * n * 15731u + 789221u) + 1376312589u;
}

fn compute_key(position: vec3<u32>, normal: vec3<u32>) -> u32 {
    var key = pcg_hash(position.x);
    key = pcg_hash(key + position.y);
    key = pcg_hash(key + position.z);
    key = pcg_hash(key + normal.x);
    key = pcg_hash(key + normal.y);
    key = pcg_hash(key + normal.z);
    return key & (WORLD_CACHE_SIZE - 1u);
}

fn compute_checksum(position: vec3<u32>, normal: vec3<u32>) -> u32 {
    var key = iqint_hash(position.x);
    key = iqint_hash(key + position.y);
    key = iqint_hash(key + position.z);
    key = iqint_hash(key + normal.x);
    key = iqint_hash(key + normal.y);
    key = iqint_hash(key + normal.z);
    // 0 is an empty cell.
    return max(key, 1u);
}

// The irradiance the cache holds around a surface, its cell kept alive for
// `cell_lifetime` frames; a cell not there yet starts here and gives none.
fn query_world_cache(
    world_position_in: vec3<f32>,
    world_normal: vec3<f32>,
    cell_lifetime: u32,
    rng: ptr<function, u32>,
) -> vec3<f32> {
    var world_position = world_position_in;
    var cell_size = get_cell_size(world_position, rng);
    // Blurs the cache, so it does not read as a grid.
    let basis = orthonormalize(world_normal);
    let offset = (rand_vec2f(rng) * 2.0 - 1.0) * cell_size * 0.5;
    world_position += offset.x * basis[0] + offset.y * basis[1];
    cell_size = get_cell_size(world_position, rng);

    let position = bitcast<vec3<u32>>(vec3<i32>(floor(world_position / cell_size + 0.0001)));
    let normal = bitcast<vec3<u32>>(vec3<i32>(floor(world_normal + 0.0001)));
    var key = compute_key(position, normal);
    let checksum = compute_checksum(position, normal);
    for (var step = 0u; step < WORLD_CACHE_MAX_SEARCH_STEPS; step++) {
        let cas = atomicCompareExchangeWeak(&world_cache.checksums[key], WORLD_CACHE_EMPTY_CELL, checksum);
        let existing = cas.old_value;
        if existing == checksum || existing == WORLD_CACHE_EMPTY_CELL {
            atomicMax(&world_cache.life[key], cell_lifetime);
        }
        if existing == checksum {
            return world_cache.radiance[key].rgb;
        }
        if existing == WORLD_CACHE_EMPTY_CELL && cas.exchanged {
            world_cache.geometry_data[key].world_position = world_position;
            world_cache.geometry_data[key].world_normal = world_normal;
            return vec3<f32>(0.0);
        }
        key = (key + 1u) & (WORLD_CACHE_SIZE - 1u);
    }
    return vec3<f32>(0.0);
}

var<workgroup> w1: array<u32, 1024>;
var<workgroup> w2: array<u32, 1024>;

@compute @workgroup_size(1024, 1, 1)
fn decay_world_cache(@builtin(global_invocation_id) id: vec3<u32>) {
    var life = atomicLoad(&world_cache.life[id.x]);
    if life > 0u {
        life -= 1u;
        atomicStore(&world_cache.life[id.x], life);
        if life == 0u {
            atomicStore(&world_cache.checksums[id.x], WORLD_CACHE_EMPTY_CELL);
            world_cache.radiance[id.x] = vec4<f32>(0.0);
            world_cache.luminance_deltas[id.x] = 0.0;
        }
    }
}

// An exclusive scan of the cells' liveness within each block of 1024.
@compute @workgroup_size(1024, 1, 1)
fn compact_world_cache_single_block(
    @builtin(global_invocation_id) cell_id: vec3<u32>,
    @builtin(local_invocation_index) t: u32,
) {
    if t == 0u { w1[0u] = 0u; } else { w1[t] = u32(atomicLoad(&world_cache.life[cell_id.x - 1u]) != 0u); }
    workgroupBarrier();
    if t < 1u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 1u]; }
    workgroupBarrier();
    if t < 2u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 2u]; }
    workgroupBarrier();
    if t < 4u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 4u]; }
    workgroupBarrier();
    if t < 8u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 8u]; }
    workgroupBarrier();
    if t < 16u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 16u]; }
    workgroupBarrier();
    if t < 32u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 32u]; }
    workgroupBarrier();
    if t < 64u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 64u]; }
    workgroupBarrier();
    if t < 128u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 128u]; }
    workgroupBarrier();
    if t < 256u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 256u]; }
    workgroupBarrier();
    if t < 512u { world_cache.a[cell_id.x] = w2[t]; } else { world_cache.a[cell_id.x] = w2[t] + w2[t - 512u]; }
}

// The same scan over the blocks' totals.
@compute @workgroup_size(1024, 1, 1)
fn compact_world_cache_blocks(@builtin(local_invocation_index) t: u32) {
    if t == 0u {
        w1[0u] = 0u;
    } else {
        w1[t] = world_cache.a[t * 1024u - 1u] + u32(atomicLoad(&world_cache.life[t * 1024u - 1u]) != 0u);
    }
    workgroupBarrier();
    if t < 1u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 1u]; }
    workgroupBarrier();
    if t < 2u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 2u]; }
    workgroupBarrier();
    if t < 4u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 4u]; }
    workgroupBarrier();
    if t < 8u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 8u]; }
    workgroupBarrier();
    if t < 16u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 16u]; }
    workgroupBarrier();
    if t < 32u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 32u]; }
    workgroupBarrier();
    if t < 64u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 64u]; }
    workgroupBarrier();
    if t < 128u { w1[t] = w2[t]; } else { w1[t] = w2[t] + w2[t - 128u]; }
    workgroupBarrier();
    if t < 256u { w2[t] = w1[t]; } else { w2[t] = w1[t] + w1[t - 256u]; }
    workgroupBarrier();
    if t < 512u { world_cache.b[t] = w2[t]; } else { world_cache.b[t] = w2[t] + w2[t - 512u]; }
}

@compute @workgroup_size(1024, 1, 1)
fn compact_world_cache_write_active_cells(
    @builtin(global_invocation_id) cell_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_index) thread_index: u32,
) {
    let compacted_index = world_cache.a[cell_id.x] + world_cache.b[workgroup_id.x];
    let cell_active = atomicLoad(&world_cache.life[cell_id.x]) != 0u;
    if cell_active {
        world_cache.active_cell_indices[compacted_index] = cell_id.x;
    }
    if thread_index == 1023u && workgroup_id.x == 1023u {
        let active_cell_count = compacted_index + u32(cell_active);
        world_cache.active_cells_count = active_cell_count;
        active_cells_dispatch = vec3<u32>((active_cell_count + 63u) / 64u, 1u, 1u);
    }
}

fn cell_skipped(rng: ptr<function, u32>) -> bool {
    return rand_f(rng) >= WORLD_CACHE_CELL_UPDATES_SOFT_TARGET / f32(world_cache.active_cells_count);
}

@compute @workgroup_size(64, 1, 1)
fn sample_di(@builtin(workgroup_id) workgroup_id: vec3<u32>, @builtin(global_invocation_id) active_cell_id: vec3<u32>) {
    if active_cell_id.x >= world_cache.active_cells_count {
        return;
    }
    let cell_index = world_cache.active_cell_indices[active_cell_id.x];
    let geometry = world_cache.geometry_data[cell_index];
    var rng = cell_index + params.state.x;
    if cell_skipped(&rng) {
        return;
    }
    world_cache.active_cells_new_radiance[active_cell_id.x] =
        sample_random_light_ris(geometry.world_position, geometry.world_normal, workgroup_id.xy, &rng);
}

@compute @workgroup_size(64, 1, 1)
fn sample_gi(@builtin(global_invocation_id) active_cell_id: vec3<u32>) {
    if active_cell_id.x >= world_cache.active_cells_count {
        return;
    }
    let cell_index = world_cache.active_cell_indices[active_cell_id.x];
    let geometry = world_cache.geometry_data[cell_index];
    var rng = cell_index + params.state.x;
    if cell_skipped(&rng) {
        return;
    }
    let direction = sample_cosine_hemisphere(geometry.world_normal, &rng);
    let origin = geometry.world_position + geometry.world_normal * ray_offset(distance(params.camera.xyz, geometry.world_position));
    let ray = ray_closest(origin, direction, WORLD_CACHE_MAX_GI_RAY_DISTANCE);
    if ray.hit {
        let hit = resolve_hit(ray, origin, direction);
        let life = atomicLoad(&world_cache.life[cell_index]);
        let radiance = query_world_cache(hit.position, hit.geometric_normal, life, &rng);
        world_cache.active_cells_new_radiance[active_cell_id.x] += hit.material.base_color * radiance;
        if hit.masked {
            world_cache.active_cells_new_radiance[active_cell_id.x] += PI * rows_past_leaves(origin, direction);
        }
    } else {
        // Cut short at the ray's reach: past it a surface may have stood.
        world_cache.active_cells_new_radiance[active_cell_id.x] += PI * sky_radiance(direction, 1.0);
    }
}

@compute @workgroup_size(64, 1, 1)
fn blend_new_samples(@builtin(global_invocation_id) active_cell_id: vec3<u32>) {
    if active_cell_id.x >= world_cache.active_cells_count {
        return;
    }
    let cell_index = world_cache.active_cell_indices[active_cell_id.x];
    var rng = cell_index + params.state.x;
    if cell_skipped(&rng) {
        return;
    }
    let reset = params.state.z != 0u;
    let old_radiance = world_cache.radiance[cell_index];
    let new_radiance = world_cache.active_cells_new_radiance[active_cell_id.x];
    let luminance_delta = world_cache.luminance_deltas[cell_index];
    let sample_count = min(old_radiance.a + 1.0, WORLD_CACHE_MAX_TEMPORAL_SAMPLES);
    let alpha = abs(luminance_delta) / max(luminance(old_radiance.rgb), 0.001);
    let max_sample_count = mix(WORLD_CACHE_MAX_TEMPORAL_SAMPLES, 1.0, pow(saturate(alpha), 1.0 / 8.0));
    let blend_amount = select(1.0 / min(sample_count, max_sample_count), 1.0, reset);
    let blended_radiance = mix(old_radiance.rgb, new_radiance, blend_amount);
    let moved = luminance(blended_radiance) - luminance(old_radiance.rgb);
    world_cache.radiance[cell_index] = vec4<f32>(blended_radiance, sample_count);
    world_cache.luminance_deltas[cell_index] = select(mix(luminance_delta, moved, 1.0 / 8.0), 0.0, reset);
}

// A tile for a workgroup: its threads share what they read.
fn light_tile_start(workgroup_id: vec2<u32>, bounce: u32) -> u32 {
    var workgroup_rng = (workgroup_id.x * 5782582u) + workgroup_id.y + bounce;
    return rand_range_u(LIGHT_TILES, &workgroup_rng) * TILE_SAMPLES;
}

fn sample_random_light_ris(
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    workgroup_id: vec2<u32>,
    rng: ptr<function, u32>,
) -> vec3<f32> {
    let tile_start = light_tile_start(workgroup_id, 0u);
    var weight_sum = 0.0;
    var selected_radiance = vec3<f32>(0.0);
    var selected_target = 0.0;
    var selected_place = vec4<f32>(0.0);
    let mis_weight = 1.0 / f32(WORLD_CACHE_DIRECT_LIGHT_SAMPLES);
    for (var i = 0u; i < WORLD_CACHE_DIRECT_LIGHT_SAMPLES; i++) {
        let sample = light_tiles[tile_start + rand_range_u(TILE_SAMPLES, rng)];
        let light = light_contribution(sample, world_position);
        let contribution = light.radiance * saturate(dot(light.wi, world_normal));
        let target_function = luminance(contribution);
        let resampling_weight = mis_weight * target_function * light.inverse_pdf;
        weight_sum += resampling_weight;
        if rand_f(rng) * weight_sum < resampling_weight {
            selected_radiance = contribution;
            selected_target = target_function;
            selected_place = sample.place;
        }
    }
    if selected_target <= 0.0 {
        return vec3<f32>(0.0);
    }
    let origin = world_position + world_normal * ray_offset(distance(params.camera.xyz, world_position));
    return selected_radiance * (weight_sum / selected_target) * trace_visibility(origin, selected_place);
}

struct LobeReflectances {
    specular: vec3<f32>,
    diffuse: vec3<f32>,
};

fn lobe_reflectances(material: GiMaterial, n_dot_v: f32) -> LobeReflectances {
    let f0 = mix(vec3<f32>(0.04), material.base_color, material.metallic);
    return LobeReflectances(
        environment_brdf(f0, material.roughness, n_dot_v),
        (1.0 - material.metallic) * material.base_color,
    );
}

fn specular_lobe_probability(specular: f32, diffuse: f32, roughness: f32) -> f32 {
    let energy_probability = specular / max(specular + diffuse, 0.0001);
    // Smooth dielectric reflections get more samples, not to be undersampled.
    return max(energy_probability, mix(0.5, energy_probability, roughness));
}

// Bounded VNDF sampling for Smith-GGX reflections
// (https://gpuopen.com/download/Bounded_VNDF_Sampling_for_Smith-GGX_Reflections.pdf).
// The result is invalid where z <= 0.
fn sample_ggx_vndf(wi_tangent: vec3<f32>, alpha: f32, rng: ptr<function, u32>) -> vec3<f32> {
    if alpha <= MIRROR_ROUGHNESS_THRESHOLD {
        return vec3<f32>(-wi_tangent.xy, wi_tangent.z);
    }
    let i = wi_tangent;
    let random = rand_vec2f(rng);
    let i_std = normalize(vec3<f32>(i.xy * alpha, i.z));
    let phi = PI_2 * random.x;
    let s = 1.0 + length(i.xy);
    let a2 = alpha * alpha;
    let s2 = s * s;
    let k = (1.0 - a2) * s2 / (s2 + a2 * i.z * i.z);
    let b = select(i_std.z, k * i_std.z, i.z > 0.0);
    let z = fma(1.0 - random.y, 1.0 + b, -b);
    let sin_theta = sqrt(saturate(1.0 - z * z));
    let o_std = vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), z);
    let m_std = i_std + o_std;
    let m = normalize(vec3<f32>(m_std.xy * alpha, m_std.z));
    return 2.0 * dot(i, m) * m - i;
}

fn d_ggx(alpha: f32, n_dot_h: f32) -> f32 {
    let a2 = alpha * alpha;
    let denominator = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denominator * denominator);
}

fn infinity() -> f32 {
    return bitcast<f32>(0x7F800000u);
}

fn ggx_vndf_pdf(wi_tangent: vec3<f32>, wo_tangent: vec3<f32>, alpha: f32) -> f32 {
    if alpha <= MIRROR_ROUGHNESS_THRESHOLD {
        let mirror = vec3<f32>(-wi_tangent.xy, wi_tangent.z);
        return select(0.0, infinity(), distance(mirror, wo_tangent) < 0.0001);
    }
    let i = wi_tangent;
    let m = normalize(i + wo_tangent);
    let ndf = d_ggx(alpha, saturate(m.z));
    let ai = alpha * i.xy;
    let len2 = dot(ai, ai);
    let t = sqrt(len2 + i.z * i.z);
    if i.z >= 0.0 {
        let s = 1.0 + length(i.xy);
        let a2 = alpha * alpha;
        let s2 = s * s;
        let k = (1.0 - a2) * s2 / (s2 + a2 * i.z * i.z);
        return ndf / (2.0 * (k * i.z + t));
    }
    return ndf * (t - i.z) / max(2.0 * len2, 1e-12);
}

struct BrdfSample {
    wi: vec3<f32>,
    throughput: vec3<f32>,
    pdf: f32,
    diffuse: bool,
};

// The surface's response from `wi` toward `wo`, the cosine included.
fn evaluate_brdf(wo: vec3<f32>, wi: vec3<f32>, normal: vec3<f32>, material: GiMaterial) -> vec3<f32> {
    let n_dot_l = dot(normal, wi);
    let n_dot_v = dot(normal, wo);
    if n_dot_l < 0.0001 || n_dot_v < 0.0001 {
        return vec3<f32>(0.0);
    }
    let diffuse = (1.0 - material.metallic) * material.base_color / PI;
    if material.alpha <= MIRROR_ROUGHNESS_THRESHOLD {
        return diffuse * n_dot_l;
    }
    let f0 = mix(vec3<f32>(0.04), material.base_color, material.metallic);
    return (diffuse + brdf_ggx(wi, wo, normal, material.roughness, f0)) * n_dot_l;
}

fn brdf_pdf(wo: vec3<f32>, wi: vec3<f32>, normal: vec3<f32>, material: GiMaterial) -> f32 {
    let rho = lobe_reflectances(material, max(dot(normal, wo), 0.0001));
    let specular_weight = specular_lobe_probability(luminance(rho.specular), luminance(rho.diffuse), material.roughness);
    let basis = orthonormalize(normal);
    let wo_tangent = wo * basis;
    let wi_tangent = wi * basis;
    return (1.0 - specular_weight) * wi_tangent.z / PI
        + specular_weight * ggx_vndf_pdf(wo_tangent, wi_tangent, material.alpha);
}

fn evaluate_and_sample_brdf(
    wo: vec3<f32>,
    normal: vec3<f32>,
    material: GiMaterial,
    rng: ptr<function, u32>,
) -> BrdfSample {
    let n_dot_v = dot(normal, wo);
    if n_dot_v < 0.0001 {
        return BrdfSample(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, false);
    }
    let rho = lobe_reflectances(material, n_dot_v);
    let specular_weight = specular_lobe_probability(luminance(rho.specular), luminance(rho.diffuse), material.roughness);
    let diffuse_weight = 1.0 - specular_weight;
    let basis = orthonormalize(normal);
    let wo_tangent = wo * basis;
    var wi: vec3<f32>;
    var wi_tangent: vec3<f32>;
    let diffuse_selected = rand_f(rng) < diffuse_weight;
    if diffuse_selected {
        wi = sample_cosine_hemisphere(normal, rng);
        wi_tangent = wi * basis;
    } else {
        wi_tangent = sample_ggx_vndf(wo_tangent, material.alpha, rng);
        if !(wi_tangent.z > 0.0) {
            return BrdfSample(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, false);
        }
        wi = basis * wi_tangent;
        // A mirror's lobe is a delta function.
        if material.alpha <= MIRROR_ROUGHNESS_THRESHOLD {
            return BrdfSample(wi, rho.specular / specular_weight, infinity(), false);
        }
    }
    let pdf = diffuse_weight * wi_tangent.z / PI
        + specular_weight * ggx_vndf_pdf(wo_tangent, wi_tangent, material.alpha);
    return BrdfSample(wi, evaluate_brdf(wo, wi, normal, material) / max(pdf, 1e-12), pdf, diffuse_selected);
}

// The chance of NEE on a surface, for lights rays can also hit: bright where
// it is diffuse, none on a metal mirror, which NEE cannot sample.
fn nee_probability(material: GiMaterial) -> f32 {
    if material.alpha <= MIRROR_ROUGHNESS_THRESHOLD && material.metallic > 0.9999 {
        return 0.0;
    }
    return mix(1.0, material.roughness, material.metallic);
}

struct DiSample {
    unbiased_contribution_weight: f32,
    place: vec4<f32>,
    wi: vec3<f32>,
    brdf_radiance: vec3<f32>,
    inverse_solid_angle_pdf: f32,
    brdf_rays_can_hit: bool,
};

// Resample a light from a tile. On the primary surface the forward pass has
// lit it with every light row, so only what rays can also hit counts, and
// only its diffuse lobe (`emissive_only`), its albedo left to the composite.
fn sample_light_ris(
    origin: vec3<f32>,
    normal: vec3<f32>,
    wo: vec3<f32>,
    material: GiMaterial,
    di_samples: u32,
    emissive_only: bool,
    workgroup_id: vec2<u32>,
    bounce: u32,
    rng: ptr<function, u32>,
) -> DiSample {
    let tile_start = light_tile_start(workgroup_id, bounce);
    var weight_sum = 0.0;
    var selected = DiSample(0.0, vec4<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), 0.0, false);
    var selected_target = 0.0;
    let mis_weight = 1.0 / f32(di_samples);
    for (var i = 0u; i < di_samples; i++) {
        let sample = light_tiles[tile_start + rand_range_u(TILE_SAMPLES, rng)];
        let light = light_contribution(sample, origin);
        var brdf: vec3<f32>;
        if emissive_only {
            brdf = vec3<f32>(select(0.0, saturate(dot(normal, light.wi)) / PI, light.brdf_rays_can_hit));
        } else {
            brdf = evaluate_brdf(wo, light.wi, normal, material);
        }
        let brdf_radiance = brdf * light.radiance;
        let target_function = luminance(brdf_radiance);
        let resampling_weight = mis_weight * target_function * light.inverse_pdf;
        weight_sum += resampling_weight;
        if rand_f(rng) * weight_sum < resampling_weight {
            selected_target = target_function;
            selected = DiSample(0.0, sample.place, light.wi, brdf_radiance, light.inverse_solid_angle_pdf, light.brdf_rays_can_hit);
        }
    }
    if selected_target > 0.0 {
        selected.unbiased_contribution_weight = weight_sum / selected_target;
    }
    return selected;
}

// A light's share by NEE at a vertex, weighted against a BRDF ray finding it.
fn nee_radiance(
    origin: vec3<f32>,
    normal: vec3<f32>,
    wo: vec3<f32>,
    material: GiMaterial,
    di_samples: u32,
    emissive_only: bool,
    workgroup_id: vec2<u32>,
    bounce: u32,
    rng: ptr<function, u32>,
) -> vec3<f32> {
    let p_nee = nee_probability(material);
    if p_nee == 0.0 {
        return vec3<f32>(0.0);
    }
    let di = sample_light_ris(origin, normal, wo, material, di_samples, emissive_only, workgroup_id, bounce, rng);
    if luminance(di.brdf_radiance) <= 0.0 {
        return vec3<f32>(0.0);
    }
    // Only lights rays can also find may be skipped: NEE is the sole way to
    // a light row such as the sun.
    let p_keep = select(1.0, p_nee, di.brdf_rays_can_hit);
    if rand_f(rng) >= p_keep {
        return vec3<f32>(0.0);
    }
    let weight = di.unbiased_contribution_weight * trace_visibility(origin, di.place);
    if weight <= 0.0 {
        return vec3<f32>(0.0);
    }
    var mis = 1.0;
    if di.brdf_rays_can_hit && di.inverse_solid_angle_pdf > 0.0 {
        let p_strategy = f32(di_samples) * p_nee / di.inverse_solid_angle_pdf;
        var p_brdf = saturate(dot(normal, di.wi)) / PI;
        if !emissive_only {
            p_brdf = brdf_pdf(wo, di.wi, normal, material);
        }
        mis = power_heuristic(p_strategy, p_brdf);
    }
    return di.brdf_radiance * weight * mis / p_keep;
}

// A primary surface's walk along its mirror chain, by bounce.
struct PsrState {
    finished: bool,
    // The surface gives way to what it reflects, or only says how that moves.
    replace_fully: bool,
    // The chain's reflections, as a rotor and the parity of their count.
    mirror_rotor: vec4<f32>,
    mirror_parity_odd: bool,
    // The distance from the primary surface through every mirror.
    reflection_length: f32,
    // What the reflected surface's colour is scaled by on its way back.
    throughput: vec3<f32>,
};

// What the guides say of a pixel once its path has walked.
struct PsrGuides {
    kind: f32,
    motion: vec2<f32>,
    depth: f32,
    diffuse: vec3<f32>,
    specular: vec3<f32>,
    normal_roughness: vec4<f32>,
};

fn mirror_chain_rotor(plane_normal: vec3<f32>, parity_odd: bool) -> vec4<f32> {
    if parity_odd {
        return vec4<f32>(plane_normal.y, -plane_normal.x, 0.0, plane_normal.z);
    }
    return vec4<f32>(-plane_normal.y, plane_normal.x, 0.0, plane_normal.z);
}

fn quaternion_multiply(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(a.w * b.xyz + b.w * a.xyz + cross(a.xyz, b.xyz), a.w * b.w - dot(a.xyz, b.xyz));
}

fn apply_mirror_chain(psr: PsrState, v: vec3<f32>) -> vec3<f32> {
    var reflected = v;
    if psr.mirror_parity_odd {
        reflected.z = -reflected.z;
    }
    let t = 2.0 * cross(psr.mirror_rotor.xyz, reflected);
    return reflected + psr.mirror_rotor.w * t + cross(psr.mirror_rotor.xyz, t);
}

fn is_delta_mirror(material: GiMaterial) -> bool {
    if material.alpha > MIRROR_ROUGHNESS_THRESHOLD {
        return false;
    }
    let rho = lobe_reflectances(material, 1.0);
    let specular = luminance(rho.specular);
    return specular / max(specular + luminance(rho.diffuse), 0.0001) >= FULL_REPLACEMENT_FRACTION;
}

// The angle the normal turns through over a pixel, from the world's guide.
fn reflector_curvature(pixel: vec2<u32>, normal: vec3<f32>) -> f32 {
    let last = params.viewport.xy + params.viewport.zw - vec2<u32>(1u);
    let right = textureLoad(normal_guide, min(pixel + vec2<u32>(1u, 0u), last), 0).xyz;
    let down = textureLoad(normal_guide, min(pixel + vec2<u32>(0u, 1u), last), 0).xyz;
    let least = min(abs(dot(normal, normalize(right))), abs(dot(normal, normalize(down))));
    return max(0.0, sqrt(max(0.0, 2.0 * (1.0 - least))) - NORMAL_QUANTIZATION_FLOOR);
}

// How far the point `previous` has moved on screen to the pixel showing it
// now, as `rusty::motion::motion_at` measures it.
fn pixel_motion(pixel: vec2<u32>, previous: vec4<f32>) -> vec2<f32> {
    if previous.w <= 1e-6 {
        return vec2<f32>(0.0);
    }
    return vec2<f32>(pixel) + 0.5 - previous.xy / previous.w;
}

// Where a path's hit stood in the previous frame. A scatter copy stands
// still.
fn previous_hit_position(ray: RayHit, position: vec3<f32>) -> vec3<f32> {
    if ray_parts[ray.part].source.y != 0u {
        return position;
    }
    let local = ray.world_to_object * vec4<f32>(position, 1.0);
    return (previous_models[ray.part] * vec4<f32>(local, 1.0)).xyz;
}

// The surface a mirror chain ended on, seen through it: where it seems to
// stand and how it seems to move, and, where the primary surface gives way
// to it, its guides reflected into the mirror's virtual space.
// https://developer.nvidia.com/blog/rendering-perfect-reflections-and-refractions-in-path-traced-games/#primary_surface_replacement
fn replace_primary_surface(
    pixel: vec2<u32>,
    psr: PsrState,
    ray: RayHit,
    hit: GiHit,
    primary_position: vec3<f32>,
    primary_normal: vec3<f32>,
    primary_wo: vec3<f32>,
    guides: PsrGuides,
    specular_fraction: f32,
) -> PsrGuides {
    var result = guides;
    let path_length = distance(primary_position, params.camera.xyz) + psr.reflection_length;
    // Too curved to follow: the pixel keeps its own surface's motion.
    if psr.replace_fully
        && 2.0 * reflector_curvature(pixel, primary_normal) * psr.reflection_length > MAX_VIRTUAL_DEPTH_JITTER * path_length {
        return result;
    }
    let virtual_position = primary_position - primary_wo * psr.reflection_length;
    let world_motion = previous_hit_position(ray, hit.position) - hit.position;
    let virtual_previous = virtual_position + apply_mirror_chain(psr, world_motion);
    let eye = params.camera.xyz;
    result.motion = pixel_motion(pixel, previous_pixel_from(eye, virtual_previous - eye));
    result.kind = GUIDE_SPECULAR_MOTION;
    if !psr.replace_fully {
        return result;
    }
    let wo = normalize(eye - virtual_position);
    let virtual_normal = normalize(apply_mirror_chain(psr, hit.normal));
    let clip = params.view_proj * vec4<f32>(virtual_position, 1.0);
    let rho = lobe_reflectances(hit.material, max(dot(virtual_normal, wo), 0.0001));
    result.kind = GUIDE_REPLACED;
    result.depth = clip.z / clip.w;
    result.diffuse = psr.throughput * rho.diffuse + guides.diffuse;
    result.specular = mix(guides.specular, psr.throughput * rho.specular, specular_fraction);
    result.normal_roughness = vec4<f32>(virtual_normal, hit.material.roughness);
    return result;
}

struct GiPath {
    radiance: vec3<f32>,
    diffuse: bool,
    guides: PsrGuides,
};

// One path from a primary surface. It starts on one of the surface's lobes,
// and all it finds is that lobe's: a cosine sample's mean is the irradiance
// over pi, a VNDF sample's the split-sum radiance (`environment_brdf`, in the
// specular guide, is the rest), so neither lobe carries its albedo.
fn trace_path(
    position: vec3<f32>,
    surface_normal: vec3<f32>,
    wo: vec3<f32>,
    specular_share: f32,
    roughness: f32,
    specular_fraction: f32,
    guides: PsrGuides,
    pixel: vec2<u32>,
    workgroup_id: vec2<u32>,
    rng: ptr<function, u32>,
) -> GiPath {
    var result = GiPath(vec3<f32>(0.0), rand_f(rng) >= specular_share, guides);
    let alpha = roughness * roughness;
    // The chain opens with the primary surface's own reflection.
    var psr = PsrState(
        result.diffuse || alpha > SPECULAR_MOTION_ALPHA,
        alpha <= MIRROR_ROUGHNESS_THRESHOLD && specular_fraction >= FULL_REPLACEMENT_FRACTION,
        mirror_chain_rotor(surface_normal, false),
        true,
        0.0,
        vec3<f32>(1.0),
    );
    // The reflectance of the mirror the walk stands on, and whether the last
    // lobe was diffuse.
    var reflectance = guides.specular;
    var lobe_diffuse = result.diffuse;
    var origin = position + surface_normal * ray_offset(distance(params.camera.xyz, position));
    var throughput = vec3<f32>(1.0 / select(specular_share, 1.0 - specular_share, result.diffuse));
    var wi: vec3<f32>;
    var pdf: f32;
    // Whether light sampling at the last vertex could have found what a ray
    // finds next.
    var nee_samples = 0u;
    var p_nee = 0.0;
    // The sky level a ray that misses reads.
    var lobe_roughness = 1.0;
    let surface = GiMaterial(vec3<f32>(0.0), 0.0, 1.0, 1.0);
    if result.diffuse {
        // The forward pass drew the light rows: only emissive parts are left.
        if params.state.w > 0u {
            result.radiance += throughput
                * nee_radiance(origin, surface_normal, wo, surface, PRIMARY_DI_SAMPLES, true, workgroup_id, 0u, rng);
            nee_samples = PRIMARY_DI_SAMPLES;
            p_nee = 1.0;
        }
        wi = sample_cosine_hemisphere(surface_normal, rng);
        pdf = max(dot(surface_normal, wi), 0.0) / PI;
    } else {
        let basis = orthonormalize(surface_normal);
        let wo_tangent = wo * basis;
        let wi_tangent = sample_ggx_vndf(wo_tangent, alpha, rng);
        if !(wi_tangent.z > 0.0) {
            return result;
        }
        wi = basis * wi_tangent;
        pdf = ggx_vndf_pdf(wo_tangent, wi_tangent, alpha);
        lobe_roughness = roughness;
    }
    for (var bounce = 0u; bounce < MAX_BOUNCES; bounce++) {
        let ray = ray_closest(origin, wi, RAY_T_MAX);
        if !ray.hit {
            result.radiance += throughput * sky_radiance(wi, lobe_roughness);
            // The sky stands infinitely far along the view through the chain.
            if !psr.finished && !lobe_diffuse {
                result.guides.motion = pixel_motion(pixel, previous_pixel_toward(-wo));
                result.guides.kind = GUIDE_SPECULAR_MOTION;
            }
            break;
        }
        let hit = resolve_hit(ray, origin, wi);
        if hit.masked {
            result.radiance += throughput * rows_past_leaves(origin, wi);
        }
        if !psr.finished {
            if lobe_diffuse {
                psr.finished = true;
            } else {
                psr.throughput *= reflectance;
                psr.reflection_length += ray.t;
                if alpha <= MIRROR_ROUGHNESS_THRESHOLD && is_delta_mirror(hit.material) {
                    psr.mirror_rotor = quaternion_multiply(
                        psr.mirror_rotor,
                        mirror_chain_rotor(hit.normal, psr.mirror_parity_odd),
                    );
                    psr.mirror_parity_odd = !psr.mirror_parity_odd;
                } else {
                    psr.finished = true;
                    result.guides = replace_primary_surface(
                        pixel, psr, ray, hit, position, surface_normal, wo, result.guides, specular_fraction,
                    );
                }
            }
        }
        // An emissive surface a ray found, weighted against NEE finding it.
        if any(hit.emission > vec3<f32>(0.0)) {
            var mis = 1.0;
            if hit.triangles > 0.0 && nee_samples > 0u {
                let cosine = max(dot(hit.normal, -wi), 0.0001);
                let area_pdf = 1.0 / (f32(params.state.y) * hit.triangles * hit.triangle_area);
                let p_light = area_pdf * ray.t * ray.t / cosine;
                mis = power_heuristic(pdf, p_light * p_nee * f32(nee_samples));
            }
            result.radiance += throughput * hit.emission * mis;
        }
        // Into the cache where the lobe was wide enough for its noise to be
        // worth less than its error. A short ray ends there too: among leaves
        // most are short, and walking on doubled what a path cost.
        let wide = 1.0 / pdf >= CACHE_TERMINATION_MIN_SOLID_ANGLE;
        if wide || bounce == MAX_BOUNCES - 1u {
            let cached = query_world_cache(hit.position, hit.geometric_normal, WORLD_CACHE_CELL_LIFETIME, rng);
            result.radiance += throughput * (hit.material.base_color / PI) * cached;
            break;
        }
        let away = distance(params.camera.xyz, hit.position);
        origin = hit.position + hit.geometric_normal * ray_offset(away);
        result.radiance += throughput
            * nee_radiance(origin, hit.normal, -wi, hit.material, SECONDARY_DI_SAMPLES, false, workgroup_id, bounce + 1u, rng);
        let next = evaluate_and_sample_brdf(-wi, hit.normal, hit.material, rng);
        if next.pdf == 0.0 {
            break;
        }
        throughput *= next.throughput;
        wi = next.wi;
        pdf = next.pdf;
        lobe_diffuse = next.diffuse;
        reflectance = lobe_reflectances(hit.material, max(dot(hit.normal, -wi), 0.0001)).specular;
        nee_samples = SECONDARY_DI_SAMPLES;
        p_nee = nee_probability(hit.material);
        lobe_roughness = hit.material.roughness;
        // Russian roulette, past the first bounce's surface.
        if bounce > 0u {
            let survival = saturate(luminance(throughput));
            if rand_f(rng) >= survival {
                break;
            }
            throughput /= survival;
        }
    }
    return result;
}

// A thread takes a pair of pixels side by side and traces one of them, the two
// changing places each frame and each row, so a frame traces a checkerboard
// and the composite fills the rest. A smooth surface traces both: its
// reflection has no neighbours to borrow and its guides must hold from frame
// to frame.
@compute @workgroup_size(8, 8, 1)
fn trace_pixels(@builtin(workgroup_id) workgroup_id: vec3<u32>, @builtin(global_invocation_id) id: vec3<u32>) {
    // The frame's seed is twice an odd number: its second bit turns each frame.
    let traced = vec2<u32>(id.x * 2u + ((id.y + (params.state.x >> 1u)) & 1u), id.y);
    let other = vec2<u32>(traced.x ^ 1u, id.y);
    for (var turn = 0u; turn < 2u; turn++) {
        let at = select(other, traced, turn == 0u);
        if any(at >= params.viewport.zw) {
            continue;
        }
        let roughness = textureLoad(normal_guide, at + params.viewport.xy, 0).w;
        trace_pixel(at, turn == 0u || roughness * roughness <= SPECULAR_MOTION_ALPHA, workgroup_id.xy);
    }
}

fn trace_pixel(at: vec2<u32>, traced: bool, workgroup_id: vec2<u32>) {
    let pixel = at + params.viewport.xy;
    let d = textureLoad(depth, pixel, 0);
    let diffuse = textureLoad(diffuse_guide, pixel, 0).rgb;
    let specular = textureLoad(specular_guide, pixel, 0).rgb;
    let diffuse_weight = luminance(diffuse);
    let specular_weight = luminance(specular);
    let normal_roughness = textureLoad(normal_guide, pixel, 0);
    var guides = PsrGuides(GUIDE_NONE, vec2<f32>(0.0), 0.0, diffuse, specular, normal_roughness);
    // Nothing there, a surface the guides call unlit, or left to the
    // composite. Alpha 0: nothing a neighbour may borrow.
    if !traced || d >= 1.0 || diffuse_weight + specular_weight <= 0.0 {
        textureStore(irradiance_out, pixel, vec4<f32>(0.0));
        textureStore(specular_out, pixel, vec4<f32>(0.0));
        write_guides(pixel, guides);
        return;
    }
    let uv = (vec2<f32>(at) + 0.5) / vec2<f32>(params.viewport.zw);
    let clip = params.inverse_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let position = clip.xyz / clip.w;
    let wo = normalize(params.camera.xyz - position);
    // As the world shaded it, not toward the camera: grass and leaves carry
    // normals that face the sky however they are seen.
    let normal = normalize(normal_roughness.xyz);
    var rng = at.x + at.y * params.viewport.z + params.state.x;
    let share = specular_lobe_probability(specular_weight, diffuse_weight, normal_roughness.w);
    let fraction = specular_weight / max(specular_weight + diffuse_weight, 0.0001);
    let path = trace_path(position, normal, wo, share, normal_roughness.w, fraction, guides, pixel, workgroup_id, &rng);
    let light = vec4<f32>(path.radiance, 1.0);
    let none = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    textureStore(irradiance_out, pixel, select(none, light, path.diffuse));
    textureStore(specular_out, pixel, select(light, none, path.diffuse));
    write_guides(pixel, path.guides);
}

fn write_guides(pixel: vec2<u32>, guides: PsrGuides) {
    textureStore(guide_record_out, pixel, vec4<f32>(guides.motion, guides.depth, guides.kind));
    textureStore(diffuse_out, pixel, vec4<f32>(guides.diffuse, 1.0));
    textureStore(specular_guide_out, pixel, vec4<f32>(guides.specular, 1.0));
    textureStore(normal_out, pixel, guides.normal_roughness);
}

// Ray Reconstruction's depth and motion, once the blended parts have added
// theirs: the world's, but where a path replaced the surface. A pixel whose
// lobe this frame found nothing to say keeps its reflection's last motion:
// alternating with the surface's, the denoiser would blur the reflection.
@compute @workgroup_size(8, 8, 1)
fn resolve_guides(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= resolve_params.viewport.zw) {
        return;
    }
    let pixel = id.xy + resolve_params.viewport.xy;
    let surface_motion = textureLoad(resolve_motion, pixel, 0);
    let record = textureLoad(resolve_record, pixel, 0);
    let replaced = record.w == GUIDE_REPLACED;
    let specular = vec4<f32>(record.xy, surface_motion.zw);
    textureStore(motion_out, pixel, select(surface_motion, specular, replaced));
    textureStore(depth_out, pixel, vec4<f32>(select(textureLoad(resolve_depth, pixel, 0), record.z, replaced)));
    let roughness = textureLoad(resolve_normal, pixel, 0).w;
    if record.w != GUIDE_NONE {
        textureStore(specular_motion_out, pixel, specular);
    } else if roughness * roughness > SPECULAR_MOTION_ALPHA {
        textureStore(specular_motion_out, pixel, surface_motion);
    }
}
