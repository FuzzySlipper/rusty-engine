// The standard shader's world pass: parts drawn with their material and the
// pass's light rows into the view's linear HDR target, which the finish pass
// finishes (`rusty::finish`). Each material compiles the features it uses
// (`lib.rs` `Features`): UNLIT, MASK, VOXEL_SURFACE, NORMAL_MAP,
// EMISSIVE_MAP, OCCLUSION_MAP, TRIPLANAR, TERRAIN_LAYERS, STOCHASTIC_TILING,
// FLAT_SHADING, WIND, WATER; and the mesh's
// streams: VERTEX_TANGENTS, LAYER_WEIGHTS, VERTEX_OCCLUSION. A product shader (PRODUCT_SHADER)
// shades the surface in place of `rusty::shade`, and may place its vertices
// (PRODUCT_DISPLACES).

#import rusty::types::{texture_space_position, Vertex}
#import rusty::view::{frame, parts, instances, mask_coverage, fade_toward_origin}
#ifdef WIND
#import rusty::wind::wind_displace
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
#ifdef PRODUCT_SHADER
#import rusty::product::shade
#endif
#ifdef PRODUCT_DISPLACES
#import rusty::product::displace
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
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
    @location(5) uv1: vec2<f32>,
#endif
    @builtin(instance_index) instance: u32,
) -> VsOut {
    let part = instances[instance];
    let row = parts[part];
    var world = fade_toward_origin(row, (row.model * vec4<f32>(position, 1.0)).xyz);
    let world_normal = mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * normal;
#ifdef WIND
#ifdef LAYER_WEIGHTS
    world = wind_displace(world, row.model[3].xyz, 1.0);
#else
#ifdef VERTEX_OCCLUSION
    world = wind_displace(world, row.model[3].xyz, 1.0);
#else
    world = wind_displace(world, row.model[3].xyz, color.a);
#endif
#endif
#endif
#ifdef PRODUCT_DISPLACES
    var vertex: Vertex;
    vertex.world_position = world;
    vertex.world_normal = world_normal;
    vertex.position = position;
    vertex.normal = normal;
    vertex.uv = uv;
    vertex.color = color;
    vertex.origin = row.model[3].xyz;
    vertex.part = part;
    world = displace(vertex);
#endif
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_position = world;
    out.normal = world_normal;
    out.uv = uv;
#ifdef WATER
    out.clip_position = out.clip;
#endif
    out.part = part;
    out.color = color;
#ifdef VERTEX_TANGENTS
    let model = mat3x3<f32>(row.model[0].xyz, row.model[1].xyz, row.model[2].xyz);
    out.tangent = vec4<f32>(model * tangent.xyz, tangent.w * sign(determinant(model)));
    out.uv1 = uv1;
#endif
#ifdef TRIPLANAR
    out.texture_position = texture_space_position(row, position);
    out.texture_normal = normal;
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
    var normal = normalize(mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * mapped);
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
// The water surface (WATER): the standard surface tinted by the depth of
// the scene behind it along the view ray, foam where that scene lies within
// the shoreline width below the surface, the normal rippled by two
// scrolling normal maps (or procedural ripples when the material has none),
// and the alpha raised toward the Fresnel reflectance at grazing angles.
fn water_surface(in: VsOut, standard: Surface) -> Surface {
    var surface = standard;
    let flags = u32(material.factors.w + 0.5);
    let time = frame.time.x;
    let wave_uv = in.world_position.xz / max(material.water_params.w, 1e-3);
    // The normal: the maps' slopes added in the water's own frame (its
    // surface lies along the ground), scaled by the normal scale.
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
    surface.normal = normalize(surface.normal + vec3<f32>(slope.x, 0.0, slope.y) * material.normal_scale);
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
    return surface;
}
#endif

fn world_color(in: VsOut, front: bool, masked: bool) -> vec4<f32> {
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
#endif
#endif
#ifdef PRODUCT_SHADER
    return shade(surface);
#else
    return standard_shade(surface);
#endif
}

// Blended passes: the colour and its alpha, blended over the world.
@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    return world_color(in, front, true);
}

struct OpaqueOut {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

// Opaque passes cover their pixel whatever the colour's alpha: the finish
// pass composites the world over the background by its coverage. A masked
// material covers its samples by its alpha instead (`mask_coverage`), so its
// cut edges resolve anti-aliased under multisampling.
@fragment
fn fs_world_opaque(in: VsOut, @builtin(front_facing) front: bool) -> OpaqueOut {
#ifdef MASK
    let color = world_color(in, front, false);
    return OpaqueOut(vec4<f32>(color.rgb, 1.0), mask_coverage(color.a, material.alpha_cutoff));
#else
    return OpaqueOut(vec4<f32>(world_color(in, front, true).rgb, 1.0), 0xffffffffu);
#endif
}
