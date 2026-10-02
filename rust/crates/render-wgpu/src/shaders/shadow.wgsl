// Shadow caster pass: parts drawn into one shadow layer's depth. Only MASK
// materials have a fragment stage, discarding below their cutoff (voxel
// surfaces remapped and triplanar planes blended as in the world pass).

#import rusty::types::texture_space_position
#import rusty::view::{parts, instances, shadow_views}
#import rusty::material::{material, albedo, albedo_sampler}
#import rusty::surface::{transform_uv, voxel_uv, triplanar_uvs, triplanar_weights}

struct Layer {
    index: u32,
};

@group(2) @binding(0) var<uniform> layer: Layer;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) part: u32,
    @location(2) alpha: f32,
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
    var out: VsOut;
    out.clip = shadow_views[layer.index] * parts[part].model * vec4<f32>(position, 1.0);
    out.uv = uv;
    out.part = part;
    out.alpha = color.a;
#ifdef TRIPLANAR
    out.texture_position = texture_space_position(parts[part], position);
    out.texture_normal = normal;
#endif
    return out;
}

#ifdef MASK
fn base_alpha(surface_uv: vec2<f32>) -> f32 {
#ifdef VOXEL_SURFACE
    let uv = voxel_uv(surface_uv, material.tile, material.sample_rect);
#else
    let uv = surface_uv;
#endif
    return textureSampleLevel(albedo, albedo_sampler, transform_uv(material.base_uv_u, material.base_uv_v, uv), 0.0).a;
}

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
    if alpha < material.alpha_cutoff {
        discard;
    }
}
#endif
