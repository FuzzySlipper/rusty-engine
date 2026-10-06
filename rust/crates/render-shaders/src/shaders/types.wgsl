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
    // y: fog (0 off, 1 linear, 2 exponential, 3 exponential squared);
    // z: colour grading (0 off, 1 on)
    modes: vec4<u32>,
    // x: the Engine's presentation time in seconds: it advances with the
    // simulation, holds while it is paused, and is the same in every view.
    time: vec4<f32>,
    // Colour grading: xyz scale the white point in LMS (temperature and
    // tint).
    balance: vec4<f32>,
    // Colour grading: x the contrast exponent about middle grey, y the
    // saturation factor.
    grading: vec4<f32>,
    // The sun, the brightest directional world light: xyz toward it, w 1
    // when there is one.
    sun: vec4<f32>,
    // rgb: the sun's colour; w: its intensity.
    sun_color: vec4<f32>,
    // Atmosphere: x the fog's base height, y its falloff height (0: one
    // density everywhere), z the haze exponent (0: no haze), w the sun
    // disc's angular radius in radians (0: none).
    atmosphere: vec4<f32>,
    // rgb: the linear haze colour; w: the sun halo's strength.
    haze: vec4<f32>,
};

struct Part {
    model: mat4x4<f32>,
    // The normal matrix's columns; w: the texture-space origin (x, y, z).
    normal_x: vec4<f32>,
    normal_y: vec4<f32>,
    normal_z: vec4<f32>,
    color: vec4<f32>,
    // w: texture-space cells per model unit.
    emission: vec4<f32>,
};

// Where a triplanar material projects a model-space position from: the
// part's mesh texture space, or the model space itself.
fn texture_space_position(row: Part, position: vec3<f32>) -> vec3<f32> {
    return position * row.emission.w + vec3<f32>(row.normal_x.w, row.normal_y.w, row.normal_z.w);
}

// kind in color_kind.w: 0 ambient, 1 hemisphere, 2 directional, 3 point, 4 spot.
struct Light {
    // rgb: colour * intensity (hemisphere: sky)
    color_kind: vec4<f32>,
    // xyz: world position; w: range (0 = unbounded). A shadowed directional
    // light: each cascade's far view depth.
    position_range: vec4<f32>,
    // xyz: world travel direction; w: decay exponent
    direction_decay: vec4<f32>,
    // hemisphere: ground colour * intensity; spot: (cos outer, cos inner);
    // a shadowed directional light: xyz the view axis its cascades were
    // fitted to; w: first shadow layer + 1, or 0 without a shadow
    extra: vec4<f32>,
};

// A shadow layer (render-wgpu `shadows.rs`): its view, and where its map
// lies in the atlas.
struct ShadowView {
    view_proj: mat4x4<f32>,
    // xy: the tile's corner in its page (uv); z: its side (uv); w: the page.
    tile: vec4<f32>,
    // x: 1 for a soft (5×5) filter; y: the tile's side in texels.
    params: vec4<f32>,
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
    // a GLB material's KHR_texture_transform, or an Engine material's texture
    // transform on its base and normal slots.
    base_uv_u: vec4<f32>,
    base_uv_v: vec4<f32>,
    emissive_uv_u: vec4<f32>,
    emissive_uv_v: vec4<f32>,
    normal_uv_u: vec4<f32>,
    normal_uv_v: vec4<f32>,
    occlusion_uv_u: vec4<f32>,
    occlusion_uv_v: vec4<f32>,
    // x: occlusion strength; y: triplanar sharpness; z: stochastic tiling
    // contrast.
    factors: vec4<f32>,
    // The uv set (0 or 1) each slot reads: base, emissive, normal, occlusion.
    tex_coords: vec4<u32>,
    // A product shader's own values (`MaterialShaderDescriptor::parameters`).
    parameters: array<vec4<f32>, 4>,
    // Terrain layers 1 to 3 (TERRAIN_LAYERS): each one's `tile` and
    // `sample_rect`; layer_factors xyz: each one's normal scale, w: the
    // weight contrast.
    layer_tile: array<vec4<f32>, 3>,
    layer_rect: array<vec4<f32>, 3>,
    layer_factors: vec4<f32>,
};

// What the standard surface stages make of a world fragment, which the shade
// stage lights and finishes: `rusty::shade::standard_shade`, or a product
// shader's `shade`.
struct Surface {
    // Base colour and alpha: `tint` times the base texture.
    base: vec4<f32>,
    // The material colour and texture tint, times the node and vertex
    // colour: for a product sampling a texture of its own.
    tint: vec4<f32>,
    // The shading normal, normal map applied, facing the camera.
    normal: vec3<f32>,
    world_position: vec3<f32>,
    // The mesh uv; a voxel surface's tile coordinates in cells.
    uv: vec2<f32>,
    roughness: f32,
    metalness: f32,
    // Scales ambient light (the occlusion map).
    occlusion: f32,
    // Emitted light: the part's emission times the emissive map.
    emission: vec3<f32>,
};

// What the shadow pass gives a product shader's caster stage
// (`fn cast_shadow(caster: Caster)`), which may discard.
struct Caster {
    // The mesh uv; a voxel surface's tile coordinates in cells.
    uv: vec2<f32>,
    world_position: vec3<f32>,
    // The part and vertex alpha times the base texture's.
    alpha: f32,
};

const PI: f32 = 3.141592653589793;
