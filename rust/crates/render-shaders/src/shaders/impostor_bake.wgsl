// An impostor's views baked (render-wgpu `impostor.rs`): a static mesh drawn
// orthographically from each of a hemi-octahedral grid of directions into
// its cell of two atlases, its base colour with coverage and its normals in
// the mesh's own space, each mesh group with its own material's base texture,
// colour and alpha cutoff.

#import rusty::material::{material, albedo, albedo_sampler}
#import rusty::surface::transform_uv

struct Bake {
    view_proj: mat4x4<f32>,
    // The group's material colour (linear rgba).
    color: vec4<f32>,
    // x: the alpha cutoff (0: opaque).
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> bake: Bake;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_bake(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.clip = bake.view_proj * vec4<f32>(position, 1.0);
    out.uv = uv;
    out.normal = normal;
    out.color = color;
    return out;
}

struct BakeOut {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_bake(in: VsOut, @builtin(front_facing) front: bool) -> BakeOut {
    let texel = textureSample(albedo, albedo_sampler, transform_uv(material.base_uv_u, material.base_uv_v, in.uv));
    let base = texel * bake.color * vec4<f32>(in.color.rgb, 1.0);
    if bake.params.x > 0.0 && base.a < bake.params.x {
        discard;
    }
    let facing = select(-1.0, 1.0, front);
    let normal = normalize(in.normal) * facing;
    return BakeOut(vec4<f32>(base.rgb, 1.0), vec4<f32>(normal * 0.5 + 0.5, 1.0));
}
