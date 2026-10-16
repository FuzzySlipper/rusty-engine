#define_import_path rusty::view

// Group 0 of the world, sprite and ghost passes (`Layouts::frame`); the
// shadow caster pass binds `parts`, `instances` and `shadow_views` at the
// same numbers (`Layouts::casters`).

#import rusty::types::{Frame, Part, Light, ShadowView, CloudRegion}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> lights: array<Light>;
// Part ids in draw order; a draw's instances index this list.
@group(0) @binding(3) var<storage, read> instances: array<u32>;
@group(0) @binding(4) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;
@group(0) @binding(6) var<storage, read> shadow_views: array<ShadowView>;
// Light clusters (`light_clusters.wgsl`): per cluster a count and light row
// indices, then the global list; read when `frame.cluster_grid.w` is 1.
@group(0) @binding(7) var<storage, read> clusters: array<u32>;
// The sky's light (render-wgpu `sky_light.rs`): the background's radiance
// prefiltered by roughness down the cube's mips, and its irradiance as nine
// cosine-convolved spherical-harmonics coefficients (`Frame.sky_light`).
@group(0) @binding(8) var sky_specular: texture_cube<f32>;
@group(0) @binding(9) var sky_sampler: sampler;
@group(0) @binding(10) var<storage, read> sky_irradiance: array<vec4<f32>, 9>;
// The indirect light volume (render-wgpu `probes`): a 3D texture of each
// probe's four L1 irradiance coefficients (Y0, Y1 by y, z, x), the red, green
// and blue channels' slabs following each other along its depth (one slab
// in the compact encoding), sampled trilinearly, then (not in the compact
// encoding) a slab of where walls cross each probe's cell
// (`probe_cell_side`); `Frame.probes` and
// `Frame.probe_grid` place it.
@group(0) @binding(11) var probes: texture_3d<f32>;
@group(0) @binding(12) var probes_sampler: sampler;
// The cloud regions (`rusty::clouds`), `frame.cloud_drift.z` of them.
@group(0) @binding(13) var<storage, read> cloud_regions: array<CloudRegion>;

// Where `p` stands in the world: itself, or in a backdrop pass
// (`Frame.backdrop`) the world point the backdrop's `p` stands for, so fog
// and the cloud layer's shade reach the backdrop at its world-equivalent
// distance and place.
fn world_equivalent(p: vec3<f32>) -> vec3<f32> {
    if frame.backdrop.w > 0.0 {
        return frame.backdrop.xyz + p * frame.backdrop.w;
    }
    return p;
}

// Coverage for a masked surface's pixel (MASK, `fs_world_opaque`): the
// alpha sharpened about the cutoff over one pixel's change of it, as a share
// of the view's samples, so a leaf's edge resolves anti-aliased where the
// cutoff alone would alias it. The surface writes alpha 1 to the samples it
// covers: the finish pass reads the target's alpha as coverage. Call before
// any discard: it takes a derivative.
// A part's distance fade (`Part::fade`): its vertex drawn toward the part's
// origin as the camera nears the fade's end (x to y), so a scattered copy
// sinks into its spot rather than popping out, and grown out of it from the
// near band's start to its end (z to w; whole from z when they are equal),
// where another patch hands the copy over.
fn fade_toward_origin(row: Part, world: vec3<f32>) -> vec3<f32> {
    if (row.fade.y <= 0.0 && row.fade.w <= 0.0) {
        return world;
    }
    let origin = row.model[3].xyz;
    let away = distance(origin, frame.camera.xyz);
    var keep = 1.0;
    if (row.fade.y > 0.0) {
        keep = 1.0 - smoothstep(row.fade.x, row.fade.y, away);
    }
    if (row.fade.w > 0.0) {
        if (row.fade.w > row.fade.z) {
            keep *= smoothstep(row.fade.z, row.fade.w, away);
        } else {
            keep *= step(row.fade.z, away);
        }
    }
    return origin + (world - origin) * keep;
}

// A morphing part's share of its targets at model-space position `p`
// (MORPH, a heightfield tile): `Part::fade` is then the range (x, y) and the
// focus (z, w: x and z in the part's own space). 0 within x of the focus, 1
// from y, linear between, by the larger horizontal distance; 0 without a
// focus (y 0).
fn morph_share(row: Part, p: vec3<f32>) -> f32 {
    if (row.fade.y <= 0.0) {
        return 0.0;
    }
    let reach = max(abs(p.x - row.fade.z), abs(p.z - row.fade.w));
    return clamp((reach - row.fade.x) / (row.fade.y - row.fade.x), 0.0, 1.0);
}

// A morph target's normal from its x and z (the positive remainder is y).
fn morph_normal(goal: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(goal.y, sqrt(max(1.0 - goal.y * goal.y - goal.z * goal.z, 0.0)), goal.z);
}

fn mask_coverage(alpha: f32, cutoff: f32) -> u32 {
    let sharpened = clamp((alpha - cutoff) / max(fwidth(alpha), 1e-5) + 0.5, 0.0, 1.0);
    let samples = max(frame.counts.z, 1u);
    let covered = u32(round(sharpened * f32(samples)));
    return (1u << covered) - 1u;
}
