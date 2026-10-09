use crate::*;

/// Opaque Engine-owned camera identity. A product may select, update, replace,
/// and dispose a camera, but never receives a renderer or backend object.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraHandle {
    pub value: u64,
}

/// Opaque Engine-owned offscreen target identity. The renderer owns the GPU
/// target; products only select typed dimensions and sampling facts.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraTargetHandle {
    pub value: u64,
}

/// Copied target reference used by a composition view. Zero names the Engine
/// primary surface; nonzero values refer to one live Engine-owned target.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraTargetReference {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraBasisMode {
    Derived = 0,
    Explicit = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraProjectionKind {
    Perspective = 1,
    Orthographic = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetColor {
    Rgba8Srgb = 0,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetDepth {
    Depth24 = 0,
    None = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetSampling {
    Linear = 0,
    Nearest = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraPose {
    pub position: NativeVec3,
    pub pitch_degrees: f64,
    pub yaw_degrees: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraBasis {
    pub forward: NativeVec3,
    pub right: NativeVec3,
    pub up: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraProjection {
    pub kind: NativeCameraProjectionKind,
    pub fov_y_degrees: f64,
    pub vertical_size: f64,
    pub near: f64,
    pub far: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraViewport {
    /// Normalized to the current Engine-owned presentation surface.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One retained offscreen target. Its revision is Engine-owned and changes
/// only when a live target is updated or replaced.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetDescriptor {
    pub width: u32,
    pub height: u32,
    pub color: NativeCameraTargetColor,
    pub depth: NativeCameraTargetDepth,
    pub sampling: NativeCameraTargetSampling,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetUpdateRequest {
    pub target: NativeCameraTargetHandle,
    pub descriptor: NativeCameraTargetDescriptor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetReplaceRequest {
    /// The prior handle becomes a tombstone if replacement succeeds.
    pub target: NativeCameraTargetHandle,
    pub replacement: NativeCameraTargetDescriptor,
}

/// A target reference of zero selects the Engine primary surface. Nonzero
/// references select retained Engine-owned offscreen targets.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionView {
    pub camera: NativeCameraHandle,
    pub target: NativeCameraTargetReference,
    pub viewport: NativeCameraViewport,
    pub order: u64,
}

/// Presents one retained offscreen target to the Engine primary surface.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionPresentation {
    pub source_target: NativeCameraTargetHandle,
    pub destination: NativeCameraViewport,
    pub order: u64,
}

/// Borrowed composition slices are copied before the direct call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionRequest {
    pub views: *const NativeCameraCompositionView,
    pub views_len: usize,
    pub presentations: *const NativeCameraCompositionPresentation,
    pub presentations_len: usize,
}

/// Typed product facts for one Engine-owned view. The viewport is normalized,
/// so the Engine host realizes it against current resize observations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraDescriptor {
    pub pose: NativeCameraPose,
    pub basis_mode: NativeCameraBasisMode,
    pub basis: NativeCameraBasis,
    pub projection: NativeCameraProjection,
    pub viewport: NativeCameraViewport,
    /// The viewmodel layer's vertical field of view in degrees, for a
    /// perspective camera; 0 draws it with the camera's own projection.
    pub viewmodel_fov_y_degrees: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraUpdateRequest {
    pub camera: NativeCameraHandle,
    pub descriptor: NativeCameraDescriptor,
}

/// Selects the opt-in renderer presentation sampling applied to a camera
/// update. `Latest` publishes the descriptor immediately; the interpolation
/// variants retain the supplied product timeline facts for the renderer.
/// The generated safe C# API exposes only these values; raw table consumers
/// must likewise pass one of these declared discriminants.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraInterpolation {
    Latest = 0,
    Position = 1,
    Pose = 2,
}

/// The presentation surface camera viewports are normalized to, as the
/// page showing the product last reported it, in stream and window output
/// alike. `reported` is false until a page reports. `revision` changes with
/// every change, so a product notices a resize by comparing it.
///
/// `watching` is false only in stream output while no page pulls frames: it
/// turns false within 1.25 seconds of the last watching page leaving and true
/// as soon as one asks for a frame. In window output, and with no render
/// output, it is always true. It is not part of `revision`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeCameraSurfaceReadout {
    pub reported: bool,
    pub watching: bool,
    pub css_width: f64,
    pub css_height: f64,
    /// The surface in device pixels: its CSS size times the pixel ratio.
    pub device_width: f64,
    pub device_height: f64,
    pub device_pixel_ratio: f64,
    /// The scale the product UI applies to itself (`ui.setScale`).
    pub ui_scale: f64,
    pub revision: u64,
}

/// Makes a camera's primary views follow the product UI element the UI
/// anchors under `anchor` (`viewport.anchor(name, element)` in the UI
/// context): while the page reports that element's rect, the views draw there
/// instead of at their viewports, on resize and layout change with no product
/// call. An empty `anchor` removes it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewportAnchorRequest {
    pub camera: NativeCameraHandle,
    pub anchor: crate::NativeUtf8Slice,
}

/// Names the product UI anchor (`viewport.anchor(name, element)`) to read.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewportAnchorReadRequest {
    pub anchor: crate::NativeUtf8Slice,
}

/// The rect of the UI element anchored under a name, as the page last
/// reported it: normalized to the presentation surface with a bottom-left
/// origin, like a camera viewport, and clipped to the surface. `reported` is
/// false, and the rect zero, while the page reports no element under the name
/// or one wholly outside the surface. `revision` is the surface revision of
/// the report it comes from ([`NativeCameraSurfaceReadout::revision`]).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeCameraViewportAnchorReadout {
    pub reported: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub revision: u64,
}

/// One camera descriptor update with an optional renderer sampling contract.
/// The product supplies its admitted timeline and explicit cuts; the Engine
/// owns retained metadata and the opaque sample identity.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraSampleRequest {
    pub camera: NativeCameraHandle,
    pub descriptor: NativeCameraDescriptor,
    pub sample_time_seconds: f64,
    pub delay_seconds: f64,
    pub interpolation: NativeCameraInterpolation,
    pub cut: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraReplaceRequest {
    /// This handle becomes a tombstone if replacement succeeds.
    pub camera: NativeCameraHandle,
    pub replacement: NativeCameraDescriptor,
}

/// Explicit empty requests keep the generated direct API uniformly typed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeClearActiveCameraRequest {
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeClearSkyBackgroundRequest {
    pub reserved: u32,
}

/// Selects one opaque renderer-owned viewport clear color. Selecting a sky later
/// replaces this color, and clearing the sky returns to the Engine default.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSetBackgroundColorRequest {
    pub color: NativeColor,
}

/// Distance fog mode. `Off` clears the fog.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFogMode {
    Off = 0,
    /// No fog before `start`, full fog at `end` and beyond (metres).
    Linear = 1,
    /// Visibility `exp(-density × distance)`.
    Exponential = 2,
    /// Visibility `exp(-(density × distance)²)`: clearer near, denser far.
    ExponentialSquared = 3,
}

/// Distance fog over everything drawn in the world, by distance from the
/// camera, toward `color` (linear RGB; alpha is ignored). It blends after
/// exposure and tone mapping and never touches the background, so a fog
/// colour equal to the background colour fades geometry exactly into it.
/// Linear fog reads `start` and `end`; the exponential modes read `density`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFogRequest {
    pub mode: NativeFogMode,
    pub color: NativeColor,
    pub start: f32,
    pub end: f32,
    pub density: f32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeToneMappingOperator {
    /// Colour clamps at the target's range (the default).
    None = 0,
    /// Khronos PBR Neutral: base colours stay true, highlights compress.
    Neutral = 1,
    /// ACES filmic: film-like contrast, highlights roll toward white.
    AcesFilmic = 2,
}

/// How lit colour reaches the output: a linear exposure multiplier (1 by
/// default), then the operator. Applies to everything drawn in the world, not
/// the background. Image captures use their own exposure and tone mapping.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeToneMappingRequest {
    pub operator: NativeToneMappingOperator,
    pub exposure: f32,
}

/// Bloom: the world's light above `threshold` (with a soft knee below it)
/// spreads into a glow added at `intensity` before exposure and tone
/// mapping. Intensity 0 (the default) turns it off; at most 16.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeBloomRequest {
    pub threshold: f32,
    pub intensity: f32,
}

/// Auto exposure, when `enabled`: the exposure moves toward the one that
/// brings the world's average luminance to middle grey, kept between
/// `min_exposure` and `max_exposure`, closing `1 - e^(-speed·t)` of the gap
/// in `t` seconds of Engine presentation time (none while it is held). The
/// tone mapping exposure multiplies it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAutoExposureRequest {
    pub enabled: bool,
    pub speed: f32,
    pub min_exposure: f32,
    pub max_exposure: f32,
}

/// The air and the sun in the sky. The sun is the brightest enabled
/// directional world light. Fog thins with height: it has its `SetFog`
/// density at `fog_base_height` (render world y) and falls by `e` every
/// `fog_falloff_height` up (0: one density at every height). Looking toward
/// the sun the fog turns toward `haze_color` (linear RGB, alpha ignored) by
/// the cosine between the view and the sun raised to `haze_exponent` (0: no
/// haze). The sky (panorama or clear colour) shows a disc of
/// `sun_radius_degrees` (0 to 20; 0: none) and a halo of strength `sun_halo`
/// (0: none). All zero (the default) turns the atmosphere off.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAtmosphereRequest {
    pub fog_base_height: f32,
    pub fog_falloff_height: f32,
    pub haze_color: NativeColor,
    pub haze_exponent: f32,
    pub sun_radius_degrees: f32,
    pub sun_halo: f32,
}

/// Sun shafts: the sky around the sun (the brightest enabled directional
/// world light) blurred along rays from it, so light streams past whatever
/// stands in front of it. `intensity` (0 to 16) scales the light added; 0
/// (the default) turns them off. `length` (0 to 1) is how far the rays
/// reach, as a fraction of the way from each pixel to the sun; 0 takes the
/// Engine's default.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSunShaftsRequest {
    pub intensity: f32,
    pub length: f32,
}

/// The scene's wind: what materials with a wind bend or flutter
/// (`NativeMaterialRequest`) and product displace stages sway in.
/// `direction` is over the ground (world x, z; any length); `strength` (0
/// to 16) scales every material's bend and flutter, and 0 (the default)
/// stills the scene; `gust` (0 to 1) is the share of the lean that rises and
/// falls in gusts rather than holding steady.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWindRequest {
    pub direction: NativeVec2,
    pub strength: f32,
    pub gust: f32,
}

/// What clouds a layer or region holds, shaping the volumetric clouds by
/// height: a thin flat sheet, heaped clouds with flat bottoms (the
/// default), or towering storm cells.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeCloudKind {
    #[default]
    Cumulus = 0,
    Stratus = 1,
    Cumulonimbus = 2,
}

/// The sky's cloud layer, drawn over the sky panorama and lit by the sun.
/// `coverage` (0 to 1) is how much of the sky it covers, and 0 (the
/// default) draws no clouds; `drift` is its velocity over the ground (world
/// x, z) in metres per second (at most 1000); `altitude` (above 0, at most
/// 100 km) is the layer's height and `scale` (likewise) the size of one
/// cloud, in metres; `color` (linear, 0 to 16 a channel) tints the light
/// they take. The volumetric clouds stand from `altitude` up by `thickness`
/// metres (0: six tenths of the altitude), shaped by `kind`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCloudsRequest {
    pub coverage: f32,
    pub drift: NativeVec2,
    pub altitude: f32,
    pub scale: f32,
    pub color: NativeVec3,
    pub thickness: f32,
    pub kind: NativeCloudKind,
}

/// How wet the scene's lit surfaces are, after rain. `wetness` (0 to 1, 0
/// the default) darkens their diffuse colour and smooths them, most where
/// they face up and only under the open sky; `puddles` (0 to 1) gathers
/// standing water in patches on flat ground as they wet.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWetnessRequest {
    pub wetness: f32,
    pub puddles: f32,
}

/// A cloud region, placed or replaced by `id`: where the sky holds more
/// cloud than the layer's coverage, such as a weather front's storm.
/// `coverage` (0 to 1) fills a disc `radius` metres across around `center`
/// (world x, z), fading over its outer third and drifting at `drift` metres
/// per second; `darkness` (0 to 1) darkens its clouds' undersides. The cloud
/// layer, flat or volumetric, and its shade on the ground take the most
/// cloud of the layer and every region. At most 32 regions. Its volumetric
/// clouds are of `kind` and stand `thickness` metres tall from the layer's
/// altitude (0: the layer's thickness), blending into the layer's over its
/// outer third: a towering storm front beside a thin overcast at one base.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCloudRegionRequest {
    pub id: u32,
    pub center: NativeVec2,
    pub radius: f32,
    pub coverage: f32,
    pub darkness: f32,
    pub drift: NativeVec2,
    pub kind: NativeCloudKind,
    pub thickness: f32,
}

/// Links the backdrop layer (`RenderLayer.Backdrop`) to a camera's world
/// views. Each such view draws what is placed in it behind the world, after
/// the sky and clouds, from `origin + (eye - anchor) / scale` with the
/// view's own rotation and field of view; the world then draws over it.
///
/// `camera` is the camera whose views take this link; `0` links every view
/// without a link of its own. `anchor` is a world point, exact (integer
/// metres and a fraction). With `session`, it is resolved into that spatial
/// session's local frame at its current world origin, again after every
/// rebase, so the backdrop stays put with no product call; without one
/// (`0`), it is in the cameras' local frame as given. `origin` is the
/// backdrop point the anchor maps to; `scale` (above 0) is world metres per
/// backdrop unit: 1 for a backdrop at world scale, 1000 for a 1:1000
/// miniature. Fog and the cloud layer's shade reach the backdrop at its
/// world-equivalent distance and place.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeBackdropRequest {
    pub camera: NativeCameraHandle,
    pub session: crate::NativeSpatialSessionHandle,
    pub anchor: crate::NativeWorldOriginGlobalPosition,
    pub origin: NativeVec3,
    pub scale: f32,
}

/// Unlinks a camera's views from the backdrop (`0`: the link every view
/// without its own takes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeClearBackdropRequest {
    pub camera: NativeCameraHandle,
}

/// Removes cloud region `id`; removing one not placed does nothing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCloudRegionRemoval {
    pub id: u32,
}

/// The medium volumetric fog lights, while the renderer's `VolumetricFog`
/// setting is on. `density` (0 to 1, extinction per metre at `base_height`;
/// 0 fills no air, so only fog volumes draw) falls by e every
/// `falloff_height` metres up (0: one density everywhere); `albedo` (0 to 1
/// a channel) is the colour it scatters; `anisotropy` (-0.9 to 0.9)
/// scatters forward when positive, so it glows toward the sun; `distance`
/// (8 to 1000 m) is how far the fog grid reaches; `ambient` (0 to 4) scales
/// the ambient and sky light it scatters.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVolumetricFogRequest {
    pub density: f32,
    pub albedo: NativeVec3,
    pub anisotropy: f32,
    pub base_height: f32,
    pub falloff_height: f32,
    pub distance: f32,
    pub ambient: f32,
}

/// The shape of a fog volume.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFogVolumeShape {
    Box = 0,
    Ellipsoid = 1,
}

/// A fog volume, placed or replaced by `id`: a box or ellipsoid of fog the
/// volumetric fog adds to its medium, in the renderer's world space (as an
/// indirect light volume's centre). `half_extents` (above 0, at most 10 km),
/// turned `yaw_degrees` about the vertical; `density` (0 to 4) at its heart,
/// fading to nothing over `edge` (0 to 1) of its half extent; `albedo` (0
/// to 1) and `emission` (0 to 16, light per unit of density); 3D noise cells
/// `noise_scale` metres across (0: none) thin it by `noise_strength` (0 to 1)
/// as they drift at `noise_velocity` metres per second. At most 64 volumes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFogVolumeRequest {
    pub id: u32,
    pub shape: NativeFogVolumeShape,
    pub center: NativeVec3,
    pub half_extents: NativeVec3,
    pub yaw_degrees: f32,
    pub density: f32,
    pub albedo: NativeVec3,
    pub emission: NativeVec3,
    pub edge: f32,
    pub noise_scale: f32,
    pub noise_strength: f32,
    pub noise_velocity: NativeVec3,
}

/// Removes fog volume `id`; removing one not placed does nothing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFogVolumeRemoval {
    pub id: u32,
}

/// A product image effect over each primary view's finished picture.
/// `shader` is a resource opened from a `.wgsl` file defining
/// `fn image_effect(pixel: ImagePixel) -> vec4<f32>` (handle 0 removes the
/// effect); `parameter_0` to `parameter_3` are its own values
/// (`effect_parameter(0..3)` in WGSL); `texture_a` and `texture_b` are its
/// own textures (`effect_texture_a`, `effect_texture_b`; white when 0).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImageEffectRequest {
    pub shader: crate::NativeRenderResourceReference,
    pub parameter_0: crate::NativeVec4,
    pub parameter_1: crate::NativeVec4,
    pub parameter_2: crate::NativeVec4,
    pub parameter_3: crate::NativeVec4,
    pub texture_a: crate::NativeRenderResourceReference,
    pub texture_b: crate::NativeRenderResourceReference,
}

/// What falls in the precipitation around the camera: thin streaks
/// stretched along their velocity (rain; the zero default), or round flakes
/// facing the camera (snow, ash, glitter).
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativePrecipitationShape {
    #[default]
    Streak = 0,
    Flake = 1,
}

/// Rain or snow around the camera, drawn by the Engine with no simulation:
/// `drops` (at most 200,000; 0 the default draws none) fall at `velocity`
/// (world metres per second, wind included; at most 200) through a box
/// `radius` metres to each side of the camera and `height` metres above
/// and below it (each 1 to 500), which wraps as the camera moves so the
/// drops stay put in the world. Each drop is `size` metres across (above 0,
/// at most 4); a streak is as long as it travels in `streak_seconds` (0 to
/// 1). `color` is linear radiance (0 to 16 a channel) and alpha, added to
/// the frame when `additive`, else blended over it. No drop falls where an
/// ambient light's sky layer says the sky is closed overhead.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePrecipitationRequest {
    pub drops: u32,
    pub shape: NativePrecipitationShape,
    pub velocity: NativeVec3,
    pub size: f32,
    pub streak_seconds: f32,
    pub color: crate::NativeColor,
    pub additive: bool,
    pub radius: f32,
    pub height: f32,
}

/// Which light the ambient rows give inside an indirect light volume.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeIndirectAmbient {
    /// The ambient light is the sky's: the probes see it where the world
    /// opens to it, and it reaches nothing enclosed.
    #[default]
    Sky = 0,
    /// The ambient light is a flat floor that stays everywhere; the probes
    /// add the hemisphere, bounce and emission over it.
    Floor = 1,
}

/// Indirect light: one irradiance probe volume the Engine bakes from the
/// scene's own geometry and lights, over the box `center ± extent`, with
/// probes `spacing` apart (0.5 to 8 m) and `bounces` passes of light (1 to
/// 4). An extent of zero on every axis turns it off. The bake runs on
/// worker threads and the volume follows retained changes inside the box.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeIndirectLightRequest {
    pub center: NativeVec3,
    pub extent: NativeVec3,
    pub spacing: f32,
    pub bounces: u32,
    pub ambient: NativeIndirectAmbient,
}

/// The sky's light: the background (a sky panorama, two blended, or the
/// clear colour) lights the world as an environment, prefiltered for each
/// surface's roughness, its radiance scaled by `intensity` (0 to 16). 0 (the
/// default) turns it off.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSkyLightRequest {
    pub intensity: f32,
}

/// Colour grading of the world with the tone mapping, after exposure and
/// before the operator. Each control runs from -1 to 1, and 0 (the default)
/// leaves the colour as it is: `temperature` cools toward blue or warms
/// toward yellow, `tint` shifts toward green or magenta, `contrast` scales
/// the distance from middle grey (flat at -1, doubled at 1) and
/// `saturation` the distance from grey (none at -1, doubled at 1).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeColorGradingRequest {
    pub temperature: f32,
    pub tint: f32,
    pub contrast: f32,
    pub saturation: f32,
}

/// Blends two retained equirectangular panoramas. Amount is in [0,1]; the
/// product supplies its clock-derived value. Neither texture is recreated.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSkyBackgroundBlendRequest {
    pub first: NativeRenderResourceHandle,
    pub second: NativeRenderResourceHandle,
    pub amount: f32,
}
