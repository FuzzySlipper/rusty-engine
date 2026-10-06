// EXPLORE #9513: dual contouring of a coarse lattice. See gpu_dc.rs.

struct Chunk {
    dims: vec3<u32>,
    sample_base: u32,
    vertex_base: u32,
    index_base: u32,
    index_capacity: u32,
    counter: u32,
    owner_min: vec3<i32>,
    pad1: i32,
    owner_max: vec3<i32>,
    pad2: i32,
};

struct Vertex {
    position: vec3<f32>,
    material: u32,
    normal: vec3<f32>,
    used: u32,
};

struct Counters {
    indices: atomic<u32>,
    vertices: atomic<u32>,
    overflow: atomic<u32>,
    pad: u32,
};

@group(0) @binding(0) var<uniform> chunk: Chunk;
@group(0) @binding(1) var<storage, read> values: array<f32>;
@group(0) @binding(2) var<storage, read> materials: array<u32>;
@group(0) @binding(3) var<storage, read_write> vertices: array<Vertex>;
@group(0) @binding(4) var<storage, read_write> indices: array<u32>;
@group(0) @binding(5) var<storage, read_write> counters: array<Counters>;

fn sample_index(p: vec3<u32>) -> u32 {
    return chunk.sample_base + (p.z * chunk.dims.y + p.y) * chunk.dims.x + p.x;
}

fn cell_dims() -> vec3<u32> {
    return chunk.dims - vec3<u32>(1u);
}

fn cell_index(c: vec3<u32>) -> u32 {
    let d = cell_dims();
    return (c.z * d.y + c.y) * d.x + c.x;
}

fn corner(i: u32) -> vec3<u32> {
    return vec3<u32>(i & 1u, (i >> 1u) & 1u, (i >> 2u) & 1u);
}

// Trilinear gradient of the eight corner values at local point `p`.
fn gradient(v: array<f32, 8>, p: vec3<f32>) -> vec3<f32> {
    let x = p.x; let y = p.y; let z = p.z;
    let dx = (v[1] - v[0]) * (1.0 - y) * (1.0 - z) + (v[3] - v[2]) * y * (1.0 - z)
        + (v[5] - v[4]) * (1.0 - y) * z + (v[7] - v[6]) * y * z;
    let dy = (v[2] - v[0]) * (1.0 - x) * (1.0 - z) + (v[3] - v[1]) * x * (1.0 - z)
        + (v[6] - v[4]) * (1.0 - x) * z + (v[7] - v[5]) * x * z;
    let dz = (v[4] - v[0]) * (1.0 - x) * (1.0 - y) + (v[5] - v[1]) * x * (1.0 - y)
        + (v[6] - v[2]) * (1.0 - x) * y + (v[7] - v[3]) * x * y;
    return vec3<f32>(dx, dy, dz);
}

@compute @workgroup_size(64)
fn cs_cells(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = cell_dims();
    let count = d.x * d.y * d.z;
    let linear = id.x;
    if linear >= count {
        return;
    }
    let c = vec3<u32>(linear % d.x, (linear / d.x) % d.y, linear / (d.x * d.y));
    var v: array<f32, 8>;
    var inside: array<bool, 8>;
    var materials_in: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {
        let s = sample_index(c + corner(i));
        v[i] = values[s];
        inside[i] = v[i] > 0.0;
        materials_in[i] = materials[s];
    }
    // The twelve edges of a cell: corner pairs.
    var edges = array<vec2<u32>, 12>(
        vec2<u32>(0u, 1u), vec2<u32>(2u, 3u), vec2<u32>(4u, 5u), vec2<u32>(6u, 7u),
        vec2<u32>(0u, 2u), vec2<u32>(1u, 3u), vec2<u32>(4u, 6u), vec2<u32>(5u, 7u),
        vec2<u32>(0u, 4u), vec2<u32>(1u, 5u), vec2<u32>(2u, 6u), vec2<u32>(3u, 7u),
    );
    var sum = vec3<f32>(0.0);
    var normal = vec3<f32>(0.0);
    var crossings = 0u;
    for (var e = 0u; e < 12u; e++) {
        let a = edges[e].x;
        let b = edges[e].y;
        if inside[a] == inside[b] {
            continue;
        }
        let t = v[a] / (v[a] - v[b]);
        let p = mix(vec3<f32>(corner(a)), vec3<f32>(corner(b)), t);
        sum += p;
        // Outward from the solid: toward decreasing (negative) values.
        normal += normalize(-gradient(v, p) + vec3<f32>(1e-9));
        crossings += 1u;
    }
    let slot = chunk.vertex_base + linear;
    if crossings == 0u {
        vertices[slot].used = 0u;
        return;
    }
    // Majority inside material: the most frequent among the inside corners.
    var best = 0u;
    var best_count = 0u;
    for (var i = 0u; i < 8u; i++) {
        if !inside[i] { continue; }
        var n = 0u;
        for (var j = 0u; j < 8u; j++) {
            if inside[j] && materials_in[j] == materials_in[i] { n += 1u; }
        }
        if n > best_count { best_count = n; best = materials_in[i]; }
    }
    let position = vec3<f32>(c) + sum / f32(crossings) + vec3<f32>(0.5);
    vertices[slot] = Vertex(position, best, normalize(normal + vec3<f32>(0.0, 1e-6, 0.0)), 1u);
    atomicAdd(&counters[chunk.counter].vertices, 1u);
}

fn in_owner(p: vec3<i32>) -> bool {
    return all(p >= chunk.owner_min) && all(p < chunk.owner_max);
}

@compute @workgroup_size(64)
fn cs_edges(@builtin(global_invocation_id) id: vec3<u32>) {
    let samples = chunk.dims.x * chunk.dims.y * chunk.dims.z;
    if id.x >= samples * 3u {
        return;
    }
    let axis = id.x / samples;
    let linear = id.x % samples;
    let s = vec3<u32>(linear % chunk.dims.x, (linear / chunk.dims.x) % chunk.dims.y, linear / (chunk.dims.x * chunk.dims.y));
    var step = vec3<u32>(0u);
    step[axis] = 1u;
    if s[axis] + 1u >= chunk.dims[axis] {
        return;
    }
    let start_inside = values[sample_index(s)] > 0.0;
    let end_inside = values[sample_index(s + step)] > 0.0;
    if start_inside == end_inside {
        return;
    }
    // The inside endpoint must lie in the chunk's own region.
    let inside_sample = select(s + step, s, start_inside);
    if !in_owner(vec3<i32>(inside_sample)) {
        return;
    }
    // The four cells around the edge, a right-handed loop facing +axis.
    var u = 1u; var w = 2u;
    if axis == 1u { u = 2u; w = 0u; }
    if axis == 2u { u = 0u; w = 1u; }
    if s[u] == 0u || s[w] == 0u {
        return;
    }
    let d = cell_dims();
    if s[axis] >= d[axis] {
        return;
    }
    var eu = vec3<u32>(0u); eu[u] = 1u;
    var ew = vec3<u32>(0u); ew[w] = 1u;
    var cells = array<vec3<u32>, 4>(s - eu - ew, s - ew, s, s - eu);
    var verts: array<u32, 4>;
    for (var i = 0u; i < 4u; i++) {
        verts[i] = chunk.vertex_base + cell_index(cells[i]);
    }
    // Winding: outward exactly when the edge exits the solid.
    if !start_inside {
        let t = verts[0]; verts[0] = verts[3]; verts[3] = t;
        let t2 = verts[1]; verts[1] = verts[2]; verts[2] = t2;
    }
    let p0 = vertices[verts[0]].position;
    let p1 = vertices[verts[1]].position;
    let p2 = vertices[verts[2]].position;
    let p3 = vertices[verts[3]].position;
    var tri: array<u32, 6>;
    if dot(p0 - p2, p0 - p2) <= dot(p1 - p3, p1 - p3) {
        tri = array<u32, 6>(verts[0], verts[1], verts[2], verts[0], verts[2], verts[3]);
    } else {
        tri = array<u32, 6>(verts[0], verts[1], verts[3], verts[1], verts[2], verts[3]);
    }
    let at = atomicAdd(&counters[chunk.counter].indices, 6u);
    if at + 6u > chunk.index_capacity {
        atomicAdd(&counters[chunk.counter].overflow, 1u);
        return;
    }
    for (var i = 0u; i < 6u; i++) {
        indices[chunk.index_base + at + i] = tri[i];
    }
}
