// The standard shader's world pass: parts drawn with their material and the
// pass's light rows into the view's linear HDR target, which the finish pass
// finishes (`rusty::finish`). Each material compiles the features it uses
// (`lib.rs` `Features`): UNLIT, MASK, VOXEL_SURFACE, NORMAL_MAP,
// EMISSIVE_MAP, OCCLUSION_MAP, TRIPLANAR, TERRAIN_LAYERS, STOCHASTIC_TILING,
// FLAT_SHADING, WIND, WATER, WATER_SCENE; and the mesh's
// streams: VERTEX_TANGENTS, LAYER_WEIGHTS, VERTEX_OCCLUSION, SKINNED. A
// product shader (PRODUCT_SHADER) shades the surface in place of
// `rusty::shade`, and may place its vertices (PRODUCT_DISPLACES) and give
// their normals (PRODUCT_DISPLACES_NORMAL).
// With MOTION (a temporal stage's inputs, render-wgpu `motion.rs`) it also
// writes each fragment's motion to location 1 (`rusty::motion`).

#import rusty::types::{texture_space_position, Vertex}
#import rusty::view::{frame, parts, instances, copies, copy_row, selected_copy, impostor_card, fade_mask, mask_coverage, fade_toward_origin, morph_share, morph_normal, projected}
#ifdef WIND
#import rusty::wind::wind_displace
#endif
#ifdef MOTION
#import rusty::motion::{previous_pixel, previous_model, previous_copy_model, motion_output}
#ifdef WIND
#import rusty::wind::wind_displace_at
#import rusty::motion::previous_time
#endif
#endif
#import rusty::material::{
    material,
    albedo,
    albedo_sampler,
    emissive_map,
    emissive_sampler,
    normal_map,
    normal_sampler,
    occlusion_map,
    occlusion_sampler,
    product_map_a,
    product_sampler_a,
    product_map_b,
    product_sampler_b,
    layer_albedo_1,
    layer_albedo_2,
    layer_albedo_3,
    layer_normal_1,
    layer_normal_2,
    layer_normal_3,
}
#import rusty::surface::{
    transform_uv,
    voxel_uv,
    voxel_lod,
    tiled_texture,
    layer_shares,
    perturb_normal,
    tangent_normal,
    triplanar_uvs,
    triplanar_weights,
    triplanar_normal,
    HexSample,
    HexTiles,
    hex_tiles,
    hex_texture,
    hex_normal, hex_data,
}
#import rusty::types::Surface
#import rusty::shade::standard_shade
#import rusty::lighting::{weathered, snow_sunk, copy_open_sky}
#ifdef SCENE_COLOR
#import rusty::lighting::water_reflection
#endif
#ifdef DECALS
#import rusty::decals::decaled
#endif
#ifdef PRODUCT_SHADER
#import rusty::product::shade
#endif
#ifdef PRODUCT_DISPLACES
#import rusty::product::displace
#endif
#ifdef PRODUCT_DISPLACES_NORMAL
#import rusty::product::displace_normal
#endif

// The view's screen-space ambient occlusion (`ambient_occlusion.wgsl`),
// sampled by target pixel; a 1×1 white map with strength 0 when it is off.
struct AmbientOcclusion {
    // x: strength (0 off); yz: 1 / target size (pixels).
    params: vec4<f32>,
};

#ifdef WATER
// A water material draws blended, without the view's occlusion: its group 2
// is the opaque pass's depth instead (render-wgpu `water.rs`), by target
// pixel: what lies behind the water's surface.
@group(2) @binding(0) var scene_depth: texture_depth_2d;
#ifdef SCENE_COLOR
// The opaque pass's colour (linear HDR; alpha its coverage), copied with
// the depth when a water reads the scene or the view reflects in its water;
// a 1×1 stand-in otherwise, which nothing reads.
@group(2) @binding(1) var scene_color: texture_2d<f32>;
@group(2) @binding(2) var scene_sampler: sampler;
#endif
#else
@group(2) @binding(0) var ambient_occlusion_map: texture_2d<f32>;
@group(2) @binding(1) var ambient_occlusion_sampler: sampler;
@group(2) @binding(2) var<uniform> ambient_occlusion: AmbientOcclusion;
#endif

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
    @location(4) color: vec4<f32>,
#ifdef VERTEX_TANGENTS
    // World tangent; w: handedness, flipped under a mirroring model matrix.
    @location(5) tangent: vec4<f32>,
    @location(6) uv1: vec2<f32>,
#endif
#ifdef TRIPLANAR
    // Texture-space position and normal, which the planes project.
    @location(7) texture_position: vec3<f32>,
    @location(8) texture_normal: vec3<f32>,
#endif
#ifdef WATER
    // The clip position, for the pixel's place in the scene depth.
    @location(9) clip_position: vec4<f32>,
#endif
#ifdef SCATTER_SELECTED
    // The copy's share of the samples (`rusty::view::fade_mask`).
    @location(10) @interpolate(flat) fade: f32,
#endif
#ifdef SCATTER
    // The copy's own normal matrix (an impostor's, its turn): the fragment
    // stage's part row is its patch's, without the copy's turn.
    @location(11) @interpolate(flat) turn_x: vec3<f32>,
    @location(12) @interpolate(flat) turn_y: vec3<f32>,
    @location(13) @interpolate(flat) turn_z: vec3<f32>,
    // How open the sky is over the copy, for one that sinks under snow
    // (`rusty::lighting::copy_open_sky`).
    @location(14) @interpolate(flat) snow_open: f32,
#endif
#ifdef MOTION
    // Where the point stood in the previous frame (`rusty::motion`).
    @location(15) previous: vec4<f32>,
#endif
};

// The uv set (0 or 1) a slot reads; without the second stream, set 0.
fn slot_uv(in: VsOut, uv_set: u32) -> vec2<f32> {
#ifdef VERTEX_TANGENTS
    return select(in.uv, in.uv1, uv_set == 1u);
#else
    return in.uv;
#endif
}

@vertex
fn vs_world(
    @location(0) mesh_position: vec3<f32>,
    @location(1) mesh_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
    @location(5) uv1: vec2<f32>,
#endif
#ifdef MORPH
    // Height change, target normal x and z.
    @location(6) morph: vec3<f32>,
#endif
#ifdef MOTION
#ifdef SKINNED
    // The skinned vertex in the previous frame's pose (render-wgpu
    // `animated.rs`): its position while the pose has not changed since.
    @location(7) previous_position: vec3<f32>,
#endif
#endif
    @builtin(instance_index) instance: u32,
) -> VsOut {
#ifdef SCATTER_SELECTED
    let selected = selected_copy(instance);
    let part = selected.part;
    let row = selected.row;
    let top = selected.top;
#else
#ifdef SCATTER
    let scattered = copies[instance];
    let part = bitcast<u32>(scattered.tint.w);
    let row = copy_row(scattered);
    let top = scattered.detail.y;
#else
    let part = instances[instance];
    let row = parts[part];
#endif
#endif
#ifdef IMPOSTOR
    // A card facing the camera, showing the baked view nearest its direction.
    let card = impostor_card(row, mesh_position, material.parameters[0], material.parameters[1].x);
    let position = mesh_position;
    let normal = mesh_normal;
    var world = fade_toward_origin(row, card.world);
#else
#ifdef MORPH
    let share = morph_share(row, mesh_position);
    let position = mesh_position + vec3<f32>(0.0, morph.x * share, 0.0);
    let normal = normalize(mix(mesh_normal, morph_normal(morph), share));
    var world = (row.model * vec4<f32>(position, 1.0)).xyz;
#else
    let position = mesh_position;
    let normal = mesh_normal;
    var world = fade_toward_origin(row, (row.model * vec4<f32>(position, 1.0)).xyz);
#endif
#endif
    var world_normal = mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * normal;
#ifdef IMPOSTOR
    world_normal = card.normal;
#endif
#ifdef WIND
#ifdef LAYER_WEIGHTS
    let wind_weight = 1.0;
#else
#ifdef VERTEX_OCCLUSION
    let wind_weight = 1.0;
#else
    let wind_weight = color.a;
#endif
#endif
    let unswayed = world;
    world = wind_displace(world, row.model[3].xyz, wind_weight);
#endif
#ifdef SCATTER
    // Grass and flowers sink into the snow where the sky is open over them.
    var snow_open = 0.0;
    if material.weather.y > 0.5 {
        snow_open = copy_open_sky((row.model * vec4<f32>(0.0, top, 0.0, 1.0)).xyz);
        world = snow_sunk(world, row.model[3].xyz, snow_open);
    }
#endif
#ifdef PRODUCT_SHADER
    // The vertex as placed so far, which a product's vertex stages move and
    // turn.
    let vertex = Vertex(world, world_normal, position, normal, uv, color, row.model[3].xyz, part);
#endif
#ifdef PRODUCT_DISPLACES
    world = displace(vertex);
#endif
#ifdef PRODUCT_DISPLACES_NORMAL
    world_normal = normalize(displace_normal(vertex));
#endif
    var out: VsOut;
    out.clip = projected(world);
#ifdef MOTION
    // Where the vertex stood: moved as its model moved since (a card by its
    // origin) and a skinned one as its pose did, and swayed as the wind
    // swayed it then. A distance fade, the
    // snow and a product's displacement keep their offsets.
#ifdef SCATTER_SELECTED
    let earlier = previous_copy_model(part, selected.copy);
#else
#ifdef SCATTER
    let earlier = previous_copy_model(part, scattered);
#else
    let earlier = previous_model(part);
#endif
#endif
#ifdef IMPOSTOR
    let anchor = vec4<f32>(0.0, 0.0, 0.0, 1.0);
#else
    let anchor = vec4<f32>(position, 1.0);
#endif
#ifdef SKINNED
    // A skinned vertex also moves by its pose.
    let earlier_anchor = vec4<f32>(previous_position, 1.0);
#else
    let earlier_anchor = anchor;
#endif
    var previous = world + (earlier * earlier_anchor).xyz - (row.model * anchor).xyz;
#ifdef WIND
    previous += wind_displace_at(unswayed, row.model[3].xyz, wind_weight, previous_time())
        - wind_displace(unswayed, row.model[3].xyz, wind_weight);
#endif
    out.previous = previous_pixel(previous);
#endif
    out.world_position = world;
    out.normal = world_normal;
#ifdef IMPOSTOR
    out.uv = card.uv;
    out.turn_x = card.turn[0];
    out.turn_y = card.turn[1];
    out.turn_z = card.turn[2];
#else
    out.uv = uv;
#ifdef SCATTER
    out.turn_x = row.normal_x.xyz;
    out.turn_y = row.normal_y.xyz;
    out.turn_z = row.normal_z.xyz;
#endif
#endif
#ifdef SCATTER
    out.snow_open = snow_open;
#endif
#ifdef WATER
    out.clip_position = out.clip;
#endif
    out.part = part;
#ifdef SCATTER_SELECTED
    out.color = vec4<f32>(color.rgb * selected.tint, color.a);
    out.fade = selected.fade;
#else
#ifdef SCATTER
    out.color = vec4<f32>(color.rgb * scattered.tint.rgb, color.a);
#else
    out.color = color;
#endif
#endif
#ifdef VERTEX_TANGENTS
    let model = mat3x3<f32>(row.model[0].xyz, row.model[1].xyz, row.model[2].xyz);
    out.tangent = vec4<f32>(model * tangent.xyz, tangent.w * sign(determinant(model)));
    out.uv1 = uv1;
#endif
#ifdef TRIPLANAR
    out.texture_position = texture_space_position(row, position);
#ifdef PRODUCT_DISPLACES_NORMAL
    // The product's normal back in texture space (the mesh's, scaled), which
    // the fragment's normal matrix returns to the world.
    out.texture_normal = transpose(mat3x3<f32>(row.model[0].xyz, row.model[1].xyz, row.model[2].xyz)) * world_normal;
#else
    out.texture_normal = normal;
#endif
#endif
    return out;
}

// The base texture at a surface uv: tile coordinates through the voxel
// surface tiling, or a mesh uv.
fn base_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef VOXEL_SURFACE
    return textureSampleLevel(albedo, albedo_sampler,
        transform_uv(material.base_uv_u, material.base_uv_v, voxel_uv(uv, material.tile, material.sample_rect)),
        voxel_lod(uv, material.tile, material.sample_rect, vec2<f32>(textureDimensions(albedo, 0))));
#else
    return textureSample(albedo, albedo_sampler, transform_uv(material.base_uv_u, material.base_uv_v, uv));
#endif
}

// The normal map at a surface uv, as `base_texture` samples.
fn normal_texture(uv: vec2<f32>) -> vec3<f32> {
#ifdef VOXEL_SURFACE
    return textureSampleLevel(normal_map, normal_sampler,
        transform_uv(material.normal_uv_u, material.normal_uv_v, voxel_uv(uv, material.tile, material.sample_rect)),
        voxel_lod(uv, material.tile, material.sample_rect, vec2<f32>(textureDimensions(normal_map, 0)))).rgb;
#else
    return textureSample(normal_map, normal_sampler,
        transform_uv(material.normal_uv_u, material.normal_uv_v, uv)).rgb;
#endif
}

// The occlusion, roughness, metalness map at a surface uv, as `base_texture`
// samples (ORM_MAP).
fn orm_texture(uv: vec2<f32>) -> vec3<f32> {
#ifdef VOXEL_SURFACE
    return textureSampleLevel(occlusion_map, occlusion_sampler,
        transform_uv(material.occlusion_uv_u, material.occlusion_uv_v, voxel_uv(uv, material.tile, material.sample_rect)),
        voxel_lod(uv, material.tile, material.sample_rect, vec2<f32>(textureDimensions(occlusion_map, 0)))).rgb;
#else
    return textureSample(occlusion_map, occlusion_sampler,
        transform_uv(material.occlusion_uv_u, material.occlusion_uv_v, uv)).rgb;
#endif
}

// The base texture at a mesh uv as hex tiles, and the normal map read through
// the same tiles and shares (STOCHASTIC_TILING; never on a voxel surface).
struct Tiled {
    base: HexSample,
    tiles: HexTiles,
}

fn base_tiled(uv: vec2<f32>) -> Tiled {
    var tiled: Tiled;
    tiled.tiles = hex_tiles(transform_uv(material.base_uv_u, material.base_uv_v, uv));
    tiled.base = hex_texture(albedo, albedo_sampler, tiled.tiles, material.factors.z);
    return tiled;
}

fn tiled_normal(tiled: Tiled) -> vec3<f32> {
    return hex_normal(normal_map, normal_sampler, tiled.tiles, tiled.base.shares);
}

// Terrain layer `layer` (1 to 3)'s base texture and normal map at a
// surface uv, through its own tiling.
fn layer_texture(layer: u32, uv: vec2<f32>) -> vec4<f32> {
    let tile = material.layer_tile[layer - 1u];
    let rect = material.layer_rect[layer - 1u];
    switch layer {
        case 1u: { return tiled_texture(layer_albedo_1, albedo_sampler, uv, tile, rect); }
        case 2u: { return tiled_texture(layer_albedo_2, albedo_sampler, uv, tile, rect); }
        default: { return tiled_texture(layer_albedo_3, albedo_sampler, uv, tile, rect); }
    }
}

fn layer_normal(layer: u32, uv: vec2<f32>) -> vec3<f32> {
    let tile = material.layer_tile[layer - 1u];
    let rect = material.layer_rect[layer - 1u];
    switch layer {
        case 1u: { return tiled_texture(layer_normal_1, normal_sampler, uv, tile, rect).rgb; }
        case 2u: { return tiled_texture(layer_normal_2, normal_sampler, uv, tile, rect).rgb; }
        default: { return tiled_texture(layer_normal_3, normal_sampler, uv, tile, rect).rgb; }
    }
}

// Each terrain layer's share of this fragment: the mesh's layer weights
// through the material's contrast, or all layer 0 on a mesh without them.
fn terrain_shares(in: VsOut) -> vec4<f32> {
#ifdef LAYER_WEIGHTS
#ifdef VERTEX_OCCLUSION
    // The fourth layer's weight is what the first three leave; the alpha
    // is occlusion.
    let weights = vec4<f32>(in.color.rgb, max(1.0 - in.color.r - in.color.g - in.color.b, 0.0));
#else
    let weights = in.color;
#endif
    return layer_shares(weights, material.layer_factors.w);
#else
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
#endif
}

// The standard surface stages: what the material's maps, the part and the
// vertex make of this fragment. Every map is sampled, and every derivative
// taken, here, before the shade stage may discard.
fn standard_surface(in: VsOut, front: bool) -> Surface {
    let row = parts[in.part];
#ifdef TERRAIN_LAYERS
    let shares = terrain_shares(in);
#endif
#ifdef TRIPLANAR
    let planes = triplanar_uvs(in.texture_position, in.texture_normal);
    let weights = triplanar_weights(in.texture_normal, material.factors.y);
#ifdef STOCHASTIC_TILING
    let tiled = array<Tiled, 3>(base_tiled(planes[0]), base_tiled(planes[1]), base_tiled(planes[2]));
    var texture_color = tiled[0].base.color * weights.x + tiled[1].base.color * weights.y
        + tiled[2].base.color * weights.z;
#else
    var texture_color = base_texture(planes[0]) * weights.x + base_texture(planes[1]) * weights.y
        + base_texture(planes[2]) * weights.z;
#endif
#ifdef TERRAIN_LAYERS
    texture_color = texture_color * shares.x;
    for (var layer = 1u; layer <= 3u; layer++) {
        texture_color += (layer_texture(layer, planes[0]) * weights.x
            + layer_texture(layer, planes[1]) * weights.y
            + layer_texture(layer, planes[2]) * weights.z) * shares[layer];
    }
#endif
#else
#ifdef VOXEL_SURFACE
    var texture_color = base_texture(in.uv);
#ifdef TERRAIN_LAYERS
    texture_color = texture_color * shares.x + layer_texture(1u, in.uv) * shares.y
        + layer_texture(2u, in.uv) * shares.z + layer_texture(3u, in.uv) * shares.w;
#endif
#else
#ifdef STOCHASTIC_TILING
    let tiled = base_tiled(slot_uv(in, material.tex_coords.x));
    let texture_color = tiled.base.color;
#else
    let texture_color = base_texture(slot_uv(in, material.tex_coords.x));
#endif
#endif
#endif
    var surface: Surface;
#ifdef LAYER_WEIGHTS
    // The vertex colour holds layer weights.
    surface.tint = row.color;
#else
#ifdef VERTEX_OCCLUSION
    // The vertex colour's alpha holds occlusion.
    surface.tint = row.color * vec4<f32>(in.color.rgb, 1.0);
#else
    surface.tint = row.color * in.color;
#endif
#endif
    surface.base = surface.tint * texture_color;
    surface.world_position = in.world_position;
    surface.uv = in.uv;
    surface.roughness = 1.0;
    surface.metalness = 0.0;
#ifdef VERTEX_OCCLUSION
    surface.occlusion = in.color.a;
#else
    surface.occlusion = 1.0;
#endif
    surface.emission = vec3<f32>(0.0);
#ifdef FLAT_SHADING
    // The triangle's own plane, turned to face the way the mesh's normal
    // does, so a mirrored part keeps its outside.
    let flat = cross(dpdx(in.world_position), dpdy(in.world_position));
    let geometric = normalize(flat * sign(dot(flat, in.normal) + 1e-20));
#else
    let geometric = normalize(in.normal);
#endif
#ifdef UNLIT
    surface.normal = select(-geometric, geometric, front);
#else
#ifdef EMISSIVE_MAP
    let emissive = textureSample(emissive_map, emissive_sampler,
        transform_uv(material.emissive_uv_u, material.emissive_uv_v, slot_uv(in, material.tex_coords.y))).rgb;
#else
    let emissive = vec3<f32>(1.0);
#endif
    surface.emission = row.emission.rgb * emissive;
#ifdef OCCLUSION_MAP
    let occlusion_sample = textureSample(occlusion_map, occlusion_sampler,
        transform_uv(material.occlusion_uv_u, material.occlusion_uv_v, slot_uv(in, material.tex_coords.w))).r;
    // Multiplied into the baked vertex occlusion (VERTEX_OCCLUSION), so a
    // neutral map keeps it.
    surface.occlusion *= 1.0 + material.factors.x * (occlusion_sample - 1.0);
#endif
#ifdef ORM_MAP
    // Packed occlusion, roughness, metalness, read as the base texture is.
#ifdef TRIPLANAR
#ifdef STOCHASTIC_TILING
    let orm = hex_data(occlusion_map, occlusion_sampler, tiled[0].tiles, tiled[0].base.shares) * weights.x
        + hex_data(occlusion_map, occlusion_sampler, tiled[1].tiles, tiled[1].base.shares) * weights.y
        + hex_data(occlusion_map, occlusion_sampler, tiled[2].tiles, tiled[2].base.shares) * weights.z;
#else
    let orm = orm_texture(planes[0]) * weights.x + orm_texture(planes[1]) * weights.y
        + orm_texture(planes[2]) * weights.z;
#endif
#else
#ifdef VOXEL_SURFACE
    let orm = orm_texture(in.uv);
#else
#ifdef STOCHASTIC_TILING
    let orm = hex_data(occlusion_map, occlusion_sampler, tiled.tiles, tiled.base.shares);
#else
    let orm = orm_texture(slot_uv(in, material.tex_coords.w));
#endif
#endif
#endif
    surface.occlusion *= 1.0 + material.factors.x * (orm.r - 1.0);
    let map_roughness = orm.g;
    let map_metalness = orm.b;
#else
    let map_roughness = 1.0;
    let map_metalness = 1.0;
#endif
#ifdef NORMAL_MAP
#ifdef TRIPLANAR
    // Blended in texture space, then into the world by the normal matrix.
#ifdef STOCHASTIC_TILING
    let samples = array<vec3<f32>, 3>(tiled_normal(tiled[0]), tiled_normal(tiled[1]),
        tiled_normal(tiled[2]));
#else
    let samples = array<vec3<f32>, 3>(normal_texture(planes[0]), normal_texture(planes[1]),
        normal_texture(planes[2]));
#endif
    var mapped = triplanar_normal(normalize(in.texture_normal), samples, weights, material.normal_scale);
#ifdef TERRAIN_LAYERS
    mapped = mapped * shares.x;
    for (var layer = 1u; layer <= 3u; layer++) {
        let layer_samples = array<vec3<f32>, 3>(layer_normal(layer, planes[0]),
            layer_normal(layer, planes[1]), layer_normal(layer, planes[2]));
        mapped += triplanar_normal(normalize(in.texture_normal), layer_samples, weights,
            material.layer_factors[layer - 1u]) * shares[layer];
    }
    mapped = normalize(mapped);
#endif
#ifdef SCATTER
    var normal = normalize(mat3x3<f32>(in.turn_x, in.turn_y, in.turn_z) * mapped);
#else
    var normal = normalize(mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * mapped);
#endif
#else
#ifdef VOXEL_SURFACE
    // Through the same tiling as the base texture; the frame comes from the
    // continuous tile coordinate, which does not jump at tile seams.
    var normal = perturb_normal(geometric, in.world_position, (in.uv - material.tile.zw) / material.tile.xy,
        normal_texture(in.uv), material.normal_scale);
#ifdef TERRAIN_LAYERS
    normal = normal * shares.x;
    for (var layer = 1u; layer <= 3u; layer++) {
        let tile = material.layer_tile[layer - 1u];
        normal += perturb_normal(geometric, in.world_position, (in.uv - tile.zw) / tile.xy,
            layer_normal(layer, in.uv), material.layer_factors[layer - 1u]) * shares[layer];
    }
    normal = normalize(normal);
#endif
#else
    let normal_uv = transform_uv(material.normal_uv_u, material.normal_uv_v, slot_uv(in, material.tex_coords.z));
#ifdef STOCHASTIC_TILING
    let normal_sample = tiled_normal(tiled);
#else
    let normal_sample = textureSample(normal_map, normal_sampler, normal_uv).rgb;
#endif
#ifdef VERTEX_TANGENTS
    var normal = tangent_normal(geometric, in.tangent, normal_sample, material.normal_scale);
#else
    var normal = perturb_normal(geometric, in.world_position, normal_uv, normal_sample, material.normal_scale);
#endif
#endif
#endif
#else
    var normal = geometric;
#endif
#ifdef IMPOSTOR
    // The baked view's normal, in the mesh's own space, turned with the copy.
    let baked = textureSample(normal_map, normal_sampler, in.uv).rgb * 2.0 - 1.0;
    normal = normalize(mat3x3<f32>(in.turn_x, in.turn_y, in.turn_z) * baked);
#endif
    var facing = geometric;
    if !front {
        normal = -normal;
        facing = -facing;
    }
    surface.normal = normal;
    let normal_change = max(abs(dpdx(facing)), abs(dpdy(facing)));
    let geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);
    surface.roughness = min(max(material.roughness * map_roughness, 0.0525) + geometry_roughness, 1.0);
    surface.metalness = material.metalness * map_metalness;
#endif
    return surface;
}

#ifdef WATER
#ifdef SCENE_COLOR
// The view's viewport in its target, pixels (xy origin, zw size), from a
// fragment's place in it: `ndc` its normalized device xy, `pixel` its
// target position. Called in uniform control flow (screen derivatives).
fn viewport_of(ndc: vec2<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let size = 2.0 / vec2<f32>(dpdx(ndc.x), -dpdy(ndc.y));
    return vec4<f32>(pixel - (ndc * vec2<f32>(0.5, -0.5) + 0.5) * size, size);
}

// Where world point `point` falls in the target, pixels, under the view's
// viewport; w below 0 behind the camera.
fn target_pixel(point: vec3<f32>, viewport: vec4<f32>) -> vec4<f32> {
    let clip = projected(point);
    let ndc = clip.xyz / clip.w;
    return vec4<f32>(viewport.xy + (ndc.xy * vec2<f32>(0.5, -0.5) + 0.5) * viewport.zw, ndc.z, clip.w);
}

// The world point the depth copy holds at target pixel `pixel`.
fn scene_point(pixel: vec2<f32>, viewport: vec4<f32>) -> vec3<f32> {
    let z = textureLoad(scene_depth, vec2<i32>(pixel), 0);
    let ndc = ((pixel - viewport.xy) / viewport.zw - 0.5) * vec2<f32>(2.0, -2.0);
    let behind = frame.inv_view_proj * vec4<f32>(ndc, z, 1.0);
    return behind.xyz / behind.w;
}

// The opaque scene seen along the ray from `origin` in `direction`
// (`Frame.water`): the ray out to the march's reach, cut at the camera's
// plane and where it leaves the viewport, is marched across the depth copy
// in the march's steps, spread evenly over the screen and offset per pixel.
// Where it first passes behind the scene, by less than its step, the hit is
// refined by halving and the colour there taken, its weight fading toward
// the viewport's edges, the march's reach and what the copy does not
// cover. 0 where the ray leaves the viewport first or passes behind
// something thicker than its step.
fn water_reflection_of(origin: vec3<f32>, direction: vec3<f32>, viewport: vec4<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let steps = u32(frame.water.y);
    let reach = frame.water.z;
    let start = projected(origin);
    var end = projected(origin + direction * reach);
    var length = reach;
    if end.w < WATER_REFLECTION_NEAR {
        let cut = (start.w - WATER_REFLECTION_NEAR) / (start.w - end.w);
        end = mix(start, end, cut);
        length = reach * cut;
    }
    // Where the line leaves the viewport, as a share of it.
    let first = clip_pixel(start, viewport);
    let span = clip_pixel(end, viewport) - first;
    let low = viewport.xy + 0.5;
    let high = viewport.xy + viewport.zw - 0.5;
    var leave = 1.0;
    for (var axis = 0u; axis < 2u; axis = axis + 1u) {
        if span[axis] > 1e-4 {
            leave = min(leave, (high[axis] - first[axis]) / span[axis]);
        } else if span[axis] < -1e-4 {
            leave = min(leave, (low[axis] - first[axis]) / span[axis]);
        }
    }
    leave = clamp(leave, 0.0, 1.0);
    // Interleaved gradient noise: neighbouring pixels start their steps
    // apart, trading banding for fine noise.
    let jitter = fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
    var previous = 0.0;
    var previous_along = 0.0;
    for (var index = 0u; index < steps; index = index + 1u) {
        let share = leave * (f32(index) + jitter) / f32(steps);
        let at = ray_at(start, end, share, length, viewport);
        if at.z > textureLoad(scene_depth, vec2<i32>(at.xy), 0) {
            // Behind the scene: a hit if within this step of its surface;
            // otherwise the ray went behind something whose far side the
            // view does not show.
            let gap = distance(origin + direction * at.w, frame.camera.xyz)
                - distance(scene_point(at.xy, viewport), frame.camera.xyz);
            if gap > max(at.w - previous_along, WATER_REFLECTION_THICKNESS) {
                return vec4<f32>(0.0);
            }
            var below = previous;
            var above = share;
            for (var refine = 0u; refine < 4u; refine = refine + 1u) {
                let middle = 0.5 * (below + above);
                let probe = ray_at(start, end, middle, length, viewport);
                if probe.z > textureLoad(scene_depth, vec2<i32>(probe.xy), 0) {
                    above = middle;
                } else {
                    below = middle;
                }
            }
            let hit = ray_at(start, end, above, length, viewport);
            let colour = textureSampleLevel(scene_color, scene_sampler,
                hit.xy / vec2<f32>(textureDimensions(scene_color)), 0.0);
            let edge = min(min(hit.x - viewport.x, viewport.x + viewport.z - hit.x),
                min(hit.y - viewport.y, viewport.y + viewport.w - hit.y));
            let fade = smoothstep(0.0, WATER_REFLECTION_EDGE * min(viewport.z, viewport.w), edge)
                * (1.0 - smoothstep(0.75, 1.0, hit.w / reach)) * colour.a;
            return vec4<f32>(colour.rgb / max(colour.a, 1e-4), fade);
        }
        previous = share;
        previous_along = at.w;
    }
    return vec4<f32>(0.0);
}

// Clip-space point `clip`'s target pixel under `viewport`.
fn clip_pixel(clip: vec4<f32>, viewport: vec4<f32>) -> vec2<f32> {
    return viewport.xy + (clip.xy / clip.w * vec2<f32>(0.5, -0.5) + 0.5) * viewport.zw;
}

// The point a share `share` of the way across the screen from the ray's
// clip-space `start` to `end`, `length` metres apart in the world: xy its
// target pixel under `viewport`, z its depth, w its distance along the ray
// (perspective-correct: 1 / w is linear across the screen).
fn ray_at(start: vec4<f32>, end: vec4<f32>, share: f32, length: f32, viewport: vec4<f32>) -> vec4<f32> {
    let along = share / end.w / mix(1.0 / start.w, 1.0 / end.w, share);
    let clip = mix(start, end, along);
    return vec4<f32>(clip_pixel(clip, viewport), clip.z / clip.w, along * length);
}

// The share of the viewport's smaller side over which a reflection fades
// into the standard one at its edges.
const WATER_REFLECTION_EDGE: f32 = 0.08;
// How far above the surface plane (the cosine to its normal) a reflected
// ray turns fully into the reflection.
const WATER_REFLECTION_ABOVE: f32 = 0.05;
// The least depth, metres, the reflected scene is taken to have behind its
// surface.
const WATER_REFLECTION_THICKNESS: f32 = 0.3;
// The nearest a reflected ray is followed toward the camera's plane, in
// clip w.
const WATER_REFLECTION_NEAR: f32 = 1e-3;
#endif

#ifdef WATER_SCENE
// The most the refraction shifts the scene, as a share of the viewport's
// smaller side, and the deepest water whose depth adds to it, metres.
const WATER_REFRACTION_LIMIT: f32 = 0.05;
const WATER_REFRACTION_DEPTH: f32 = 2.0;

// What shows through the water (WATER_SCENE): the opaque scene's colour,
// refracted and absorbed, its alpha the copy's coverage there; the shade
// stage's colour goes over it (`world_color`).
var<private> water_behind: vec4<f32>;
#endif

// The water surface (WATER): the standard surface tinted by the depth of
// the scene behind it along the view ray, foam where that scene lies within
// the shoreline width below the surface, the normal rippled by two
// scrolling normal maps (or procedural ripples when the material has none),
// and the alpha raised toward the Fresnel reflectance at grazing angles.
// With the scene's colour (SCENE_COLOR) the view may reflect the opaque
// scene in it; reading it (WATER_SCENE) the water absorbs and refracts what
// shows through.
fn water_surface(in: VsOut, standard: Surface) -> Surface {
    var surface = standard;
    let flags = u32(material.factors.w + 0.5);
    let time = frame.time.x;
    let wave_uv = in.world_position.xz / max(material.water_params.w, 1e-3);
    // The normal: the maps' slopes added in the water's own frame (its
    // surface lies along the ground), scaled by the ripple scale.
    var slope = vec2<f32>(0.0);
    if (flags & 4u) != 0u {
        slope += (textureSample(normal_map, normal_sampler, wave_uv + material.water_scroll.zw * time).xy * 2.0 - 1.0);
    }
    if (flags & 2u) != 0u {
        slope += (textureSample(product_map_b, product_sampler_b, wave_uv * 1.37 + material.water_params.xy * time).xy * 2.0 - 1.0);
    }
    if (flags & 6u) == 0u {
        let phase = wave_uv * 6.2832;
        slope = vec2<f32>(
            sin(phase.x + time * 1.3) * 0.5 + sin(phase.x * 0.7 + phase.y * 0.4 + time * 0.9) * 0.3,
            cos(phase.y * 1.1 + time * 1.7) * 0.5 + cos(phase.x * 0.3 - phase.y * 0.8 + time * 0.7) * 0.3,
        ) * 0.25;
    }
    let tilt = vec3<f32>(slope.x, 0.0, slope.y) * material.normal_scale;
    let flat = surface.normal;
    surface.normal = normalize(surface.normal + tilt);
    // What lies behind the surface along this pixel's ray.
    let ndc = in.clip_position.xy / in.clip_position.w;
    let scene_z = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0);
    let behind = frame.inv_view_proj * vec4<f32>(ndc, scene_z, 1.0);
    let scene_world = behind.xyz / behind.w;
    let through = distance(scene_world, in.world_position);
    let vertical = max(in.world_position.y - scene_world.y, 0.0);
    let absorbed = 1.0 - exp(-through / max(material.water_shallow.w, 1e-3));
    let colour = mix(material.water_shallow.rgb, material.water_deep.rgb, absorbed);
    // Over the material's colour and texture (the standard base).
    var base = vec4<f32>(standard.base.rgb * colour, mix(standard.base.a, 1.0, absorbed));
    // Foam along the shore: the foam texture scrolled over the water, or
    // bands rolling in, above the threshold.
    let shoreline = max(material.water_deep.w, 1e-3);
    let shore = 1.0 - smoothstep(0.0, shoreline, vertical);
    var foam_mask = 0.5 + 0.5 * sin((vertical / shoreline - time * 0.8) * 9.4248);
    if (flags & 1u) != 0u {
        foam_mask = textureSample(product_map_a, product_sampler_a, wave_uv * 2.0 + material.water_scroll.xy * time).r;
    }
    let threshold = material.water_params.z;
    let foam = shore * smoothstep(threshold - 0.15, threshold + 0.05, foam_mask);
    base = vec4<f32>(mix(base.rgb, vec3<f32>(1.0), foam), max(base.a, foam));
    // Grazing views reflect more than they let through.
    let view = normalize(frame.camera.xyz - in.world_position);
    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(surface.normal, view), 0.0), 5.0);
    surface.base = vec4<f32>(base.rgb, max(base.a, fresnel));
#ifdef SCENE_COLOR
    let viewport = viewport_of(ndc, in.clip.xy);
    if frame.water.y > 0.0 {
        // A ripple tilted far enough reflects the ray under the surface,
        // where it would find the bed: only rays above it reflect.
        let reflected = reflect(-view, surface.normal);
        water_reflection = water_reflection_of(in.world_position, reflected, viewport, in.clip.xy)
            * smoothstep(0.0, WATER_REFLECTION_ABOVE, dot(reflected, flat));
    }
#endif
#ifdef WATER_SCENE
    // The scene shifted along the ripples' tilt by the water's depth there,
    // within the limit; the unshifted pixel where the shift lands on
    // something in front of the water.
    var seen = in.clip.xy;
    if material.water_refraction.x > 0.0 {
        let shift = tilt * material.water_refraction.x * min(through, WATER_REFRACTION_DEPTH);
        let at = target_pixel(scene_world + shift, viewport);
        var offset = at.xy - in.clip.xy;
        let limit = WATER_REFRACTION_LIMIT * min(viewport.z, viewport.w);
        offset = offset * min(1.0, limit / max(length(offset), 1e-4));
        let shifted = clamp(in.clip.xy + offset, viewport.xy, viewport.xy + viewport.zw - 1.0);
        // Kept where it lands behind the water, on the scene unless the
        // pixel itself shows none.
        let shifted_z = textureLoad(scene_depth, vec2<i32>(shifted), 0);
        if at.w > 0.0 && shifted_z > in.clip.z && (shifted_z < 1.0 || scene_z >= 1.0) {
            seen = shifted;
        }
    }
    let seen_world = scene_point(seen, viewport);
    var shown = textureSampleLevel(scene_color, scene_sampler, seen / vec2<f32>(textureDimensions(scene_color)), 0.0);
    // Each metre of water along the ray keeps its share of the light.
    if material.water_absorption.w > 0.0 {
        let depth = distance(seen_world, in.world_position);
        shown = vec4<f32>(shown.rgb * pow(max(material.water_absorption.rgb, vec3<f32>(1e-4)),
            vec3<f32>(depth / material.water_absorption.w)), shown.a);
    }
    water_behind = shown;
#endif
    return surface;
}

#ifdef WATER_SCENE
// The shaded water over what shows through it: where the copy covers, the
// water draws the scene behind (absorbed, refracted) itself and covers the
// pixel; where it does not (the background), it blends as plain water.
fn water_over_scene(color: vec4<f32>) -> vec4<f32> {
    let covered = water_behind.a;
    let alpha = color.a + (1.0 - color.a) * covered;
    let rgb = (color.rgb * color.a + (1.0 - color.a) * water_behind.rgb) / max(alpha, 1e-4);
    return vec4<f32>(rgb, alpha);
}
#endif
#endif

// `opaque`: the part draws in an opaque pass, so decals lay over it.
fn world_color(in: VsOut, front: bool, masked: bool, opaque: bool) -> vec4<f32> {
    var surface = standard_surface(in, front);
#ifdef WATER
    surface = water_surface(in, surface);
#endif
#ifdef MASK
    if masked && surface.base.a < material.alpha_cutoff {
        discard;
    }
#endif
#ifndef UNLIT
#ifndef WATER
    // Screen-space occlusion scales the ambient and hemisphere light with
    // the occlusion map, through the same term.
    let screen_occlusion = textureSample(ambient_occlusion_map, ambient_occlusion_sampler,
        in.clip.xy * ambient_occlusion.params.yz).r;
    surface.occlusion *= mix(1.0, screen_occlusion, ambient_occlusion.params.x);
    // Rain wets what stands under the open sky (`Frame.weather`), unless
    // the material keeps dry, and snow lies on what faces up there.
#ifdef SCATTER
    let pales = material.weather.y > 0.5;
    let copy_open = in.snow_open;
#else
    let pales = false;
    let copy_open = 0.0;
#endif
    surface = weathered(surface, material.weather.x < 0.5, pales, copy_open);
#ifdef DECALS
    // Decals over the opaque world, after the weather, so tracks show in
    // the snow. A scatter run's copies are vegetation; an animated mesh's
    // part has no fade and its fade's x below 0 (render-wgpu
    // `tables::ANIMATED_PART_FADE`).
    if opaque {
#ifdef SCATTER
        surface = decaled(surface, true, false);
#else
        surface = decaled(surface, false, parts[in.part].fade.x < 0.0);
#endif
    }
#endif
#endif
#endif
#ifdef PRODUCT_SHADER
    let shaded = shade(surface);
#else
    let shaded = standard_shade(surface);
#endif
#ifdef WATER_SCENE
    return water_over_scene(shaded);
#else
    return shaded;
#endif
}

struct BlendedOut {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    // Blended over the opaque motion by the colour's alpha (water keeps
    // what lies under it: its target writes nothing).
    @location(1) motion: vec4<f32>,
#endif
};

// Blended passes: the colour and its alpha, blended over the world.
@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> BlendedOut {
    var out: BlendedOut;
    out.color = world_color(in, front, true, false);
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, out.color.a);
#endif
    return out;
}

struct OpaqueOut {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    @location(1) motion: vec4<f32>,
#endif
    @builtin(sample_mask) mask: u32,
};

// Opaque passes cover their pixel whatever the colour's alpha: the finish
// pass composites the world over the background by its coverage. A masked
// material covers its samples by its alpha instead (`mask_coverage`), so its
// cut edges resolve anti-aliased under multisampling.
@fragment
fn fs_world_opaque(in: VsOut, @builtin(front_facing) front: bool) -> OpaqueOut {
#ifdef SCATTER_SELECTED
    // A copy crossfading between levels covers its share of the samples.
    let selected = fade_mask(in.fade, in.clip.xy);
#else
    let selected = 0xffffffffu;
#endif
    var out: OpaqueOut;
#ifdef MASK
    let color = world_color(in, front, false, true);
    out.color = vec4<f32>(color.rgb, 1.0);
    out.mask = mask_coverage(color.a, material.alpha_cutoff) & selected;
#else
    out.color = vec4<f32>(world_color(in, front, true, true).rgb, 1.0);
    out.mask = selected;
#endif
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, 1.0);
#endif
    return out;
}
