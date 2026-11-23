// Skinning (`render-wgpu/src/skin.rs`): one thread per output vertex of an
// animated instance's skinned primitive blends its vertex's four joints'
// palette matrices, as the CPU does (`animated.rs`), and writes the posed
// vertex in place, into the instance's range of an arena buffer that the
// draws read. One dispatch poses every queued primitive skinned from the
// same rest data into the same arena buffers, each a job of whole
// workgroups. A cut's
// piece draws copies of its triangles' vertices after the primitive's own,
// each skinned by the piece's palette set.

struct SkinParams {
    // x: the dispatch's first workgroup in the frame's table; y: threads per
    // row of workgroups; z: where the jobs start in `table`; w: where the
    // workgroups' jobs start.
    at: vec4<u32>,
    // x: the dispatch's workgroups.
    count: vec4<u32>,
};

// A job (`JOB_WORDS` words in `table`): its output vertices; the
// primitive's own vertices, after which the copies come; its first palette
// joint; joints per palette set; its first vertex in `output`; its first
// in `output_extra` (when it writes the extra stream); its first copy in
// `table`; flags (1: write the extra stream, 2: skin its tangents); its
// first workgroup in the frame.
const JOB_WORDS: u32 = 9u;

// Each joint's matrix, rows 0 to 2 (the fourth is 0, 0, 0, 1).
@group(0) @binding(0) var<storage, read> palettes: array<vec4<f32>>;
// The jobs, then each workgroup's job, then each copy's source vertex and
// palette set.
@group(0) @binding(1) var<storage, read> table: array<u32>;
@group(0) @binding(2) var<uniform> params: SkinParams;

// The primitive at rest in the renderer's vertex layout (12 floats:
// position, normal, uv, colour).
@group(1) @binding(0) var<storage, read> source: array<f32>;
// Per vertex: joints 0 and 1, 2 and 3 (16 bits each), then four weights.
@group(1) @binding(1) var<storage, read> skin: array<u32>;
// The extra stream at rest (6 floats: tangent xyz and handedness, uv1).
@group(1) @binding(2) var<storage, read> source_extra: array<f32>;
// The arena buffers the jobs' vertices and extra streams are ranges of.
@group(1) @binding(3) var<storage, read_write> output: array<f32>;
@group(1) @binding(4) var<storage, read_write> output_extra: array<f32>;

const WORKGROUP: u32 = 64u;
const VERTEX_FLOATS: u32 = 12u;
const EXTRA_FLOATS: u32 = 6u;
const SKIN_WORDS: u32 = 6u;

// `v` normalized, or `fallback` when it has no finite length (glam's
// `normalize_or`).
fn normalize_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let recip = 1.0 / sqrt(dot(v, v));
    // Not finite (a zero or overflowing length) or not positive.
    if !(recip > 0.0 && recip < 3.4e38) {
        return fallback;
    }
    return v * recip;
}

fn rotate(rows: array<vec4<f32>, 3>, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        rows[0].x * v.x + rows[0].y * v.y + rows[0].z * v.z,
        rows[1].x * v.x + rows[1].y * v.y + rows[1].z * v.z,
        rows[2].x * v.x + rows[2].y * v.y + rows[2].z * v.z,
    );
}

@compute @workgroup_size(64)
fn cs_skin(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let local = group.x + group.y * (params.at.y / WORKGROUP);
    if local >= params.count.x {
        return;
    }
    let workgroup = params.at.x + local;
    let job = params.at.z + table[params.at.w + workgroup] * JOB_WORDS;
    let outputs = table[job];
    let own = table[job + 1u];
    let joints = table[job + 3u];
    let flags = table[job + 7u];
    let index = (workgroup - table[job + 8u]) * WORKGROUP + lane;
    if index >= outputs {
        return;
    }
    var vertex = index;
    var palette_set = 0u;
    if index >= own {
        let copy = table[job + 6u] + (index - own) * 2u;
        vertex = table[copy];
        palette_set = table[copy + 1u];
    }
    let first = table[job + 2u] + palette_set * joints;
    var rows = array<vec4<f32>, 3>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    for (var influence = 0u; influence < 4u; influence += 1u) {
        let weight = bitcast<f32>(skin[vertex * SKIN_WORDS + 2u + influence]);
        let packed = skin[vertex * SKIN_WORDS + influence / 2u];
        let joint = (packed >> (16u * (influence % 2u))) & 0xffffu;
        if weight > 0.0 && joint < joints {
            let at = (first + joint) * 3u;
            rows[0] += palettes[at] * weight;
            rows[1] += palettes[at + 1u] * weight;
            rows[2] += palettes[at + 2u] * weight;
        }
    }
    let read_at = vertex * VERTEX_FLOATS;
    let write_at = (table[job + 4u] + index) * VERTEX_FLOATS;
    let position = vec3<f32>(source[read_at], source[read_at + 1u], source[read_at + 2u]);
    let placed = rotate(rows, position) + vec3<f32>(rows[0].w, rows[1].w, rows[2].w);
    let normal = normalize_or(
        rotate(rows, vec3<f32>(source[read_at + 3u], source[read_at + 4u], source[read_at + 5u])),
        vec3<f32>(0.0, 1.0, 0.0),
    );
    output[write_at] = placed.x;
    output[write_at + 1u] = placed.y;
    output[write_at + 2u] = placed.z;
    output[write_at + 3u] = normal.x;
    output[write_at + 4u] = normal.y;
    output[write_at + 5u] = normal.z;
    // Uv and colour as at rest.
    for (var k = 6u; k < VERTEX_FLOATS; k += 1u) {
        output[write_at + k] = source[read_at + k];
    }
    if (flags & 1u) == 0u {
        return;
    }
    let extra_read = vertex * EXTRA_FLOATS;
    let extra_write = (table[job + 5u] + index) * EXTRA_FLOATS;
    var tangent = vec3<f32>(
        source_extra[extra_read],
        source_extra[extra_read + 1u],
        source_extra[extra_read + 2u],
    );
    if (flags & 2u) != 0u {
        tangent = normalize_or(rotate(rows, tangent), vec3<f32>(1.0, 0.0, 0.0));
    }
    output_extra[extra_write] = tangent.x;
    output_extra[extra_write + 1u] = tangent.y;
    output_extra[extra_write + 2u] = tangent.z;
    for (var k = 3u; k < EXTRA_FLOATS; k += 1u) {
        output_extra[extra_write + k] = source_extra[extra_read + k];
    }
}
