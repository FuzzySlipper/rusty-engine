// The standard shader's world pass: parts drawn with their material and the
// pass's light rows. Each material compiles the features it uses
// (`shaders.rs` `Features`): UNLIT, MASK, VOXEL_SURFACE, NORMAL_MAP,
// EMISSIVE_MAP, OCCLUSION_MAP.

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
#import rusty::surface::{transform_uv, voxel_uv, perturb_normal}
#import rusty::lighting::standard_radiance

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
    @location(4) color: vec4<f32>,
};

@vertex
fn vs_world(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
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
    return out;
}

@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let row = parts[in.part];
#ifdef VOXEL_SURFACE
    let uv = voxel_uv(in.uv, material.tile, material.sample_rect);
#else
    let uv = in.uv;
#endif
    let base = row.color * in.color
        * textureSample(albedo, albedo_sampler, transform_uv(material.base_uv_u, material.base_uv_v, uv));
#ifdef UNLIT
#ifdef MASK
    if base.a < material.alpha_cutoff {
        discard;
    }
#endif
    return base;
#else
    // Every map is sampled, and every derivative taken, before the discard.
#ifdef EMISSIVE_MAP
    let emissive = textureSample(emissive_map, emissive_sampler,
        transform_uv(material.emissive_uv_u, material.emissive_uv_v, in.uv)).rgb;
#else
    let emissive = vec3<f32>(1.0);
#endif
#ifdef OCCLUSION_MAP
    let occlusion_sample = textureSample(occlusion_map, occlusion_sampler,
        transform_uv(material.occlusion_uv_u, material.occlusion_uv_v, in.uv)).r;
    let occlusion = 1.0 + material.occlusion.x * (occlusion_sample - 1.0);
#else
    let occlusion = 1.0;
#endif
    let geometric = normalize(in.normal);
#ifdef NORMAL_MAP
    let normal_uv = transform_uv(material.normal_uv_u, material.normal_uv_v, in.uv);
    let normal_sample = textureSample(normal_map, normal_sampler, normal_uv).rgb;
    var normal = perturb_normal(geometric, in.world_position, normal_uv, normal_sample, material.normal_scale);
#else
    var normal = geometric;
#endif
    var facing = geometric;
    if !front {
        normal = -normal;
        facing = -facing;
    }
    let normal_change = max(abs(dpdx(facing)), abs(dpdy(facing)));
    let geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);
#ifdef MASK
    if base.a < material.alpha_cutoff {
        discard;
    }
#endif
    let roughness = min(max(material.roughness, 0.0525) + geometry_roughness, 1.0);
    let radiance = standard_radiance(base.rgb, normal, in.world_position, roughness, material.metalness, occlusion)
        + row.emission.rgb * emissive;
    return vec4<f32>(radiance, base.a);
#endif
}
