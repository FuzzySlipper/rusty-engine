// Splat characters casting into a shadow layer (render-wgpu `splats.rs`,
// #10815), drawn in the shadow pass after its mesh and sprite casters, its
// groups those of `shadow.wgsl`'s casters: 0 the casters' (`shadow_views`),
// 2 the layer, and 1 here the run buffers.
//
// - `vs_splat_caster`, `fs_splat_caster`: the instance's Gaussians as
//   `splats.wgsl` `cs_project` projected them for the layer's light, unsorted
//   (depth tests them), every slot drawn and the culled ones dropped, each
//   quad cut where the Gaussian's alpha falls under `CASTER_ALPHA`.
// - `vs_ellipsoid_caster`: a unit sphere per row, placed by the row's matrix
//   (an ellipsoid around the Gaussians one joint carries, posed with it).

#import rusty::view::shadow_views

struct CasterLayer {
    index: u32,
};

@group(2) @binding(0) var<uniform> caster_layer: CasterLayer;

// `splats.wgsl` `Projected`.
struct Projected {
    centre: vec3<f32>,
    color: u32,
    axes: vec4<f32>,
    motion: vec4<f32>,
};

@group(1) @binding(20) var<storage, read> drawn: array<Projected>;
// The projection's sort keys, slot by slot: `CULLED` for a Gaussian it
// dropped.
@group(1) @binding(21) var<storage, read> keys: array<u32>;

const CULLED: u32 = 0xffffu;
// A Gaussian casts where its alpha reaches this, as the web SDK's splat
// shadows do.
const CASTER_ALPHA: f32 = 0.3;
const EXP4: f32 = 0.018315639;

struct CasterVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
};

@vertex
fn vs_splat_caster(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) slot: u32,
) -> CasterVarying {
    var out: CasterVarying;
    if keys[slot] == CULLED {
        // Off the layer: a point outside clip space draws nothing.
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let splat = drawn[slot];
    let opacity = unpack4x8unorm(splat.color).a;
    // The quad shrinks to where the falloff can still reach the cut:
    // opacity · (e^−4A − e^−4) / (1 − e^−4) ≥ CASTER_ALPHA within
    // |uv|² ≤ A. A Gaussian too faint to reach it casts nothing.
    let falloff = CASTER_ALPHA / max(opacity, 1e-6) * (1.0 - EXP4) + EXP4;
    if falloff >= 1.0 {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let reach = sqrt(min(-log(falloff) * 0.25, 1.0));
    let corner = (vec2<f32>(f32(vertex & 1u), f32((vertex >> 1u) & 1u)) * 2.0 - 1.0) * reach;
    out.position = vec4<f32>(
        splat.centre.xy + corner.x * splat.axes.xy + corner.y * splat.axes.zw,
        splat.centre.z,
        1.0,
    );
    out.uv = corner;
    out.opacity = opacity;
    return out;
}

@fragment
fn fs_splat_caster(in: CasterVarying) {
    let a = dot(in.uv, in.uv);
    let alpha = (exp(-4.0 * a) - EXP4) / (1.0 - EXP4) * in.opacity;
    if a > 1.0 || alpha < CASTER_ALPHA {
        discard;
    }
}

struct EllipsoidIn {
    @location(0) position: vec3<f32>,
    @location(3) column0: vec4<f32>,
    @location(4) column1: vec4<f32>,
    @location(5) column2: vec4<f32>,
    @location(6) column3: vec4<f32>,
};

@vertex
fn vs_ellipsoid_caster(in: EllipsoidIn) -> @builtin(position) vec4<f32> {
    let place = mat4x4<f32>(in.column0, in.column1, in.column2, in.column3);
    return shadow_views[caster_layer.index].view_proj * (place * vec4<f32>(in.position, 1.0));
}
