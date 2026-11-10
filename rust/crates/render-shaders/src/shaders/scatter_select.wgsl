// Per-copy selection of scatter runs (render-wgpu `scatter_select.rs`): one
// thread per copy of every scatter run a view may see decides whether the
// copy draws and at which level of detail (`cs_select`); then its place in
// the indirect draws of that level's parts is counted out in the runs' and
// draws' order (`cs_scan_runs`, `cs_place`) and the copy written there
// (`cs_write`). Every draw so holds its copies in a fixed order, run by run
// and copy by copy, and two copies meeting a sample at one depth resolve the
// same way every frame. A copy draws while it is within its patch's
// fade, past its near cut and kept by its patch's thinning (which scales it
// up so its ground stays as covered), at the level its distance from the
// camera reaches; within a level's band it draws at both levels, sharing
// the samples between them (`rusty::view::fade_mask`). The camera's list
// also needs the copy inside the frustum and, with occluders, not wholly
// behind the view's opaque depth; the shadow list takes every copy
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
    // x: chunks, y: workgroups a dispatch row holds, z: spheres, w: runs.
    counts: vec4<u32>,
    // The fixed shadow layers' spheres (centre, radius).
    spheres: array<vec4<f32>, 16>,
    // The camera's view and projection, for the occlusion test.
    view_proj: mat4x4<f32>,
    // x, y: the view's region in the pyramid's level-0 texels; z: its
    // levels (0: no occlusion test).
    occlusion: vec4<u32>,
    // Where `scratch`'s regions start: x: the chunks' offsets, y: the runs'
    // totals, z: the level parts' bases; w: the draws' arguments (the
    // camera's, then the shadows').
    scratch: vec4<u32>,
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
    // x: the run casts shadows; y: its last level that is a mesh; z: its
    // first chunk; w: its first copy's decision in `scratch`.
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
// 4: outside the frustum, 5..7: camera draws at levels 1 to 3, 8: inside
// the frustum but occluded.
@group(0) @binding(8) var<storage, read_write> stats: array<atomic<u32>, 9>;
// The view's farthest-depth pyramid (render-wgpu `occlusion.rs`): level 0
// covers the view's region from its top-left texel.
@group(0) @binding(9) var occluders: texture_2d<f32>;
// The pass's working words: each copy's decision (from 0); each chunk's
// copies in each list (`Params::scratch.x`), then where they start in their
// run's; each run's copies in each list (y); each level part's first entry
// in each of its draws (z). A list is a level's camera (0 to 3) or shadow
// (4 to 7) entries.
@group(0) @binding(10) var<storage, read_write> scratch: array<u32>;
// For each draw's arguments, where its level parts start in what follows
// (and the next where they end); then, for each, in the order they draw,
// its run's total in its list and its first entry, as indices into the
// `scratch` regions.
@group(0) @binding(11) var<storage, read> draw_parts: array<u32>;

// Two halves: each step of `sum_lanes` reads one and writes the other.
var<workgroup> lane_counts: array<vec2<u32>, 128>;
// A run's chunks' counts, two halves as `lane_counts` (`cs_scan_runs`).
var<workgroup> run_lanes: array<vec4<u32>, 128>;
// A chunk's copies in each list, a byte each (`cs_select`).
var<workgroup> chunk_lists: array<atomic<u32>, 2>;
var<workgroup> part_bases: array<u32, 64>;

// The largest scale-up an entry carries (`rusty::view::SELECTED_MAX_SCALE`).
const MAX_SCALE: f32 = 4.0;
// The most texels across an occlusion test reads at its level (up to one
// more than this each way): the finer the level, the closer the farthest
// depth follows the copy's outline.
const OCCLUSION_SPAN: f32 = 4.0;
// How far below its share a copy's rank fades it in.
const THIN_SOFTNESS: f32 = 0.04;
// A copy's decision: its scale-up (bits 0 to 11), its weight at its level
// (12 to 18) and at the next (19 to 25) for the camera, its level (26, 27),
// whether it casts (28) and at which level (29, 30). 0: it draws nowhere.
const OUTGOING: u32 = 12u;
const INCOMING: u32 = 19u;
const LEVEL: u32 = 26u;
const CASTS: u32 = 0x10000000u;
const CASTER_LEVEL: u32 = 29u;

fn visible(center: vec3<f32>, radius: f32) -> bool {
    for (var index = 0; index < 6; index++) {
        let plane = params.planes[index];
        if dot(plane.xyz, center) + plane.w < -radius {
            return false;
        }
    }
    return true;
}

// A copy `radius` about `center` lies wholly behind the view's occluders:
// its bounding box's nearest depth is farther than the farthest occluder
// depth over the texels its projection covers, widened by one level-0
// texel, at the level where it spans at most `OCCLUSION_SPAN` texels. Bounds crossing
// the near plane, or wider than the pyramid's top level, are visible.
fn occluded(center: vec3<f32>, radius: f32) -> bool {
    let levels = params.occlusion.z;
    if levels == 0u {
        return false;
    }
    var low = vec3<f32>(1e30);
    var high = vec2<f32>(-1e30);
    for (var corner = 0u; corner < 8u; corner++) {
        let offset = vec3<f32>(
            select(-radius, radius, (corner & 1u) != 0u),
            select(-radius, radius, (corner & 2u) != 0u),
            select(-radius, radius, (corner & 4u) != 0u),
        );
        let clip = params.view_proj * vec4<f32>(center + offset, 1.0);
        if clip.w <= 1e-4 {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        low = min(low, ndc);
        high = max(high, ndc.xy);
    }
    if low.z <= 0.0 {
        return false;
    }
    let size = vec2<f32>(params.occlusion.xy);
    let top_left = vec2<f32>(low.x, -high.y) * 0.5 + 0.5;
    let bottom_right = vec2<f32>(high.x, -low.y) * 0.5 + 0.5;
    let a = clamp(top_left * size - 1.0, vec2<f32>(0.0), size - 1.0);
    let b = clamp(bottom_right * size + 1.0, vec2<f32>(0.0), size - 1.0);
    let level = u32(ceil(log2(max(max(b.x - a.x, b.y - a.y) / OCCLUSION_SPAN, 1.0))));
    if level >= levels {
        return false;
    }
    let texel_a = vec2<u32>(a) >> vec2<u32>(level);
    let texel_b = vec2<u32>(b) >> vec2<u32>(level);
    var farthest = 0.0;
    for (var y = texel_a.y; y <= texel_b.y; y++) {
        for (var x = texel_a.x; x <= texel_b.x; x++) {
            farthest = max(farthest, textureLoad(occluders, vec2<u32>(x, y), i32(level)).r);
        }
    }
    return low.z > farthest;
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

// The lists a decision enters, a byte each: the camera's levels 0 to 3,
// then the shadows'.
fn lists(decision: u32) -> vec2<u32> {
    var entered = vec2<u32>(0u);
    let level = (decision >> LEVEL) & 3u;
    if ((decision >> OUTGOING) & 127u) != 0u {
        entered.x |= 1u << (8u * level);
    }
    if ((decision >> INCOMING) & 127u) != 0u {
        entered.x |= 1u << (8u * (level + 1u));
    }
    if (decision & CASTS) != 0u {
        entered.y |= 1u << (8u * ((decision >> CASTER_LEVEL) & 3u));
    }
    return entered;
}

fn byte_of(counts: vec2<u32>, list: u32) -> u32 {
    return (counts[list >> 2u] >> (8u * (list & 3u))) & 255u;
}

// The workgroup's inclusive sum of `value` over lanes 0 to `lane`: eight
// counts of at most 64, a byte each.
fn sum_lanes(lane: u32, value: vec2<u32>) -> vec2<u32> {
    lane_counts[lane] = value;
    workgroupBarrier();
    var read = 0u;
    for (var step = 1u; step < 64u; step <<= 1u) {
        var sum = lane_counts[read + lane];
        if lane >= step {
            sum += lane_counts[read + lane - step];
        }
        read = 64u - read;
        lane_counts[read + lane] = sum;
        workgroupBarrier();
    }
    return lane_counts[read + lane];
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

// Whether, where and how the copy `lane` of a chunk draws (its decision).
fn decide(chunk: vec2<u32>, lane: u32) -> u32 {
    let run = runs[chunk.x];
    let index = chunk.y + lane;
    if index >= run.copies.y {
        return 0u;
    }
    let row_index = run.copies.x + index;
    let scattered = copies[row_index];
    let row = parts[run.copies.z];
    let origin = (row.model * vec4<f32>(scattered.place.xyz, 1.0)).xyz;
    let distance = distance(origin, params.eye.xyz);
    atomicAdd(&stats[0], 1u);
    // Beyond the patch's fade or within its near cut, nothing draws.
    if (row.fade.y > 0.0 && distance >= row.fade.y) || (row.fade.w > 0.0 && distance < row.fade.z) {
        return 0u;
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
            return 0u;
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
    var decision = scale_bits | (level << LEVEL);
    let model_scale = length(row.model[0].xyz);
    let radius = run.shape.z * scattered.place.w * scale * model_scale;
    let inside = visible(origin, radius);
    let hidden = inside && occluded(origin, radius);
    if inside && !hidden {
        let outgoing = u32(round((1.0 - incoming) * kept * 127.0));
        if outgoing > 0u {
            decision |= outgoing << OUTGOING;
            atomicAdd(&stats[1], 1u);
            if level > 0u {
                atomicAdd(&stats[4u + level], 1u);
            }
        }
        let coming = u32(round(incoming * kept * 127.0));
        if coming > 0u {
            decision |= coming << INCOMING;
            atomicAdd(&stats[1], 1u);
            atomicAdd(&stats[5u + level], 1u);
        }
    } else if hidden {
        atomicAdd(&stats[8], 1u);
    } else {
        atomicAdd(&stats[4], 1u);
    }
    if run.flags.x != 0u && cast_reaches(origin, distance, radius) {
        // An impostor level casts at the last mesh level.
        let caster_level = min(select(level, level + 1u, incoming >= 0.5), run.flags.y);
        decision |= CASTS | (caster_level << CASTER_LEVEL);
        atomicAdd(&stats[2], 1u);
    }
    return decision;
}

// One workgroup per chunk: each copy's decision, and the chunk's copies in
// each list.
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
    let decision = decide(chunk, lane);
    let first = runs[chunk.x].flags.w;
    if chunk.y + lane < runs[chunk.x].copies.y {
        scratch[first + chunk.y + lane] = decision;
    }
    // The chunk's counts need no order: each lane adds its lists.
    let entered = lists(decision);
    if entered.x != 0u {
        atomicAdd(&chunk_lists[0], entered.x);
    }
    if entered.y != 0u {
        atomicAdd(&chunk_lists[1], entered.y);
    }
    workgroupBarrier();
    if lane < 8u {
        let counts = vec2<u32>(atomicLoad(&chunk_lists[0]), atomicLoad(&chunk_lists[1]));
        scratch[params.scratch.x + chunk_index * 8u + lane] = byte_of(counts, lane);
    }
}

// The workgroup's inclusive sum of `value` over lanes 0 to `lane`: eight
// counts of at most 4096 (64 chunks of 64 copies), two to a word.
fn sum_chunks(lane: u32, value: vec4<u32>) -> vec4<u32> {
    run_lanes[lane] = value;
    workgroupBarrier();
    var read = 0u;
    for (var step = 1u; step < 64u; step <<= 1u) {
        var sum = run_lanes[read + lane];
        if lane >= step {
            sum += run_lanes[read + lane - step];
        }
        read = 64u - read;
        run_lanes[read + lane] = sum;
        workgroupBarrier();
    }
    return run_lanes[read + lane];
}

fn half_of(counts: vec4<u32>, list: u32) -> u32 {
    return (counts[list >> 1u] >> (16u * (list & 1u))) & 0xffffu;
}

// One workgroup per run: where each of its chunks' copies start in each list
// of the run, and the run's copies in each. Most runs hold one chunk.
@compute @workgroup_size(64)
fn cs_scan_runs(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let run_index = group.x + group.y * params.counts.y;
    if run_index >= params.counts.w {
        return;
    }
    let run = runs[run_index];
    let chunk_total = (run.copies.y + 63u) / 64u;
    let totals = params.scratch.y + run_index * 8u;
    if chunk_total == 1u {
        if lane < 8u {
            let at = params.scratch.x + run.flags.z * 8u + lane;
            scratch[totals + lane] = scratch[at];
            scratch[at] = 0u;
        }
        return;
    }
    var carry = array<u32, 8>();
    for (var block = 0u; block < chunk_total; block += 64u) {
        let chunk = block + lane;
        let at = params.scratch.x + (run.flags.z + chunk) * 8u;
        var own = vec4<u32>(0u);
        if chunk < chunk_total {
            for (var pair = 0u; pair < 4u; pair++) {
                own[pair] = scratch[at + 2u * pair] | (scratch[at + 2u * pair + 1u] << 16u);
            }
        }
        let before = sum_chunks(lane, own) - own;
        if chunk < chunk_total {
            for (var list = 0u; list < 8u; list++) {
                scratch[at + list] = carry[list] + half_of(before, list);
            }
        }
        let block_total = run_lanes[63];
        for (var list = 0u; list < 8u; list++) {
            carry[list] += half_of(block_total, list);
        }
        workgroupBarrier();
    }
    if lane < 8u {
        scratch[totals + lane] = carry[lane];
    }
}

// One workgroup per draw: where each of its level parts' entries start, in
// the order its level parts were listed, and its instances.
@compute @workgroup_size(64)
fn cs_place(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let draw = group.x + group.y * params.counts.y;
    if draw >= params.scratch.w {
        return;
    }
    let listed = params.scratch.w + 1u;
    let begin = draw_parts[draw];
    let end = draw_parts[draw + 1u];
    var carry = 0u;
    for (var block = begin; block < end; block += 64u) {
        let element = block + lane;
        var own = 0u;
        if element < end {
            own = scratch[params.scratch.y + draw_parts[listed + 2u * element]];
        }
        part_bases[lane] = own;
        workgroupBarrier();
        for (var step = 1u; step < 64u; step <<= 1u) {
            var sum = part_bases[lane];
            if lane >= step {
                sum += part_bases[lane - step];
            }
            workgroupBarrier();
            part_bases[lane] = sum;
            workgroupBarrier();
        }
        if element < end {
            scratch[params.scratch.z + draw_parts[listed + 2u * element + 1u]] = carry + part_bases[lane] - own;
        }
        carry += part_bases[63];
        workgroupBarrier();
    }
    if lane == 0u {
        atomicStore(&draws[draw].instance_count, carry);
    }
}

// The copy's entries in `list` (a level's, the shadows' when `shadow`):
// one in each of the level's parts' draws, `within` its run's first.
fn put(run: Run, level: u32, shadow: bool, within: u32, word: u32, scale_bits: u32) {
    let first = run.level_first[level];
    for (var index = 0u; index < run.level_count[level]; index++) {
        let part = first + index;
        let entry = level_parts[part];
        let draw = select(entry.camera, entry.shadow, shadow);
        let base = scratch[params.scratch.z + 2u * part + u32(shadow)];
        let at = 2u * (draws[draw].first_instance + base + within);
        entries[at] = word;
        entries[at + 1u] = entry.part | (scale_bits << 20u);
    }
}

// One workgroup per chunk again: each copy's entries, after its run's copies
// before it in each list.
@compute @workgroup_size(64)
fn cs_write(
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
    var decision = 0u;
    if index < run.copies.y {
        decision = scratch[run.flags.w + index];
    }
    let entered = lists(decision);
    let before = sum_lanes(lane, entered) - entered;
    if all(entered == vec2<u32>(0u)) {
        return;
    }
    let row_index = run.copies.x + index;
    let scale_bits = decision & 4095u;
    let level = (decision >> LEVEL) & 3u;
    let chunk_starts = params.scratch.x + chunk_index * 8u;
    let outgoing = (decision >> OUTGOING) & 127u;
    if outgoing != 0u {
        let within = scratch[chunk_starts + level] + byte_of(before, level);
        put(run, level, false, within, row_index | (outgoing << 24u), scale_bits);
    }
    let coming = (decision >> INCOMING) & 127u;
    if coming != 0u {
        let list = level + 1u;
        let within = scratch[chunk_starts + list] + byte_of(before, list);
        put(run, list, false, within, row_index | (coming << 24u) | 0x80000000u, scale_bits);
    }
    if (decision & CASTS) != 0u {
        let caster = (decision >> CASTER_LEVEL) & 3u;
        let within = scratch[chunk_starts + 4u + caster] + byte_of(before, 4u + caster);
        put(run, caster, true, within, row_index | (127u << 24u), scale_bits);
    }
}
