// Shadow caster pass: parts drawn into one shadow layer's depth, their
// vertices placed as the world pass places them (the wind, a product's
// `displace`). Only MASK materials and product shaders with a caster stage
// have a fragment stage: MASK discards below the cutoff (voxel surfaces
// remapped and triplanar planes blended as in the world pass), then a
// product's `cast_shadow` may discard.

#import rusty::types::{texture_space_position, Caster, Vertex}
#import rusty::view::{parts, instances, shadow_views}
#import rusty::material::{material, albedo, albedo_sampler}
#import rusty::surface::{transform_uv, voxel_uv, triplanar_uvs, triplanar_weights, hex_tiles, hex_texture}
#ifdef WIND
#import rusty::wind::wind_displace
#endif
#ifdef PRODUCT_CASTS
#import rusty::product::cast_shadow
#endif
#ifdef PRODUCT_DISPLACES
#import rusty::product::displace
#endif

struct Layer {
    index: u32,
};

@group(2) @binding(0) var<uniform> layer: Layer;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) part: u32,
    @location(2) alpha: f32,
    @location(5) world_position: vec3<f32>,
#ifdef TRIPLANAR
    @location(3) texture_position: vec3<f32>,
    @location(4) texture_normal: vec3<f32>,
#endif
};

@vertex
fn vs_shadow(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @builtin(instance_index) instance: u32,
) -> VsOut {
    let part = instances[instance];
    let row = parts[part];
    var world = (row.model * vec4<f32>(position, 1.0)).xyz;
#ifdef LAYER_WEIGHTS
    // The vertex colour holds layer weights.
    let alpha = 1.0;
#else
    let alpha = color.a;
#endif
#ifdef WIND
    world = wind_displace(world, row.model[3].xyz, alpha);
#endif
#ifdef PRODUCT_DISPLACES
    var vertex: Vertex;
    vertex.world_position = world;
    vertex.world_normal = mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * normal;
    vertex.position = position;
    vertex.normal = normal;
    vertex.uv = uv;
    vertex.color = color;
    vertex.origin = row.model[3].xyz;
    vertex.part = part;
    world = displace(vertex);
#endif
    var out: VsOut;
    out.clip = shadow_views[layer.index].view_proj * vec4<f32>(world, 1.0);
    out.world_position = world;
    out.uv = uv;
    out.part = part;
    out.alpha = alpha;
#ifdef TRIPLANAR
    out.texture_position = texture_space_position(parts[part], position);
    out.texture_normal = normal;
#endif
    return out;
}

fn base_alpha(surface_uv: vec2<f32>) -> f32 {
#ifdef VOXEL_SURFACE
    let uv = voxel_uv(surface_uv, material.tile, material.sample_rect);
#else
    let uv = surface_uv;
#endif
#ifdef STOCHASTIC_TILING
    return hex_texture(albedo, albedo_sampler, hex_tiles(transform_uv(material.base_uv_u, material.base_uv_v, uv)),
        material.factors.z).color.a;
#else
    return textureSampleLevel(albedo, albedo_sampler, transform_uv(material.base_uv_u, material.base_uv_v, uv), 0.0).a;
#endif
}

#ifdef CASTER_FRAGMENT
@fragment
fn fs_shadow(in: VsOut) {
#ifdef TRIPLANAR
    let planes = triplanar_uvs(in.texture_position, in.texture_normal);
    let weights = triplanar_weights(in.texture_normal, material.factors.y);
    let texture_alpha = dot(vec3<f32>(base_alpha(planes[0]), base_alpha(planes[1]), base_alpha(planes[2])), weights);
#else
    let texture_alpha = base_alpha(in.uv);
#endif
    let alpha = parts[in.part].color.a * in.alpha * texture_alpha;
#ifdef MASK
    if alpha < material.alpha_cutoff {
        discard;
    }
#endif
#ifdef PRODUCT_CASTS
    var caster: Caster;
    caster.uv = in.uv;
    caster.world_position = in.world_position;
    caster.alpha = alpha;
    cast_shadow(caster);
#endif
}
#endif
