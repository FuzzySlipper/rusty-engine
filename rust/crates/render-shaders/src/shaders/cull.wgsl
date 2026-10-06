// GPU visibility (`render-wgpu/src/culling.rs`): one thread per candidate
// part of a view's opaque draw list tests the part's world bounds against
// the view frustum and, when it may be visible, appends its id to its
// batch's run of visible instances and counts it in the batch's indirect
// draw arguments. The batches, their meshes and materials still come from
// the retained frame; only which parts draw moves here.

struct CullParams {
    // Inward-pointing frustum planes for wgpu clip space.
    planes: array<vec4<f32>, 6>,
    // x: candidate instances, y: batches.
    counts: vec4<u32>,
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
// Each candidate's batch.
@group(0) @binding(3) var<storage, read> candidate_batches: array<u32>;
// Each batch's arguments; `first_instance` is the batch's visible run.
@group(0) @binding(4) var<storage, read_write> draws: array<DrawArgs>;
// Visible part ids, one run per batch at its `first_instance`.
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
// x: visible instances this view.
@group(0) @binding(6) var<storage, read_write> stats: array<atomic<u32>, 4>;

const WORKGROUP: u32 = 64u;

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

@compute @workgroup_size(64)
fn cs_cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let candidate = id.x;
    if candidate >= params.counts.x {
        return;
    }
    let part = candidates[candidate];
    if !inside(bounds[part]) {
        return;
    }
    let batch = candidate_batches[candidate];
    let slot = atomicAdd(&draws[batch].instance_count, 1u);
    visible[draws[batch].first_instance + slot] = part;
    atomicAdd(&stats[0], 1u);
}
