//! The Engine's wgpu renderer over the retained `PresentationWorld`.
//!
//! The runtime applies the deltas `PresentationWorld::apply` returns (a fresh
//! renderer applies the world's snapshot frame first), installs the committed
//! camera composition, then renders it into an offscreen target or a window
//! surface. Output image jobs render their frozen frame in isolation. There is
//! no serialization, realizer layer or mirrored contract between the retained
//! model and the GPU.
//!
//! Inside, the renderer is typed handle-keyed tables (see [`tables`]) and a
//! fixed pass pipeline (see `frame` and `composition`): apply deltas,
//! propagate dirty transforms, upload changed rows, then per view pass build
//! the draw list and encode it, then present or read back. It has no scheduler, plugin registration, generic
//! query layer or second retained world, and it neither validates nor
//! sequences product mutations.
//!
//! Only this crate may depend on wgpu (`scripts/dependency_boundary_check.py`);
//! no wgpu type appears in its public API.

#![forbid(unsafe_code)]

mod ambient_occlusion;
mod animated;
mod apply;
mod batch;
mod camera;
mod capture;
mod cloud_march;
mod compose;
mod composition;
mod convert;
mod culling;
mod decals;
mod distance_fields;
mod dlss;
mod driver;
mod effects;
mod finish;
mod frame;
mod ghost;
mod glb;
mod gpu;
mod image_effect;
mod impostor;
mod inspection;
mod labels;
mod light_clusters;
mod mesh_pool;
mod motion;
mod occlusion;
mod overlays;
mod particles;
mod pipelines;
mod post;
mod precipitation;
mod primitives;
mod probes;
mod resources;
mod scatter;
mod scatter_select;
mod shaders;
mod shadows;
mod sky_light;
mod stars;
mod surface;
mod tables;
mod target;
mod timing;
mod upscale;
mod video;
mod volumetric_fog;
mod voxel;
mod water;
#[cfg(feature = "ui-overlay")]
pub mod web;

use std::collections::HashMap;

use render_model::{
    AmbientOcclusionMode, Antialiasing, IndirectLightDescriptor, RendererSettingsDescriptor,
    RendererSettingsOverrides,
};

/// The CPU-side realization vocabulary, for readers that need exactly the
/// geometry and materials the renderer draws without a device
/// (`render-export`): decoded animated GLBs, mesh streams, built-in
/// primitives and slot colours. Values cross as plain arrays; glam stays
/// private to this crate.
pub mod cpu {
    pub use crate::animated::{decode_animated_asset, joint_nodes};
    pub use crate::apply::slot_color;
    pub use crate::convert::rotation_facing;
    pub use crate::glb::{
        Channel, GlbAlpha, GlbClip, GlbMaterial, GlbModel, GlbNode, GlbPrimitive, GlbSkin,
        GlbTexture, Interp, Path, Trs,
    };
    pub use crate::pipelines::VERTEX_FLOATS;
    pub use crate::primitives::{builtin, line, Geometry};
    pub use crate::resources::{mesh_streams, DecodedImage, MeshStreams};
    pub use crate::tables::Builtin;
}

pub use ambient_occlusion::{AmbientOcclusion, AmbientOcclusionPath, AmbientOcclusionReadout};
pub use animated::AnimationFact;
pub use apply::ApplyIssue;
pub use camera::CameraSampleReadout;
pub use composition::{DrawnCamera, TargetReadout, TargetStatus, ViewCompositionReadout};
pub use culling::GpuCullingReadout;
pub use decals::DecalReadout;
pub use distance_fields::DistanceFieldReadout;
pub use driver::{
    Capture, PresentationLead, PrimaryCamera, SceneChange, SceneDriver, SceneFrame, SceneState,
    SceneView,
};
pub use frame::FrameStats;
pub use impostor::ImpostorReadout;
pub use occlusion::OcclusionReadout;
pub use scatter_select::ScatterSelectionReadout;
pub use water::WaterReadout;

/// Scatter patches taken in since the renderer was made (cumulative: read
/// twice and subtract): the patches and copies created, the CPU time
/// spent turning them into drawable parts, and the bytes of GPU rows
/// those parts upload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScatterUploadReadout {
    pub patches: u64,
    pub copies: u64,
    pub apply_microseconds: u64,
    pub row_bytes: u64,
}
pub use ghost::GhostPlateReadout;
pub use gpu::{AdapterSummary, ComputeLimits, Gpu, GpuError};
pub use inspection::InspectionView;
pub use light_clusters::LightClusterReadout;
pub use motion::MotionFrame;
pub use overlays::{
    CollisionShown, LandmarkDrawn, LandmarkMeasure, LandmarkSetReadout, OverlayLine,
    OverlaySetReadout, OverlayShown, PLAYER_VIEW,
};
pub use particles::EntityPositions;
pub use probes::IndirectLightReadout;
pub use resources::{decode_png_rgba, encode_png, NoResources, ResourceSource};
pub use surface::{PresentSkip, SurfaceFrame, WindowSurface};
pub use target::OffscreenTarget;
pub use timing::GpuPassTiming;
pub use upscale::{FrameResolution, UpscaleMethod};
pub use video::{VideoFact, VideoFailure};
pub use volumetric_fog::VolumetricFogReadout;

use pipelines::{Layouts, Pipelines};
use shaders::{Entry, Features};
use tables::{Builtin, GpuMesh, GpuTexture, Tables};

/// The default world rig: a hemisphere light and a key light at (5, 8, 6),
/// unless the product disables it.
pub(crate) const NEUTRAL_HEMISPHERE_INTENSITY: f32 = 2.4;
/// Hemisphere ground colour 0x263238, sRGB.
pub(crate) const NEUTRAL_GROUND_SRGB: [f32; 3] = [38.0 / 255.0, 50.0 / 255.0, 56.0 / 255.0];
pub(crate) const NEUTRAL_KEY_INTENSITY: f32 = 2.2;
pub(crate) const NEUTRAL_KEY_POSITION: [f32; 3] = [5.0, 8.0, 6.0];
/// The viewmodel rig's key light, in camera-local coordinates
/// (a key light at (2, 3, 2)).
pub(crate) const NEUTRAL_VIEWMODEL_KEY_POSITION: [f32; 3] = [2.0, 3.0, 2.0];
/// Clear colour with no background or sky selected: 0x101820, sRGB.
pub(crate) const DEFAULT_CLEAR_SRGB: [f32; 3] = [16.0 / 255.0, 24.0 / 255.0, 32.0 / 255.0];

/// The sRGB transfer function, as an sRGB texture is decoded. The C# SDK's
/// `Color.SrgbToLinear` is the same (`scripts/fixtures/ColorSpaceChecks.cs`
/// pins it at these values).
pub(crate) fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
#[test]
fn srgb_decodes_as_the_sdk_helper_does() {
    for (srgb, linear) in [
        (0.0, 0.0),
        (0.040_45, 0.003_130_805),
        (0.5, 0.214_041_14),
        (1.0, 1.0),
    ] {
        assert!(
            (srgb_to_linear(srgb) - linear).abs() <= 1e-6,
            "{srgb} decodes to {}",
            srgb_to_linear(srgb)
        );
    }
}

/// What the renderer's GPU passes report, for diagnostics and evidence
/// (`engine.renderer`, `rusty-scene-render`). A pass that is timed adds its
/// [`GpuPassTiming`] to `passes`, so every pass reports its cost the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuReadout {
    /// The device has timestamp queries, so the passes are timed.
    pub timestamps: bool,
    pub limits: ComputeLimits,
    /// The timed passes, in frame order.
    pub passes: Vec<GpuPassTiming>,
    pub ambient_occlusion: AmbientOcclusionReadout,
    /// The chunk field atlas the `DistanceField` occlusion path traces.
    pub distance_fields: DistanceFieldReadout,
    pub light_clusters: LightClusterReadout,
    pub gpu_culling: GpuCullingReadout,
    /// The per-copy selection of scatter runs (`scatter_select.rs`).
    pub scatter_selection: ScatterSelectionReadout,
    /// The impostors baked (`impostor.rs`).
    pub impostors: ImpostorReadout,
    /// The occlusion test of selected scatter copies (`occlusion.rs`).
    pub occlusion: OcclusionReadout,
    /// The indirect light volume (`probes.rs`).
    pub indirect_light: IndirectLightReadout,
    /// The volumetric fog the last world view drew (`volumetric_fog.rs`).
    pub volumetric_fog: volumetric_fog::VolumetricFogReadout,
    /// The backdrop the last world view drew, if it drew one.
    pub backdrop: Option<BackdropReadout>,
    /// How wet the scene's surfaces are, and how frozen their puddles
    /// (`lighting.wgsl` `wet_under`).
    pub wetness: Option<render_model::WetnessDescriptor>,
    /// The snow lying on the scene's surfaces (`lighting.wgsl` `snowed`).
    pub snow: Option<render_model::SnowDescriptor>,
    /// The projected decals (`decals.rs`).
    pub decals: DecalReadout,
    /// Whether water reads the opaque scene's colour on this device.
    pub water: water::WaterReadout,
    /// The last primary frame's resolutions and the upscale stage that
    /// brought its picture to the output, if one ran (`upscale.rs`).
    pub upscale: UpscaleReadout,
    /// Why DLSS does not draw, while the settings ask for it.
    pub dlss: Option<DlssRefusal>,
    /// Why the device was lost, once it has been ([`Gpu::lost`]).
    pub device_lost: Option<String>,
}

/// The last primary frame's render and output resolutions, and its upscale
/// stage (`GpuReadout::upscale`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UpscaleReadout {
    pub resolution: FrameResolution,
    pub method: Option<UpscaleMethod>,
}

/// Why DLSS draws no frame while asked for (`Antialiasing::Dlss`), as the
/// readout's refusal names it and in NGX's words where it has them
/// (`dlss.rs`); TAAU draws in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlssRefusal {
    pub refusal: SettingRefusal,
    pub detail: String,
}

/// The backdrop camera a world view drew its backdrop with (`SetBackdrop`):
/// its eye in backdrop units and its depth range, fitted around what the
/// backdrop shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackdropReadout {
    pub eye: [f32; 3],
    pub near: f32,
    pub far: f32,
}

/// Host choices that are not part of the retained model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererOptions {
    /// `RustyEngineProductDefaultWorldLights`: the neutral rig lights the world
    /// unless the product disables it.
    pub default_world_lights: bool,
    /// `RustyEngineProductDefaultViewmodelLights`: the neutral rig lights the
    /// viewmodel layer unless the product disables it.
    pub default_viewmodel_lights: bool,
    /// Render shadow maps for world lights whose `shadow_intent` requests
    /// them. Off by default; C# products enable it in their manifest.
    pub shadows: bool,
    /// The brightest casting sun also casts in the backdrop, through
    /// cascades fitted to each backdrop camera (`SetBackdrop`). On by
    /// default; it needs `shadows`.
    pub backdrop_shadows: bool,
    /// `RustyEngineProductShadowBudget`: at most this many shadow layers
    /// at once, the requesting lights chosen by priority then distance from
    /// the camera (`shadows::choose`); unlimited without one.
    pub shadow_budget: Option<u32>,
    /// Screen-space ambient occlusion on world views
    /// (`renderer.lighting.ambientOcclusion` in a product's manifest). Off by
    /// default.
    pub ambient_occlusion: AmbientOcclusion,
    /// Bin a world view's lights into a cluster grid before its pass and
    /// shade each fragment from its cluster (`light_clusters.rs`), instead of
    /// looping over every light. Off by default; a device without compute
    /// shaders loops regardless.
    pub clustered_lighting: bool,
    /// Test each view's opaque parts against its frustum on the GPU and
    /// draw them indirectly (`culling.rs`), instead of building the draw
    /// list on the CPU each time the camera moves. Off by default; a device
    /// without indirect draws keeps the CPU list regardless.
    pub gpu_culling: bool,
    /// Drop a world view's selected scatter copies hidden behind its opaque
    /// non-scatter depth (`occlusion.rs`). Off by default; a device
    /// without compute or without the selection draws them regardless.
    pub occlusion_culling: bool,
    /// Samples per pixel of the primary destination (the offscreen primary
    /// target and the window surface): 1, 2 or 4. Offscreen render targets
    /// and captures stay single-sample.
    pub samples: u32,
    /// The temporal upscale stage (`upscale.rs`) brings the primary picture
    /// to the output, at any render scale; its primary destination is
    /// single-sample. A software adapter draws without it.
    pub temporal: bool,
    /// With `temporal`, NVIDIA DLSS draws the stage in TAAU's place
    /// (`dlss.rs`); where it is refused, TAAU draws.
    pub dlss: bool,
    /// Window output waits for the display's refresh before presenting;
    /// off presents as soon as a frame is drawn. Streamed output has no
    /// display.
    pub vsync: bool,
    /// How the desktop window shows; the window applies it
    /// (`Renderer::window_mode`). Streamed output has no window.
    pub window_mode: render_model::WindowMode,
    /// The fraction of the primary destination's size the primary views
    /// draw at, the render resolution (`upscale.rs`): 0.33 to 1.
    pub render_scale: f32,
    /// Light the volumetric fog's medium and volumes in a froxel grid
    /// (`volumetric_fog.rs`). Off by default; a device without compute
    /// shaders, or a software one, draws without it.
    pub volumetric_fog: render_model::VolumetricFogQuality,
    /// Draw the sky's cloud layer raymarched, with thickness (`sky.wgsl`
    /// `fs_clouds_volumetric`). Off by default; a software adapter draws the
    /// flat layer.
    pub volumetric_clouds: render_model::VolumetricCloudsQuality,
    /// Water reflects what the view shows (`world.wgsl`
    /// `water_reflection_of`), from the opaque scene's colour copy
    /// (`water.rs`). Off by default; a software adapter, or a device that
    /// cannot bind the copy, reflects the sky's light and the probes alone.
    pub water_reflections: render_model::WaterReflectionsQuality,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            default_world_lights: true,
            default_viewmodel_lights: true,
            shadows: false,
            backdrop_shadows: true,
            shadow_budget: None,
            ambient_occlusion: AmbientOcclusion::default(),
            clustered_lighting: false,
            gpu_culling: false,
            occlusion_culling: false,
            samples: RendererSettingsDescriptor::DEFAULT.antialiasing.samples(),
            temporal: false,
            dlss: false,
            vsync: RendererSettingsDescriptor::DEFAULT.vsync,
            window_mode: RendererSettingsDescriptor::DEFAULT.window_mode,
            render_scale: RendererSettingsDescriptor::DEFAULT.render_scale,
            volumetric_fog: render_model::VolumetricFogQuality::Off,
            volumetric_clouds: render_model::VolumetricCloudsQuality::Off,
            water_reflections: render_model::WaterReflectionsQuality::Off,
        }
    }
}

impl RendererOptions {
    /// These options with a product's settings realized. The Engine's own
    /// choices stay here: which occlusion path draws a mode (kept while the
    /// mode is unchanged, so a host's choice of the compute path survives),
    /// and the default light rigs.
    pub fn with_settings(mut self, settings: &RendererSettingsDescriptor) -> Self {
        self.shadows = settings.shadows;
        self.backdrop_shadows = settings.backdrop_shadows;
        self.shadow_budget = settings.shadow_budget;
        let mode = settings.ambient_occlusion.mode;
        self.ambient_occlusion = AmbientOcclusion {
            path: if self.settings().ambient_occlusion.mode == mode {
                self.ambient_occlusion.path
            } else {
                match mode {
                    AmbientOcclusionMode::Disabled => AmbientOcclusionPath::Off,
                    // The raster path draws the compute path's image and
                    // costs less on the GPUs measured (Den `compute-ao-9510`).
                    AmbientOcclusionMode::ScreenSpace => AmbientOcclusionPath::Raster,
                    AmbientOcclusionMode::DistanceField => AmbientOcclusionPath::DistanceField,
                }
            },
            strength: settings.ambient_occlusion.strength,
            radius: settings.ambient_occlusion.radius,
        };
        self.samples = settings.antialiasing.samples();
        self.temporal = settings.antialiasing.temporal();
        self.dlss = settings.antialiasing == Antialiasing::Dlss;
        self.vsync = settings.vsync;
        self.window_mode = settings.window_mode;
        self.render_scale = settings.render_scale;
        self.clustered_lighting = settings.clustered_lighting;
        self.gpu_culling = settings.gpu_culling;
        self.occlusion_culling = settings.occlusion_culling;
        self.volumetric_fog = settings.volumetric_fog;
        self.volumetric_clouds = settings.volumetric_clouds;
        self.water_reflections = settings.water_reflections;
        self
    }

    /// The settings these options realize, as a product reads them back.
    pub fn settings(&self) -> RendererSettingsDescriptor {
        RendererSettingsDescriptor {
            shadows: self.shadows,
            backdrop_shadows: self.backdrop_shadows,
            shadow_budget: self.shadow_budget,
            ambient_occlusion: render_model::AmbientOcclusionSettings {
                mode: match self.ambient_occlusion.path {
                    AmbientOcclusionPath::Off => AmbientOcclusionMode::Disabled,
                    AmbientOcclusionPath::Compute | AmbientOcclusionPath::Raster => {
                        AmbientOcclusionMode::ScreenSpace
                    }
                    AmbientOcclusionPath::DistanceField => AmbientOcclusionMode::DistanceField,
                },
                strength: self.ambient_occlusion.strength,
                radius: self.ambient_occlusion.radius,
            },
            antialiasing: if self.temporal && self.dlss {
                Antialiasing::Dlss
            } else if self.temporal {
                Antialiasing::Temporal
            } else {
                Antialiasing::from_samples(self.samples).unwrap_or_default()
            },
            render_scale: self.render_scale,
            vsync: self.vsync,
            window_mode: self.window_mode,
            clustered_lighting: self.clustered_lighting,
            gpu_culling: self.gpu_culling,
            occlusion_culling: self.occlusion_culling,
            volumetric_fog: self.volumetric_fog,
            volumetric_clouds: self.volumetric_clouds,
            water_reflections: self.water_reflections,
        }
    }
}

/// Why the device cannot realize a setting as asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingRefusal {
    /// The adapter has no compute shaders.
    NoComputeShaders,
    /// The adapter cannot draw from GPU-written indirect arguments.
    NoIndirectDraws,
    /// The primary destination's formats cannot multisample at that count.
    UnsupportedSampleCount,
    /// The display can present only in step with its refresh, so vsync stays
    /// on.
    VsyncOnly,
    /// A software adapter draws without the feature (a GPU-only feature,
    /// docs/verification.md#gpu-verification).
    SoftwareAdapter,
    /// The device binds too few textures per shader stage for the feature.
    TooFewTextureBindings,
    /// This build of the Engine does not have the feature (DLSS, built only
    /// with render-wgpu's `dlss` feature).
    NotBuilt,
    /// The adapter cannot run the feature (DLSS off NVIDIA RTX GPUs).
    UnsupportedAdapter,
    /// The feature's library failed to start on this device (DLSS's NGX).
    InitializationFailed,
}

/// The settings in effect and what the device refused: what a product
/// reads back through `RendererSettings`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererSettingsReadout {
    /// What is asked of the device: the product's request with the
    /// player's choices over it.
    pub requested: RendererSettingsDescriptor,
    /// What the product (or its manifest) asked for.
    pub product: RendererSettingsDescriptor,
    /// The player's choices (video options), applied over the product's.
    pub player: RendererSettingsOverrides,
    /// What draws: the request with each refused setting replaced by what
    /// the device does instead.
    pub effective: RendererSettingsDescriptor,
    pub ambient_occlusion: Option<SettingRefusal>,
    pub antialiasing: Option<SettingRefusal>,
    pub vsync: Option<SettingRefusal>,
    pub clustered_lighting: Option<SettingRefusal>,
    pub gpu_culling: Option<SettingRefusal>,
    pub occlusion_culling: Option<SettingRefusal>,
    pub volumetric_fog: Option<SettingRefusal>,
    pub volumetric_clouds: Option<SettingRefusal>,
    pub water_reflections: Option<SettingRefusal>,
}

pub struct Renderer {
    gpu: Gpu,
    /// The options in effect: `product_settings` with `player_settings` over
    /// them.
    options: RendererOptions,
    product_settings: RendererSettingsDescriptor,
    player_settings: RendererSettingsOverrides,
    layouts: Layouts,
    pipelines: Vec<Pipelines>,
    tables: Tables,
    white: GpuTexture,
    builtins: HashMap<Builtin, GpuMesh>,
    unlit_material: wgpu::BindGroup,
    lit_fallback_material: wgpu::BindGroup,
    frame_buffer: wgpu::Buffer,
    /// The cloud regions (`rusty::view` binding 13), rewritten each frame.
    cloud_regions_buffer: wgpu::Buffer,
    parts_buffer: wgpu::Buffer,
    lights_buffer: wgpu::Buffer,
    lights: frame::LightRanges,
    /// The brightest enabled directional world light: the sky's sun, unless
    /// the atmosphere places one (`sky_sun`).
    sun: Option<frame::Sun>,
    /// The clouds the current view's background drew, which the sun's glow
    /// is seen through.
    sky_clouds: frame::SkyClouds,
    /// The current view's backdrop pass drew the sun's glow for bloom, kept
    /// where the backdrop leaves the sky open, which its world view uses.
    glow_kept: bool,
    /// The sky's light: the background prefiltered for the standard shader.
    sky_light: sky_light::SkyLight,
    /// The indirect light volume and the bake that keeps it current
    /// (`probes.rs`).
    probes: probes::ProbeVolume,
    /// Unbounded world lights in the last upload (`upload_lights`): the
    /// clustered path loops when they exceed its global list.
    global_lights: u32,
    /// Part ids: the world and viewmodel view lists, then each shadow
    /// layer's casters.
    instances_buffer: wgpu::Buffer,
    /// The shadow layers' caster ids are in the instance buffer.
    casters_uploaded: bool,
    /// Per view layer (world, viewmodel, backdrop): the last draw list.
    views: [Option<frame::ViewCache>; 3],
    /// The backdrop camera the last world view drew its backdrop with.
    backdrop: Option<BackdropReadout>,
    /// The volumetric clouds' reduced targets and histories, per view.
    cloud_march: cloud_march::CloudMarch,
    /// The star field's loaded catalogue and parameters (`SetStars`).
    star_field: stars::StarField,
    shadows: shadows::ShadowMaps,
    /// World lights requesting shadows, by light row.
    shadow_candidates: Vec<shadows::ShadowCandidate>,
    /// The candidates casting now.
    casting: std::collections::HashSet<render_model::RenderHandle>,
    /// The eye a shadow budget last chose from, and whether this frame chose.
    shadow_eye: glam::Vec3,
    shadows_chosen: bool,
    /// Shadow layers and casters rendered since the frame began.
    shadows_rendered: (u32, u32),
    /// The frame's shadow-layer passes, first to last, in the frames that
    /// render layers.
    /// The sky's light was captured after the first primary view of this
    /// frame (`frame.rs` `capture_sky`).
    sky_captured: bool,
    shadow_timer: Option<timing::PassTimer>,
    /// The backdrop's cascades (`backdrop_shadows`), in their own passes.
    backdrop_shadow_timer: Option<timing::PassTimer>,
    /// Auto exposure adapted in this frame's first world view.
    exposure_adapted: bool,
    /// A reading draws (`PrimaryDraw::Reading`, an inspection view's
    /// capture): views read the histories the output's frames keep (the
    /// volumetric clouds', the fog's, auto exposure's) and keep none.
    reading: bool,
    frame_bind_group: wgpu::BindGroup,
    caster_bind_group: wgpu::BindGroup,
    sky_bind_group: Option<wgpu::BindGroup>,
    /// The opaque depth (and colour) the blend pass's water surfaces read.
    water: water::WaterScene,
    /// The Engine presentation timeline animated poses advance on.
    animation_time: f64,
    /// The presentation time frames draw at (`frame.time`, clouds, wind,
    /// precipitation): `animation_time`, or up to a step past it while the
    /// world runs between calls (`Renderer::set_presentation_time`).
    presentation_time: f64,
    animation_facts: Vec<animated::AnimationFact>,
    /// Realization generation per source entity, as the runtime reads it.
    animation_generations: HashMap<u64, u64>,
    /// Animated instances whose skinned vertices hold a previous pose for
    /// the motion (`animated.rs` `settle_skinned_history`).
    skinned_settling: Vec<render_model::RenderHandle>,
    /// Static mesh instances whose replaced payload holds the shape the
    /// last frame with motion drew, and the motion frame each new shape was
    /// written for (`apply.rs` `deformed_history`).
    deformed_settling: HashMap<render_model::RenderHandle, u64>,
    ghosts: HashMap<render_presentation::GhostPlateHandle, ghost::GhostPlate>,
    ghost_pipelines: ghost::GhostPipelines,
    /// Overlay sets and what inspection shows (`overlays.rs`).
    overlays: overlays::Overlays,
    /// Engine-owned cameras drawn beside the product's (`inspection.rs`).
    inspection_views: Vec<inspection::InspectionView>,
    /// The ones the last composition frame drew.
    drawn_inspection: Vec<inspection::InspectionView>,
    /// Counts applied deltas; offscreen composition targets re-render when
    /// it moves past the value they were drawn at.
    scene_generation: u64,
    compose: compose::Compose,
    /// The picture the primary views draw into at render resolution while
    /// an upscale stage or an image effect runs, kept while its size and
    /// sample count hold.
    picture: Option<target::RenderPicture>,
    /// While a primary frame draws under a temporal stage, its picture's
    /// viewmodel depth, which the viewmodel passes draw against
    /// (`RenderPicture::viewmodel_depth`).
    viewmodel_depth: Option<wgpu::TextureView>,
    /// The last primary frame's resolutions and upscale stage, for the
    /// readout.
    upscaled: UpscaleReadout,
    /// The step from that picture to output resolution (`upscale.rs`).
    upscale: upscale::Upscale,
    /// The jitter, motion target and history a temporal stage reads
    /// (`motion.rs`).
    temporal: motion::Temporal,
    /// Whether the window the renderer last drew to can present only in step
    /// with its refresh (`WindowSurface::vsync_only`); `None` before a window
    /// frame.
    display_vsync_only: Option<bool>,
    /// CPU time this frame spent building draw lists (`batch`), reported as
    /// `FrameStats::cpu_batch_us`.
    batch_time: std::time::Duration,
    /// What scattered patches have cost to take in (`ScatterUploadReadout`).
    scatter_upload: ScatterUploadReadout,
    /// Every scatter run's copies (`scatter.rs`).
    scatter_copies: scatter::ScatterCopies,
    ambient_occlusion: ambient_occlusion::AmbientOcclusionPass,
    /// The world's HDR targets and the finish pass.
    finish: finish::Finish,
    distance_fields: distance_fields::DistanceFields,
    /// The world boxes of the resident distance fields, at the scene
    /// generation they were gathered at (`frame.rs` `gather_field_entries`).
    field_entries: Option<(u64, Vec<distance_fields::FieldEntry>)>,
    light_clusters: light_clusters::LightClusters,
    culling: culling::GpuCulling,
    /// Scatter runs selected copy by copy (`scatter_select.rs`).
    scatter_select: scatter_select::ScatterSelect,
    /// Impostor levels' baked views (`impostor.rs`).
    impostors: impostor::Impostors,
    /// Selected scatter copies tested against the view's opaque depth
    /// (`occlusion.rs`).
    occlusion: occlusion::Occlusion,
    /// The meshes the selected scatter copies draw, pooled (`mesh_pool.rs`).
    mesh_pool: mesh_pool::MeshPool,
    composition: composition::ViewComposition,
    effects: effects::Effects,
    /// Rain or snow around the camera (`precipitation.rs`).
    precipitation: precipitation::Precipitation,
    /// Projected decals over the opaque world (`decals.rs`).
    decals: decals::Decals,
    /// The volumetric fog over each world view (`volumetric_fog.rs`).
    volumetric_fog: volumetric_fog::VolumetricFog,
    /// The product's image effect over each primary view (`image_effect.rs`).
    image_effect: image_effect::ImageEffect,
    particles: particles::Particles,
    labels: labels::Labels,
    video: video::Video,
    /// The size of the window surface last presented, if any.
    surface_size: Option<(u32, u32)>,
}

/// The water reflection's march at low and high quality: its steps and its
/// reach in metres.
const WATER_REFLECTION_MARCH: [(u32, f32); 2] = [(16, 24.0), (40, 48.0)];

/// Initial storage sizes; both grow by doubling.
const INITIAL_PARTS_BYTES: u64 = 64 * 1024;
const INITIAL_LIGHTS_BYTES: u64 = 4 * 1024;
const INITIAL_INSTANCES_BYTES: u64 = 16 * 1024;

impl Renderer {
    pub fn new(gpu: &Gpu, options: RendererOptions) -> Self {
        let device = &gpu.device;
        let mut layouts = Layouts::new(
            device,
            gpu.decals_refusal().is_none(),
            gpu.scene_color_refusal().is_none(),
        );
        let frame_buffer = frame::frame_uniform_buffer(device);
        let cloud_regions_buffer = frame::storage_buffer(
            device,
            "render-wgpu cloud regions",
            frame::CLOUD_REGION_BYTES * render_model::CloudRegionDescriptor::MAX_REGIONS as u64,
        );
        let parts_buffer = frame::storage_buffer(device, "render-wgpu parts", INITIAL_PARTS_BYTES);
        let lights_buffer =
            frame::storage_buffer(device, "render-wgpu lights", INITIAL_LIGHTS_BYTES);
        let instances_buffer =
            frame::storage_buffer(device, "render-wgpu instances", INITIAL_INSTANCES_BYTES);
        let shadows = shadows::ShadowMaps::new(device, &layouts.shadow_layer);
        let sky_light = sky_light::SkyLight::new(gpu);
        let shadow_timer = timing::PassTimer::new(gpu, "shadows");
        let backdrop_shadow_timer = timing::PassTimer::new(gpu, "backdrop-shadows");
        let probes = probes::ProbeVolume::new(gpu);
        let scatter_copies = scatter::ScatterCopies::new(gpu);
        let light_clusters = light_clusters::LightClusters::new(
            gpu,
            pipelines::standard(layouts.shaders.module(
                device,
                Entry::LightClusters,
                Features::default(),
            )),
        );
        let culling = culling::GpuCulling::new(
            gpu,
            pipelines::standard(
                layouts
                    .shaders
                    .module(device, Entry::Cull, Features::default()),
            ),
        );
        let scatter_select = scatter_select::ScatterSelect::new(
            gpu,
            pipelines::standard(layouts.shaders.module(
                device,
                Entry::ScatterSelect,
                Features::default(),
            )),
        );
        let occlusion = occlusion::Occlusion::new(
            gpu,
            pipelines::standard(
                layouts
                    .shaders
                    .module(device, Entry::Hiz, Features::default()),
            ),
        );
        let temporal = motion::Temporal::new(gpu);
        let frame_bind_group = frame::frame_bind_group(
            device,
            &layouts.frame,
            frame::FrameBindings {
                frame: &frame_buffer,
                parts: &parts_buffer,
                lights: &lights_buffer,
                instances: &instances_buffer,
                shadows: &shadows,
                sky_light: &sky_light,
                clusters: &light_clusters.clusters,
                probes: &probes,
                cloud_regions: &cloud_regions_buffer,
                copies: scatter_copies.buffer(),
                temporal: &temporal,
            },
        );
        let caster_bind_group = frame::caster_bind_group(
            device,
            &layouts.casters,
            &frame_buffer,
            &parts_buffer,
            &instances_buffer,
            &shadows,
            scatter_copies.buffer(),
        );
        let white = white_texture(gpu);
        let decals = decals::Decals::new(gpu, layouts.decals.as_ref(), &white);
        let (unlit_material, lit_fallback_material) =
            apply::builtin_materials(device, &layouts.material, &white);
        let finish = finish::Finish::new(gpu, &mut layouts.shaders);
        let water = water::WaterScene::new(gpu, &mut layouts.shaders, gpu.scene_color_refusal());
        let effects = effects::Effects::new(
            device,
            &layouts.frame,
            pipelines::standard(layouts.shaders.module(
                device,
                Entry::Effects,
                Features::default(),
            )),
        );
        let volumetric_fog = volumetric_fog::VolumetricFog::new(
            gpu,
            pipelines::standard(layouts.shaders.module(
                device,
                Entry::VolumetricFog,
                Features::default(),
            )),
            &layouts.frame,
        );
        let precipitation = precipitation::Precipitation::new(
            device,
            &mut layouts.shaders,
            &layouts.frame,
            effects.scene_depth_layouts(),
        );
        let ghost_shader = pipelines::standard(layouts.shaders.module(
            device,
            Entry::Ghost,
            Features::default(),
        ));
        let compose_shader = pipelines::standard(layouts.shaders.module(
            device,
            Entry::Compose,
            Features::default(),
        ));
        let ambient_occlusion_shader = pipelines::standard(layouts.shaders.module(
            device,
            Entry::AmbientOcclusion,
            Features::default(),
        ));
        let ambient_occlusion = ambient_occlusion::AmbientOcclusionPass::new(
            gpu,
            ambient_occlusion_shader,
            &layouts.ambient_occlusion,
        );
        let distance_fields = distance_fields::DistanceFields::new(
            gpu,
            pipelines::standard(layouts.shaders.module(
                device,
                Entry::DistanceField,
                Features::default(),
            )),
        );
        let mut renderer = Self {
            gpu: gpu.clone(),
            product_settings: options.settings(),
            player_settings: RendererSettingsOverrides::default(),
            options,
            layouts,
            pipelines: Vec::new(),
            tables: Tables::new(),
            white,
            builtins: HashMap::new(),
            unlit_material,
            lit_fallback_material,
            frame_buffer,
            cloud_regions_buffer,
            parts_buffer,
            lights_buffer,
            lights: Default::default(),
            sun: None,
            sky_clouds: Default::default(),
            glow_kept: false,
            sky_light,
            probes,
            global_lights: 0,
            instances_buffer,
            casters_uploaded: false,
            views: Default::default(),
            backdrop: None,
            cloud_march: cloud_march::CloudMarch::new(gpu),
            star_field: stars::StarField::new(gpu),
            shadows,
            shadow_candidates: Vec::new(),
            casting: Default::default(),
            shadow_eye: glam::Vec3::ZERO,
            shadows_chosen: false,
            shadows_rendered: (0, 0),
            sky_captured: false,
            shadow_timer,
            backdrop_shadow_timer,
            exposure_adapted: false,
            reading: false,
            frame_bind_group,
            caster_bind_group,
            sky_bind_group: None,
            water,
            surface_size: None,
            animation_time: 0.0,
            presentation_time: 0.0,
            animation_facts: Vec::new(),
            animation_generations: HashMap::new(),
            skinned_settling: Vec::new(),
            deformed_settling: HashMap::new(),
            ghosts: HashMap::new(),
            ghost_pipelines: ghost::GhostPipelines::new(device, ghost_shader),
            overlays: overlays::Overlays::new(),
            inspection_views: Vec::new(),
            drawn_inspection: Vec::new(),
            scene_generation: 0,
            compose: compose::Compose::new(device, compose_shader),
            picture: None,
            viewmodel_depth: None,
            upscaled: UpscaleReadout::default(),
            upscale: upscale::Upscale::new(gpu),
            temporal,
            display_vsync_only: None,
            batch_time: std::time::Duration::ZERO,
            scatter_upload: ScatterUploadReadout::default(),
            scatter_copies,
            ambient_occlusion,
            finish,
            distance_fields,
            field_entries: None,
            light_clusters,
            culling,
            scatter_select,
            impostors: impostor::Impostors::new(device),
            occlusion,
            mesh_pool: mesh_pool::MeshPool::new(gpu),
            composition: Default::default(),
            effects,
            precipitation,
            decals,
            volumetric_fog,
            image_effect: image_effect::ImageEffect::new(gpu),
            particles: Default::default(),
            labels: Default::default(),
            video: video::Video::new(device),
        };
        for kind in [
            Builtin::Cube,
            Builtin::Sphere,
            Builtin::Quad,
            Builtin::Point,
        ] {
            let geometry = primitives::builtin(kind);
            let count = geometry.indices.len() as u32;
            let mesh = renderer
                .upload_vertices(
                    "render-wgpu builtin",
                    &geometry.vertices,
                    &geometry.indices,
                    tables::Topology::Triangles,
                    vec![(0, 0, count)],
                    Default::default(),
                )
                .expect("a device has room for the builtin meshes");
            renderer.builtins.insert(kind, mesh);
        }
        renderer.tables.parts.selecting = renderer.scatter_select.available();
        if renderer.options.dlss {
            renderer.upscale.prepare_dlss(&renderer.gpu);
        }
        renderer
    }

    pub fn options(&self) -> RendererOptions {
        self.options
    }

    /// Target pixels per CSS pixel of the output: the desktop window's scale
    /// factor, or the stream viewer's device pixel ratio. The host sets it,
    /// not the product. Labels, pixel-sized sprites and particle points are
    /// authored in CSS pixels and keep that size on a denser target. Labels rasterize again at a new ratio.
    pub fn set_pixel_ratio(&mut self, ratio: f32) {
        if ratio.is_finite() && ratio > 0.0 {
            self.labels.set_pixel_ratio(ratio);
        }
    }

    pub fn pixel_ratio(&self) -> f32 {
        self.labels.pixel_ratio()
    }

    /// Change host options; lights (and shadow layers) are re-derived on the
    /// next render. Retained chunk fields take or give up their atlas bricks
    /// as the occlusion path enters or leaves the distance-field path.
    /// Replace the options. Their settings are the product's: the player's
    /// choices (`set_player_settings`) still apply over them.
    pub fn set_options(&mut self, options: RendererOptions) {
        self.product_settings = options.settings();
        let layered = options.with_settings(&self.player_settings.apply(self.product_settings));
        self.realize_options(layered);
    }

    /// The player's choices (video options), applied over every product
    /// request from now on, and over the current one at once.
    pub fn set_player_settings(&mut self, player: RendererSettingsOverrides) {
        self.player_settings = player;
        self.set_options(self.options.with_settings(&self.product_settings));
    }

    fn realize_options(&mut self, options: RendererOptions) {
        let traced = |options: &RendererOptions| {
            options.ambient_occlusion.path == AmbientOcclusionPath::DistanceField
        };
        let (was, now) = (traced(&self.options), traced(&options));
        if options.dlss {
            self.upscale.prepare_dlss(&self.gpu);
        }
        self.options = options;
        self.tables.lights_dirty = true;
        if was != now {
            self.field_entries = None;
            for mesh in self.tables.payload_meshes.values_mut() {
                let Some(field) = &mut mesh.distance_field else {
                    continue;
                };
                if now {
                    field.slot = field
                        .slot
                        .or_else(|| self.distance_fields.allocate(&self.gpu, &field.data));
                } else if let Some(slot) = field.slot.take() {
                    self.distance_fields.release(slot);
                }
            }
        }
    }

    /// Realize a product's settings (`RenderDiff::SetRendererSettings`).
    pub(crate) fn set_settings(&mut self, settings: &RendererSettingsDescriptor) {
        self.set_options(self.options.with_settings(settings));
    }

    /// Samples per pixel the primary destination should have: the option,
    /// or 4 where this device cannot multisample at it. Hosts size their
    /// targets by it.
    pub fn samples(&self) -> u32 {
        if self.gpu.samples_supported(self.options.samples) {
            self.options.samples
        } else {
            RendererSettingsDescriptor::DEFAULT.antialiasing.samples()
        }
    }

    /// Why frames draw without the temporal upscale stage: a software
    /// adapter refuses it (a GPU-only feature,
    /// docs/verification.md#gpu-verification). They then draw without
    /// anti-aliasing, at the render scale through the bilinear stage or a
    /// product image effect.
    pub(crate) fn temporal_refusal(&self) -> Option<SettingRefusal> {
        self.gpu
            .is_software()
            .then_some(SettingRefusal::SoftwareAdapter)
    }

    /// Why DLSS does not draw the temporal stage, if it cannot
    /// (`dlss.rs`): TAAU draws in its place.
    pub(crate) fn dlss_refusal(&self) -> Option<DlssRefusal> {
        self.upscale.dlss_refusal()
    }

    /// Whether window output should wait for the display's refresh.
    pub fn vsync(&self) -> bool {
        self.options.vsync
    }

    /// How the desktop window should show.
    pub fn window_mode(&self) -> render_model::WindowMode {
        self.options.window_mode
    }

    /// The window shows `mode` other than by request: it reopened in the
    /// player's remembered mode, or the player changed it through the window
    /// manager. The request takes it, so the product reads how the window
    /// shows and a request for any other mode is a change the window applies.
    pub fn adopt_window_mode(&mut self, mode: render_model::WindowMode) {
        self.product_settings.window_mode = mode;
        if self.player_settings.window_mode.is_some() {
            self.player_settings.window_mode = Some(mode);
        }
        self.options.window_mode = mode;
    }

    /// The present modes of the display the renderer draws to, from which
    /// the readout judges whether a request for no vsync is realized
    /// (`WindowSurface::vsync_only_among`). The window paths note their
    /// surface's every frame; a host presenting another way notes its own.
    pub fn set_display_present_modes(&mut self, modes: &[wgpu::PresentMode]) {
        self.display_vsync_only = Some(WindowSurface::vsync_only_among(modes));
    }

    /// The settings in effect and what the device refused.
    pub fn settings_readout(&self) -> RendererSettingsReadout {
        let requested = self.options.settings();
        let mut effective = requested;
        let antialiasing = if self.options.temporal {
            self.temporal_refusal()
                .inspect(|_| {
                    effective.antialiasing = Antialiasing::Off;
                })
                .or_else(|| {
                    self.options
                        .dlss
                        .then(|| self.dlss_refusal())
                        .flatten()
                        .map(|refused| {
                            effective.antialiasing = Antialiasing::Temporal;
                            refused.refusal
                        })
                })
        } else {
            (!self.gpu.samples_supported(self.options.samples)).then(|| {
                effective.antialiasing =
                    Antialiasing::from_samples(self.samples()).unwrap_or_default();
                SettingRefusal::UnsupportedSampleCount
            })
        };
        let ambient_occlusion = (requested.ambient_occlusion.mode
            == AmbientOcclusionMode::DistanceField
            && !self.distance_fields.available())
        .then(|| {
            effective.ambient_occlusion.mode = AmbientOcclusionMode::ScreenSpace;
            SettingRefusal::NoComputeShaders
        });
        let clustered_lighting = (requested.clustered_lighting
            && self.light_clusters.readout().refused.is_some())
        .then(|| {
            effective.clustered_lighting = false;
            SettingRefusal::NoComputeShaders
        });
        let gpu_culling = (requested.gpu_culling && !self.culling.available()).then(|| {
            effective.gpu_culling = false;
            SettingRefusal::NoIndirectDraws
        });
        let occlusion_culling = (requested.occlusion_culling
            && !(self.occlusion.available() && self.scatter_select.available()))
        .then(|| {
            effective.occlusion_culling = false;
            if self.occlusion.available() {
                SettingRefusal::NoIndirectDraws
            } else {
                SettingRefusal::NoComputeShaders
            }
        });
        let vsync = (!requested.vsync && self.display_vsync_only == Some(true)).then(|| {
            effective.vsync = true;
            SettingRefusal::VsyncOnly
        });
        let volumetric_fog = (requested.volumetric_fog != render_model::VolumetricFogQuality::Off
            && self.volumetric_fog.refused().is_some())
        .then(|| {
            effective.volumetric_fog = render_model::VolumetricFogQuality::Off;
            if self.gpu.is_software() {
                SettingRefusal::SoftwareAdapter
            } else {
                SettingRefusal::NoComputeShaders
            }
        });
        let volumetric_clouds = (requested.volumetric_clouds
            != render_model::VolumetricCloudsQuality::Off
            && self.gpu.is_software())
        .then(|| {
            effective.volumetric_clouds = render_model::VolumetricCloudsQuality::Off;
            SettingRefusal::SoftwareAdapter
        });
        let water_reflections = (requested.water_reflections
            != render_model::WaterReflectionsQuality::Off)
            .then(|| self.water_reflections_refusal())
            .flatten()
            .inspect(|_| {
                effective.water_reflections = render_model::WaterReflectionsQuality::Off;
            });
        RendererSettingsReadout {
            requested,
            product: self.product_settings,
            player: self.player_settings,
            effective,
            ambient_occlusion,
            antialiasing,
            vsync,
            clustered_lighting,
            gpu_culling,
            occlusion_culling,
            volumetric_fog,
            volumetric_clouds,
            water_reflections,
        }
    }

    /// Why water cannot reflect the view on this device, if it cannot: a
    /// software adapter, or a device that cannot bind the scene's colour.
    fn water_reflections_refusal(&self) -> Option<SettingRefusal> {
        if self.gpu.is_software() {
            Some(SettingRefusal::SoftwareAdapter)
        } else if !self.water.scene_color() {
            Some(SettingRefusal::TooFewTextureBindings)
        } else {
            None
        }
    }

    /// The water reflection's march, steps and reach in metres, as it
    /// draws: none when off or refused.
    pub(crate) fn water_reflection_march(&self) -> Option<(u32, f32)> {
        match self.options.water_reflections {
            render_model::WaterReflectionsQuality::Off => None,
            _ if self.water_reflections_refusal().is_some() => None,
            render_model::WaterReflectionsQuality::Low => Some(WATER_REFLECTION_MARCH[0]),
            render_model::WaterReflectionsQuality::High => Some(WATER_REFLECTION_MARCH[1]),
        }
    }

    /// What the scatter patches made so far cost to take in.
    pub fn scatter_upload_readout(&self) -> ScatterUploadReadout {
        self.scatter_upload
    }

    /// The renderer's GPU passes (ambient occlusion's, then the world, its
    /// bloom and exposure, and its finish): each timed pass's cost, the
    /// adapter's compute limits, and the ambient occlusion the last world
    /// view took.
    pub fn gpu_readout(&self) -> GpuReadout {
        GpuReadout {
            timestamps: self
                .gpu
                .device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY),
            limits: self.gpu.compute_limits(),
            passes: self
                .ambient_occlusion
                .timings()
                .into_iter()
                .chain(self.finish.timings())
                .chain([
                    self.shadow_timer
                        .as_ref()
                        .map_or_else(|| timing::untimed("shadows"), timing::PassTimer::readout),
                    self.backdrop_shadow_timer.as_ref().map_or_else(
                        || timing::untimed("backdrop-shadows"),
                        timing::PassTimer::readout,
                    ),
                    self.light_clusters.timing(),
                    self.volumetric_fog.timing(),
                    self.culling.timing(),
                    self.occlusion.timing(),
                    self.scatter_select.timing(),
                    self.sky_light.timing(),
                    self.image_effect.timing(),
                    self.upscale.timing(),
                    self.water.timing(),
                ])
                .collect(),
            ambient_occlusion: self.ambient_occlusion.readout(),
            distance_fields: self.distance_fields.readout(),
            light_clusters: self.light_clusters.readout(),
            gpu_culling: self.culling.readout(),
            scatter_selection: self.scatter_select.readout(self.scatter_copies.refused),
            impostors: self.impostors.readout(),
            occlusion: self.occlusion.readout(),
            indirect_light: self.probes.readout(),
            volumetric_fog: self.volumetric_fog.readout(),
            backdrop: self.backdrop,
            wetness: self.tables.wetness,
            snow: self.tables.snow,
            decals: self.decals.readout(),
            water: self.water.readout(),
            upscale: self.upscaled,
            dlss: self.options.dlss.then(|| self.dlss_refusal()).flatten(),
            device_lost: self.gpu.lost(),
        }
    }

    /// Request the indirect light volume, move the one there is, or with
    /// `None` drop it, as `RenderDiff::SetIndirectLight` does. A request
    /// bakes after the scene has been still for `probes::DEBOUNCE`.
    /// The image effect's shader modules composed and pipelines made so
    /// far: a parameter change builds neither.
    pub fn image_effect_builds(&self) -> (u64, u64) {
        self.image_effect.builds()
    }

    /// The volumetric fog's medium, as `RenderDiff::SetVolumetricFog` sets
    /// it.
    pub fn set_volumetric_fog(&mut self, fog: render_model::VolumetricFogDescriptor) {
        self.tables.volumetric_fog = fog;
    }

    /// Whether the sky's clouds draw raymarched: asked for, and not on a
    /// software adapter.
    pub(crate) fn volumetric_clouds_drawn(&self) -> bool {
        self.options.volumetric_clouds != render_model::VolumetricCloudsQuality::Off
            && !self.gpu.is_software()
    }

    /// Whether the scene has fog for volumetric fog to light: a medium with
    /// density, or fog volumes.
    pub fn has_volumetric_fog(&self) -> bool {
        self.tables.volumetric_fog.density > 0.0 || !self.tables.fog_volumes.is_empty()
    }

    pub fn set_indirect_light(&mut self, indirect_light: Option<IndirectLightDescriptor>) {
        self.tables.indirect_light = indirect_light;
        if self.probes.request(&self.gpu, indirect_light) {
            self.rebind_frame();
        }
    }

    /// A retained change every brick of the indirect light volume should
    /// follow: a material, a texture or the sky.
    pub(crate) fn touch_indirect_light(&mut self) {
        if self.probes.mark_all() {
            self.probes.touch(std::time::Instant::now());
        }
    }

    /// Bake the volume's dirty bricks now, on this thread's workers, and
    /// upload them: for tools and tests that want the volume before the
    /// next frame. `None` without a request.
    pub fn bake_indirect_light_now(&mut self) -> Option<IndirectLightReadout> {
        self.tables.indirect_light?;
        // The batch reads the nodes' world state, which a frame would have
        // settled first.
        self.propagate_transforms();
        if let Some(batch) = self.probe_batch() {
            self.probes.bake_now(&self.gpu, batch);
        } else {
            self.probes.settle();
        }
        Some(self.probes.readout())
    }

    pub fn indirect_light_readout(&self) -> IndirectLightReadout {
        self.probes.readout()
    }

    /// Retained table sizes, for diagnostics and tests.
    pub fn mesh_memory(&self) -> MeshMemory {
        let mut memory = MeshMemory::default();
        let mut seen = std::collections::HashSet::new();
        let mut count = |mesh: &tables::GpuMesh| {
            memory.meshes += 1;
            memory.gpu_bytes += mesh.vertices.size() + mesh.indices.size();
            if seen.insert(std::sync::Arc::as_ptr(&mesh.cpu)) {
                memory.cpu_geometry_bytes +=
                    (mesh.cpu.positions.len() * 12 + mesh.cpu.indices.len() * 4) as u64;
            }
        };
        self.tables.static_meshes.values().for_each(&mut count);
        self.tables.payload_meshes.values().for_each(&mut count);
        self.tables
            .voxel_objects
            .values()
            .flat_map(|row| row.meshes.iter())
            .for_each(&mut count);
        self.for_each_animated_mesh(&mut count);
        memory
    }

    pub fn shadow_report(&self) -> ShadowReport {
        ShadowReport {
            layers: self.shadows.layers.len(),
            pages: self.shadows.pages(),
            budget: self.options.shadow_budget,
            casting: self.casting.len(),
            skipped: self
                .shadow_candidates
                .iter()
                .filter(|candidate| !self.casting.contains(&candidate.light))
                .map(|candidate| candidate.light.raw())
                .collect(),
            rendered_layers: self.shadows_rendered.0,
            rendered_casters: self.shadows_rendered.1,
            atlas_bytes: self.shadows.atlas_bytes(),
            static_cache_bytes: self.shadows.cache_bytes(),
        }
    }

    /// Render every shadow layer again in the next frame, as if each were
    /// stale: an offline run measures the uncached cost with it.
    pub fn rerender_shadows(&mut self) {
        for layer in &mut self.shadows.layers {
            layer.stale = true;
        }
    }

    pub fn table_counts(&self) -> TableCounts {
        TableCounts {
            textures: self.tables.textures.len(),
            materials: self.tables.materials.len(),
            static_meshes: self.tables.static_meshes.len(),
            nodes: self.tables.nodes.len(),
            parts: self.tables.parts.meta.iter().flatten().count(),
            lights: self.tables.lights.len(),
            atlases: self.tables.atlases.len(),
            voxel_objects: self.tables.voxel_objects.len(),
            animated_meshes: self.tables.animated_assets.len(),
            animated_instances: self.tables.animated.len(),
            shadow_layers: self.shadows.layers.len(),
            shader_variants: self.layouts.shader_variants(),
        }
    }
}

/// The scene's shadows, for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShadowReport {
    /// Layers in the atlas and pages holding them.
    pub layers: usize,
    pub pages: usize,
    /// The manifest's budget in layers, if it sets one.
    pub budget: Option<u32>,
    /// Lights casting, and those requesting a shadow that the budget left
    /// out (by render handle).
    pub casting: usize,
    pub skipped: Vec<u64>,
    /// Layers re-rendered in the last frame and the casters drawn into them.
    pub rendered_layers: u32,
    pub rendered_casters: u32,
    /// The GPU bytes of the atlas's depth pages, allocated ones included.
    pub atlas_bytes: u64,
    /// The static cache's bytes: an atlas-sized depth array, built once a
    /// light's layer has moving casters; 0 before.
    pub static_cache_bytes: u64,
}

/// Mesh memory: the CPU geometry copies kept for bounds and wireframe, beside
/// the GPU vertex and index bytes of the same meshes (#8849).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeshMemory {
    pub meshes: usize,
    pub cpu_geometry_bytes: u64,
    pub gpu_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCounts {
    pub textures: usize,
    pub materials: usize,
    pub static_meshes: usize,
    pub nodes: usize,
    pub parts: usize,
    pub lights: usize,
    pub atlases: usize,
    pub voxel_objects: usize,
    pub animated_meshes: usize,
    pub animated_instances: usize,
    pub shadow_layers: usize,
    /// Standard shader feature sets compiled for the world and caster passes.
    pub shader_variants: usize,
}

/// A 1×1 white texture: untextured materials sample it.
fn white_texture(gpu: &Gpu) -> GpuTexture {
    use wgpu::util::DeviceExt;
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("render-wgpu white"),
            size: target::extent(1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &[255; 4],
    );
    GpuTexture {
        size: (1, 1),
        view: texture.create_view(&Default::default()),
        sampler: gpu.device.create_sampler(&Default::default()),
        mipped: None,
    }
}
