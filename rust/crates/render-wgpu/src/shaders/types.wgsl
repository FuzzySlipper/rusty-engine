#define_import_path rusty::types

// Row layouts the renderer writes: the frame uniform (`frame.rs`), part rows
// (`tables.rs`), light rows and the material uniform (`apply.rs`).

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    // x: light count, y: first light row (world lights, then viewmodel lights)
    counts: vec4<u32>,
    // x: exposure; fog y: start, z: end (linear), w: density (exponential)
    finish: vec4<f32>,
    // rgb: linear fog colour
    fog_color: vec4<f32>,
    // x: tone mapping operator (0 none, 1 neutral, 2 ACES filmic);
    // y: fog (0 off, 1 linear, 2 exponential, 3 exponential squared)
    modes: vec4<u32>,
};

struct Part {
    model: mat4x4<f32>,
    normal_x: vec4<f32>,
    normal_y: vec4<f32>,
    normal_z: vec4<f32>,
    color: vec4<f32>,
    emission: vec4<f32>,
};

// kind in color_kind.w: 0 ambient, 1 hemisphere, 2 directional, 3 point, 4 spot.
struct Light {
    // rgb: colour * intensity (hemisphere: sky)
    color_kind: vec4<f32>,
    // xyz: world position; w: range (0 = unbounded)
    position_range: vec4<f32>,
    // xyz: world travel direction; w: decay exponent
    direction_decay: vec4<f32>,
    // hemisphere: ground colour * intensity; spot: (cos outer, cos inner);
    // w: first shadow layer + 1, or 0 without a shadow
    extra: vec4<f32>,
};

struct MaterialUniform {
    roughness: f32,
    alpha_cutoff: f32,
    metalness: f32,
    normal_scale: f32,
    // Voxel surface: xy tile scale, zw tile origin (cells).
    tile: vec4<f32>,
    // Voxel surface: xy sample min, zw sample max (texture uv).
    sample_rect: vec4<f32>,
    // Each texture slot's uv transform, two rows (xyz) applied to (u, v, 1):
    // identity unless a GLB material sets KHR_texture_transform.
    base_uv_u: vec4<f32>,
    base_uv_v: vec4<f32>,
    emissive_uv_u: vec4<f32>,
    emissive_uv_v: vec4<f32>,
    normal_uv_u: vec4<f32>,
    normal_uv_v: vec4<f32>,
    occlusion_uv_u: vec4<f32>,
    occlusion_uv_v: vec4<f32>,
    // x: occlusion strength.
    occlusion: vec4<f32>,
};

const PI: f32 = 3.141592653589793;
