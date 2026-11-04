// The sun's traced shadow for a world view, one ray per pixel (render-wgpu
// `ray_shadows.rs`): from the view's opaque depth, drawn by the occlusion
// pre-pass's pipelines, each pixel's surface is found again and traced
// toward the sun (`rusty::ray`). The world pass reads the result where its
// fragment is that surface, so a pixel the world pass shades several times
// traces once. Each pixel keeps its surface's distance from the camera,
// negative where the sun is hidden, or the largest float where nothing is.

#import rusty::ray::ray_sun_visibility

struct MaskParams {
    // The world from the pre-pass's clip space.
    inverse_view_proj: mat4x4<f32>,
    // xyz: the camera; w unused.
    camera: vec4<f32>,
    // xyz: toward the sun; w unused.
    toward: vec4<f32>,
    // The viewport's origin and size, pixels.
    viewport: vec4<u32>,
};

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var mask: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: MaskParams;

const NOTHING: f32 = 3.4e38;

// The world position of the depth at `pixel`'s centre.
fn surface_at(pixel: vec2<u32>, d: f32) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(depth));
    let uv = (vec2<f32>(pixel) + 0.5) / size;
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let world = params.inverse_view_proj * clip;
    return world.xyz / world.w;
}

// The step from `centre` to the neighbour along `axis` nearer it in depth,
// facing the positive direction: a step across an edge would bend the
// normal.
fn tangent(pixel: vec2<u32>, centre: vec3<f32>, d: f32, axis: vec2<i32>) -> vec3<f32> {
    let low = vec2<i32>(params.viewport.xy);
    let high = low + vec2<i32>(params.viewport.zw) - 1;
    let ahead = clamp(vec2<i32>(pixel) + axis, low, high);
    let behind = clamp(vec2<i32>(pixel) - axis, low, high);
    let d_ahead = textureLoad(depth, ahead, 0);
    let d_behind = textureLoad(depth, behind, 0);
    if abs(d_ahead - d) <= abs(d_behind - d) && d_ahead < 1.0 {
        return surface_at(vec2<u32>(ahead), d_ahead) - centre;
    }
    if d_behind < 1.0 {
        return centre - surface_at(vec2<u32>(behind), d_behind);
    }
    return vec3<f32>(0.0);
}

@compute @workgroup_size(8, 8)
fn cs_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.viewport.zw) {
        return;
    }
    let pixel = id.xy + params.viewport.xy;
    let d = textureLoad(depth, pixel, 0);
    if d >= 1.0 {
        textureStore(mask, pixel, vec4<f32>(NOTHING));
        return;
    }
    let position = surface_at(pixel, d);
    let to_camera = params.camera.xyz - position;
    let away = length(to_camera);
    var normal = cross(
        tangent(pixel, position, d, vec2<i32>(0, 1)),
        tangent(pixel, position, d, vec2<i32>(1, 0)),
    );
    if dot(normal, normal) < 1e-20 {
        normal = to_camera;
    }
    // Facing the camera, as the surface the world pass shades does.
    normal = normalize(select(-normal, normal, dot(normal, to_camera) >= 0.0));
    let visible = ray_sun_visibility(position, normal, params.toward.xyz, away);
    textureStore(mask, pixel, vec4<f32>(select(-away, away, visible > 0.5)));
}
