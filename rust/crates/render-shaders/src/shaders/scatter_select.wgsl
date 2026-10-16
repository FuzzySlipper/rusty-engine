// Per-copy selection of scatter runs (render-wgpu `scatter_select.rs`): one
// thread per copy of every scatter run a view may see decides whether the
// copy draws and at which level of detail, then appends it to the indirect
// draws of that level's parts. A copy draws while it is within its patch's
// fade, past its near cut and kept by its patch's thinning (which scales it
// up so its ground stays as covered), at the level its distance from the
// camera reaches; within a level's band it draws at both levels, sharing
// the samples between them (`rusty::view::fade_mask`). The camera's list
// also needs the copy inside the frustum; the shadow list takes every copy
// of a run that casts within the cascades' reach of the camera or inside a
// fixed shadow layer, at its nearer-weighted level (an impostor's at the
// last mesh level), whole.

#import rusty::types::{Part, ScatterCopy}

struct Params {
    // Inward-pointing frustum planes for wgpu clip space.
    planes: array<vec4<f32>, 6>,
    // The camera, in the world; w: how far from it the cascades cast (0:
    // none).
    eye: vec4<f32>,
    // x: chunks, y: chunks a dispatch row holds, z: spheres.
    counts: vec4<u32>,
    // The fixed shadow layers' spheres (centre, radius).
    spheres: array<vec4<f32>, 16>,
};

struct Run {
    // x: first copy row, y: copies, z: the part whose row holds the model
    // and fade, w: levels.
    copies: vec4<u32>,
    // Where each level starts; level 0 at 0.
    starts: vec4<f32>,
    // x: crossfade band, y: largest scale-up, z: a copy's reach from its
    // origin at scale 1, w: thinning points.
    shape: vec4<f32>,
    thin_distance: vec4<f32>,
    thin_share: vec4<f32>,
    // Each level's first entry in `level_parts` and how many it has.
    level_first: vec4<u32>,
    level_count: vec4<u32>,
    // x: the run casts shadows; y: its last level that is a mesh.
    flags: vec4<u32>,
};

// One part of a level: its id and its draws in the camera's and the
// shadows' arguments.
struct LevelPart {
    part: u32,
    camera: u32,
    shadow: u32,
    pad: u32,
};

// `wgpu::util::DrawIndexedIndirectArgs`; `first_instance` is the part's
// entries in `entries`, two words an instance.
struct DrawArgs {
    index_count: u32,
    instance_count: atomic<u32>,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> copies: array<ScatterCopy>;
@group(0) @binding(3) var<storage, read> runs: array<Run>;
// (run, first copy within it) for each 64 copies.
@group(0) @binding(4) var<storage, read> chunks: array<vec2<u32>>;
@group(0) @binding(5) var<storage, read> level_parts: array<LevelPart>;
@group(0) @binding(6) var<storage, read_write> draws: array<DrawArgs>;
// The instance buffer (`rusty::view::instances`).
@group(0) @binding(7) var<storage, read_write> entries: array<u32>;
// 0: copies tested, 1: camera draws, 2: shadow draws, 3: thinned out,
// 4: outside the frustum, 5..7: camera draws at levels 1 to 3.
@group(0) @binding(8) var<storage, read_write> stats: array<atomic<u32>, 8>;

// The largest scale-up an entry carries (`rusty::view::SELECTED_MAX_SCALE`).
const MAX_SCALE: f32 = 4.0;
// How far below its share a copy's rank fades it in.
const THIN_SOFTNESS: f32 = 0.04;

fn visible(center: vec3<f32>, radius: f32) -> bool {
    for (var index = 0; index < 6; index++) {
        let plane = params.planes[index];
        if dot(plane.xyz, center) + plane.w < -radius {
            return false;
        }
    }
    return true;
}

// The share of copies kept `distance` from the camera.
fn kept_share(run: Run, distance: f32, points: u32) -> f32 {
    var share = run.thin_share[0];
    if distance <= run.thin_distance[0] {
        return share;
    }
    for (var index = 1u; index < points; index++) {
        let near = run.thin_distance[index - 1u];
        let far = run.thin_distance[index];
        if distance <= far {
            let t = (distance - near) / max(far - near, 1e-6);
            return mix(run.thin_share[index - 1u], run.thin_share[index], t);
        }
        share = run.thin_share[index];
    }
    return share;
}

fn append(level: u32, run: Run, word: u32, scale_bits: u32, shadow: bool) {
    let first = run.level_first[level];
    for (var index = 0u; index < run.level_count[level]; index++) {
        let entry = level_parts[first + index];
        let draw = select(entry.camera, entry.shadow, shadow);
        let slot = atomicAdd(&draws[draw].instance_count, 1u);
        let at = 2u * (draws[draw].first_instance + slot);
        entries[at] = word;
        entries[at + 1u] = entry.part | (scale_bits << 20u);
    }
}

// A shadow layer reaches a copy `away` from the camera, `radius` across:
// within the cascades' reach, or inside a fixed layer's sphere.
fn cast_reaches(origin: vec3<f32>, away: f32, radius: f32) -> bool {
    if away - radius <= params.eye.w {
        return true;
    }
    for (var index = 0u; index < params.counts.z; index++) {
        let sphere = params.spheres[index];
        if distance(origin, sphere.xyz) - radius <= sphere.w {
            return true;
        }
    }
    return false;
}

@compute @workgroup_size(64)
fn cs_select(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let chunk_index = group.x + group.y * params.counts.y;
    if chunk_index >= params.counts.x {
        return;
    }
    let chunk = chunks[chunk_index];
    let run = runs[chunk.x];
    let index = chunk.y + lane;
    if index >= run.copies.y {
        return;
    }
    let row_index = run.copies.x + index;
    let scattered = copies[row_index];
    let row = parts[run.copies.z];
    let origin = (row.model * vec4<f32>(scattered.place.xyz, 1.0)).xyz;
    let distance = distance(origin, params.eye.xyz);
    atomicAdd(&stats[0], 1u);
    // Beyond the patch's fade or within its near cut, nothing draws.
    if (row.fade.y > 0.0 && distance >= row.fade.y) || (row.fade.w > 0.0 && distance < row.fade.z) {
        return;
    }
    // Thinning: a kept copy grows so the ground stays as covered.
    var scale = 1.0;
    var kept = 1.0;
    let points = u32(run.shape.w);
    if points > 0u {
        let share = kept_share(run, distance, points);
        let rank = scattered.detail.x;
        if rank >= share {
            atomicAdd(&stats[3], 1u);
            return;
        }
        kept = clamp((share - rank) / THIN_SOFTNESS, 0.0, 1.0);
        scale = min(inverseSqrt(share), min(run.shape.y, MAX_SCALE));
    }
    let scale_bits = u32(round((scale - 1.0) / (MAX_SCALE - 1.0) * 4095.0));
    // The level its distance reaches, and the next one's share within that
    // level's band.
    let levels = run.copies.w;
    var level = 0u;
    for (var l = 1u; l < levels; l++) {
        if distance >= run.starts[l] {
            level = l;
        }
    }
    var incoming = 0.0;
    let band = run.shape.x;
    if level + 1u < levels && band > 0.0 {
        let start = run.starts[level + 1u];
        incoming = clamp((distance - (start - band)) / band, 0.0, 1.0);
    }
    let model_scale = length(row.model[0].xyz);
    let radius = run.shape.z * scattered.place.w * scale * model_scale;
    if visible(origin, radius) {
        let outgoing = u32(round((1.0 - incoming) * kept * 127.0));
        if outgoing > 0u {
            append(level, run, row_index | (outgoing << 24u), scale_bits, false);
            atomicAdd(&stats[1], 1u);
            if level > 0u {
                atomicAdd(&stats[4u + level], 1u);
            }
        }
        let coming = u32(round(incoming * kept * 127.0));
        if coming > 0u {
            append(level + 1u, run, row_index | (coming << 24u) | 0x80000000u, scale_bits, false);
            atomicAdd(&stats[1], 1u);
            atomicAdd(&stats[5u + level], 1u);
        }
    } else {
        atomicAdd(&stats[4], 1u);
    }
    if run.flags.x != 0u && cast_reaches(origin, distance, radius) {
        // An impostor level casts at the last mesh level.
        let caster_level = min(select(level, level + 1u, incoming >= 0.5), run.flags.y);
        append(caster_level, run, row_index | (127u << 24u), scale_bits, true);
        atomicAdd(&stats[2], 1u);
    }
}
