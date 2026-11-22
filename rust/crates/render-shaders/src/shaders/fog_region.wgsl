// Fog regions (render-wgpu `fog_regions.rs`): one step of every region's
// cells. A cell holds a flow (world metres per second, xyz) and a density
// factor resting at 1 (w). Each step a cell takes what the flow brought
// from upstream (semi-Lagrangian: the previous cells sampled where its
// centre was a step ago, the rest state beyond the region), settles toward
// the current and density 1, and is dragged by the stirrers near it toward
// the flow round a moving sphere (a dipole: forward ahead and behind,
// backward at its sides), so the mist parts ahead and swirls in behind.
// Inside a stirrer the mist is emptied by its clear and its flow is the
// current: the body is not mist, so the hole it leaves stays where it was
// made and fills back in as the cells settle, a wake trailing the body.
// `volumetric_fog.wgsl` reads the cells.

struct Params {
    // x: the region rows, y: the stirrers, z: the cells of every region.
    counts: vec4<u32>,
    // x: seconds this step.
    step: vec4<f32>,
};

struct Region {
    // xyz: centre; w: turn about the vertical, radians.
    center: vec4<f32>,
    // xyz: half extents; w: settle seconds.
    half_settle: vec4<f32>,
    // xyz: the current, metres per second.
    current: vec4<f32>,
    // xyz: cells along each axis; w: its first cell.
    cells: vec4<u32>,
};

struct Stirrer {
    // xyz: position; w: radius.
    position: vec4<f32>,
    // xyz: velocity; w: push.
    velocity: vec4<f32>,
    // x: clear.
    extra: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> regions: array<Region>;
@group(0) @binding(2) var<storage, read> stirrers: array<Stirrer>;
@group(0) @binding(3) var<storage, read> previous: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> cells: array<vec4<f32>>;

// How fast a stirrer drags the flow toward its own, per second at full
// push, and empties the mist inside it at full clear.
const STIR_RATE: f32 = 8.0;
const CLEAR_RATE: f32 = 30.0;
// A stirrer reaches this many radii out.
const REACH: f32 = 3.0;

// Turns `v` by `yaw` about the vertical (region to world), or back with
// `-yaw`.
fn turn(v: vec3<f32>, yaw: f32) -> vec3<f32> {
    let c = cos(yaw);
    let s = sin(yaw);
    return vec3<f32>(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

// The previous step's cells of `region` at `local` (region space, metres),
// trilinear; the rest state outside it.
fn previous_at(region: Region, local: vec3<f32>) -> vec4<f32> {
    let n = vec3<f32>(region.cells.xyz);
    let g = (local + region.half_settle.xyz) / (2.0 * region.half_settle.xyz) * n - 0.5;
    if any(g < vec3<f32>(-0.5)) || any(g > n - 0.5) {
        return vec4<f32>(region.current.xyz, 1.0);
    }
    let clamped = clamp(g, vec3<f32>(0.0), n - 1.0);
    let low = vec3<u32>(floor(clamped));
    let high = min(low + 1u, region.cells.xyz - 1u);
    let f = clamped - floor(clamped);
    let nx = region.cells.x;
    let nxy = region.cells.x * region.cells.y;
    let base = region.cells.w;
    let c000 = previous[base + low.x + nx * low.y + nxy * low.z];
    let c100 = previous[base + high.x + nx * low.y + nxy * low.z];
    let c010 = previous[base + low.x + nx * high.y + nxy * low.z];
    let c110 = previous[base + high.x + nx * high.y + nxy * low.z];
    let c001 = previous[base + low.x + nx * low.y + nxy * high.z];
    let c101 = previous[base + high.x + nx * low.y + nxy * high.z];
    let c011 = previous[base + low.x + nx * high.y + nxy * high.z];
    let c111 = previous[base + high.x + nx * high.y + nxy * high.z];
    return mix(
        mix(mix(c000, c100, f.x), mix(c010, c110, f.x), f.y),
        mix(mix(c001, c101, f.x), mix(c011, c111, f.x), f.y),
        f.z,
    );
}

@compute @workgroup_size(64, 1, 1)
fn cs_stir(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if index >= params.counts.z {
        return;
    }
    var row = 0u;
    for (var r = 1u; r < params.counts.x; r = r + 1u) {
        if regions[r].cells.w <= index {
            row = r;
        }
    }
    let region = regions[row];
    let n = region.cells.xyz;
    let local_index = index - region.cells.w;
    let cell = vec3<u32>(local_index % n.x, (local_index / n.x) % n.y, local_index / (n.x * n.y));
    if cell.z >= n.z {
        return;
    }
    let half = region.half_settle.xyz;
    let yaw = region.center.w;
    let local = (vec3<f32>(cell) + 0.5) / vec3<f32>(n) * 2.0 * half - half;
    let world = region.center.xyz + turn(local, yaw);
    let dt = params.step.x;
    let current = region.current.xyz;

    let here = previous[index];
    var moved = previous_at(region, local - turn(here.xyz, -yaw) * dt);
    let settle = 1.0 - exp(-dt / max(region.half_settle.w, 1e-3));
    var flow = mix(moved.xyz, current, settle);
    var density = mix(moved.w, 1.0, settle);

    for (var s = 0u; s < params.counts.y; s = s + 1u) {
        let stirrer = stirrers[s];
        let radius = stirrer.position.w;
        let offset = world - stirrer.position.xyz;
        let distance = length(offset);
        if distance >= REACH * radius {
            continue;
        }
        let body = stirrer.velocity.xyz;
        // The flow round a sphere moving at `body` relative to the current,
        // in the current's frame; the current alone inside it.
        var drag = current;
        if distance >= radius {
            let toward = offset / distance;
            let r = radius / distance;
            let moving = body - current;
            drag = current + 0.5 * r * r * r * (3.0 * dot(moving, toward) * toward - moving);
        }
        let reach = 1.0 - smoothstep((REACH - 1.0) * radius, REACH * radius, distance);
        let weight = clamp(stirrer.velocity.w * reach * dt * STIR_RATE, 0.0, 1.0);
        flow = mix(flow, drag, weight);
        let inside = 1.0 - smoothstep(0.75 * radius, radius, distance);
        density = density * (1.0 - clamp(stirrer.extra.x * inside * dt * CLEAR_RATE, 0.0, 1.0));
    }
    cells[index] = vec4<f32>(flow, clamp(density, 0.0, 4.0));
}
