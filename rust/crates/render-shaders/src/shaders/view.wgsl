#define_import_path rusty::view

// Group 0 of the world, sprite and ghost passes (`Layouts::frame`); the
// shadow caster pass binds `parts`, `instances`, `shadow_views` and `copies`
// at the same numbers (`Layouts::casters`).

#import rusty::types::{Frame, Part, Light, ShadowView, CloudRegion, ScatterCopy}

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

// The copies of every scatter run; a SCATTER draw's instances index them.
// The shadow caster pass binds them at the same number.
@group(0) @binding(14) var<storage, read> copies: array<ScatterCopy>;

// The part row a SCATTER instance draws with: its run's part, with the
// copy placed, turned and scaled under the part's model.
fn copy_row(scattered: ScatterCopy) -> Part {
    var row = parts[bitcast<u32>(scattered.tint.w)];
    let q = scattered.turn;
    let turn = mat3x3<f32>(
        vec3<f32>(1.0 - 2.0 * (q.y * q.y + q.z * q.z), 2.0 * (q.x * q.y + q.w * q.z), 2.0 * (q.x * q.z - q.w * q.y)),
        vec3<f32>(2.0 * (q.x * q.y - q.w * q.z), 1.0 - 2.0 * (q.x * q.x + q.z * q.z), 2.0 * (q.y * q.z + q.w * q.x)),
        vec3<f32>(2.0 * (q.x * q.z + q.w * q.y), 2.0 * (q.y * q.z - q.w * q.x), 1.0 - 2.0 * (q.x * q.x + q.y * q.y)),
    );
    let scale = scattered.place.w;
    row.model = row.model * mat4x4<f32>(
        vec4<f32>(turn[0] * scale, 0.0),
        vec4<f32>(turn[1] * scale, 0.0),
        vec4<f32>(turn[2] * scale, 0.0),
        vec4<f32>(scattered.place.xyz, 1.0),
    );
    // The inverse transpose of a turn and a uniform scale: the turn over
    // the scale.
    let normal = mat3x3<f32>(row.normal_x.xyz, row.normal_y.xyz, row.normal_z.xyz) * turn * (1.0 / scale);
    row.normal_x = vec4<f32>(normal[0], row.normal_x.w);
    row.normal_y = vec4<f32>(normal[1], row.normal_y.w);
    row.normal_z = vec4<f32>(normal[2], row.normal_z.w);
    return row;
}

// A copy the per-copy selection chose (render-wgpu `scatter_select.rs`):
// its row with the part and scale-up the selection gave it, its tint, and
// its share of the samples (negative: the share a crossfade's incoming level
// takes, from the other end of the samples).
struct SelectedCopy {
    row: Part,
    part: u32,
    tint: vec3<f32>,
    fade: f32,
    // The copy mesh's top (`ScatterCopy.detail.y`).
    top: f32,
};

// The largest scale-up a selection entry carries (`scatter_select.rs`).
const SELECTED_MAX_SCALE: f32 = 4.0;

// A SCATTER_SELECTED instance's copy: an entry of two words in `instances`,
// the copy's row index with its share (7 bits) and whether it comes in
// (top bit), then its part (20 bits) with its scale-up (12 bits).
fn selected_copy(instance: u32) -> SelectedCopy {
    let word = instances[2u * instance];
    let detail = instances[2u * instance + 1u];
    var scattered = copies[word & 0xffffffu];
    let tint = scattered.tint.xyz;
    scattered.tint.w = bitcast<f32>(detail & 0xfffffu);
    scattered.place.w *= 1.0 + f32(detail >> 20u) / 4095.0 * (SELECTED_MAX_SCALE - 1.0);
    let share = f32((word >> 24u) & 0x7fu) / 127.0;
    return SelectedCopy(copy_row(scattered), detail & 0xfffffu, tint, select(share, -share, (word >> 31u) != 0u), scattered.detail.y);
}

// Impostor views (render-wgpu `impostor.rs`): a direction at or above the
// horizon of the mesh's own space to the unit square, hemi-octahedrally, and
// a square's point back to its direction.
fn hemi_octahedral(direction: vec3<f32>) -> vec2<f32> {
    let d = vec3<f32>(direction.x, max(direction.y, 0.0), direction.z);
    let p = d.xz / max(abs(d.x) + d.y + abs(d.z), 1e-6);
    return vec2<f32>(p.x + p.y, p.x - p.y) * 0.5 + 0.5;
}

fn hemi_octahedral_direction(uv: vec2<f32>) -> vec3<f32> {
    let f = uv * 2.0 - 1.0;
    let p = vec2<f32>(f.x + f.y, f.x - f.y) * 0.5;
    return normalize(vec3<f32>(p.x, 1.0 - abs(p.x) - abs(p.y), p.y));
}

// The right of a view looking back along `view`, in the mesh's own space
// (up is +y, or +z looking straight down).
fn impostor_right(view: vec3<f32>) -> vec3<f32> {
    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(view.y) > 0.999);
    return normalize(cross(up, view));
}

// A copy's impostor card at a quad corner: the baked view nearest the
// camera's direction in the copy's own space, faced as it was baked, around
// the mesh's bounding sphere (`shape`: centre, radius) in a grid of
// `frames` views a side.
struct ImpostorCard {
    world: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    turn: mat3x3<f32>,
};

fn impostor_card(row: Part, corner: vec3<f32>, shape: vec4<f32>, frames: f32) -> ImpostorCard {
    let scale = length(row.model[0].xyz);
    let turn = mat3x3<f32>(row.model[0].xyz / scale, row.model[1].xyz / scale, row.model[2].xyz / scale);
    let center = (row.model * vec4<f32>(shape.xyz, 1.0)).xyz;
    let toward = transpose(turn) * normalize(frame.camera.xyz - center);
    let grid = max(frames, 1.0);
    let cell = clamp(floor(hemi_octahedral(toward) * grid), vec2<f32>(0.0), vec2<f32>(grid - 1.0));
    let view = hemi_octahedral_direction((cell + 0.5) / grid);
    let right = impostor_right(view);
    let up = cross(view, right);
    let offset = corner.xy * 2.0;
    var card: ImpostorCard;
    card.world = center + turn * (right * offset.x + up * offset.y) * shape.w * scale;
    card.normal = turn * view;
    card.uv = (cell + vec2<f32>(offset.x * 0.5 + 0.5, 0.5 - offset.y * 0.5)) / grid;
    card.turn = turn;
    return card;
}

// The samples a crossfading copy covers at a pixel: `fade` of them (a
// negative fade, the incoming level, takes them from the other end), the
// remainder spread over pixels by an ordered dither, so two levels fading
// opposite ways cover each sample once between them.
fn fade_mask(fade: f32, pixel: vec2<f32>) -> u32 {
    let samples = max(frame.counts.z, 1u);
    let all = (1u << samples) - 1u;
    let p = vec2<u32>(pixel) & vec2<u32>(3u);
    let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let rotation = (bayer[p.y * 4u + p.x] + 0.5) / 16.0;
    let count = f32(samples);
    if fade >= 0.0 {
        let covered = min(u32(floor(fade * count + rotation)), samples);
        return (1u << covered) - 1u;
    }
    let covered = samples - min(u32(floor((1.0 + fade) * count + rotation)), samples);
    return all ^ ((1u << (samples - covered)) - 1u);
}

// The clip position of world point `p`, projected from its offset from the
// camera (`Frame.relative_view_proj`). Projected whole, a point kilometres
// from the origin rounds its clip depth and its w apart by about 1e-4 each,
// and the division of one by the other turns that into centimetres of depth
// at 20 m: a surface millimetres behind another shows through it in bands.
// The offset is small where depth needs its precision, near the camera.
fn projected(p: vec3<f32>) -> vec4<f32> {
    return frame.relative_view_proj * vec4<f32>(p - frame.camera.xyz, 1.0);
}

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
