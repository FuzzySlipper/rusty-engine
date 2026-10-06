// Distance-field ambient occlusion (`render-wgpu/src/distance_fields.rs`,
// #5911): for each half-resolution texel of a world view, cone-trace the
// chunks' coarse signed distance fields from the surface along a few
// directions around the normal and write the occlusion the world pass
// multiplies into `Surface.occlusion`, like the screen-space paths.
//
// The fields sit in one 3D atlas of bricks; a lookup grid around the camera
// maps a world cell (one chunk's box) to its brick, or 0 for no field. A
// point in a cell without a field reads as open space.

struct FieldParams {
    // View space from NDC, and world from view.
    inv_projection: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    // The view's region of the occlusion texture: xy origin, zw size.
    region: vec4<u32>,
    // xyz: world origin of the lookup grid's first cell; w: cell size (a
    // chunk's box).
    grid_origin: vec4<f32>,
    // x: lookup cells per axis; y: bricks per atlas axis; z: brick texels
    // per axis; w: unused.
    shape: vec4<u32>,
    // x: the field's reach in world units (distance clamp); y: cone half
    // angle tangent; z: intensity; w: unused.
    tuning: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: FieldParams;
@group(0) @binding(1) var depth_map: texture_depth_2d;
@group(0) @binding(2) var occlusion_out: texture_storage_2d<rgba8unorm, write>;
// `cells³` entries: a brick index + 1, or 0 for no field.
@group(0) @binding(3) var<storage, read> lookup: array<u32>;
@group(0) @binding(4) var atlas: texture_3d<f32>;
@group(0) @binding(5) var atlas_sampler: sampler;

const CONES: u32 = 5u;
const STEPS: u32 = 6u;
const WORKGROUP: u32 = 16u;

fn view_position(texel: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = (texel + 0.5 - vec2<f32>(params.region.xy)) / vec2<f32>(params.region.zw);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let view = params.inv_projection * vec4<f32>(ndc, depth, 1.0);
    return view.xyz / view.w;
}

fn load_depth(texel: vec2<i32>) -> f32 {
    let clamped = clamp(texel, vec2<i32>(params.region.xy), vec2<i32>(params.region.xy + params.region.zw) - 1);
    return textureLoad(depth_map, clamped, 0);
}

// The signed distance (world units) at a world point, from the field of the
// cell it lies in; the reach where no field is resident there.
fn field_distance(world: vec3<f32>) -> f32 {
    let cell_size = params.grid_origin.w;
    let cells = i32(params.shape.x);
    let local = (world - params.grid_origin.xyz) / cell_size;
    let cell = vec3<i32>(floor(local));
    if any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(cells)) {
        return params.tuning.x;
    }
    let slot = lookup[(u32(cell.z) * params.shape.x + u32(cell.y)) * params.shape.x + u32(cell.x)];
    if slot == 0u {
        return params.tuning.x;
    }
    let brick = slot - 1u;
    let bricks = params.shape.y;
    let texels = f32(params.shape.z);
    let brick_origin = vec3<f32>(
        f32(brick % bricks),
        f32((brick / bricks) % bricks),
        f32(brick / (bricks * bricks)),
    ) * texels;
    // Within the brick, sampling at texel centres and not past them.
    let within = clamp(fract(local) * texels, vec3<f32>(0.5), vec3<f32>(texels - 0.5));
    let uvw = (brick_origin + within) / (f32(bricks) * texels);
    let encoded = textureSampleLevel(atlas, atlas_sampler, uvw, 0.0).r;
    // `svc_mesh::distance_field`: bytes encode (distance / reach + 1) / 2
    // in cells; a cell is the chunk box over the brick's texels.
    return (encoded * 2.0 - 1.0) * params.tuning.x;
}

// Normal from the depth neighbours, in view space, facing the camera.
fn reconstruct_normal(centre: vec3<f32>, left: vec3<f32>, right: vec3<f32>, up: vec3<f32>, down: vec3<f32>) -> vec3<f32> {
    let to_right = right - centre;
    let to_left = centre - left;
    let dx = select(to_left, to_right, abs(to_right.z) < abs(to_left.z));
    let to_down = down - centre;
    let to_up = centre - up;
    let dy = select(to_up, to_down, abs(to_down.z) < abs(to_up.z));
    var normal = normalize(cross(dy, dx));
    if normal.z < 0.0 {
        normal = -normal;
    }
    return normal;
}

// Visibility along one cone from `start` toward `direction`: the smallest
// ratio of field distance to cone radius along the march.
fn cone_visibility(start: vec3<f32>, direction: vec3<f32>) -> f32 {
    let reach = params.tuning.x;
    let tangent = params.tuning.y;
    var visibility = 1.0;
    var travelled = reach * 0.08;
    for (var step = 0u; step < STEPS; step++) {
        let point = start + direction * travelled;
        let distance = field_distance(point);
        let radius = max(travelled * tangent, reach * 0.02);
        visibility = min(visibility, clamp(distance / radius, 0.0, 1.0));
        travelled += max(distance, reach * 0.08);
        if travelled > reach || visibility <= 0.0 {
            break;
        }
    }
    return visibility;
}

@compute @workgroup_size(16, 16)
fn cs_field_occlusion(@builtin(global_invocation_id) global: vec3<u32>) {
    if any(global.xy >= params.region.zw) {
        return;
    }
    let texel = vec2<i32>(params.region.xy + global.xy);
    let depth = load_depth(texel);
    if depth >= 1.0 {
        textureStore(occlusion_out, texel, vec4<f32>(1.0));
        return;
    }
    let position = vec2<f32>(texel);
    let centre = view_position(position, depth);
    let normal_view = reconstruct_normal(
        centre,
        view_position(position - vec2<f32>(1.0, 0.0), load_depth(texel - vec2<i32>(1, 0))),
        view_position(position + vec2<f32>(1.0, 0.0), load_depth(texel + vec2<i32>(1, 0))),
        view_position(position - vec2<f32>(0.0, 1.0), load_depth(texel - vec2<i32>(0, 1))),
        view_position(position + vec2<f32>(0.0, 1.0), load_depth(texel + vec2<i32>(0, 1))),
    );
    let world = (params.inv_view * vec4<f32>(centre, 1.0)).xyz;
    let normal = normalize((params.inv_view * vec4<f32>(normal_view, 0.0)).xyz);
    // A tangent frame around the normal for the tilted cones.
    let helper = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normal.y) > 0.9);
    let tangent = normalize(cross(helper, normal));
    let bitangent = cross(normal, tangent);
    let tilt = 0.6;
    let directions = array<vec3<f32>, 5>(
        normal,
        normalize(normal + tangent * tilt),
        normalize(normal - tangent * tilt),
        normalize(normal + bitangent * tilt),
        normalize(normal - bitangent * tilt),
    );
    let start = world + normal * params.tuning.x * 0.05;
    var visibility = 0.0;
    for (var cone = 0u; cone < CONES; cone++) {
        visibility += cone_visibility(start, directions[cone]);
    }
    visibility = visibility / f32(CONES);
    let occlusion = clamp(1.0 - params.tuning.z * (1.0 - visibility), 0.0, 1.0);
    textureStore(occlusion_out, texel, vec4<f32>(occlusion));
}
