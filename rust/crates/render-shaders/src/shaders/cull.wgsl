// GPU visibility (`render-wgpu/src/culling.rs`): one thread per candidate
// part of a view's opaque draw list tests the part's world bounds against
// the view frustum and marks whether it may be visible (`shown`); then one
// thread per batch writes its visible candidates, in the list's order, to
// its run of visible instances and their count to its indirect draw
// arguments, so parts draw in the order the CPU list draws them. The
// batches, their meshes and materials still come from the retained frame;
// only which parts draw moves here.
//
// With occlusion (`cs_cull_occluded`), a part inside the frustum must also
// not lie wholly behind the view's farthest-depth pyramid (render-wgpu
// `occlusion.rs`). That depth is drawn first, at this frame's camera, from
// the parts the last cull found visible (`cs_occluders`), so a part the
// test drops is hidden behind geometry this frame draws.

struct CullParams {
    // Inward-pointing frustum planes for wgpu clip space.
    planes: array<vec4<f32>, 6>,
    // x: candidate instances, y: batches, z: 1 to select every candidate
    // inside the frustum as an occluder (no cull has marked any yet).
    counts: vec4<u32>,
    // The camera's view and projection, for the occlusion test.
    view_proj: mat4x4<f32>,
    // x, y: the view's region in the pyramid's level-0 texels; z: its
    // levels (0: no occlusion test).
    occlusion: vec4<u32>,
};

// A part's world bounds: min (xyz) and max (xyz); w unused. Empty bounds
// have min > max.
struct Bounds {
    min: vec4<f32>,
    max: vec4<f32>,
};

// `wgpu::util::DrawIndexedIndirectArgs`.
struct DrawArgs {
    index_count: u32,
    instance_count: atomic<u32>,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0) var<uniform> params: CullParams;
@group(0) @binding(1) var<storage, read> bounds: array<Bounds>;
// Candidate part ids, in batch order.
@group(0) @binding(2) var<storage, read> candidates: array<u32>;
// Each candidate's batch; the top bit set when its material may not
// occlude (it may discard, or it draws lines).
@group(0) @binding(3) var<storage, read> candidate_batches: array<u32>;
// Each batch's arguments; `first_instance` is the batch's visible run.
@group(0) @binding(4) var<storage, read_write> draws: array<DrawArgs>;
// Visible part ids, one run per batch at its `first_instance`.
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
// 0: visible instances, 1: inside the frustum but occluded, 2: occluders
// selected.
@group(0) @binding(6) var<storage, read_write> stats: array<atomic<u32>, 4>;
// Per part slot: 1 when the last cull of its layer found it visible; the
// cull writes it, then the batches' compaction reads it.
@group(0) @binding(7) var<storage, read_write> shown: array<u32>;

// The view's farthest-depth pyramid: level 0 covers the view's region from
// its top-left texel.
@group(1) @binding(0) var occluders: texture_2d<f32>;

const WORKGROUP: u32 = 64u;
const NO_OCCLUDER: u32 = 0x80000000u;
// The most texels across an occlusion test reads at its level (up to one
// more than this each way), as the scatter selection's.
const OCCLUSION_SPAN: f32 = 4.0;
// How far behind the occluders, as a share of the distance, bounds must lie
// (about 5 cm at 50 m). The occluder depth projects from world positions,
// the world pass from camera-relative ones, so far from the origin a surface
// flush with an occluder's face may round in front of or behind it; such a
// surface stays drawn, as it is drawn without the test.
const OCCLUSION_MARGIN: f32 = 1e-3;

fn inside(bounds: Bounds) -> bool {
    if any(bounds.min.xyz > bounds.max.xyz) {
        return false;
    }
    for (var index = 0; index < 6; index++) {
        let plane = params.planes[index];
        let farthest = select(bounds.min.xyz, bounds.max.xyz, plane.xyz >= vec3<f32>(0.0));
        if dot(plane.xyz, farthest) + plane.w < 0.0 {
            return false;
        }
    }
    return true;
}

// Bounds lying wholly behind the view's occluders: their nearest depth is
// farther than the farthest occluder depth over the texels their
// projection covers, widened by one level-0 texel, at the level where they
// span at most `OCCLUSION_SPAN` texels, by `OCCLUSION_MARGIN` of the
// distance. Bounds crossing the near plane, or wider than the pyramid's top
// level, are visible.
fn occluded(bounds: Bounds) -> bool {
    let levels = params.occlusion.z;
    if levels == 0u {
        return false;
    }
    var low = vec3<f32>(1e30);
    var high = vec2<f32>(-1e30);
    for (var corner = 0u; corner < 8u; corner++) {
        let point = select(
            bounds.min.xyz,
            bounds.max.xyz,
            vec3<bool>((corner & 1u) != 0u, (corner & 2u) != 0u, (corner & 4u) != 0u),
        );
        let clip = params.view_proj * vec4<f32>(point, 1.0);
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
    // In wgpu's depth (0 near, 1 far, far beyond near), 1 - depth falls
    // as one over the distance, so this margin is a share of it.
    return low.z > farthest + OCCLUSION_MARGIN * (1.0 - farthest);
}

// The parts the last cull found visible, inside the frustum, whose
// materials keep every sample they cover: the occluder depth draws them, in
// any order.
@compute @workgroup_size(64)
fn cs_occluders(@builtin(global_invocation_id) id: vec3<u32>) {
    let candidate = id.x;
    if candidate >= params.counts.x || (candidate_batches[candidate] & NO_OCCLUDER) != 0u {
        return;
    }
    let part = candidates[candidate];
    if (params.counts.z == 0u && shown[part] == 0u) || !inside(bounds[part]) {
        return;
    }
    let batch = candidate_batches[candidate] & ~NO_OCCLUDER;
    let slot = atomicAdd(&draws[batch].instance_count, 1u);
    visible[draws[batch].first_instance + slot] = part;
    atomicAdd(&stats[2], 1u);
}

@compute @workgroup_size(64)
fn cs_cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let candidate = id.x;
    if candidate >= params.counts.x {
        return;
    }
    let part = candidates[candidate];
    let seen = inside(bounds[part]);
    shown[part] = u32(seen);
    if seen {
        atomicAdd(&stats[0], 1u);
    }
}

@compute @workgroup_size(64)
fn cs_cull_occluded(@builtin(global_invocation_id) id: vec3<u32>) {
    let candidate = id.x;
    if candidate >= params.counts.x {
        return;
    }
    let part = candidates[candidate];
    let box = bounds[part];
    if !inside(box) {
        shown[part] = 0u;
        return;
    }
    if occluded(box) {
        shown[part] = 0u;
        atomicAdd(&stats[1], 1u);
        return;
    }
    shown[part] = 1u;
    atomicAdd(&stats[0], 1u);
}

// One batch's visible candidates, in the list's order, to its run. A
// batch's candidates follow the batch before's, as its run's start does.
@compute @workgroup_size(64)
fn cs_compact(@builtin(global_invocation_id) id: vec3<u32>) {
    let batch = id.x;
    let batches = params.counts.y;
    if batch >= batches {
        return;
    }
    let base = draws[0].first_instance;
    let first = draws[batch].first_instance;
    var end = params.counts.x;
    if batch + 1u < batches {
        end = draws[batch + 1u].first_instance - base;
    }
    var count = 0u;
    for (var candidate = first - base; candidate < end; candidate++) {
        let part = candidates[candidate];
        if shown[part] != 0u {
            visible[first + count] = part;
            count++;
        }
    }
    atomicStore(&draws[batch].instance_count, count);
}
