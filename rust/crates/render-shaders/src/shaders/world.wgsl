// The standard shader's world pass: parts drawn with their material and the
// pass's light rows, finished by exposure, tone mapping and fog
// (`rusty::finish`). Each material compiles the features it uses
// (`lib.rs` `Features`): UNLIT, MASK, VOXEL_SURFACE, NORMAL_MAP,
// EMISSIVE_MAP, OCCLUSION_MAP, TRIPLANAR. A product shader (PRODUCT_SHADER)
// shades the surface in place of `rusty::shade`.

#import rusty::types::texture_space_position
#import rusty::view::{frame, parts, instances}
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
}
#import rusty::surface::{
    transform_uv,
    voxel_uv,
    voxel_lod,
    perturb_normal,
    tangent_normal,
    triplanar_uvs,
    triplanar_weights,
    triplanar_normal,
}
#import rusty::types::Surface
#import rusty::shade::standard_shade
#ifdef PRODUCT_SHADER
#import rusty::product::shade
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
    let world = row.model * vec4<f32>(position, 1.0);
    var out: VsOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz;
    out.normal = mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * normal;
    out.uv = uv;
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

// The standard surface stages: what the material's maps, the part and the
// vertex make of this fragment. Every map is sampled, and every derivative
// taken, here, before the shade stage may discard.
fn standard_surface(in: VsOut, front: bool) -> Surface {
    let row = parts[in.part];
#ifdef TRIPLANAR
    let planes = triplanar_uvs(in.texture_position, in.texture_normal);
    let weights = triplanar_weights(in.texture_normal, material.factors.y);
    let texture_color = base_texture(planes[0]) * weights.x + base_texture(planes[1]) * weights.y
        + base_texture(planes[2]) * weights.z;
#else
#ifdef VOXEL_SURFACE
    let texture_color = base_texture(in.uv);
#else
    let texture_color = base_texture(slot_uv(in, material.tex_coords.x));
#endif
#endif
    var surface: Surface;
    surface.tint = row.color * in.color;
    surface.base = surface.tint * texture_color;
    surface.world_position = in.world_position;
    surface.uv = in.uv;
    surface.roughness = 1.0;
    surface.metalness = 0.0;
    surface.occlusion = 1.0;
    surface.emission = vec3<f32>(0.0);
    let geometric = normalize(in.normal);
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
    surface.occlusion = 1.0 + material.factors.x * (occlusion_sample - 1.0);
#endif
#ifdef NORMAL_MAP
#ifdef TRIPLANAR
    // Blended in texture space, then into the world by the normal matrix.
    let samples = array<vec3<f32>, 3>(normal_texture(planes[0]), normal_texture(planes[1]),
        normal_texture(planes[2]));
    let mapped = triplanar_normal(normalize(in.texture_normal), samples, weights, material.normal_scale);
    var normal = normalize(mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * mapped);
#else
#ifdef VOXEL_SURFACE
    // Through the same tiling as the base texture; the frame comes from the
    // continuous tile coordinate, which does not jump at tile seams.
    var normal = perturb_normal(geometric, in.world_position, (in.uv - material.tile.zw) / material.tile.xy,
        normal_texture(in.uv), material.normal_scale);
#else
    let normal_uv = transform_uv(material.normal_uv_u, material.normal_uv_v, slot_uv(in, material.tex_coords.z));
    let normal_sample = textureSample(normal_map, normal_sampler, normal_uv).rgb;
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
    surface.roughness = min(max(material.roughness, 0.0525) + geometry_roughness, 1.0);
    surface.metalness = material.metalness;
#endif
    return surface;
}

@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let surface = standard_surface(in, front);
#ifdef MASK
    if surface.base.a < material.alpha_cutoff {
        discard;
    }
#endif
#ifdef PRODUCT_SHADER
    return shade(surface);
#else
    return standard_shade(surface);
#endif
}
