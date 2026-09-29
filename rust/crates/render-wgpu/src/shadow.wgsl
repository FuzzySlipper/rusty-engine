// Shadow caster pass: parts drawn into one shadow layer's depth, with mask
// materials discarding below their cutoff (voxel surfaces remapped as in
// world.wgsl).

struct Part {
    model: mat4x4<f32>,
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
    color: vec4<f32>,
    emission: vec4<f32>,
};

struct MaterialUniform {
    roughness: f32,
    alpha_cutoff: f32,
    flags: u32,
    pad: u32,
    tile: vec4<f32>,
    sample_rect: vec4<f32>,
};

struct Layer {
    index: u32,
};

const FLAG_MASK: u32 = 2u;
const FLAG_VOXEL_SURFACE: u32 = 4u;

@group(0) @binding(0) var<storage, read> parts: array<Part>;
@group(0) @binding(1) var<storage, read> instances: array<u32>;
@group(0) @binding(2) var<storage, read> shadow_views: array<mat4x4<f32>>;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var albedo: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;
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

@fragment
fn fs_shadow(in: VsOut) {
    if (material.flags & FLAG_MASK) == 0u {
        return;
    }
    var uv = in.uv;
    if (material.flags & FLAG_VOXEL_SURFACE) != 0u {
        let repeated = fract((uv - material.tile.zw) / material.tile.xy);
        uv = mix(material.sample_rect.xy, material.sample_rect.zw, repeated);
    }
    let alpha = parts[in.part].color.a * in.alpha * textureSampleLevel(albedo, albedo_sampler, uv, 0.0).a;
    if alpha < material.alpha_cutoff {
        discard;
    }
}
