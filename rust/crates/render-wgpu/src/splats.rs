//! Gaussian splats: animated-mesh primitives drawn as Gaussians
//! (`docs/splat-characters.md`, `splats.wgsl`).
//!
//! **Resources.** A GLB's splat primitive (`glb::GlbSplats`) is uploaded once
//! per asset as a [`SplatSet`]: 32 bytes per Gaussian (centre, display colour
//! and opacity, rotation, standard deviations) in a storage buffer every
//! instance of the asset reads.
//!
//! **Runs.** A view's splat instances take their places among the blended
//! draws by the squared distance of their bounds' centre, as blended parts
//! do (`frame.rs` `draw_blended`). Instances that follow each other there are
//! one run. Each run's Gaussians are projected for the view and sorted back
//! to front together on the GPU, every frame (`cs_project`, the radix sort),
//! so overlapping instances interleave Gaussian by Gaussian.
//!
//! **Layers.** A run is drawn into a layer of its own, an RGBA16F image the
//! size of the view's target, tested against the world's opaque depth and
//! writing none. Splat colours are display-referred and were trained by
//! blending those values, so the layer blends them as they are: blended in
//! linear light, as the HDR target would, a figure comes out lighter. The
//! layer is composited into the HDR target at the run's place among the
//! blended draws, decoded to linear light there.
//!
//! **Depth and motion.** The same draw keeps, in the layer's surface
//! target, the depth and motion of what it drew, blended as the colour is:
//! each Gaussian's depth and centre's motion weighted by its share of the
//! pixel, its centre placed where it stood by the set's previous model and
//! palette (kept by instance and set from the last frame that drew
//! motion). Where the layer covers at least half a pixel, a second
//! composite draw writes those into the view's depth and the motion target
//! where there is one: the finish fogs a figure at its own distance, what
//! draws after it meets it, and a temporal stage follows it. The colour
//! composite marks the reactive mask under the rest of the layer. Writing
//! the surface target costs about as much as the layer's colour, so a run
//! keeps it only when something after it reads the view's depth or motion
//! (`frame.rs` says what does); otherwise the run leaves the depth as the
//! opaque world left it.
//!
//! **Pixel grid.** An instance on a pixel grid (`SplatAppearance`) draws in
//! a run of its own, its layer untested against the world so a block an
//! occluder crosses keeps the figure's coverage. Its composite is one draw
//! over the blocks its bounds project to (`splats.wgsl` `grid_block`): each
//! block the layer's coverage-weighted average in display space, snapped
//! to its palette, at the layer's depth there, tested pixel by pixel
//! against the world's.
//!
//! **Lighting.** A lit instance (`SplatAppearance::lighting`) draws in runs
//! of lit instances. Its projection turns each Gaussian's normal (the
//! asset's `_NORMAL`, or its shortest axis pointing away from the centre of
//! its joint's Gaussians, `normal_words`) to view space, and its layer keeps
//! that normal's xy beside the depth, blended as the colour is: in the
//! surface target, or under motion in a target of its own. Its composite
//! (`splat_light.wgsl`) lights the blended colour per pixel with the
//! world's `standard_radiance` at the blended depth and normal; a pixel
//! grid's block is averaged, lit once, then snapped.
//!
//! **Casting.** An instance whose appearance casts (`SplatShadowCaster`)
//! is listed for each shadow layer that sees it (`prepare_casters`). As its
//! Gaussians, `cs_project` re-projects them with the layer's light into the
//! run buffers before the layer's pass (`project_casters`), and the pass
//! draws them unsorted, each cut where its alpha falls under 0.3
//! (`splat_caster.wgsl`). As ellipsoids, a unit sphere per joint stands
//! around the Gaussians that joint carries most, posed with it.
//!
//! **Cost.** Projecting and sorting run on the GPU for every view each frame
//! with a splat run; nothing is copied to or from the CPU per frame. The
//! readout counts the frame's runs, instances and Gaussians, and the
//! `splats` pass timer covers the projection, the sort and the layers of the
//! frame's first view with a run.

use std::collections::HashMap;

use glam::{DMat4, Mat4, Vec3};

use crate::glb::GlbSplats;
use crate::motion::MotionWrite;
use crate::tables::Aabb;
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::timing::PassTimer;
use crate::Gpu;

/// Bytes of one resident Gaussian (`splats.wgsl` `Gaussian`).
const GAUSSIAN_BYTES: u64 = 32;
/// Bytes of one Gaussian's skin (`splats.wgsl` `skins`): four joints and
/// four weights, a byte each.
const SKIN_BYTES: u64 = 8;
/// The most joints a skinned set's Gaussians index: a joint is one byte.
pub(crate) const MAX_SKIN_JOINTS: usize = 256;
/// Bytes of one projected Gaussian (`splats.wgsl` `Projected`).
const PROJECTED_BYTES: u64 = 48;
/// One dynamic uniform slot of the parameters buffer (`splats.wgsl`
/// `Project` within it).
const PARAMS_SLOT: u64 = 512;
const WORKGROUP: u32 = 256;
/// Keys a sort block takes (`splats.wgsl` `TILE`).
const TILE: u32 = 1024;
const DIGITS: u32 = 256;
/// Workgroup storage the sort's scatter uses: two tiles of keys and values
/// and three digit tables.
const SORT_SHARED_BYTES: u32 = 2 * TILE * 4 + 3 * DIGITS * 4;
/// The most Gaussians one run sorts: the digit counts are scanned by groups
/// of `TILE` whose totals one workgroup scans, so at most `TILE` groups.
pub(crate) const MAX_RUN_GAUSSIANS: u32 = TILE * TILE * TILE / DIGITS;
/// The sort keys' digit shifts: 16-bit keys, two passes of eight bits.
const SORT_SHIFTS: [u32; 2] = [0, 8];
/// How far past a Gaussian's centre its bounds reach, in standard
/// deviations: its quad's reach, 2·sqrt(2)σ, rounded up.
const BOUNDS_SIGMAS: f32 = 3.0;
const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// A layer's surface target (`splats.wgsl` `fs_splat`): its Gaussians'
/// `1 − depth` and, while the view draws motion, their motion,
/// premultiplied and blended as the colour is. Half floats keep a depth's
/// distance to about a part in 2048.
/// A layer's run parameters (`splat_layer.wgsl` `Grid`): seven rows of
/// four words, then each palette colour's linear and sRGB values.
const GRID_HEADER_BYTES: u64 = 112;
const GRID_BYTES: u64 = GRID_HEADER_BYTES + render_model::PARTS_RIG_PALETTE_MAX as u64 * 2 * 16;
/// Bounds reaching this close to the eye's plane (clip w) draw smooth.
const GRID_NEAREST_W: f32 = 1e-4;

/// A lit run's normal target under motion (`splats.wgsl`
/// `fs_splat_lit_motion`): its Gaussians' view-space normal xy, weighted by
/// alpha.
const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

fn surface_format(motion: bool) -> wgpu::TextureFormat {
    if motion {
        wgpu::TextureFormat::Rgba16Float
    } else {
        wgpu::TextureFormat::R16Float
    }
}

/// One splat primitive of an animated asset, resident on the GPU.
pub(crate) struct SplatSet {
    /// Its Gaussians.
    pub count: u32,
    /// The Gaussians to `BOUNDS_SIGMAS`, in the primitive's space.
    pub bounds: Aabb,
    /// For a skinned primitive (its centres in bind space), the Gaussians
    /// each skin joint moves (`animated.rs` `posed_splat_bounds`).
    pub joint_bounds: Option<Vec<JointReach>>,
    /// The ellipsoids it casts as (`SplatShadowCaster::Ellipsoids`).
    pub ellipsoids: Vec<Ellipsoid>,
    group: wgpu::BindGroup,
}

/// An ellipsoid caster: the box of the centres whose heaviest joint is
/// `joint` (the whole set's when unskinned), widened by their mean reach, in
/// the set's space (bind space when skinned).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ellipsoid {
    pub joint: Option<usize>,
    pub centre: Vec3,
    pub half: Vec3,
}

/// The Gaussians one skin joint moves: under a pose every centre stands
/// within the union of these centre boxes, each moved by its joint (a
/// blend of positions lies in their hull). The palette moves the centres
/// only: the shader turns a Gaussian by the blend with its columns'
/// lengths removed, so its reach stays as it is in the instance's space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JointReach {
    /// The centres, in bind space.
    pub centres: Aabb,
    /// The farthest any of them reaches past its centre, `BOUNDS_SIGMAS`
    /// along its longest axis.
    pub reach: f32,
}

/// One set of an instance as a view draws it.
pub(crate) struct SplatDraw {
    pub set: SetRef,
    /// The set's space to the world: the instance's node for a skinned
    /// set (its palette takes bind space to the instance's), and the
    /// GLB node's place under it for a rigid one.
    pub model: Mat4,
    /// A skinned set's joint matrices in the instance's space, scale
    /// removed, after the inverse binds: rows 0 to 2 of each.
    pub palette: Vec<[f32; 12]>,
    /// Linear-light colour multiplier (`SplatAppearance`).
    pub multiplier: [f32; 3],
}

/// A set of a run, resolved for drawing.
pub(crate) struct RunSet<'a> {
    pub set: &'a SplatSet,
    pub draw: &'a SplatDraw,
    /// Its instance's node and its index among the instance's sets: what
    /// its previous placing is kept by.
    pub key: (render_model::RenderHandle, usize),
}

/// What the splats drew over the last frame, summed over its views.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplatReadout {
    /// Runs drawn: each a layer, its own projection and sort.
    pub runs: u32,
    /// Splat instances drawn.
    pub instances: u32,
    /// Gaussians projected and sorted.
    pub gaussians: u64,
    /// Splat instances that had no room in their run
    /// ([`MAX_RUN_GAUSSIANS`]) and drew nothing.
    pub refused_instances: u32,
    /// Why splats draw nothing on this device, if they do not.
    pub refused: Option<String>,
    /// Why figures that cast as their Gaussians cast as ellipsoids, if
    /// any did.
    pub refused_casters: Option<String>,
}

/// One instance's sets as one view draws them.
pub(crate) struct SplatInstance {
    /// Its node: the order between instances at one distance.
    pub handle: render_model::RenderHandle,
    pub sets: Vec<SplatDraw>,
    /// The sets' world bounds.
    pub bounds: Aabb,
    /// Squared distance from the eye to the bounds' centre: its place among
    /// the blended draws.
    pub depth: f32,
    /// Its pixel grid: it draws in a run of its own.
    pub grid: Option<SplatGrid>,
    /// Lit by the world's lights (`SplatAppearance::lighting`), with its
    /// light floor: it draws in runs of lit instances with that floor.
    pub light: Option<f32>,
    /// How it casts into shadow layers, when it casts.
    pub caster: Option<render_model::SplatShadowCaster>,
}

/// A set by its asset's slot and its index among the asset's sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SetRef {
    pub slot: u32,
    pub set: usize,
}

/// An instance drawn on a screen-pixel grid
/// (`SplatAppearance::pixel_grid`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SplatGrid {
    /// Block side in output pixels.
    pub pixels: u32,
    pub cutoff: f32,
    /// Linear colours a block takes the nearest of, by sRGB value.
    pub palette: Vec<[f32; 3]>,
    /// The instance's origin in the world, where the grid is anchored.
    pub origin: Vec3,
}

/// A run as a view draws it.
pub(crate) struct SplatRun<'a> {
    pub sets: Vec<RunSet<'a>>,
    /// Its nearest and farthest view depth, which its sort keys span.
    pub depth_range: (f32, f32),
    /// Whether something after it reads the view's depth or motion, as
    /// everything does while the view draws motion.
    pub read_after: bool,
    /// Its one instance's pixel grid, with the instance's world bounds.
    pub grid: Option<(&'a SplatGrid, Aabb)>,
    /// Lit, with its instances' light floor.
    pub light: Option<f32>,
}

/// How a run's layer is composited.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Finish {
    /// Smooth, writing the depth and motion its surface kept, if it kept them.
    Smooth { surface: bool, lit: bool },
    /// On a pixel grid: its blocks, inside the scissor rectangle
    /// (x, y, width, height in target pixels).
    Grid { scissor: [u32; 4], lit: bool },
    /// On a pixel grid wholly outside the viewport: nothing.
    Off,
}

impl Finish {
    /// Whether the run's layer keeps its surface: a grid's blocks and a lit
    /// run's light take their depth from it.
    fn keeps_surface(&self) -> bool {
        match self {
            Self::Smooth { surface, lit } => *surface || *lit,
            Self::Grid { .. } | Self::Off => true,
        }
    }

    fn lit(&self) -> bool {
        matches!(
            self,
            Self::Smooth { lit: true, .. } | Self::Grid { lit: true, .. }
        )
    }

    /// Whether the run's layer is tested against the world's depth. A
    /// grid's is drawn whole, as a parts rig's scratch image is, so a block
    /// an occluder crosses keeps the figure's coverage behind it; each of
    /// its pixels is tested as the block draws.
    fn tested(&self) -> bool {
        matches!(self, Self::Smooth { .. })
    }
}

/// A block side in target pixels, a grid anchor (a block corner) and the
/// blocks it covers (x0, y0, x1, y1, the far edges exclusive), or why there
/// are none.
#[derive(Clone, Copy, Debug, PartialEq)]
enum GridPlacement {
    Grid {
        side: u32,
        anchor: [i32; 2],
        rect: [i32; 4],
    },
    Off,
    /// The bounds reach the eye's plane: their projection is unbounded,
    /// and the figure draws smooth.
    Smooth,
}

/// A grid of blocks of `side` target pixels anchored at `origin`'s
/// projection, over the projection of `bounds` widened by a pixel, cut to
/// the viewport.
fn place_grid(
    bounds: &Aabb,
    origin: Vec3,
    view_proj: &Mat4,
    viewport: crate::frame::PixelRect,
    side: u32,
) -> GridPlacement {
    let size = glam::Vec2::new(viewport.width as f32, viewport.height as f32);
    let corner = glam::Vec2::new(viewport.x as f32, viewport.y as f32);
    let pixel = |point: Vec3| {
        let clip = *view_proj * point.extend(1.0);
        (clip.w > GRID_NEAREST_W).then(|| {
            let ndc = clip.truncate().truncate() / clip.w;
            corner + glam::Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * size
        })
    };
    let Some(anchor) = pixel(origin) else {
        return GridPlacement::Smooth;
    };
    let (mut low, mut high) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for index in 0..8 {
        let point = Vec3::new(
            if index & 1 == 0 {
                bounds.min.x
            } else {
                bounds.max.x
            },
            if index & 2 == 0 {
                bounds.min.y
            } else {
                bounds.max.y
            },
            if index & 4 == 0 {
                bounds.min.z
            } else {
                bounds.max.z
            },
        );
        let Some(at) = pixel(point) else {
            return GridPlacement::Smooth;
        };
        low = low.min(at);
        high = high.max(at);
    }
    let anchor = anchor.round();
    let step = side as f32;
    let snap = |value: f32, anchor: f32, round: fn(f32) -> f32| {
        anchor + round((value - anchor) / step) * step
    };
    let rect = [
        snap(low.x - 1.0, anchor.x, f32::floor).max(corner.x),
        snap(low.y - 1.0, anchor.y, f32::floor).max(corner.y),
        snap(high.x + 1.0, anchor.x, f32::ceil).min(corner.x + size.x),
        snap(high.y + 1.0, anchor.y, f32::ceil).min(corner.y + size.y),
    ];
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return GridPlacement::Off;
    }
    GridPlacement::Grid {
        side,
        anchor: [anchor.x as i32, anchor.y as i32],
        rect: rect.map(|value| value as i32),
    }
}

/// The view a run is drawn for.
pub(crate) struct SplatTarget<'a> {
    pub depth: &'a wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub viewport: crate::frame::PixelRect,
    /// The camera the pass draws with (jittered when it is).
    pub camera: crate::camera::CameraMatrices,
    /// Render pixels per output pixel: a pixel grid's blocks are whole
    /// output pixels.
    pub render_scale: f32,
    /// While the view draws motion: its previous projection.
    pub motion: Option<SplatMotion>,
}

/// A view's previous projection (`motion.rs` `MotionPass`), for the motion
/// of each Gaussian's centre.
pub(crate) struct SplatMotion {
    /// The frame's index among frames that draw motion
    /// (`Temporal::motion_frames`).
    pub frame: u64,
    /// The previous frame's projection of a point's offset from its camera.
    pub previous_view_proj: Mat4,
    /// Clip space to this frame's jittered target pixels.
    pub to_pixels: Mat4,
    pub previous_eye: Vec3,
}

/// Where a set stood: its model and palette (`SplatDraw`).
#[derive(Clone)]
struct Placed {
    model: Mat4,
    palette: Vec<[f32; 12]>,
}

/// A set's placing in the last frame that drew motion and in the one
/// before it.
struct History {
    frame: u64,
    current: Placed,
    previous: Placed,
}
/// One shadow layer's splat casters as the frame lists them
/// (`Renderer::prepare_splat_casters`).
pub(crate) struct LayerCasters<'a> {
    /// The layer's light as a camera (`ShadowLayer::light_camera`), the
    /// side of its tile in texels, and whether it is a cascade (its
    /// projection orthographic, `view_proj` alone).
    pub camera: crate::camera::CameraMatrices,
    pub tile: u32,
    pub cascade: bool,
    /// The sets cast as their Gaussians.
    pub splats: Vec<(&'a SplatSet, &'a SplatDraw)>,
    /// The sets cast as ellipsoids.
    pub ellipsoids: Vec<(&'a SplatSet, &'a SplatDraw)>,
}

/// What one shadow layer's splat instances cast this frame.
#[derive(Default)]
struct CasterLayer {
    /// Each set cast as its Gaussians: its slot in the caster parameters,
    /// its Gaussians and its bind group.
    sets: Vec<(u32, u32, wgpu::BindGroup)>,
    /// Their Gaussians in all: the slots its draw reads.
    total: u32,
    /// Its rows of the ellipsoid buffer.
    ellipsoids: std::ops::Range<u32>,
}

/// Splat instances casting into shadow layers: their projection parameters
/// and palettes (apart from a view's, which the same frame writes), each
/// ellipsoid's placing, the bind groups over the run buffers, and the
/// caster pipelines.
struct Casters {
    params: wgpu::Buffer,
    slots: u64,
    palettes: wgpu::Buffer,
    palette_bytes: u64,
    ellipsoids: wgpu::Buffer,
    ellipsoid_bytes: u64,
    /// The run buffers' generation they name, the projection's group and
    /// the draw's.
    groups: Option<(u64, wgpu::BindGroup, wgpu::BindGroup)>,
    layers: Vec<CasterLayer>,
    /// The Gaussians' caster and the ellipsoids'.
    pipelines: Option<[wgpu::RenderPipeline; 2]>,
}

/// Floats an ellipsoid row takes: its unit sphere's placing.
const ELLIPSOID_ROW_FLOATS: usize = 16;

impl Casters {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            params: params_buffer(device, 16),
            slots: 16,
            palettes: palette_buffer(device, 4096),
            palette_bytes: 4096,
            ellipsoids: ellipsoid_buffer(device, 4096),
            ellipsoid_bytes: 4096,
            groups: None,
            layers: Vec::new(),
            pipelines: None,
        }
    }
}

fn ellipsoid_buffer(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu splat ellipsoids"),
        size: bytes,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A palette joint (rows 0 to 2 of its matrix) as a matrix.
fn joint_matrix(rows: &[f32; 12]) -> Mat4 {
    Mat4::from_cols_array(&[
        rows[0], rows[4], rows[8], 0.0, rows[1], rows[5], rows[9], 0.0, rows[2], rows[6], rows[10],
        0.0, rows[3], rows[7], rows[11], 1.0,
    ])
}

struct Pipelines {
    project_layout: wgpu::BindGroupLayout,
    gaussian_layout: wgpu::BindGroupLayout,
    sort_layout: wgpu::BindGroupLayout,
    draw_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    draw_pipeline_layout: wgpu::PipelineLayout,
    composite_pipeline_layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    /// `splat_light.wgsl`: a lit run's composite.
    light_module: wgpu::ShaderModule,
    project: wgpu::ComputePipeline,
    sort_count: wgpu::ComputePipeline,
    scan_local: wgpu::ComputePipeline,
    scan_sums: wgpu::ComputePipeline,
    scan_add: wgpu::ComputePipeline,
    sort_scatter: wgpu::ComputePipeline,
}

/// The buffers a run is projected and sorted in, reused by every run.
struct RunBuffers {
    capacity: u32,
    /// The projection's records, and its keys and values: two passes sort
    /// them back into that pair (a test reads them back). A shadow layer's
    /// casters are projected into the same buffers, unsorted.
    projected: wgpu::Buffer,
    keys: wgpu::Buffer,
    values: wgpu::Buffer,
    args: wgpu::Buffer,
    project_group: wgpu::BindGroup,
    /// Pass 0 sorts the projection's keys into the second pair, pass 1 back.
    sort_groups: [wgpu::BindGroup; 2],
    draw_group: wgpu::BindGroup,
}

/// A run's layer: colour and the surface target at the view's samples,
/// resolved when multisampled, and the bind group the composite reads them
/// by.
struct Layer {
    /// Width, height, samples, whether its surface holds more than depth
    /// (motion or a lit run's normal), and whether it has a normal target.
    size: (u32, u32, u32, bool, bool),
    color: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
    surface: wgpu::TextureView,
    surface_resolve: Option<wgpu::TextureView>,
    /// A lit run's normal target under motion, and its resolve.
    normal: Option<(wgpu::TextureView, Option<wgpu::TextureView>)>,
    /// Its run's parameters (`splat_layer.wgsl` `Grid`).
    grid: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub(crate) struct Splats {
    pipelines: Option<Pipelines>,
    refusal: Option<String>,
    run: Option<RunBuffers>,
    params: wgpu::Buffer,
    params_slots: u64,
    /// The view's skinned sets' palettes (`splats.wgsl` `palettes`).
    palettes: wgpu::Buffer,
    palette_bytes: u64,
    /// Each view's runs draw into the first of these; a view submits its
    /// work before the next view reuses them.
    layers: Vec<Layer>,
    /// The most layers a view drew into this frame; the rest are dropped at
    /// the next.
    layers_used: usize,
    /// By samples, whether the layer is tested against the world's depth,
    /// when it keeps its surface whether that holds motion, and whether the
    /// run is lit.
    draws: HashMap<(u32, bool, Option<bool>, bool), wgpu::RenderPipeline>,
    /// How each of the view's runs is composited.
    finishes: Vec<Finish>,
    /// The viewport the view's runs were drawn for, which a grid's scissor
    /// is put back to.
    viewport: crate::frame::PixelRect,
    /// By target: the colour composite, the depth and motion one, and a
    /// pixel grid's.
    composites: HashMap<ColorTarget, [wgpu::RenderPipeline; 3]>,
    /// By target: a lit run's colour composite and its pixel grid's
    /// (`splat_light.wgsl`).
    lit_composites: HashMap<ColorTarget, [wgpu::RenderPipeline; 2]>,
    /// Each drawn set's placing, while frames draw motion.
    history: HashMap<(render_model::RenderHandle, usize), History>,
    /// Counts the run buffers made: the casters' bind groups name one.
    run_generation: u64,
    /// What the frame's shadow layers' splat instances cast.
    casters: Casters,
    /// A software adapter casts ellipsoids for `SplatShadowCaster::Splats`.
    software: bool,
    timer: Option<PassTimer>,
    timed: bool,
    counts: SplatReadout,
    last: SplatReadout,
}

impl Splats {
    pub fn new(
        gpu: &Gpu,
        module: wgpu::ShaderModule,
        light_module: wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let device = &gpu.device;
        let refusal = gpu
            .compute_refusal([WORKGROUP, 1, 1], SORT_SHARED_BYTES)
            .or_else(|| {
                let flags = gpu.adapter.get_downlevel_capabilities().flags;
                (!flags.contains(wgpu::DownlevelFlags::VERTEX_STORAGE))
                    .then(|| "the adapter cannot read storage buffers in vertex shaders".to_owned())
                    .or_else(|| {
                        (!flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)).then(|| {
                            "the adapter cannot draw from GPU-written indirect arguments".to_owned()
                        })
                    })
            });
        let pipelines = refusal
            .is_none()
            .then(|| Pipelines::new(device, module, light_module, frame_layout));
        Self {
            pipelines,
            refusal,
            run: None,
            params: params_buffer(device, 16),
            params_slots: 16,
            palettes: palette_buffer(device, 4096),
            palette_bytes: 4096,
            layers: Vec::new(),
            layers_used: 0,
            draws: HashMap::new(),
            finishes: Vec::new(),
            viewport: crate::frame::PixelRect::whole(1, 1),
            composites: HashMap::new(),
            lit_composites: HashMap::new(),
            history: HashMap::new(),
            run_generation: 0,
            casters: Casters::new(device),
            software: gpu.is_software(),
            timer: PassTimer::new(gpu, "splats"),
            timed: false,
            counts: SplatReadout::default(),
            last: SplatReadout::default(),
        }
    }

    /// Whether this device draws splats.
    pub fn draws(&self) -> bool {
        self.pipelines.is_some()
    }

    /// Upload a splat primitive's Gaussians, or `None` on a device that
    /// draws no splats.
    pub fn upload(&self, gpu: &Gpu, label: &str, splats: &GlbSplats) -> Option<SplatSet> {
        let pipelines = self.pipelines.as_ref()?;
        let (words, bounds) = gaussian_words(splats);
        let count = splats.centres.len() as u32;
        let skin = skin_words(splats);
        let storage = |words: &[u32], room: usize| {
            // An empty buffer cannot be bound: keep one record's room.
            let mut bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
            bytes.resize(bytes.len().max(room), 0);
            crate::gpu::buffer_init(
                &gpu.device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::STORAGE,
                },
            )
        };
        let gaussians = storage(&words, GAUSSIAN_BYTES as usize);
        let normals = storage(&normal_words(splats), 4);
        let skin_buffer = storage(
            skin.as_ref().map_or(&[][..], |(words, _)| words.as_slice()),
            SKIN_BYTES as usize,
        );
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipelines.gaussian_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gaussians.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: normals.as_entire_binding(),
                },
            ],
        });
        Some(SplatSet {
            count,
            bounds,
            joint_bounds: skin.map(|(_, bounds)| bounds),
            ellipsoids: ellipsoids(splats),
            group,
        })
    }

    /// At the start of a frame: take the last frame's counts and timing,
    /// and drop the layers the last frame did not draw into.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        self.timed = false;
        self.last = std::mem::take(&mut self.counts);
        self.layers.truncate(std::mem::take(&mut self.layers_used));
    }

    /// Whether this device casts `SplatShadowCaster::Splats` as Gaussians:
    /// a software adapter casts ellipsoids instead.
    pub fn casts_splats(&self) -> bool {
        self.pipelines.is_some() && !self.software
    }

    /// A figure that casts as its Gaussians cast as ellipsoids this frame.
    pub fn caster_refused(&mut self) {
        self.counts.refused_casters =
            Some("a software adapter casts splat figures as ellipsoids".to_owned());
    }

    /// List what each shadow layer's splat instances cast this frame
    /// (`layers[i]` for layer `i`, `None` for one that takes none): write
    /// the Gaussians' projection parameters and palettes, and each
    /// ellipsoid's placing.
    pub fn prepare_casters(&mut self, gpu: &Gpu, layers: &[Option<LayerCasters<'_>>]) {
        self.casters.layers.clear();
        if self.pipelines.is_none() {
            return;
        }
        let largest = layers
            .iter()
            .flatten()
            .map(|layer| layer.splats.iter().map(|(set, _)| set.count).sum::<u32>())
            .max()
            .unwrap_or(0);
        if largest > 0 {
            self.ensure_run(&gpu.device, largest);
        }
        let (mut bytes, mut palettes, mut rows) = (Vec::new(), Vec::<f32>::new(), Vec::new());
        for layer in layers {
            let mut out = CasterLayer::default();
            if let Some(layer) = layer {
                let view_proj = layer.camera.view_proj.as_dmat4();
                let side = layer.tile as f32;
                // A cascade's view is its orthographic `view_proj` scaled to
                // its tile's texels (one texel a unit); a perspective layer
                // has its camera's view and lens.
                let (view, lens, ortho) = if layer.cascade {
                    let texels = DMat4::from_scale(glam::DVec3::new(
                        f64::from(side) * 0.5,
                        f64::from(side) * 0.5,
                        1.0,
                    ));
                    (texels * view_proj, [1.0, 1.0, side, side], 1.0)
                } else {
                    let projection = layer.camera.projection;
                    (
                        layer.camera.view.as_dmat4(),
                        [
                            projection.x_axis.x * 0.5 * side,
                            projection.y_axis.y * 0.5 * side,
                            side,
                            side,
                        ],
                        0.0,
                    )
                };
                for (set, draw) in &layer.splats {
                    let palette_first = (palettes.len() / 4) as u32;
                    for joint in &draw.palette {
                        palettes.extend_from_slice(joint);
                    }
                    let skinned = set.joint_bounds.is_some() && !draw.palette.is_empty();
                    let model = draw.model.as_dmat4();
                    let slot = ProjectSlot {
                        model_view: view * model,
                        clip_from_model: view_proj * model,
                        previous_pixels: DMat4::IDENTITY,
                        lens,
                        depth: [0.0, 1.0, ortho, 0.0],
                        range: [out.total, set.count, 0, 0],
                        skin: [palette_first, u32::from(skinned), palette_first, 0],
                        tint: [1.0, 1.0, 1.0, 0.0],
                        pixels: [0.0; 4],
                    };
                    out.sets
                        .push((bytes.len() as u32, set.count, set.group.clone()));
                    bytes.extend_from_slice(&slot.bytes());
                    out.total += set.count;
                }
                let first = (rows.len() / ELLIPSOID_ROW_FLOATS) as u32;
                for (set, draw) in &layer.ellipsoids {
                    for ellipsoid in &set.ellipsoids {
                        let joint = match ellipsoid.joint {
                            Some(joint) => match draw.palette.get(joint) {
                                Some(rows) => joint_matrix(rows),
                                None => continue,
                            },
                            None => Mat4::IDENTITY,
                        };
                        // The unit sphere's radius is a half.
                        let place = draw.model
                            * joint
                            * Mat4::from_translation(ellipsoid.centre)
                            * Mat4::from_scale(ellipsoid.half * 2.0);
                        rows.extend_from_slice(&place.to_cols_array());
                    }
                }
                out.ellipsoids = first..(rows.len() / ELLIPSOID_ROW_FLOATS) as u32;
            }
            self.casters.layers.push(out);
        }
        let device = &gpu.device;
        let slots = (bytes.len() as u64 / PARAMS_SLOT).max(1);
        if slots > self.casters.slots {
            self.casters.slots = slots.next_power_of_two();
            self.casters.params = params_buffer(device, self.casters.slots);
            self.casters.groups = None;
        }
        let palette_bytes = (palettes.len() as u64 * 4).max(16);
        if palette_bytes > self.casters.palette_bytes {
            self.casters.palette_bytes = palette_bytes.next_power_of_two();
            self.casters.palettes = palette_buffer(device, self.casters.palette_bytes);
            self.casters.groups = None;
        }
        let ellipsoid_bytes = (rows.len() as u64 * 4).max(64);
        if ellipsoid_bytes > self.casters.ellipsoid_bytes {
            self.casters.ellipsoid_bytes = ellipsoid_bytes.next_power_of_two();
            self.casters.ellipsoids = ellipsoid_buffer(device, self.casters.ellipsoid_bytes);
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.casters.params, 0, &bytes);
        }
        if !palettes.is_empty() {
            let palette_bytes: Vec<u8> = palettes.iter().flat_map(|v| v.to_le_bytes()).collect();
            gpu.queue
                .write_buffer(&self.casters.palettes, 0, &palette_bytes);
        }
        if !rows.is_empty() {
            let row_bytes: Vec<u8> = rows.iter().flat_map(|v| v.to_le_bytes()).collect();
            gpu.queue
                .write_buffer(&self.casters.ellipsoids, 0, &row_bytes);
        }
        let (Some(pipelines), Some(run)) = (&self.pipelines, &self.run) else {
            return;
        };
        if self
            .casters
            .groups
            .as_ref()
            .is_some_and(|(generation, _, _)| *generation == self.run_generation)
        {
            return;
        }
        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        let project = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat casters project"),
            layout: &pipelines.project_layout,
            entries: &[
                entry(
                    0,
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.casters.params,
                        offset: 0,
                        size: wgpu::BufferSize::new(PARAMS_SLOT),
                    }),
                ),
                entry(1, run.projected.as_entire_binding()),
                entry(2, run.keys.as_entire_binding()),
                entry(3, run.values.as_entire_binding()),
                entry(4, run.args.as_entire_binding()),
                entry(5, self.casters.palettes.as_entire_binding()),
            ],
        });
        // The caster reads each slot and its key (`CULLED` for one the
        // projection dropped) where the view's draw reads the sorted order.
        let draw = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat casters draw"),
            layout: &pipelines.draw_layout,
            entries: &[
                entry(20, run.projected.as_entire_binding()),
                entry(21, run.keys.as_entire_binding()),
            ],
        });
        self.casters.groups = Some((self.run_generation, project, draw));
    }

    /// Whether shadow layer `layer` takes splat casters this frame.
    pub fn casts_into(&self, layer: usize) -> bool {
        self.casters
            .layers
            .get(layer)
            .is_some_and(|layer| layer.total > 0 || !layer.ellipsoids.is_empty())
    }

    /// Project shadow layer `layer`'s Gaussian casters for its light, before
    /// its pass: into the run buffers, which the frame's views reuse after.
    pub fn project_casters(&self, encoder: &mut wgpu::CommandEncoder, layer: usize) {
        let (Some(pipelines), Some(layer), Some((generation, project, _))) = (
            &self.pipelines,
            self.casters.layers.get(layer),
            &self.casters.groups,
        ) else {
            return;
        };
        if layer.total == 0 || *generation != self.run_generation {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("render-wgpu splat casters project"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.project);
        for (offset, count, group) in &layer.sets {
            pass.set_bind_group(0, project, &[*offset]);
            pass.set_bind_group(1, group, &[]);
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP).max(1), 1, 1);
        }
    }

    /// Draw shadow layer `layer`'s splat casters into its pass, whose groups
    /// 0 and 2 the shadow pass bound: the Gaussians `project_casters`
    /// projected, then the ellipsoids, each a `sphere`. Returns the draws.
    pub fn draw_casters(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        layer: usize,
        sphere: Option<&crate::tables::GpuMesh>,
    ) -> u32 {
        let (Some([splats, ellipsoids]), Some(layer)) =
            (&self.casters.pipelines, self.casters.layers.get(layer))
        else {
            return 0;
        };
        let mut draws = 0;
        if let Some((_, _, draw)) = self
            .casters
            .groups
            .as_ref()
            .filter(|(generation, _, _)| layer.total > 0 && *generation == self.run_generation)
        {
            pass.set_pipeline(splats);
            pass.set_bind_group(1, draw, &[]);
            pass.draw(0..4, 0..layer.total);
            draws += 1;
        }
        if let (false, Some(sphere)) = (layer.ellipsoids.is_empty(), sphere) {
            let count = sphere.groups.first().map_or(0, |group| group.2);
            pass.set_pipeline(ellipsoids);
            pass.set_vertex_buffer(0, sphere.vertices.slice(..));
            pass.set_vertex_buffer(1, self.casters.ellipsoids.slice(..));
            pass.set_index_buffer(sphere.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..count, 0, layer.ellipsoids.clone());
            draws += 1;
        }
        draws
    }

    pub fn has_caster_pipelines(&self) -> bool {
        self.casters.pipelines.is_some()
    }

    /// Make the caster pipelines once a layer takes splat casters: groups
    /// 0 (`casters`) and 2 (`layer`) the shadow pass's, 1 the run buffers.
    pub fn ensure_caster_pipelines(
        &mut self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        casters: &wgpu::BindGroupLayout,
        layer: &wgpu::BindGroupLayout,
    ) {
        let Some(pipelines) = &self.pipelines else {
            return;
        };
        if self.casters.pipelines.is_some() {
            return;
        }
        // The ellipsoids read no run buffers: their group 1 is empty.
        let layout = |draw: Option<&wgpu::BindGroupLayout>| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu splat casters"),
                bind_group_layouts: &[Some(casters), draw, Some(layer)],
                immediate_size: 0,
            })
        };
        let splat_layout = layout(Some(&pipelines.draw_layout));
        let ellipsoid_layout = layout(None);
        let depth = || {
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: crate::pipelines::CASTER_SLOPE_BIAS,
                    clamp: 0.0,
                },
            })
        };
        let splats = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu splat casters"),
            layout: Some(&splat_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_splat_caster"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: depth(),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_splat_caster"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        let position = wgpu::vertex_attr_array![0 => Float32x3];
        let place = wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4
        ];
        let ellipsoids = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu splat ellipsoid casters"),
            layout: Some(&ellipsoid_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_ellipsoid_caster"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (crate::pipelines::VERTEX_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &position,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (ELLIPSOID_ROW_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &place,
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth(),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        self.casters.pipelines = Some([splats, ellipsoids]);
    }

    pub fn readout(&self) -> SplatReadout {
        SplatReadout {
            refused: self.refusal.clone(),
            ..self.last.clone()
        }
    }

    pub fn timing(&self) -> crate::timing::GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed("splats"), PassTimer::readout)
    }

    /// Instances refused for want of room in a run.
    pub fn refused_instances(&mut self, count: u32) {
        self.counts.refused_instances += count;
    }

    /// Make the composite's pipelines for a pass drawing into `format`
    /// before the pass borrows the renderer, a lit run's too when `lit`.
    pub fn prepare_target(&mut self, device: &wgpu::Device, format: ColorTarget, lit: bool) {
        let Some(pipelines) = &self.pipelines else {
            return;
        };
        let motion = format.motion;
        let blended = wgpu::ColorTargetState {
            format: format.format,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let opaque = wgpu::ColorTargetState {
            format: format.format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };
        let either = |with: MotionWrite| if motion { with } else { MotionWrite::None };
        let pick = |with: &'static str, without: &'static str| if motion { with } else { without };
        self.composites.entry(format).or_insert_with(|| {
            let colour = composite_pipeline(
                device,
                pipelines,
                format,
                ("render-wgpu splat composite", &pipelines.module),
                pick("fs_composite_reactive", "fs_composite"),
                None,
                blended.clone(),
                either(MotionWrite::Reactive),
            );
            // Where the layer covers at least half a pixel: its depth, for
            // the finish's fog and what draws after, and its motion.
            let surface = composite_pipeline(
                device,
                pipelines,
                format,
                ("render-wgpu splat depth", &pipelines.module),
                pick("fs_composite_depth_motion", "fs_composite_depth"),
                Some(wgpu::CompareFunction::Always),
                wgpu::ColorTargetState {
                    write_mask: wgpu::ColorWrites::empty(),
                    ..opaque.clone()
                },
                either(MotionWrite::Cover),
            );
            // A pixel grid's opaque blocks at the figure's depth.
            let grid = composite_pipeline(
                device,
                pipelines,
                format,
                ("render-wgpu splat grid", &pipelines.module),
                pick("fs_composite_grid_motion", "fs_composite_grid"),
                Some(wgpu::CompareFunction::LessEqual),
                opaque.clone(),
                either(MotionWrite::Cover),
            );
            [colour, surface, grid]
        });
        if lit {
            self.lit_composites.entry(format).or_insert_with(|| {
                let colour = composite_pipeline(
                    device,
                    pipelines,
                    format,
                    ("render-wgpu splat lit composite", &pipelines.light_module),
                    pick("fs_composite_lit_reactive", "fs_composite_lit"),
                    None,
                    blended.clone(),
                    either(MotionWrite::Reactive),
                );
                let grid = composite_pipeline(
                    device,
                    pipelines,
                    format,
                    ("render-wgpu splat lit grid", &pipelines.light_module),
                    pick("fs_composite_grid_lit_motion", "fs_composite_grid_lit"),
                    Some(wgpu::CompareFunction::LessEqual),
                    opaque.clone(),
                    either(MotionWrite::Cover),
                );
                [colour, grid]
            });
        }
    }

    /// Project, sort and draw each run into a layer of its own, in order:
    /// `runs[i]` is composited by [`Self::draw_composite`]`(.., i)`.
    pub fn encode_runs(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        splat_runs: &[SplatRun<'_>],
        target: &SplatTarget<'_>,
    ) {
        if self.pipelines.is_none() || splat_runs.is_empty() {
            return;
        }
        let runs: Vec<&[RunSet<'_>]> = splat_runs.iter().map(|run| &run.sets[..]).collect();
        let largest = runs
            .iter()
            .map(|run| run.iter().map(|entry| entry.set.count).sum::<u32>())
            .max()
            .unwrap_or(0);
        let slots = runs.iter().map(|run| run.len() as u64 + 2).sum::<u64>();
        // Each set's placing in the previous frame that drew motion; sets
        // the last frame did not draw are forgotten.
        if let Some(motion) = &target.motion {
            self.history
                .retain(|_, history| history.frame + 1 >= motion.frame);
        }
        let previous: Vec<Option<Placed>> = runs
            .iter()
            .copied()
            .flatten()
            .map(|entry| {
                let motion = target.motion.as_ref()?;
                Some(self.previous_placed(entry, motion.frame))
            })
            .collect();
        // Every skinned set's palette, in run order, each followed by its
        // previous one when it moved.
        let mut palettes: Vec<f32> = Vec::new();
        let mut palette_firsts = Vec::new();
        for (entry, previous) in runs.iter().copied().flatten().zip(&previous) {
            let first = (palettes.len() / 4) as u32;
            for joint in &entry.draw.palette {
                palettes.extend_from_slice(joint);
            }
            let previous_first = match previous {
                Some(previous)
                    if previous.palette.len() == entry.draw.palette.len()
                        && previous.palette != entry.draw.palette =>
                {
                    let at = (palettes.len() / 4) as u32;
                    for joint in &previous.palette {
                        palettes.extend_from_slice(joint);
                    }
                    at
                }
                _ => first,
            };
            palette_firsts.push((first, previous_first));
        }
        self.ensure_params(&gpu.device, slots);
        self.ensure_palettes(&gpu.device, palettes.len().max(4) as u64 * 4);
        self.ensure_run(&gpu.device, largest.max(1));
        let motion = target.motion.is_some();
        // A grid's blocks take blocks of whole output pixels, so of the render
        // pixels a scaled view draws, at least one.
        let placements: Vec<Option<GridPlacement>> = splat_runs
            .iter()
            .map(|run| {
                run.grid.map(|(grid, bounds)| {
                    let side = ((grid.pixels as f32 * target.render_scale).round() as u32).max(1);
                    place_grid(
                        &bounds,
                        grid.origin,
                        &target.camera.view_proj,
                        target.viewport,
                        side,
                    )
                })
            })
            .collect();
        self.finishes.clear();
        self.finishes
            .extend(
                splat_runs
                    .iter()
                    .zip(&placements)
                    .map(|(run, placement)| match placement {
                        Some(GridPlacement::Grid { rect, .. }) => Finish::Grid {
                            scissor: [
                                rect[0] as u32,
                                rect[1] as u32,
                                (rect[2] - rect[0]) as u32,
                                (rect[3] - rect[1]) as u32,
                            ],
                            lit: run.light.is_some(),
                        },
                        Some(GridPlacement::Off) => Finish::Off,
                        Some(GridPlacement::Smooth) | None => Finish::Smooth {
                            surface: motion || run.read_after,
                            lit: run.light.is_some(),
                        },
                    }),
            );
        self.viewport = target.viewport;
        for finish in self.finishes.clone() {
            self.ensure_draw(
                &gpu.device,
                target.samples,
                finish.tested(),
                finish.keeps_surface().then_some(motion),
                finish.lit(),
            );
        }
        for (index, finish) in self.finishes.clone().iter().enumerate() {
            let lit = finish.lit();
            self.layer(&gpu.device, index, target, motion || lit, motion && lit);
        }
        // The view's rotation, for a lit run's normals (view space to world).
        let view_to_world = [
            target.camera.view.x_axis.truncate(),
            target.camera.view.y_axis.truncate(),
            target.camera.view.z_axis.truncate(),
        ];
        for ((layer, finish), (run, placement)) in self
            .layers
            .iter()
            .zip(&self.finishes)
            .zip(splat_runs.iter().zip(&placements))
        {
            let gridded = run.grid.zip(match placement {
                Some(GridPlacement::Grid { side, anchor, rect }) => Some((side, anchor, rect)),
                _ => None,
            });
            if gridded.is_none() && run.light.is_none() {
                continue;
            }
            let ((side, anchor, rect), (cutoff, palette)) = match gridded {
                Some(((grid, _), placed)) => (placed, (grid.cutoff, &grid.palette[..])),
                None => ((&0, &[0, 0], &[0; 4]), (0.0, &[][..])),
            };
            let mut bytes = Vec::with_capacity(GRID_BYTES as usize);
            for value in [anchor[0], anchor[1], *side as i32, palette.len() as i32] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in rect {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            let normal_target = if motion && finish.lit() { 1.0 } else { 0.0 };
            for value in [cutoff, run.light.unwrap_or(0.0), normal_target, 0.0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            let viewport = target.viewport;
            for value in [
                viewport.x as f32,
                viewport.y as f32,
                viewport.width as f32,
                viewport.height as f32,
            ] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            for row in view_to_world {
                for value in [row.x, row.y, row.z, 0.0] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            for colour in palette {
                let display =
                    colour.map(|value| crate::effects::linear_srgb(value.clamp(0.0, 1.0)));
                for value in [
                    colour[0], colour[1], colour[2], 1.0, display[0], display[1], display[2], 1.0,
                ] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            gpu.queue.write_buffer(&layer.grid, 0, &bytes);
        }
        self.layers_used = self.layers_used.max(runs.len());

        // Every slot this view's dispatches read, written at once: for each
        // run its two sort passes, then its sets.
        let viewport = target.viewport;
        let (width, height) = (viewport.width as f32, viewport.height as f32);
        let projection = target.camera.projection;
        let ortho = projection.z_axis.w == 0.0;
        let lens = [
            projection.x_axis.x * 0.5 * width,
            projection.y_axis.y * 0.5 * height,
            width,
            height,
        ];
        let view = target.camera.view.as_dmat4();
        let view_proj = target.camera.view_proj.as_dmat4();
        // NDC to the target's pixels as the pass rasterises them, jitter
        // included: where a Gaussian's centre is drawn.
        let pixels = [
            viewport.x as f32 + 0.5 * width,
            viewport.y as f32 + 0.5 * height,
            0.5 * width,
            -0.5 * height,
        ];
        // A point's offset from the previous camera to this frame's jittered
        // pixels, as the world's motion places it (`motion.wgsl`).
        let previous_pixels = target.motion.as_ref().map(|motion| {
            motion.to_pixels.as_dmat4()
                * motion.previous_view_proj.as_dmat4()
                * glam::DMat4::from_translation(-motion.previous_eye.as_dvec3())
        });
        let mut bytes = Vec::new();
        let mut offsets = Vec::with_capacity(runs.len());
        let mut palette_index = palette_firsts.iter().copied();
        let mut previous_models = previous.iter();
        for ((run, &(near, far)), lit) in runs
            .iter()
            .zip(splat_runs.iter().map(|run| &run.depth_range))
            .zip(self.finishes.iter().map(Finish::lit))
        {
            let total: u32 = run.iter().map(|entry| entry.set.count).sum();
            let blocks = total.div_ceil(TILE);
            let sort = bytes.len() as u32;
            for shift in SORT_SHIFTS {
                let mut slot = Vec::with_capacity(PARAMS_SLOT as usize);
                for value in [total, blocks, shift, 0] {
                    slot.extend_from_slice(&value.to_le_bytes());
                }
                slot.resize(PARAMS_SLOT as usize, 0);
                bytes.extend_from_slice(&slot);
            }
            let mut sets = Vec::with_capacity(run.len());
            let mut first = 0u32;
            for (index, entry) in run.iter().enumerate() {
                let (set, draw) = (entry.set, entry.draw);
                let (palette_first, previous_first) =
                    palette_index.next().expect("one palette start per set");
                let previous = previous_models.next().expect("one placing per set");
                let model = draw.model.as_dmat4();
                let previous_model = previous
                    .as_ref()
                    .map_or(model, |previous| previous.model.as_dmat4());
                let span = (far - near).max(1e-3);
                let flag = |on: bool| if on { 1.0 } else { 0.0 };
                // A skinned set's palette in rows of four floats, three a
                // joint; its Gaussians stand in bind space without one.
                let skinned = set.joint_bounds.is_some() && !draw.palette.is_empty();
                let [r, g, b] = draw.multiplier;
                let slot = ProjectSlot {
                    model_view: view * model,
                    clip_from_model: view_proj * model,
                    previous_pixels: previous_pixels
                        .map_or(DMat4::IDENTITY, |to| to * previous_model),
                    lens,
                    depth: [near, 1.0 / span, flag(ortho), flag(lit)],
                    range: [first, set.count, u32::from(index == 0), 0],
                    skin: [
                        palette_first,
                        u32::from(skinned),
                        previous_first,
                        u32::from(previous_pixels.is_some()),
                    ],
                    tint: [r, g, b, 0.0],
                    pixels,
                };
                sets.push(bytes.len() as u32);
                bytes.extend_from_slice(&slot.bytes());
                first += set.count;
            }
            offsets.push((sort, sets, total, blocks));
        }
        gpu.queue.write_buffer(&self.params, 0, &bytes);
        if !palettes.is_empty() {
            let palette_bytes: Vec<u8> = palettes.iter().flat_map(|v| v.to_le_bytes()).collect();
            gpu.queue.write_buffer(&self.palettes, 0, &palette_bytes);
        }

        let pipelines = self.pipelines.as_ref().expect("checked above");
        let buffers = self.run.as_ref().expect("ensured above");
        let timer = self.timer.as_ref().filter(|_| !self.timed);
        for (index, (run, (sort, sets, total, blocks))) in runs.iter().zip(&offsets).enumerate() {
            let groups = (DIGITS * blocks).div_ceil(TILE);
            encoder.clear_buffer(&buffers.args, 0, None);
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu splats project and sort"),
                    timestamp_writes: timer
                        .filter(|_| index == 0)
                        .and_then(|timer| timer.compute_writes_between(true, false)),
                });
                pass.set_pipeline(&pipelines.project);
                for (entry, offset) in run.iter().zip(sets) {
                    pass.set_bind_group(0, &buffers.project_group, &[*offset]);
                    pass.set_bind_group(1, &entry.set.group, &[]);
                    pass.dispatch_workgroups(entry.set.count.div_ceil(WORKGROUP).max(1), 1, 1);
                }
                for (step, group) in buffers.sort_groups.iter().enumerate() {
                    pass.set_bind_group(0, group, &[sort + step as u32 * PARAMS_SLOT as u32]);
                    pass.set_pipeline(&pipelines.sort_count);
                    pass.dispatch_workgroups(*blocks, 1, 1);
                    pass.set_pipeline(&pipelines.scan_local);
                    pass.dispatch_workgroups(groups, 1, 1);
                    pass.set_pipeline(&pipelines.scan_sums);
                    pass.dispatch_workgroups(1, 1, 1);
                    pass.set_pipeline(&pipelines.scan_add);
                    pass.dispatch_workgroups(groups, 1, 1);
                    pass.set_pipeline(&pipelines.sort_scatter);
                    pass.dispatch_workgroups(*blocks, 1, 1);
                }
            }
            let layer = &self.layers[index];
            let finish = self.finishes[index];
            let kept = finish.keeps_surface();
            let draw = &self.draws[&(
                target.samples,
                finish.tested(),
                kept.then_some(motion),
                finish.lit(),
            )];
            let attachments = [
                Some(wgpu::RenderPassColorAttachment {
                    view: &layer.color,
                    depth_slice: None,
                    resolve_target: layer.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &layer.surface,
                    depth_slice: None,
                    resolve_target: layer.surface_resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                layer
                    .normal
                    .as_ref()
                    .map(|(view, resolve)| wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: resolve.as_ref(),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
            ];
            // The normal target only in a lit run under motion.
            let targets = 1 + usize::from(kept) + usize::from(kept && motion && finish.lit());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu splat layer"),
                color_attachments: &attachments[..targets],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer
                    .filter(|_| index + 1 == runs.len())
                    .and_then(|timer| timer.render_writes_between(false, true)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                viewport.x as f32,
                viewport.y as f32,
                width,
                height,
                0.0,
                1.0,
            );
            pass.set_pipeline(draw);
            pass.set_bind_group(0, &buffers.draw_group, &[]);
            pass.draw_indirect(&buffers.args, 0);
            drop(pass);
            self.counts.runs += 1;
            self.counts.gaussians += u64::from(*total);
        }
        if !self.timed {
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
            self.timed = true;
        }
    }

    /// Instances a view drew in its runs.
    pub fn drew_instances(&mut self, count: u32) {
        self.counts.instances += count;
    }

    /// After the encoder holding a view's runs was submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }

    /// Composite run `run` of the view's last [`Self::encode_runs`] into a
    /// pass drawing into `format` (the frame's bind group in group 0).
    pub fn draw_composite(&self, pass: &mut wgpu::RenderPass<'_>, format: ColorTarget, run: usize) {
        let (Some(pipelines), Some(layer), Some(finish)) = (
            self.composites.get(&format),
            self.layers.get(run),
            self.finishes.get(run),
        ) else {
            return;
        };
        // A lit run's colour and blocks come from `splat_light.wgsl`; its
        // depth and motion as an unlit run's.
        let lit = finish
            .lit()
            .then(|| self.lit_composites.get(&format))
            .flatten();
        if finish.lit() && lit.is_none() {
            return;
        }
        let (colour, grid) = match lit {
            Some([colour, grid]) => (colour, grid),
            None => (&pipelines[0], &pipelines[2]),
        };
        pass.set_bind_group(1, &layer.group, &[]);
        match *finish {
            Finish::Smooth { .. } => {
                pass.set_pipeline(colour);
                pass.draw(0..3, 0..1);
                if finish.keeps_surface() {
                    pass.set_pipeline(&pipelines[1]);
                    pass.draw(0..3, 0..1);
                }
            }
            Finish::Grid {
                scissor: [x, y, width, height],
                ..
            } => {
                pass.set_scissor_rect(x, y, width, height);
                pass.set_pipeline(grid);
                pass.draw(0..3, 0..1);
                let viewport = self.viewport;
                pass.set_scissor_rect(viewport.x, viewport.y, viewport.width, viewport.height);
            }
            Finish::Off => {}
        }
    }

    /// Where `entry` stood in the frame that drew motion before `frame`, or
    /// where it stands now when that frame did not draw it; this frame's
    /// placing is kept for the next.
    fn previous_placed(&mut self, entry: &RunSet<'_>, frame: u64) -> Placed {
        let current = Placed {
            model: entry.draw.model,
            palette: entry.draw.palette.clone(),
        };
        match self.history.get_mut(&entry.key) {
            // An earlier view of this frame kept it.
            Some(history) if history.frame == frame => history.previous.clone(),
            Some(history) if history.frame + 1 == frame => {
                history.previous = std::mem::replace(&mut history.current, current);
                history.frame = frame;
                history.previous.clone()
            }
            _ => {
                self.history.insert(
                    entry.key,
                    History {
                        frame,
                        current: current.clone(),
                        previous: current.clone(),
                    },
                );
                current
            }
        }
    }

    /// Frames no longer draw motion: no placings are kept.
    pub fn forget_history(&mut self) {
        self.history.clear();
    }

    fn ensure_params(&mut self, device: &wgpu::Device, slots: u64) {
        if slots <= self.params_slots {
            return;
        }
        self.params_slots = slots.next_power_of_two();
        self.params = params_buffer(device, self.params_slots);
        // The run's bind groups name the old buffer.
        self.run = None;
    }

    fn ensure_palettes(&mut self, device: &wgpu::Device, bytes: u64) {
        if bytes <= self.palette_bytes {
            return;
        }
        self.palette_bytes = bytes.next_power_of_two();
        self.palettes = palette_buffer(device, self.palette_bytes);
        // The run's bind groups name the old buffer.
        self.run = None;
    }

    fn ensure_run(&mut self, device: &wgpu::Device, gaussians: u32) {
        if self
            .run
            .as_ref()
            .is_some_and(|run| run.capacity >= gaussians)
        {
            return;
        }
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        let capacity = gaussians.next_power_of_two().max(TILE);
        let blocks = capacity.div_ceil(TILE);
        let storage = |label: &str, bytes: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | usage,
                mapped_at_creation: false,
            })
        };
        let words = u64::from(capacity) * 4;
        // Readable, so a test can compare the sort with the CPU's.
        let readable = wgpu::BufferUsages::COPY_SRC;
        let projected = storage(
            "render-wgpu splats projected",
            u64::from(capacity) * PROJECTED_BYTES,
            wgpu::BufferUsages::empty(),
        );
        let keys = [
            storage("render-wgpu splat keys", words, readable),
            storage("render-wgpu splat keys sorted", words, readable),
        ];
        let values = [
            storage("render-wgpu splat values", words, readable),
            storage("render-wgpu splat values sorted", words, readable),
        ];
        let counts = storage(
            "render-wgpu splat digit counts",
            u64::from(DIGITS * blocks) * 4,
            wgpu::BufferUsages::empty(),
        );
        let sums = storage(
            "render-wgpu splat digit sums",
            u64::from(TILE) * 4,
            wgpu::BufferUsages::empty(),
        );
        let args = storage(
            "render-wgpu splat draw",
            16,
            wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST | readable,
        );
        let params = wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.params,
            offset: 0,
            size: wgpu::BufferSize::new(PARAMS_SLOT),
        });
        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        let project_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splats project"),
            layout: &pipelines.project_layout,
            entries: &[
                entry(0, params.clone()),
                entry(1, projected.as_entire_binding()),
                entry(2, keys[0].as_entire_binding()),
                entry(3, values[0].as_entire_binding()),
                entry(4, args.as_entire_binding()),
                entry(5, self.palettes.as_entire_binding()),
            ],
        });
        let sort_group = |from: usize, to: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu splats sort"),
                layout: &pipelines.sort_layout,
                entries: &[
                    entry(10, params.clone()),
                    entry(11, keys[from].as_entire_binding()),
                    entry(12, values[from].as_entire_binding()),
                    entry(13, keys[to].as_entire_binding()),
                    entry(14, values[to].as_entire_binding()),
                    entry(15, counts.as_entire_binding()),
                    entry(16, sums.as_entire_binding()),
                ],
            })
        };
        let sort_groups = [sort_group(0, 1), sort_group(1, 0)];
        let draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splats draw"),
            layout: &pipelines.draw_layout,
            entries: &[
                entry(20, projected.as_entire_binding()),
                // Two passes sort the keys back into the first pair.
                entry(21, values[0].as_entire_binding()),
            ],
        });
        self.run_generation += 1;
        self.run = Some(RunBuffers {
            capacity,
            projected,
            keys: keys[0].clone(),
            values: values[0].clone(),
            args,
            project_group,
            sort_groups,
            draw_group,
        });
    }

    /// The layer's draw at `samples`, tested against the world's depth or
    /// not, keeping its surface, with or without motion, when `surface` is
    /// some.
    fn ensure_draw(
        &mut self,
        device: &wgpu::Device,
        samples: u32,
        tested: bool,
        surface: Option<bool>,
        lit: bool,
    ) {
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        self.draws
            .entry((samples, tested, surface, lit))
            .or_insert_with(|| {
                let colour = Some(wgpu::ColorTargetState {
                    format: LAYER_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                });
                let blended = |format| wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                };
                // A lit run without motion keeps its normal in the surface;
                // under motion in a target of its own.
                let normal = lit && surface == Some(true);
                let targets = [
                    colour,
                    surface.map(|motion| blended(surface_format(motion || lit))),
                    normal.then(|| blended(NORMAL_FORMAT)),
                ];
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu splat layer"),
                    layout: Some(&pipelines.draw_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &pipelines.module,
                        entry_point: Some("vs_splat"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: Some(false),
                        depth_compare: Some(if tested {
                            wgpu::CompareFunction::LessEqual
                        } else {
                            wgpu::CompareFunction::Always
                        }),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &pipelines.module,
                        entry_point: Some(match (surface, lit) {
                            (None, _) => "fs_splat_colour",
                            (Some(_), false) => "fs_splat",
                            (Some(false), true) => "fs_splat_lit",
                            (Some(true), true) => "fs_splat_lit_motion",
                        }),
                        compilation_options: Default::default(),
                        targets: &targets
                            [..1 + usize::from(surface.is_some()) + usize::from(normal)],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            });
    }

    /// Layer `index`, made or remade at the target's size, its surface
    /// RGBA16F when `wide` (motion or a lit run's normal) and with a normal
    /// target when `normal` (a lit run under motion).
    fn layer(
        &mut self,
        device: &wgpu::Device,
        index: usize,
        target: &SplatTarget<'_>,
        wide: bool,
        normal: bool,
    ) {
        let size = (target.width, target.height, target.samples, wide, normal);
        if self
            .layers
            .get(index)
            .is_some_and(|layer| layer.size == size)
        {
            return;
        }
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        let texture = |samples: u32, format: wgpu::TextureFormat, label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture(size.2, LAYER_FORMAT, "render-wgpu splat layer");
        let resolve =
            (size.2 > 1).then(|| texture(1, LAYER_FORMAT, "render-wgpu splat layer resolved"));
        let surface = texture(size.2, surface_format(size.3), "render-wgpu splat surface");
        let surface_resolve = (size.2 > 1).then(|| {
            texture(
                1,
                surface_format(size.3),
                "render-wgpu splat surface resolved",
            )
        });
        let normal = size.4.then(|| {
            (
                texture(size.2, NORMAL_FORMAT, "render-wgpu splat normal"),
                (size.2 > 1)
                    .then(|| texture(1, NORMAL_FORMAT, "render-wgpu splat normal resolved")),
            )
        });
        let grid = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu splat grid"),
            size: GRID_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat layer"),
            layout: &pipelines.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 30,
                    resource: wgpu::BindingResource::TextureView(
                        resolve.as_ref().unwrap_or(&color),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 31,
                    resource: wgpu::BindingResource::TextureView(
                        surface_resolve.as_ref().unwrap_or(&surface),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 32,
                    resource: grid.as_entire_binding(),
                },
                // Without a normal target the surface stands in, unread.
                wgpu::BindGroupEntry {
                    binding: 33,
                    resource: wgpu::BindingResource::TextureView(match &normal {
                        Some((view, resolve)) => resolve.as_ref().unwrap_or(view),
                        None => surface_resolve.as_ref().unwrap_or(&surface),
                    }),
                },
            ],
        });
        let layer = Layer {
            size,
            color,
            resolve,
            surface,
            surface_resolve,
            normal,
            grid,
            group,
        };
        if index < self.layers.len() {
            self.layers[index] = layer;
        } else {
            self.layers.push(layer);
        }
    }
}

/// A composite pipeline into `format`: `vs_composite` and the fragment
/// `entry` of `module` (labelled), inside the blended pass, testing its
/// depth with `depth_compare` and writing it, or neither without one.
#[allow(clippy::too_many_arguments, reason = "one pipeline's states")]
fn composite_pipeline(
    device: &wgpu::Device,
    pipelines: &Pipelines,
    format: ColorTarget,
    (label, module): (&str, &wgpu::ShaderModule),
    entry: &str,
    depth_compare: Option<wgpu::CompareFunction>,
    color: wgpu::ColorTargetState,
    motion_write: MotionWrite,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipelines.composite_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &pipelines.module,
            entry_point: Some("vs_composite"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        // Inside the blended pass, whose depth only a grid's blocks test
        // (each pixel of a block against what stands there): the layer was
        // tested against it.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_compare.is_some()),
            depth_compare: Some(depth_compare.unwrap_or(wgpu::CompareFunction::Always)),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: format.multisample(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &format.targets(color, motion_write),
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        module: wgpu::ShaderModule,
        light_module: wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let storage = |binding: u32, read_only: bool, visibility: wgpu::ShaderStages| {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        };
        let params = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(PARAMS_SLOT),
            },
            count: None,
        };
        let compute = wgpu::ShaderStages::COMPUTE;
        let layout = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let project_layout = layout(
            "render-wgpu splats project",
            &[
                params(0),
                storage(1, false, compute),
                storage(2, false, compute),
                storage(3, false, compute),
                storage(4, false, compute),
                storage(5, true, compute),
            ],
        );
        let gaussian_layout = layout(
            "render-wgpu splat gaussians",
            &[
                storage(0, true, compute),
                storage(1, true, compute),
                storage(2, true, compute),
            ],
        );
        let sort_layout = layout(
            "render-wgpu splats sort",
            &[
                params(10),
                storage(11, true, compute),
                storage(12, true, compute),
                storage(13, false, compute),
                storage(14, false, compute),
                storage(15, false, compute),
                storage(16, false, compute),
            ],
        );
        let draw_layout = layout(
            "render-wgpu splats draw",
            &[
                storage(20, true, wgpu::ShaderStages::VERTEX),
                storage(21, true, wgpu::ShaderStages::VERTEX),
            ],
        );
        let layer_texture = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let composite_layout = layout(
            "render-wgpu splat composite",
            &[
                layer_texture(30),
                layer_texture(31),
                storage(32, true, wgpu::ShaderStages::FRAGMENT),
                layer_texture(33),
            ],
        );
        let pipeline_layout = |label: &str, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let project_pipeline_layout = pipeline_layout(
            "render-wgpu splats project",
            &[Some(&project_layout), Some(&gaussian_layout)],
        );
        let sort_pipeline_layout =
            pipeline_layout("render-wgpu splats sort", &[Some(&sort_layout)]);
        let draw_pipeline_layout =
            pipeline_layout("render-wgpu splats draw", &[Some(&draw_layout)]);
        let composite_pipeline_layout = pipeline_layout(
            "render-wgpu splat composite",
            &[Some(frame_layout), Some(&composite_layout)],
        );
        let kernel = |layout: &wgpu::PipelineLayout, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let project = kernel(&project_pipeline_layout, "cs_project");
        let sort_count = kernel(&sort_pipeline_layout, "cs_sort_count");
        let scan_local = kernel(&sort_pipeline_layout, "cs_scan_local");
        let scan_sums = kernel(&sort_pipeline_layout, "cs_scan_sums");
        let scan_add = kernel(&sort_pipeline_layout, "cs_scan_add");
        let sort_scatter = kernel(&sort_pipeline_layout, "cs_sort_scatter");
        Self {
            project_layout,
            gaussian_layout,
            sort_layout,
            draw_layout,
            composite_layout,
            draw_pipeline_layout,
            composite_pipeline_layout,
            module,
            light_module,
            project,
            sort_count,
            scan_local,
            scan_sums,
            scan_add,
            sort_scatter,
        }
    }
}

fn params_buffer(device: &wgpu::Device, slots: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu splat parameters"),
        size: slots * PARAMS_SLOT,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn palette_buffer(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu splat palettes"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// One set's projection parameters (`splats.wgsl` `Project`), a slot of
/// the parameters buffer.
struct ProjectSlot {
    model_view: DMat4,
    clip_from_model: DMat4,
    previous_pixels: DMat4,
    lens: [f32; 4],
    depth: [f32; 4],
    range: [u32; 4],
    skin: [u32; 4],
    tint: [f32; 4],
    pixels: [f32; 4],
}

impl ProjectSlot {
    fn bytes(&self) -> Vec<u8> {
        let mut slot = Vec::with_capacity(PARAMS_SLOT as usize);
        push_matrix(&mut slot, &self.model_view);
        push_matrix(&mut slot, &self.clip_from_model);
        push_matrix(&mut slot, &self.previous_pixels);
        for value in self.lens.into_iter().chain(self.depth) {
            slot.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.range.into_iter().chain(self.skin) {
            slot.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.tint.into_iter().chain(self.pixels) {
            slot.extend_from_slice(&value.to_le_bytes());
        }
        slot.resize(PARAMS_SLOT as usize, 0);
        slot
    }
}

/// The joints whose Gaussians a set casts as ellipsoids: fewer than this
/// cast nothing.
const ELLIPSOID_MIN_GAUSSIANS: usize = 8;

/// A set's ellipsoid casters: one per joint that is the heaviest of at
/// least `ELLIPSOID_MIN_GAUSSIANS` Gaussians (one for an unskinned set),
/// the box of those centres widened by their mean largest σ.
fn ellipsoids(splats: &GlbSplats) -> Vec<Ellipsoid> {
    let mut boxes: Vec<(Aabb, f32, usize)> = Vec::new();
    for (index, centre) in splats.centres.iter().enumerate() {
        let joint = match (&splats.joints, &splats.weights) {
            (Some(joints), Some(weights)) => {
                let slot = (0..4)
                    .max_by(|a, b| weights[index][*a].total_cmp(&weights[index][*b]))
                    .unwrap_or(0);
                usize::from(joints[index][slot])
            }
            _ => 0,
        };
        if boxes.len() <= joint {
            boxes.resize(joint + 1, (Aabb::EMPTY, 0.0, 0));
        }
        let entry = &mut boxes[joint];
        entry.0.include(Vec3::from(*centre));
        entry.1 += splats.scales[index]
            .iter()
            .fold(0.0f32, |a, b| a.max(b.abs()));
        entry.2 += 1;
    }
    let skinned = splats.joints.is_some() && splats.weights.is_some();
    boxes
        .into_iter()
        .enumerate()
        .filter(|(_, (_, _, count))| *count >= ELLIPSOID_MIN_GAUSSIANS)
        .map(|(joint, (bounds, reach, count))| Ellipsoid {
            joint: skinned.then_some(joint),
            centre: (bounds.min + bounds.max) * 0.5,
            half: (bounds.max - bounds.min) * 0.5 + Vec3::splat(reach / count as f32),
        })
        .collect()
}

fn push_matrix(bytes: &mut Vec<u8>, matrix: &DMat4) {
    for value in matrix.as_mat4().to_cols_array() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// The GPU words of a splat primitive's Gaussians (`splats.wgsl`
/// `Gaussian`) and their bounds to `BOUNDS_SIGMAS`.
pub(crate) fn gaussian_words(splats: &GlbSplats) -> (Vec<u32>, Aabb) {
    let mut words = Vec::with_capacity(splats.centres.len() * 8);
    let mut bounds = Aabb::EMPTY;
    for (((centre, rotation), scale), color) in splats
        .centres
        .iter()
        .zip(&splats.rotations)
        .zip(&splats.scales)
        .zip(&splats.colors)
    {
        let unorm = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
        let packed_color =
            unorm(color[0]) | unorm(color[1]) << 8 | unorm(color[2]) << 16 | unorm(color[3]) << 24;
        let length = rotation.iter().map(|c| c * c).sum::<f32>().sqrt();
        let rotation = if length > 1e-8 {
            rotation.map(|c| c / length)
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        let snorm = |value: f32| ((value.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
        words.extend_from_slice(&[
            centre[0].to_bits(),
            centre[1].to_bits(),
            centre[2].to_bits(),
            packed_color,
            snorm(rotation[0]) | snorm(rotation[1]) << 16,
            snorm(rotation[2]) | snorm(rotation[3]) << 16,
            u32::from(half_bits(scale[0])) | u32::from(half_bits(scale[1])) << 16,
            u32::from(half_bits(scale[2])),
        ]);
        let reach = BOUNDS_SIGMAS * scale.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let centre = Vec3::from(*centre);
        bounds.include(centre - Vec3::splat(reach));
        bounds.include(centre + Vec3::splat(reach));
    }
    (words, bounds)
}

/// A skinned primitive's skin words (`splats.wgsl` `skins`) and each
/// joint's bounds in bind space, or `None` for an unskinned one or one
/// indexing more than [`MAX_SKIN_JOINTS`] joints (drawn in bind space).
fn skin_words(splats: &GlbSplats) -> Option<(Vec<u32>, Vec<JointReach>)> {
    let (joints, weights) = (splats.joints.as_ref()?, splats.weights.as_ref()?);
    let highest = joints.iter().flatten().copied().max().unwrap_or(0) as usize;
    if highest >= MAX_SKIN_JOINTS {
        return None;
    }
    let mut bounds = vec![
        JointReach {
            centres: Aabb::EMPTY,
            reach: 0.0,
        };
        highest + 1
    ];
    let mut words = Vec::with_capacity(joints.len() * 2);
    for (index, (joint, weight)) in joints.iter().zip(weights).enumerate() {
        let unorm = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
        words.push(
            u32::from(joint[0])
                | u32::from(joint[1]) << 8
                | u32::from(joint[2]) << 16
                | u32::from(joint[3]) << 24,
        );
        words.push(
            unorm(weight[0])
                | unorm(weight[1]) << 8
                | unorm(weight[2]) << 16
                | unorm(weight[3]) << 24,
        );
        let centre = Vec3::from(splats.centres[index]);
        let reach = BOUNDS_SIGMAS
            * splats.scales[index]
                .iter()
                .fold(0.0f32, |a, b| a.max(b.abs()));
        for (joint, weight) in joint.iter().zip(weight) {
            if *weight > 0.0 {
                let joint = &mut bounds[usize::from(*joint)];
                joint.centres.include(centre);
                joint.reach = joint.reach.max(reach);
            }
        }
    }
    Some((words, bounds))
}

/// Each Gaussian's unit normal, octahedral (`splats.wgsl` `normals`): the
/// asset's `_NORMAL` when it has one, otherwise its shortest axis pointing
/// away from the centre of the Gaussians its heaviest joint carries (of the
/// whole set when unskinned). A flat Gaussian on a figure's surface lies
/// along it, so its shortest axis is the surface's normal; a round one's is
/// a guess.
fn normal_words(splats: &GlbSplats) -> Vec<u32> {
    let heaviest = |index: usize| -> usize {
        match (&splats.joints, &splats.weights) {
            (Some(joints), Some(weights)) => {
                let (slot, _) = weights[index].iter().enumerate().fold(
                    (0, f32::MIN),
                    |best, (slot, weight)| {
                        if *weight > best.1 {
                            (slot, *weight)
                        } else {
                            best
                        }
                    },
                );
                usize::from(joints[index][slot])
            }
            _ => 0,
        }
    };
    let mut sums: Vec<(Vec3, f32)> = Vec::new();
    if splats.normals.is_none() {
        for (index, centre) in splats.centres.iter().enumerate() {
            let joint = heaviest(index);
            if sums.len() <= joint {
                sums.resize(joint + 1, (Vec3::ZERO, 0.0));
            }
            sums[joint].0 += Vec3::from(*centre);
            sums[joint].1 += 1.0;
        }
    }
    (0..splats.centres.len())
        .map(|index| {
            let normal = match &splats.normals {
                Some(normals) => Vec3::from(normals[index]),
                None => {
                    let rotation = splats.rotations[index];
                    let q =
                        glam::Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
                            .normalize();
                    let scale = splats.scales[index];
                    let shortest = (0..3)
                        .min_by(|a, b| scale[*a].abs().total_cmp(&scale[*b].abs()))
                        .unwrap_or(2);
                    let axis = q * Vec3::AXES[shortest];
                    let (sum, count) = sums[heaviest(index)];
                    let away = Vec3::from(splats.centres[index]) - sum / count.max(1.0);
                    if axis.dot(away) < 0.0 {
                        -axis
                    } else {
                        axis
                    }
                }
            };
            octahedral(normal.normalize_or(Vec3::Z))
        })
        .collect()
}

/// A unit vector's octahedral encoding as two snorm16 (`splats.wgsl`
/// `octahedral`).
fn octahedral(normal: Vec3) -> u32 {
    let n = normal / (normal.x.abs() + normal.y.abs() + normal.z.abs());
    let (x, y) = if n.z < 0.0 {
        let sign = |value: f32| if value >= 0.0 { 1.0 } else { -1.0 };
        ((1.0 - n.y.abs()) * sign(n.x), (1.0 - n.x.abs()) * sign(n.y))
    } else {
        (n.x, n.y)
    };
    let snorm = |value: f32| ((value.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
    snorm(x) | snorm(y) << 16
}

/// The IEEE half nearest a non-negative `value` (a standard deviation):
/// tiny values flush toward zero, large ones stop at the largest half.
fn half_bits(value: f32) -> u16 {
    let value = value.abs();
    if !value.is_finite() || value >= 65504.0 {
        return 0x7bff;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent <= 0 {
        // Subnormal half: the value in units of 2^-24.
        return (value * 16_777_216.0).round().min(1023.0) as u16;
    }
    // Round the 23-bit mantissa to 10 bits, carrying into the exponent.
    let rounded = (((exponent as u32) << 10) | (mantissa >> 13)) + ((mantissa >> 12) & 1);
    rounded.min(0x7bff) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PixelRect;

    fn read_words(gpu: &Gpu, buffer: &wgpu::Buffer, words: u64) -> Vec<u32> {
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("splat test readback"),
            size: words * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, Some(words * 4));
        gpu.queue.submit([encoder.finish()]);
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let bytes = staging
            .slice(..)
            .get_mapped_range()
            .expect("mapped")
            .to_vec();
        let (words, _) = bytes.as_chunks::<4>();
        words.iter().map(|word| u32::from_le_bytes(*word)).collect()
    }

    /// Gaussians at pseudo-random depths in front of a camera at the origin
    /// looking down −z, a few behind it and a few invisible.
    fn scattered(count: usize, seed: u32) -> GlbSplats {
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut splats = GlbSplats {
            centres: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
            colors: Vec::new(),
            normals: None,
            joints: None,
            weights: None,
        };
        for index in 0..count {
            // Every 97th behind the camera; every 89th invisible.
            let depth = if index % 97 == 0 {
                -1.0
            } else {
                // Coarse depths, so equal keys show the sort is stable.
                1.0 + (next() * 400.0).floor() * 0.05
            };
            splats
                .centres
                .push([next() * 0.2 - 0.1, next() * 0.2 - 0.1, -depth]);
            splats.rotations.push([0.0, 0.0, 0.0, 1.0]);
            splats.scales.push([0.001; 3]);
            let opacity = if index % 89 == 0 { 0.0 } else { 1.0 };
            splats.colors.push([1.0, 0.5, 0.25, opacity]);
        }
        splats
    }

    /// `splats.wgsl` `octahedral`, on the CPU.
    fn from_octahedral(word: u32) -> Vec3 {
        let unpack = |bits: u32| f32::from(bits as u16 as i16) / 32767.0;
        let (x, y) = (unpack(word & 0xffff), unpack(word >> 16));
        let mut n = Vec3::new(x, y, 1.0 - x.abs() - y.abs());
        if n.z < 0.0 {
            let sign = |value: f32| if value >= 0.0 { 1.0 } else { -1.0 };
            n = Vec3::new(
                (1.0 - n.y.abs()) * sign(n.x),
                (1.0 - n.x.abs()) * sign(n.y),
                n.z,
            );
        }
        n.normalize()
    }

    /// Normals keep their direction through the octahedral encoding, and
    /// without `_NORMAL` a flat Gaussian's shortest axis points away from
    /// the Gaussians its joint carries.
    #[test]
    fn normals_are_the_assets_or_flat_gaussians_shortest_axes_pointing_out() {
        for normal in [
            Vec3::X,
            -Vec3::Y,
            Vec3::new(0.3, -0.5, -0.8).normalize(),
            -Vec3::Z,
        ] {
            let decoded = from_octahedral(octahedral(normal));
            assert!(
                decoded.dot(normal) > 0.9999,
                "{normal} came back as {decoded}"
            );
        }
        // Two flat Gaussians either side of their joint, thin along x.
        let splats = GlbSplats {
            centres: vec![[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0]],
            rotations: vec![[0.0, 0.0, 0.0, 1.0]; 2],
            scales: vec![[0.01, 0.2, 0.2]; 2],
            colors: vec![[1.0; 4]; 2],
            normals: None,
            joints: Some(vec![[3, 0, 0, 0]; 2]),
            weights: Some(vec![[1.0, 0.0, 0.0, 0.0]; 2]),
        };
        let words = normal_words(&splats);
        assert!(from_octahedral(words[0]).dot(-Vec3::X) > 0.999);
        assert!(from_octahedral(words[1]).dot(Vec3::X) > 0.999);
        // The asset's own normals are kept.
        let given = GlbSplats {
            normals: Some(vec![[0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]),
            ..splats
        };
        let words = normal_words(&given);
        assert!(from_octahedral(words[0]).dot(Vec3::Y) > 0.999);
        assert!(from_octahedral(words[1]).dot(-Vec3::Z) > 0.999);
    }

    /// The GPU's radix sort orders a run of two sets as a stable sort by
    /// key does, the culled Gaussians last, and the draw counts the rest.
    #[test]
    fn a_run_sorts_back_to_front_as_a_stable_cpu_sort_does() {
        let gpu = Gpu::headless().expect("a wgpu adapter");
        let device = &gpu.device;
        let mut shaders = crate::shaders::Shaders::new();
        let module = crate::pipelines::standard(shaders.module(
            device,
            render_shaders::Entry::Splats,
            render_shaders::Features::default(),
        ));
        let light_module = crate::pipelines::standard(shaders.module(
            device,
            render_shaders::Entry::SplatLight,
            render_shaders::Features::default(),
        ));
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        });
        let mut splats = Splats::new(&gpu, module, light_module, &frame_layout);
        if let Some(refusal) = &splats.refusal {
            panic!("splats refused on this adapter: {refusal}");
        }
        let first = scattered(40_000, 7);
        let second = scattered(33_123, 11);
        let sets = [
            splats.upload(&gpu, "first", &first).expect("uploaded"),
            splats.upload(&gpu, "second", &second).expect("uploaded"),
        ];
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let projection = glam::camera::rh::proj::directx::perspective(1.0, 1.0, 0.1, 100.0);
        let camera = crate::camera::CameraMatrices {
            view_proj: projection,
            view: Mat4::IDENTITY,
            projection,
            eye: Vec3::ZERO,
        };
        let shift = Mat4::from_translation(Vec3::new(0.0, 0.0, -0.025));
        let draw = |model: Mat4| SplatDraw {
            set: SetRef { slot: 0, set: 0 },
            model,
            palette: Vec::new(),
            multiplier: [1.0; 3],
        };
        let draws = [draw(Mat4::IDENTITY), draw(shift)];
        let run = vec![
            RunSet {
                set: &sets[0],
                draw: &draws[0],
                key: (render_model::RenderHandle::new(1), 0),
            },
            RunSet {
                set: &sets[1],
                draw: &draws[1],
                key: (render_model::RenderHandle::new(2), 0),
            },
        ];
        let (near, far) = (0.5, 25.0);
        let mut encoder = device.create_command_encoder(&Default::default());
        splats.encode_runs(
            &gpu,
            &mut encoder,
            &[SplatRun {
                sets: run,
                depth_range: (near, far),
                read_after: false,
                grid: None,
                light: None,
            }],
            &SplatTarget {
                depth: &depth,
                width: 64,
                height: 64,
                samples: 1,
                viewport: PixelRect::whole(64, 64),
                camera,
                render_scale: 1.0,
                motion: None,
            },
        );
        gpu.queue.submit([encoder.finish()]);

        let buffers = splats.run.as_ref().expect("a run's buffers");
        let total = first.centres.len() + second.centres.len();
        let keys = read_words(&gpu, &buffers.keys, total as u64);
        let values = read_words(&gpu, &buffers.values, total as u64);
        let drawn = read_words(&gpu, &buffers.args, 4);

        // The CPU's keys: every Gaussian's view depth across the run's span.
        let mut expected: Vec<(u32, u32)> = Vec::with_capacity(total);
        for (slot, (centre, color)) in first
            .centres
            .iter()
            .zip(&first.colors)
            .map(|(c, color)| (*c, color[3]))
            .chain(
                second
                    .centres
                    .iter()
                    .zip(&second.colors)
                    .map(|(c, color)| ([c[0], c[1], c[2] - 0.025], color[3])),
            )
            .enumerate()
        {
            let depth = -centre[2];
            let key = if depth <= 0.0 || color == 0.0 {
                0xffff
            } else {
                let along = ((depth - near) / (far - near)).clamp(0.0, 1.0);
                ((1.0 - along) * 65534.0).round() as u32
            };
            expected.push((key, slot as u32));
        }
        let visible = expected.iter().filter(|(key, _)| *key != 0xffff).count();
        assert_eq!(drawn[0], 4, "four vertices a Gaussian");
        assert_eq!(drawn[1] as usize, visible, "the draw counts the visible");
        expected.sort_by_key(|&(key, _)| key);
        let gpu_order: Vec<(u32, u32)> = keys.iter().copied().zip(values.iter().copied()).collect();
        // Keys can differ from the CPU's by a rounding step; the order must
        // be ascending, stable and hold every slot once.
        for pair in gpu_order.windows(2) {
            let ((key_a, value_a), (key_b, value_b)) = (pair[0], pair[1]);
            assert!(
                key_a < key_b || (key_a == key_b && value_a < value_b),
                "sorted stably: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        let mut seen = vec![false; total];
        for &(_, value) in &gpu_order {
            assert!(
                !std::mem::replace(&mut seen[value as usize], true),
                "slot {value} once"
            );
        }
        let mut gpu_keys = vec![0u32; total];
        for &(key, value) in &gpu_order {
            gpu_keys[value as usize] = key;
        }
        for &(key, slot) in &expected {
            let gpu_key = gpu_keys[slot as usize];
            assert!(
                gpu_key.abs_diff(key) <= 1,
                "slot {slot}: {gpu_key} against {key}"
            );
        }
    }
}
