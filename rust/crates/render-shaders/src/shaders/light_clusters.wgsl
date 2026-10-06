// Clustered forward shading (`render-wgpu/src/light_clusters.rs`): bins a
// world view's light rows into a view-frustum cluster grid before the view
// pass, so `rusty::lighting` reads a fragment's cluster list instead of
// looping over every light of the pass.
//
// The grid is `tiles.x × tiles.y` screen tiles by `tiles.z` depth slices
// spaced exponentially between the near and far planes (linearly for an
// orthographic view). Each cluster holds a count and up to `CLUSTER_CAPACITY`
// light row indices; one more entry after the grid, the global list, holds
// the lights every fragment sees (ambient, hemisphere, directional, and
// point or spot lights without a range). A cluster past its capacity keeps
// the first lights; the readout counts the overflow.

#import rusty::types::Light

struct ClusterParams {
    // World to view.
    view: mat4x4<f32>,
    // The view's projection: `[0][0]`, `[1][1]` scale view x and y to NDC;
    // `[3][3]` is 1 for an orthographic projection.
    projection: mat4x4<f32>,
    // xyz: tiles on x, y and depth slices; w: unused.
    tiles: vec4<u32>,
    // x: near plane, y: far plane (view distances); z: ln(far / near);
    // w: unused.
    depth: vec4<f32>,
    // x: first light row of the pass, y: light row count.
    lights: vec4<u32>,
};

@group(0) @binding(0) var<uniform> params: ClusterParams;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
// Per cluster, `CLUSTER_STRIDE` words: the count, then light row indices.
// The entry after the last cluster is the global list.
@group(0) @binding(2) var<storage, read_write> clusters: array<u32>;
// x: clusters that overflowed `CLUSTER_CAPACITY` this view; y: lights binned
// into some cluster; z: global lights.
@group(0) @binding(3) var<storage, read_write> stats: array<atomic<u32>, 4>;

// `light_clusters::CLUSTER_STRIDE` and `CLUSTER_CAPACITY`.
const CLUSTER_STRIDE: u32 = 64u;
const CLUSTER_CAPACITY: u32 = CLUSTER_STRIDE - 1u;
const WORKGROUP: u32 = 64u;

// A light every fragment sees: no position, or no range to bound it.
fn is_global(light: Light) -> bool {
    let kind = u32(light.color_kind.w);
    return kind < 3u || light.position_range.w <= 0.0;
}

// The view-space box of cluster (x, y, slice): its screen tile swept over
// its depth range.
fn cluster_bounds(tile: vec3<u32>) -> array<vec3<f32>, 2> {
    let tiles = vec3<f32>(params.tiles.xyz);
    let near = params.depth.x;
    let far = params.depth.y;
    let orthographic = params.projection[3][3] == 1.0;
    var z0: f32;
    var z1: f32;
    if orthographic {
        z0 = near + (far - near) * f32(tile.z) / tiles.z;
        z1 = near + (far - near) * f32(tile.z + 1u) / tiles.z;
    } else {
        z0 = near * exp(params.depth.z * f32(tile.z) / tiles.z);
        z1 = near * exp(params.depth.z * f32(tile.z + 1u) / tiles.z);
    }
    // NDC extent of the tile; y runs down the screen.
    let ndc_min = vec2<f32>(f32(tile.x) / tiles.x * 2.0 - 1.0, 1.0 - f32(tile.y + 1u) / tiles.y * 2.0);
    let ndc_max = vec2<f32>(f32(tile.x + 1u) / tiles.x * 2.0 - 1.0, 1.0 - f32(tile.y) / tiles.y * 2.0);
    let scale = vec2<f32>(1.0 / params.projection[0][0], 1.0 / params.projection[1][1]);
    var lo: vec2<f32>;
    var hi: vec2<f32>;
    if orthographic {
        lo = ndc_min * scale;
        hi = ndc_max * scale;
    } else {
        // View x = ndc_x * depth / m00: the far end of the slice spans more.
        lo = min(ndc_min * scale * z0, ndc_min * scale * z1);
        hi = max(ndc_max * scale * z0, ndc_max * scale * z1);
    }
    return array<vec3<f32>, 2>(vec3<f32>(lo, -z1), vec3<f32>(hi, -z0));
}

fn sphere_touches(centre: vec3<f32>, radius: f32, bounds: array<vec3<f32>, 2>) -> bool {
    let nearest = clamp(centre, bounds[0], bounds[1]);
    let offset = nearest - centre;
    return dot(offset, offset) <= radius * radius;
}

@compute @workgroup_size(64)
fn cs_assign(@builtin(global_invocation_id) id: vec3<u32>) {
    let cluster_count = params.tiles.x * params.tiles.y * params.tiles.z;
    let cluster = id.x;
    if cluster > cluster_count {
        return;
    }
    let first = params.lights.x;
    let count = params.lights.y;
    let base = cluster * CLUSTER_STRIDE;
    if cluster == cluster_count {
        // The global list.
        var written = 0u;
        for (var index = 0u; index < count; index++) {
            if is_global(lights[first + index]) && written < CLUSTER_CAPACITY {
                clusters[base + 1u + written] = first + index;
                written++;
            }
        }
        clusters[base] = written;
        atomicStore(&stats[2], written);
        return;
    }
    let tile = vec3<u32>(
        cluster % params.tiles.x,
        (cluster / params.tiles.x) % params.tiles.y,
        cluster / (params.tiles.x * params.tiles.y),
    );
    let bounds = cluster_bounds(tile);
    var written = 0u;
    var overflowed = false;
    for (var index = 0u; index < count; index++) {
        let light = lights[first + index];
        if is_global(light) {
            continue;
        }
        let centre = (params.view * vec4<f32>(light.position_range.xyz, 1.0)).xyz;
        if sphere_touches(centre, light.position_range.w, bounds) {
            if written < CLUSTER_CAPACITY {
                clusters[base + 1u + written] = first + index;
                written++;
            } else {
                overflowed = true;
            }
        }
    }
    clusters[base] = written;
    if overflowed {
        atomicAdd(&stats[0], 1u);
    }
    atomicAdd(&stats[1], written);
}
