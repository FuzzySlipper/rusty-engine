// The renderer's compute pass (`render-wgpu/src/compute.rs`): one dispatch
// between the row uploads and the view passes. This is the foundation's
// proof workload: each invocation writes one part's world position from its
// GPU row, so a readback checks the pass against the renderer's tables. A
// candidate workload replaces the body and bindings, not the pass.

#import rusty::types::Part

struct ComputeParams {
    // x: part rows to process.
    counts: vec4<u32>,
};

@group(0) @binding(0) var<uniform> params: ComputeParams;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
// One row per part: world position (xyz), 1 for a processed row (w).
@group(0) @binding(2) var<storage, read_write> output: array<vec4<f32>>;

// `compute::WORKGROUP_SIZE`.
@compute @workgroup_size(64)
fn cs_parts(@builtin(global_invocation_id) id: vec3<u32>) {
    let row = id.x;
    if row >= params.counts.x {
        return;
    }
    let model = parts[row].model;
    output[row] = vec4<f32>(model[3].xyz, 1.0);
}
