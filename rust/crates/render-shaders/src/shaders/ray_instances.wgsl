// The ray scene's scatter copies as TLAS instances (render-wgpu
// `ray_scene.rs`): one thread per copy of every scatter run writes a
// 64-byte Vulkan instance row (`VkAccelerationStructureInstanceKHR`) for
// each part of the level it draws at, when the copy lies within the ray
// reach of the eye. A copy is kept, scaled up and faded as the selection
// draws it (`scatter_select.wgsl`), at the level its distance reaches but
// never an impostor's, and leans in the wind as the vertex stage leans it
// (`rusty::wind`, the lean alone: a shear by height above its origin).
// Rows go after the parts' rows, which the CPU writes; a row past the
// buffer is counted and dropped.

#import rusty::types::{Part, ScatterCopy, copy_place}

struct Params {
    // The eye the reach is measured from (xyz) and the reach (w).
    eye: vec4<f32>,
    // The wind's direction (x, z), strength and gustiness (`Frame.wind`).
    wind: vec4<f32>,
    // x: chunks, y: chunks a dispatch row holds, z: the first row for
    // copies, w: the rows the buffer holds.
    counts: vec4<u32>,
    // x: the presentation time.
    time: vec4<f32>,
};

// A scatter run as the selection's `Run`, with the fields the rays need.
struct Run {
    // x: first copy row, y: copies, z: the part whose row holds the model
    // and fade, w: levels.
    copies: vec4<u32>,
    starts: vec4<f32>,
    // y: largest scale-up, z: a copy's reach from its origin at scale 1,
    // w: thinning points.
    shape: vec4<f32>,
    thin_distance: vec4<f32>,
    thin_share: vec4<f32>,
    level_first: vec4<u32>,
    level_count: vec4<u32>,
    // x: its last level that is a mesh.
    flags: vec4<u32>,
};

// One part of a level: its BLAS's device address (low, high; 0 while it
// has none), its id with the instance mask in the top byte, and its
// material's wind bend (0: no wind).
struct LevelPart {
    address: vec2<u32>,
    part_mask: u32,
    bend: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> copies: array<ScatterCopy>;
@group(0) @binding(3) var<storage, read> runs: array<Run>;
// (run, first copy within it) for each 64 copies.
@group(0) @binding(4) var<storage, read> chunks: array<vec2<u32>>;
@group(0) @binding(5) var<storage, read> level_parts: array<LevelPart>;
// Four words a row: the 3×4 transform's rows, then custom data and mask,
// flags, and the BLAS address.
@group(0) @binding(6) var<storage, read_write> instances: array<vec4<u32>>;
// 0: rows written, 1: copies in reach, 2: rows past the buffer.
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>, 3>;

// `rusty::wind`'s rates and a part's phase by its place.
const GUST_RATE: f32 = 0.9;
const GUST_RATE_2: f32 = 2.3;
// The largest scale-up an entry carries (`rusty::view::SELECTED_MAX_SCALE`).
const MAX_SCALE: f32 = 4.0;

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

// How much of a copy `away` from the eye its part's fade keeps
// (`rusty::view::fade_toward_origin`).
fn fade_keep(row: Part, away: f32) -> f32 {
    var keep = 1.0;
    if row.fade.y > 0.0 {
        keep = 1.0 - smoothstep(row.fade.x, row.fade.y, away);
    }
    if row.fade.w > 0.0 {
        if row.fade.w > row.fade.z {
            keep *= smoothstep(row.fade.z, row.fade.w, away);
        } else {
            keep *= step(row.fade.z, away);
        }
    }
    return keep;
}

@compute @workgroup_size(64)
fn cs_instances(
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
    var scattered = copies[run.copies.x + index];
    let row = parts[run.copies.z];
    let origin = (row.model * vec4<f32>(scattered.place.xyz, 1.0)).xyz;
    let distance = distance(origin, params.eye.xyz);
    if (row.fade.y > 0.0 && distance >= row.fade.y) || (row.fade.w > 0.0 && distance < row.fade.z) {
        return;
    }
    var scale = 1.0;
    let points = u32(run.shape.w);
    if points > 0u {
        let share = kept_share(run, distance, points);
        if scattered.detail.x >= share {
            return;
        }
        scale = min(inverseSqrt(share), min(run.shape.y, MAX_SCALE));
    }
    let radius = run.shape.z * scattered.place.w * scale * length(row.model[0].xyz);
    if distance - radius > params.eye.w {
        return;
    }
    atomicAdd(&counters[1], 1u);
    scattered.place.w *= scale * fade_keep(row, distance);
    let model = row.model * copy_place(scattered);
    var level = 0u;
    for (var l = 1u; l < run.copies.w; l++) {
        if distance >= run.starts[l] {
            level = l;
        }
    }
    level = min(level, run.flags.x);
    // The lean's share at this place and time (`rusty::wind`).
    let place = dot(origin.xz, vec2<f32>(0.37, 0.61));
    let time = params.time.x;
    let gust = 0.5 + 0.5 * sin(time * GUST_RATE + place) * sin(time * GUST_RATE_2 + place * 1.7);
    let lean = params.wind.z * mix(1.0, gust, params.wind.w);
    let direction = vec3<f32>(params.wind.x, 0.0, params.wind.y);
    let first = run.level_first[level];
    for (var p = 0u; p < run.level_count[level]; p++) {
        let entry = level_parts[first + p];
        if all(entry.address == vec2<u32>(0u)) {
            continue;
        }
        // world += direction * k * (world.y - origin.y): each linear column
        // gains the direction times its height.
        let k = lean * entry.bend;
        var leaning = model;
        for (var column = 0; column < 3; column++) {
            leaning[column] = vec4<f32>(model[column].xyz + direction * k * model[column].y, 0.0);
        }
        let slot = atomicAdd(&counters[0], 1u);
        let at = params.counts.z + slot;
        if at >= params.counts.w {
            atomicAdd(&counters[2], 1u);
            continue;
        }
        for (var r = 0; r < 3; r++) {
            instances[4u * at + u32(r)] = bitcast<vec4<u32>>(vec4<f32>(
                leaning[0][r], leaning[1][r], leaning[2][r], leaning[3][r],
            ));
        }
        instances[4u * at + 3u] = vec4<u32>(entry.part_mask, 0u, entry.address.x, entry.address.y);
    }
}
