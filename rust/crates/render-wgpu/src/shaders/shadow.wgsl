// Shadow caster pass: parts drawn into one shadow layer's depth. Only MASK
// materials have a fragment stage, discarding below their cutoff (voxel
// surfaces remapped as in the world pass).

#import rusty::view::{parts, instances, shadow_views}
#import rusty::material::{material, albedo, albedo_sampler}
#import rusty::surface::{transform_uv, voxel_uv}

struct Layer {
    index: u32,
};

@group(2) @binding(0) var<uniform> layer: Layer;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) part: u32,
    @location(2) alpha: f32,
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
    return out;
}

#ifdef MASK
@fragment
fn fs_shadow(in: VsOut) {
#ifdef VOXEL_SURFACE
    let uv = voxel_uv(in.uv, material.tile, material.sample_rect);
#else
    let uv = in.uv;
#endif
    let base_uv = transform_uv(material.base_uv_u, material.base_uv_v, uv);
    let alpha = parts[in.part].color.a * in.alpha * textureSampleLevel(albedo, albedo_sampler, base_uv, 0.0).a;
    if alpha < material.alpha_cutoff {
        discard;
    }
}
#endif
