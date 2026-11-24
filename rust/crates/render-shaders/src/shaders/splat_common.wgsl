#define_import_path rusty::splat_common

#import rusty::hiz_common::{Hiz, hiz_level_size, hiz_index}

// What a splat run's projection shares (render-wgpu `splats.rs`): a set's
// parameters, the run's sort keys and draw, the skinning palettes, a set's
// skin and surface streams, and the skinning and group-tint helpers. A run is
// Gaussians (`splats.wgsl` `cs_project`) or one card asset's cards
// (`splat_cards.wgsl` `cs_project_card`); each declares its own resident
// records (group 1, binding 0) and projected records (group 0, binding 1),
// and both write the same 16-bit view-depth keys the radix sort orders.

// One instance's set of splats projected for a view.
struct Project {
    // Instance space to view space (camera at the origin looking down −z).
    model_view: mat4x4<f32>,
    // Instance space to the pass's clip space (jittered as the pass draws).
    clip_from_model: mat4x4<f32>,
    // The previous frame's instance space to this frame's jittered target
    // pixels, homogeneous (`rusty::motion` `previous_pixel`).
    previous_pixels: mat4x4<f32>,
    // x, y: focal lengths in render pixels; z, w: the viewport's size.
    lens: vec4<f32>,
    // x: the run's nearest view depth; y: 1 / its depth span; z: 1 for an
    // orthographic projection; w: 1 in a lit run, which projects normals.
    depth: vec4<f32>,
    // x: the set's first slot in the run; y: its splats; z: 1 for the
    // run's first set, which writes the draw's vertex count; w: 1 when a
    // lit run draws the set's albedo for its colour.
    range: vec4<u32>,
    // x: the set's first row in `palettes`; y: 1 when it is skinned; z: the
    // first row of its previous palette; w: 1 when the view draws motion.
    skin: vec4<u32>,
    // rgb: the colour multiplier in linear light.
    tint: vec4<f32>,
    // NDC to the target's pixels as the pass rasterises them: xy + ndc·zw.
    pixels: vec4<f32>,
    // The instance's tinted groups: x and y the groups a byte each, four to
    // a word, lowest first; z how many (at most 8).
    groups: vec4<u32>,
    // Each tinted group's target, sRGB-encoded, in rgb and its weight in w.
    group_tints: array<vec4<f32>, 8>,
    // The view's post-opaque farthest-depth pyramid (`hiz`): x the side in
    // target pixels of a level-0 texel's block when the set's splats are
    // tested against it, 0 when not; y its levels; zw the target pixel its
    // level 0 starts at.
    occlusion: vec4<f32>,
};

// The indirect draw of a run's layer: 4 vertices per visible splat, the
// count the compaction writes.
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
};

// A sort key past every visible one: what a culled splat sorts as.
const CULLED: u32 = 0xffffu;
const KEY_SPAN: f32 = 65534.0;
const ALPHA_MIN: f32 = 1.0 / 255.0;

@group(0) @binding(0) var<uniform> project: Project;
@group(0) @binding(2) var<storage, read_write> project_keys: array<u32>;
@group(0) @binding(3) var<storage, read_write> project_values: array<u32>;
@group(0) @binding(4) var<storage, read_write> draw_args: DrawArgs;
// Three rows a joint: a skinned set's joint matrices (instance space, scale
// removed, after the inverse binds).
@group(0) @binding(5) var<storage, read> palettes: array<vec4<f32>>;
// Each splat's streams, in one binding so the projection keeps within a
// stage's eight storage buffers: xy its skin, zw its surface (`skin_of`,
// `surface_of`).
@group(1) @binding(1) var<storage, read> streams: array<vec4<u32>>;
// The view's depth after its opaque pass, reduced to a farthest-depth
// pyramid (render-wgpu `occlusion.rs` `DepthPyramid`).
@group(2) @binding(0) var<storage, read> hiz: Hiz;

// A splat's skin: four joints (a byte each, lowest first) and their four
// weights (unorm8); zero in a set without one.
fn skin_of(index: u32) -> vec2<u32> {
    return streams[index].xy;
}

// What a lit run reads of a Gaussian: x its unit normal in the set's
// space, octahedral (snorm16 pair); y its albedo (`_ALBEDO`), sRGB unorm8
// rgb, its colour without baked shading, and in its top byte its group
// (`_GROUP`, 0 for none), which a tinted instance reads. A card reads only
// its group.
fn surface_of(index: u32) -> vec2<u32> {
    return streams[index].zw;
}

fn rotation_matrix(q: vec4<f32>) -> mat3x3<f32> {
    let x = q.x;
    let y = q.y;
    let z = q.z;
    let w = q.w;
    // Columns.
    return mat3x3<f32>(
        vec3<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)),
        vec3<f32>(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)),
        vec3<f32>(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)),
    );
}

// An IEEE half from its low 16 bits (`unpack2x16float` needs a capability
// not every device grants).
fn half_float(bits: u32) -> f32 {
    let exponent = (bits >> 10u) & 0x1fu;
    let mantissa = f32(bits & 0x3ffu);
    var magnitude: f32;
    if exponent == 0u {
        magnitude = mantissa * exp2(-24.0);
    } else {
        magnitude = (1.0 + mantissa / 1024.0) * exp2(f32(exponent) - 15.0);
    }
    return select(magnitude, -magnitude, (bits & 0x8000u) != 0u);
}

// The rows of a splat's blended joint matrix from the palette at `first`,
// and in `w` of the fourth its weights' total.
fn blended_rows(first: u32, skin: vec2<u32>) -> mat4x4<f32> {
    let weights = unpack4x8unorm(skin.y);
    var rows = mat4x4<f32>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    for (var influence = 0u; influence < 4u; influence = influence + 1u) {
        let weight = weights[influence];
        if weight > 0.0 {
            let joint = (skin.x >> (8u * influence)) & 0xffu;
            let row = first + joint * 3u;
            rows[0] = rows[0] + weight * palettes[row];
            rows[1] = rows[1] + weight * palettes[row + 1u];
            rows[2] = rows[2] + weight * palettes[row + 2u];
        }
    }
    rows[3] = vec4<f32>(0.0, 0.0, 0.0, weights.x + weights.y + weights.z + weights.w);
    return rows;
}

fn moved(rows: mat4x4<f32>, point: vec3<f32>) -> vec3<f32> {
    let p = vec4<f32>(point, 1.0);
    return vec3<f32>(dot(rows[0], p), dot(rows[1], p), dot(rows[2], p)) / rows[3].w;
}

fn cull(slot: u32) {
    project_keys[slot] = CULLED;
    project_values[slot] = slot;
}

// A unit vector from its octahedral encoding (`splats.rs` `octahedral`).
fn octahedral(word: u32) -> vec3<f32> {
    let e = unpack2x16snorm(word);
    var n = vec3<f32>(e, 1.0 - abs(e.x) - abs(e.y));
    if n.z < 0.0 {
        n = vec3<f32>((1.0 - abs(n.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0)), n.z);
    }
    return normalize(n);
}

// The tint of splat `index`'s group on this instance: its sRGB target in
// rgb and its weight in w, or none (weight 0).
fn group_tint(index: u32) -> vec4<f32> {
    if project.groups.z == 0u {
        return vec4<f32>(0.0);
    }
    let group = surface_of(index).y >> 24u;
    for (var tinted = 0u; group != 0u && tinted < project.groups.z; tinted = tinted + 1u) {
        if ((project.groups[tinted / 4u] >> (8u * (tinted % 4u))) & 0xffu) == group {
            return project.group_tints[tinted];
        }
    }
    return vec4<f32>(0.0);
}

// Whether a splat drawn within the target pixels `low` to `high`, at depths
// no nearer than `nearest` (NDC), lies wholly behind the view's opaque depth:
// farther than the farthest depth over the 2×2 texels of the pyramid level
// whose texels are at least the box's size. Every fragment it would draw
// fails the layer's depth test, so dropping it changes nothing.
fn hidden(low: vec2<f32>, high: vec2<f32>, nearest: f32) -> bool {
    let block = project.occlusion.x;
    if block < 0.5 {
        return false;
    }
    let origin = project.occlusion.zw;
    let lo = max(low - origin, vec2<f32>(0.0)) / block;
    let hi = (high - origin) / block;
    if any(hi < lo) {
        return false;
    }
    let levels = u32(project.occlusion.y);
    let extent = max(max(hi.x - lo.x, hi.y - lo.y), 1.0);
    let level = min(u32(ceil(log2(extent))), levels - 1u);
    let size = hiz_level_size(hiz.size, level);
    let last = vec2<i32>(size) - 1;
    let scale = 1.0 / f32(1u << level);
    let a = vec2<u32>(min(vec2<i32>(floor(lo * scale)), last));
    let b = vec2<u32>(min(vec2<i32>(floor(hi * scale)), last));
    let start = hiz.starts[level];
    let farthest = max(
        max(
            hiz.depth[hiz_index(start, size.x, a)],
            hiz.depth[hiz_index(start, size.x, vec2<u32>(b.x, a.y))],
        ),
        max(
            hiz.depth[hiz_index(start, size.x, vec2<u32>(a.x, b.y))],
            hiz.depth[hiz_index(start, size.x, b)],
        ),
    );
    return nearest > farthest;
}

// A visible splat in `slot` at view depth `depth`, sorted back to front:
// the farthest first. The run's compaction (`splats.wgsl` `cs_compact_*`)
// counts the visible ones for the draw.
fn keep(slot: u32, depth: f32) {
    let along = clamp((depth - project.depth.x) * project.depth.y, 0.0, 1.0);
    project_keys[slot] = u32(round((1.0 - along) * KEY_SPAN));
    project_values[slot] = slot;
}
