// Overlay lines: positions already in the pass's world (or viewmodel)
// coordinates, drawn with the view's camera, never jittered (render-wgpu
// `overlays.rs` `Overlays::write_camera`).

struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct LineVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_line(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> LineVarying {
    var out: LineVarying;
    out.position = camera.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_line(in: LineVarying) -> @location(0) vec4<f32> {
    return in.color;
}

// Under a motion target (render-wgpu `motion.rs`): the line also marks the
// reactive mask by its alpha, as `rusty::types::REACTIVE_OVERLAY`, so a
// temporal stage keeps none of its history there.
struct ReactiveLine {
    @location(0) color: vec4<f32>,
    @location(1) mask: vec4<f32>,
}

const REACTIVE_OVERLAY: f32 = 1.0;

@fragment
fn fs_line_reactive(in: LineVarying) -> ReactiveLine {
    return ReactiveLine(in.color, vec4<f32>(0.0, 0.0, REACTIVE_OVERLAY, in.color.a));
}
