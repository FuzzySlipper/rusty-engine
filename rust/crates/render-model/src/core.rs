use serde::{Deserialize, Serialize};

use crate::{
    AnimatedMeshAsset, AnimatedMeshPlaybackCommand, LightDescriptor, MaterialInstanceParameters,
    MeshPayloadDescriptor, RenderMaterialDescriptor, ShaderDescriptor, SpriteAtlasDescriptor,
    SpriteInstanceDescriptor, StaticMeshAsset, StaticMeshInstanceDescriptor, TextureDescriptor,
    VoxelObjectInstanceDescriptor, VoxelObjectRenderAsset,
};

/// Authored camera-relative sky presentation.
///
/// The referenced retained texture is interpreted as one equirectangular
/// panorama. It contributes no environment lighting, collision, or picking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkyBackgroundDescriptor {
    pub texture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<SkyBackgroundBlend>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkyBackgroundBlend {
    pub texture: String,
    pub amount: f32,
}

impl SkyBackgroundDescriptor {
    pub fn validate(&self) -> Result<(), crate::RenderAssetError> {
        crate::validate_asset_id(&self.texture, crate::RenderAssetKind::Texture)?;
        if let Some(blend) = &self.blend {
            crate::validate_asset_id(&blend.texture, crate::RenderAssetKind::Texture)?;
            if !blend.amount.is_finite() || !(0.0..=1.0).contains(&blend.amount) {
                return Err(crate::RenderAssetError::InvalidSkyBlendAmount);
            }
        }
        Ok(())
    }
}

/// Distance fog over everything drawn in the world (not the background),
/// blended toward `color` after exposure and tone mapping, by distance from
/// the camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FogDescriptor {
    /// None before `start`, full at `end` and beyond.
    Linear {
        color: [f32; 3],
        start: f32,
        end: f32,
    },
    /// Remaining visibility `exp(-density × distance)`.
    Exponential { color: [f32; 3], density: f32 },
    /// Remaining visibility `exp(-(density × distance)²)`.
    ExponentialSquared { color: [f32; 3], density: f32 },
}

impl FogDescriptor {
    pub fn color(&self) -> [f32; 3] {
        match self {
            Self::Linear { color, .. }
            | Self::Exponential { color, .. }
            | Self::ExponentialSquared { color, .. } => *color,
        }
    }

    pub fn validate(&self) -> Result<(), crate::RenderOperationError> {
        let color = self.color();
        let valid = color.iter().all(|value| value.is_finite() && *value >= 0.0)
            && match *self {
                Self::Linear { start, end, .. } => {
                    start.is_finite() && end.is_finite() && start >= 0.0 && end > start
                }
                Self::Exponential { density, .. } | Self::ExponentialSquared { density, .. } => {
                    density.is_finite() && density > 0.0
                }
            };
        if valid {
            Ok(())
        } else {
            Err(crate::RenderOperationError::Fog)
        }
    }
}

/// How lit colour maps to the output: a linear exposure multiplier, then an
/// operator. The background is not tone mapped.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToneMappingDescriptor {
    pub operator: ToneMappingOperator,
    pub exposure: f32,
}

impl ToneMappingDescriptor {
    /// Exposure 1 and no operator: colour is clamped as the target encodes it.
    pub const NONE: Self = Self {
        operator: ToneMappingOperator::None,
        exposure: 1.0,
    };
}

impl Default for ToneMappingDescriptor {
    fn default() -> Self {
        Self::NONE
    }
}

/// Bloom: the world's light above `threshold` (a soft knee below it)
/// spreads into a glow added at `intensity` before exposure and tone
/// mapping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BloomDescriptor {
    pub threshold: f32,
    pub intensity: f32,
}

impl BloomDescriptor {
    /// The largest intensity a product may set.
    pub const MAX_INTENSITY: f32 = 16.0;

    /// A finite non-negative threshold and an intensity within
    /// `0..=MAX_INTENSITY`.
    pub fn valid(&self) -> bool {
        self.threshold.is_finite()
            && self.threshold >= 0.0
            && self.intensity.is_finite()
            && (0.0..=Self::MAX_INTENSITY).contains(&self.intensity)
    }
}

/// Auto exposure: the exposure scale moves toward the one that brings the
/// world's log-average luminance to middle grey, within
/// `min_exposure..=max_exposure`, closing `1 - e^(-speed·t)` of the gap in
/// `t` presentation seconds. The tone mapping exposure multiplies it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoExposureDescriptor {
    pub speed: f32,
    pub min_exposure: f32,
    pub max_exposure: f32,
}

impl AutoExposureDescriptor {
    /// A finite positive speed and a finite positive, ordered range.
    pub fn valid(&self) -> bool {
        self.speed.is_finite()
            && self.speed > 0.0
            && self.min_exposure.is_finite()
            && self.min_exposure > 0.0
            && self.max_exposure.is_finite()
            && self.max_exposure >= self.min_exposure
    }
}

/// Colour grading with the tone mapping, after exposure and before the
/// operator. Each control runs from -1 to 1, 0 leaving the colour as it is:
/// `temperature` cools (toward blue) or warms (toward yellow) the white
/// point, `tint` shifts it toward green or magenta, `contrast` scales the
/// distance from middle grey (from flat at -1 to doubled at 1), and
/// `saturation` scales the distance from grey (from none at -1 to doubled
/// at 1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorGradingDescriptor {
    pub temperature: f32,
    pub tint: f32,
    pub contrast: f32,
    pub saturation: f32,
}

/// The air: distance fog thinning with height and brightening toward the
/// sun, and the sun drawn in the sky. The sun is the brightest enabled
/// directional light of the world. Zero leaves each part out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AtmosphereDescriptor {
    /// The height (render world y) where the fog has its set density.
    pub fog_base_height: f32,
    /// The height over which the fog thins by `e`; 0 for fog of one
    /// density at every height.
    pub fog_falloff_height: f32,
    /// Linear RGB the fog turns toward when looking at the sun.
    pub haze_color: [f32; 3],
    /// How tightly the haze gathers around the sun (a power of the cosine
    /// between the view ray and the sun); 0 for no haze.
    pub haze_exponent: f32,
    /// The sun disc's angular radius in degrees; 0 for none.
    pub sun_radius_degrees: f32,
    /// The glow around the sun; 0 for none.
    pub sun_halo: f32,
}

impl AtmosphereDescriptor {
    /// The largest sun disc radius a product may set, in degrees.
    pub const MAX_SUN_RADIUS_DEGREES: f32 = 20.0;

    /// Finite values: a non-negative falloff, haze colour, exponent and
    /// halo, and a sun radius within `0..=MAX_SUN_RADIUS_DEGREES`.
    pub fn valid(&self) -> bool {
        let non_negative = |value: f32| value.is_finite() && value >= 0.0;
        self.fog_base_height.is_finite()
            && non_negative(self.fog_falloff_height)
            && self.haze_color.iter().all(|&value| non_negative(value))
            && non_negative(self.haze_exponent)
            && (0.0..=Self::MAX_SUN_RADIUS_DEGREES).contains(&self.sun_radius_degrees)
            && non_negative(self.sun_halo)
    }
}

/// Sun shafts: the sky around the sun, blurred along rays from it, so light
/// streams past whatever stands in front of it. `intensity` scales the
/// light added (0 to 16); `length` is how far the rays reach from the sun,
/// as a fraction of the way to each pixel (0 takes the Engine's default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SunShaftsDescriptor {
    pub intensity: f32,
    pub length: f32,
}

impl SunShaftsDescriptor {
    /// The largest intensity a product may set.
    pub const MAX_INTENSITY: f32 = 16.0;

    /// An intensity within `0..=MAX_INTENSITY` and a length within `0..=1`.
    pub fn valid(&self) -> bool {
        (0.0..=Self::MAX_INTENSITY).contains(&self.intensity) && (0.0..=1.0).contains(&self.length)
    }
}

/// The scene's wind, which materials with the wind feature
/// (`RenderMaterialDescriptor::wind`) and product displace stages sway in.
/// `direction` is over the ground (world x, z; any length, normalized when
/// read); `strength` (0 to 16) scales every material's bend and flutter;
/// `gust` (0 to 1) is the share of the lean that rises and falls in gusts
/// rather than holding steady.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindDescriptor {
    pub direction: [f32; 2],
    pub strength: f32,
    pub gust: f32,
}

impl WindDescriptor {
    /// The largest strength a product may set.
    pub const MAX_STRENGTH: f32 = 16.0;

    /// A finite direction with some length, a strength within
    /// `0..=MAX_STRENGTH` and a gust share within `0..=1`.
    pub fn valid(&self) -> bool {
        let [x, z] = self.direction;
        x.is_finite()
            && z.is_finite()
            && (x * x + z * z) > 0.0
            && (0.0..=Self::MAX_STRENGTH).contains(&self.strength)
            && (0.0..=1.0).contains(&self.gust)
    }

    /// The direction at unit length.
    pub fn unit_direction(&self) -> [f32; 2] {
        let [x, z] = self.direction;
        let length = (x * x + z * z).sqrt().max(f32::MIN_POSITIVE);
        [x / length, z / length]
    }
}

/// How wet the scene's surfaces are, after rain (`CameraView.SetWetness`):
/// `wetness` (0 to 1) darkens the diffuse colour and lowers roughness of lit
/// surfaces under the open sky, most on up-facing ones; `puddles` (0 to 1)
/// gathers standing water in patches on flat ground as the surfaces wet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WetnessDescriptor {
    pub wetness: f32,
    pub puddles: f32,
}

/// The backdrop's link to the world (`CameraView.SetBackdrop`): parts in
/// the `Backdrop` layer are drawn behind the world, after the sky and clouds,
/// by a camera with each world view's rotation and field of view at
/// `origin + (eye - anchor) / scale` in backdrop space. `anchor` is a point
/// of the world, in the same local frame as the cameras (it moves with them
/// on an origin rebase); `origin` is where it lies in the backdrop; `scale` is
/// world metres per backdrop unit (1: the backdrop is at world scale, 1000: a
/// 1:1000 miniature). Fog and the cloud layer's shade reach the backdrop at
/// its world-equivalent distance and place.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackdropDescriptor {
    pub anchor: [f64; 3],
    pub origin: [f64; 3],
    pub scale: f64,
}

impl BackdropDescriptor {
    /// Finite points and a scale above 0.
    pub fn valid(&self) -> bool {
        self.anchor
            .iter()
            .chain(&self.origin)
            .all(|value| value.is_finite())
            && self.scale.is_finite()
            && self.scale > 0.0
    }

    /// Where a world eye stands in the backdrop.
    pub fn eye(&self, world: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + (world[axis] - self.anchor[axis]) / self.scale
        })
    }
}

impl WetnessDescriptor {
    /// Wetness and puddles within `0..=1`.
    pub fn valid(&self) -> bool {
        (0.0..=1.0).contains(&self.wetness) && (0.0..=1.0).contains(&self.puddles)
    }
}

/// A cloud region (`CameraView.SetCloudRegion`): where the sky holds more
/// cloud than the layer's coverage, such as a weather front's storm. Its
/// coverage (0 to 1) fills a disc of `radius` metres around `center` (world
/// x, z), fading over its outer third, drifting at `drift` metres per second;
/// `darkness` (0 to 1) darkens its clouds' undersides, for a storm. The cloud
/// layer and its shade on the ground take the most cloud of the layer and
/// every region, flat or volumetric.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudRegionDescriptor {
    pub center: [f32; 2],
    pub radius: f32,
    pub coverage: f32,
    pub darkness: f32,
    pub drift: [f32; 2],
    /// The clouds it holds (`CloudKind`), in place of the layer's within it.
    #[serde(default)]
    pub kind: CloudKind,
    /// How tall its volumetric clouds stand from the layer's altitude, in
    /// metres (0: the layer's thickness): a towering storm front beside a
    /// thin overcast at the same base.
    #[serde(default)]
    pub thickness: f32,
}

impl CloudRegionDescriptor {
    /// The most cloud regions a scene holds.
    pub const MAX_REGIONS: usize = 32;

    pub fn valid(&self) -> bool {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        finite(&self.center)
            && finite(&self.drift)
            && self.radius > 0.0
            && self.radius <= 1_000_000.0
            && (0.0..=1.0).contains(&self.coverage)
            && (0.0..=1.0).contains(&self.darkness)
            && (0.0..=CloudsDescriptor::MAX_DISTANCE).contains(&self.thickness)
    }
}

/// The participating medium volumetric fog lights (`CameraView.SetVolumetricFog`):
/// how much fog there is everywhere, what colour it scatters and which way,
/// and how far the fog grid reaches. Fog volumes (`FogVolumeDescriptor`) add
/// to it where they stand. It draws only while the renderer's
/// `volumetric_fog` setting is on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VolumetricFogDescriptor {
    /// Extinction per metre at `base_height` (0 to 1): 0 fills no air, so
    /// only fog volumes draw.
    pub density: f32,
    /// The colour the fog scatters, linear (0 to 1 a channel).
    pub albedo: [f32; 3],
    /// Henyey–Greenstein anisotropy (-0.9 to 0.9): positive scatters forward,
    /// so fog glows looking toward a light and shafts read against it.
    pub anisotropy: f32,
    /// The height `density` holds at, in world units.
    pub base_height: f32,
    /// Density falls by e every this many metres up; 0 is one density
    /// everywhere.
    pub falloff_height: f32,
    /// How far from the camera the fog grid reaches, metres (8 to 1000).
    pub distance: f32,
    /// How much of the ambient and sky light the fog scatters (0 to 4).
    pub ambient: f32,
}

impl VolumetricFogDescriptor {
    /// No medium, volumes only, reaching 96 m.
    pub const DEFAULT: Self = Self {
        density: 0.0,
        albedo: [1.0, 1.0, 1.0],
        anisotropy: 0.4,
        base_height: 0.0,
        falloff_height: 0.0,
        distance: 96.0,
        ambient: 1.0,
    };

    pub fn valid(&self) -> bool {
        let finite = |value: f32| value.is_finite();
        (0.0..=1.0).contains(&self.density)
            && self
                .albedo
                .iter()
                .all(|channel| (0.0..=1.0).contains(channel))
            && (-0.9..=0.9).contains(&self.anisotropy)
            && finite(self.base_height)
            && finite(self.falloff_height)
            && self.falloff_height >= 0.0
            && (8.0..=1000.0).contains(&self.distance)
            && (0.0..=4.0).contains(&self.ambient)
    }
}

impl Default for VolumetricFogDescriptor {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The shape of a fog volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FogVolumeShape {
    Box,
    Ellipsoid,
}

/// A fog volume (`CameraView.SetFogVolume`): a box or ellipsoid of fog the
/// volumetric fog adds to its medium, for a valley bank, a dust wall or a
/// cave's mist. Positions are in the renderer's world space, as an indirect
/// light volume's. Its density fades to nothing over `edge` of its half
/// extent inward from its boundary, and breaks up with 3D noise drifting at
/// `noise_velocity`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FogVolumeDescriptor {
    pub shape: FogVolumeShape,
    pub center: [f32; 3],
    /// Half the size along each axis, metres (above 0, at most 10 km).
    pub half_extents: [f32; 3],
    /// Turn about the vertical axis, degrees.
    pub yaw_degrees: f32,
    /// Extinction per metre at its heart (0 to 4).
    pub density: f32,
    /// The colour it scatters, linear (0 to 1 a channel).
    pub albedo: [f32; 3],
    /// Light it gives off per unit of density, linear (0 to 16 a channel):
    /// a glowing storm or an arcane haze.
    pub emission: [f32; 3],
    /// The share of the half extent over which the edge fades (0 to 1).
    pub edge: f32,
    /// The size of one noise cell, metres (0: no noise).
    pub noise_scale: f32,
    /// How much the noise thins it (0 to 1).
    pub noise_strength: f32,
    /// The noise's drift, metres per second.
    pub noise_velocity: [f32; 3],
}

impl FogVolumeDescriptor {
    /// The most fog volumes a scene holds.
    pub const MAX_VOLUMES: usize = 64;

    pub fn valid(&self) -> bool {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        finite(&self.center)
            && finite(&self.noise_velocity)
            && self.yaw_degrees.is_finite()
            && self
                .half_extents
                .iter()
                .all(|half| *half > 0.0 && *half <= 10_000.0)
            && (0.0..=4.0).contains(&self.density)
            && self
                .albedo
                .iter()
                .all(|channel| (0.0..=1.0).contains(channel))
            && self
                .emission
                .iter()
                .all(|channel| (0.0..=16.0).contains(channel))
            && (0.0..=1.0).contains(&self.edge)
            && self.noise_scale.is_finite()
            && self.noise_scale >= 0.0
            && (0.0..=1.0).contains(&self.noise_strength)
    }
}

/// What falls in a precipitation volume (`PrecipitationDescriptor`): thin
/// streaks stretched along their velocity (rain), or round flakes facing
/// the camera (snow, ash, glitter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrecipitationShape {
    Streak,
    Flake,
}

/// Precipitation around the camera: `drops` (at most `MAX_DROPS`; 0 draws
/// none) fall at `velocity` (world metres per second, wind included)
/// through a box `radius` metres to each side of the camera and `height`
/// metres above and below it, which wraps as the camera moves so the drops
/// stay put in the world. Each is `size` metres across; a streak is as
/// long as it travels in `streak_seconds`. `color` is linear radiance (0
/// to 16 a channel) and alpha (0 to 1), added to the frame when
/// `additive`, else blended over it. No drop falls where an ambient light's
/// sky layer says the sky is closed overhead.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrecipitationDescriptor {
    pub drops: u32,
    pub shape: PrecipitationShape,
    pub velocity: [f32; 3],
    pub size: f32,
    pub streak_seconds: f32,
    pub color: [f32; 4],
    pub additive: bool,
    pub radius: f32,
    pub height: f32,
}

impl PrecipitationDescriptor {
    /// The most drops one volume draws.
    pub const MAX_DROPS: u32 = 200_000;
    /// The fastest fall, metres per second.
    pub const MAX_SPEED: f32 = 200.0;
    /// The largest drop, metres across.
    pub const MAX_SIZE: f32 = 4.0;
    /// The longest streak, in seconds of travel.
    pub const MAX_STREAK_SECONDS: f32 = 1.0;
    /// The widest and tallest volume: half its side and half its height, in
    /// metres.
    pub const MAX_EXTENT: f32 = 500.0;
    /// The largest colour channel.
    pub const MAX_COLOR: f32 = 16.0;

    /// Drops within `MAX_DROPS`; a finite velocity no faster than
    /// `MAX_SPEED`; a size above 0 and within `MAX_SIZE`; a streak time
    /// within `0..=MAX_STREAK_SECONDS`; colour channels within
    /// `0..=MAX_COLOR` and alpha within `0..=1`; and a radius and height of
    /// at least 1 m and within `MAX_EXTENT`.
    pub fn valid(&self) -> bool {
        let [x, y, z] = self.velocity;
        let extent = 1.0..=Self::MAX_EXTENT;
        self.drops <= Self::MAX_DROPS
            && x.is_finite()
            && y.is_finite()
            && z.is_finite()
            && (x * x + y * y + z * z).sqrt() <= Self::MAX_SPEED
            && self.size > 0.0
            && self.size <= Self::MAX_SIZE
            && (0.0..=Self::MAX_STREAK_SECONDS).contains(&self.streak_seconds)
            && self.color[..3]
                .iter()
                .all(|channel| (0.0..=Self::MAX_COLOR).contains(channel))
            && (0.0..=1.0).contains(&self.color[3])
            && extent.contains(&self.radius)
            && extent.contains(&self.height)
    }
}

/// A product image effect over each primary view's finished picture: a
/// defined `ShaderDescriptor` whose WGSL defines `fn image_effect`, its own
/// values (`effect_parameter(0..3)` in WGSL) and up to two of its own
/// textures (`effect_texture_a`, `effect_texture_b`; white when absent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageEffectDescriptor {
    pub shader: String,
    pub parameters: [[f32; 4]; 4],
    #[serde(default)]
    pub textures: [Option<String>; 2],
}

impl ImageEffectDescriptor {
    /// A named shader and finite parameters.
    pub fn valid(&self) -> bool {
        !self.shader.is_empty()
            && self
                .parameters
                .iter()
                .flatten()
                .all(|value| value.is_finite())
    }
}

/// What clouds a layer or a region holds, which shapes the volumetric
/// clouds by height (`clouds.wgsl` `cloud_profile`): a thin flat sheet, heaped
/// clouds with flat bottoms, or towering storm cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudKind {
    Stratus,
    #[default]
    Cumulus,
    Cumulonimbus,
}

impl CloudKind {
    /// Where the kind sits on the profile the shaders blend: stratus 0,
    /// cumulus 1, cumulonimbus 2.
    pub fn profile(self) -> f32 {
        match self {
            Self::Stratus => 0.0,
            Self::Cumulus => 1.0,
            Self::Cumulonimbus => 2.0,
        }
    }
}

/// A cloud layer drawn over the sky panorama and lit by the sun. `coverage`
/// (0 to 1) is how much of the sky it covers; `drift` is its velocity over
/// the ground (world x, z) in metres per second; `altitude` is the height
/// of the layer and `scale` the size of one cloud, both in metres, which
/// together set how large clouds look and how they shrink toward the
/// horizon; `color` (linear, 0 to 16 a channel) tints the light they take.
/// The volumetric clouds stand from `altitude` up by `thickness` metres (0:
/// six tenths of the altitude), shaped by `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudsDescriptor {
    pub coverage: f32,
    pub drift: [f32; 2],
    pub altitude: f32,
    pub scale: f32,
    pub color: [f32; 3],
    #[serde(default)]
    pub thickness: f32,
    #[serde(default)]
    pub kind: CloudKind,
}

impl CloudsDescriptor {
    /// The largest altitude and cloud size, in metres.
    pub const MAX_DISTANCE: f32 = 100_000.0;
    /// The fastest drift, in metres per second.
    pub const MAX_DRIFT: f32 = 1_000.0;
    /// The largest colour channel.
    pub const MAX_COLOR: f32 = 16.0;

    /// Coverage within `0..=1`, a finite drift no faster than `MAX_DRIFT`, an
    /// altitude and scale above 0 and within `MAX_DISTANCE`, and colour
    /// channels within `0..=MAX_COLOR`.
    pub fn valid(&self) -> bool {
        let [x, z] = self.drift;
        let distance = 0.0..=Self::MAX_DISTANCE;
        (0.0..=1.0).contains(&self.coverage)
            && x.is_finite()
            && z.is_finite()
            && (x * x + z * z).sqrt() <= Self::MAX_DRIFT
            && self.altitude > 0.0
            && distance.contains(&self.altitude)
            && self.scale > 0.0
            && distance.contains(&self.scale)
            && distance.contains(&self.thickness)
            && self
                .color
                .iter()
                .all(|channel| (0.0..=Self::MAX_COLOR).contains(channel))
    }
}

/// Which light the ambient rows give inside an indirect light volume.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndirectAmbient {
    /// The ambient light is the sky's: the volume's probes see it where the
    /// world opens to it and it reaches nothing enclosed.
    #[default]
    Sky,
    /// The ambient light is a flat floor that stays everywhere; the probes
    /// add the hemisphere, bounce and emission over it.
    Floor,
}

/// Indirect light: one irradiance probe volume the Engine bakes from the
/// scene's own geometry and lights and the standard shader samples as the
/// ambient term where it covers a fragment (render-wgpu `probes.rs`). The
/// box is `center ± extent`; probes sit `spacing` apart; `bounces` passes of
/// light (1 sees direct-lit surfaces and the sky, each further pass one more
/// bounce).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndirectLightDescriptor {
    pub center: [f32; 3],
    /// Half the box's side on each axis.
    pub extent: [f32; 3],
    pub spacing: f32,
    pub bounces: u32,
    #[serde(default)]
    pub ambient: IndirectAmbient,
}

impl IndirectLightDescriptor {
    pub const MIN_SPACING: f32 = 0.5;
    pub const MAX_SPACING: f32 = 8.0;
    pub const MAX_BOUNCES: u32 = 4;
    /// The most probes one volume may hold (64³).
    pub const MAX_PROBES: u64 = 262_144;
    /// The most probes along one axis: the three colour channels stack along
    /// the volume texture's depth and stay within any adapter's 3D texture size.
    pub const MAX_AXIS: u64 = 512;

    /// Probes per axis for the box and spacing.
    pub fn dims(&self) -> [u64; 3] {
        [0, 1, 2].map(|axis| ((2.0 * self.extent[axis] / self.spacing).floor() as u64 + 1).max(2))
    }

    /// Finite values, a non-negative extent, a spacing within
    /// `MIN_SPACING..=MAX_SPACING`, bounces within `1..=MAX_BOUNCES` and at
    /// most `MAX_PROBES` probes.
    pub fn valid(&self) -> bool {
        let finite = |values: &[f32; 3]| values.iter().all(|value| value.is_finite());
        finite(&self.center)
            && finite(&self.extent)
            && self.extent.iter().all(|value| *value >= 0.0)
            && self.spacing.is_finite()
            && (Self::MIN_SPACING..=Self::MAX_SPACING).contains(&self.spacing)
            && (1..=Self::MAX_BOUNCES).contains(&self.bounces)
            && self.dims().iter().product::<u64>() <= Self::MAX_PROBES
            && self.dims().iter().all(|dim| *dim <= Self::MAX_AXIS)
    }
}

/// The sky's light: the background (a sky panorama, two blended, or the
/// clear colour) lights the world as an environment, its radiance scaled by
/// `intensity` (0 to 16).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkyLightDescriptor {
    pub intensity: f32,
}

impl SkyLightDescriptor {
    /// The largest intensity a product may set.
    pub const MAX_INTENSITY: f32 = 16.0;

    /// An intensity within `0..=MAX_INTENSITY`.
    pub fn valid(&self) -> bool {
        (0.0..=Self::MAX_INTENSITY).contains(&self.intensity)
    }
}

impl ColorGradingDescriptor {
    /// Every control finite and within -1 to 1.
    pub fn valid(&self) -> bool {
        [self.temperature, self.tint, self.contrast, self.saturation]
            .iter()
            .all(|value| (-1.0..=1.0).contains(value))
    }
}

/// Which pass darkens where surfaces meet (`RendererSettingsDescriptor`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AmbientOcclusionMode {
    #[default]
    Disabled,
    /// From the view's depth: what the view shows occludes.
    ScreenSpace,
    /// From the voxel chunks' distance fields: the world around a surface
    /// occludes, on screen or off.
    DistanceField,
}

/// How finely volumetric fog is computed (`RendererSettingsDescriptor`):
/// off (only the analytic distance fog), or a froxel grid lit by the scene's
/// lights and shadows at low or high resolution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumetricFogQuality {
    #[default]
    Off,
    Low,
    High,
}

/// How the sky's clouds are drawn (`RendererSettingsDescriptor`): the flat
/// layer, or raymarched clouds with thickness at low or high quality.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumetricCloudsQuality {
    #[default]
    Off,
    Low,
    High,
}

/// Ambient occlusion as a product selects it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AmbientOcclusionSettings {
    pub mode: AmbientOcclusionMode,
    /// 0 draws without occlusion; 1 is the full occlusion.
    pub strength: f32,
    /// How far a surface darkens its neighbours, in world units.
    pub radius: f32,
}

impl AmbientOcclusionSettings {
    pub const DEFAULT: Self = Self {
        mode: AmbientOcclusionMode::Disabled,
        strength: 1.0,
        radius: 0.75,
    };

    /// A finite non-negative strength and a finite positive radius.
    pub fn valid(&self) -> bool {
        self.strength.is_finite()
            && self.strength >= 0.0
            && self.radius.is_finite()
            && self.radius > 0.0
    }
}

impl Default for AmbientOcclusionSettings {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Renderer-wide settings a product selects: which pipeline features draw
/// and at what quality. The product manifest supplies the initial values
/// and the `RendererSettings` service changes them at runtime; the renderer
/// realizes each where the device can and reports what it refused.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererSettingsDescriptor {
    /// Render shadow maps for lights whose shadow intent requests them.
    pub shadows: bool,
    /// At most this many shadow layers at once, the requesting lights
    /// chosen by priority then distance from the camera; `None` for no
    /// limit.
    pub shadow_budget: Option<u32>,
    pub ambient_occlusion: AmbientOcclusionSettings,
    /// Samples per pixel of the primary destination: 1, 2 or 4.
    pub antialiasing: u32,
    /// The fraction of the primary destination's size the world, viewmodel,
    /// labels and effects draw at before being upscaled into it: 0.5 to 1.
    pub render_scale: f32,
    /// Window output waits for the display's refresh before presenting.
    pub vsync: bool,
    /// Bin each world view's lights into view-frustum clusters before
    /// shading, instead of shading every light per fragment.
    pub clustered_lighting: bool,
    /// Test each view's opaque parts against its frustum on the GPU and
    /// draw them indirectly, instead of building the draw list on the CPU.
    pub gpu_culling: bool,
    /// Light the fog medium and fog volumes in a froxel grid: shafts,
    /// glows and banks the analytic fog cannot draw. Settings written
    /// before it existed have it off.
    #[serde(default)]
    pub volumetric_fog: VolumetricFogQuality,
    /// Draw the cloud layer as raymarched clouds with thickness, lit
    /// through themselves, instead of a flat sheet. Settings written before
    /// it existed have it off.
    #[serde(default)]
    pub volumetric_clouds: VolumetricCloudsQuality,
}

impl RendererSettingsDescriptor {
    /// The sample counts a primary destination may have.
    pub const SAMPLE_COUNTS: [u32; 3] = [1, 2, 4];
    /// The smallest render scale.
    pub const MIN_RENDER_SCALE: f32 = 0.5;

    pub const DEFAULT: Self = Self {
        shadows: false,
        shadow_budget: None,
        ambient_occlusion: AmbientOcclusionSettings::DEFAULT,
        antialiasing: 4,
        render_scale: 1.0,
        vsync: true,
        clustered_lighting: false,
        gpu_culling: false,
        volumetric_fog: VolumetricFogQuality::Off,
        volumetric_clouds: VolumetricCloudsQuality::Off,
    };

    /// Valid occlusion values, a supported sample count and a render scale
    /// within range.
    pub fn valid(&self) -> bool {
        self.ambient_occlusion.valid()
            && Self::SAMPLE_COUNTS.contains(&self.antialiasing)
            && self.render_scale.is_finite()
            && (Self::MIN_RENDER_SCALE..=1.0).contains(&self.render_scale)
    }
}

impl Default for RendererSettingsDescriptor {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToneMappingOperator {
    /// Clamp to the target's range.
    #[default]
    None,
    /// Khronos PBR Neutral: base colours stay true, highlights compress.
    Neutral,
    /// ACES filmic (Stephen Hill's fit): film-like contrast and roll-off.
    AcesFilmic,
}

pub const JSON_SAFE_U64_MAX: u64 = (1_u64 << 53) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RenderHandle(u64);

impl RenderHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn validate(self) -> Result<(), RenderHandleError> {
        if self.0 <= JSON_SAFE_U64_MAX {
            Ok(())
        } else {
            Err(RenderHandleError::OutsideJsonSafeRange(self.0))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderHandleError {
    OutsideJsonSafeRange(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transform {
    pub translation: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    };

    pub fn validate(self) -> Result<(), TransformError> {
        if !self.translation.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidTranslation);
        }
        if !self.rotation.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidRotation);
        }
        let rotation_length = self.rotation.iter().map(|value| value * value).sum::<f32>();
        if rotation_length <= f32::EPSILON {
            return Err(TransformError::InvalidRotation);
        }
        if !self.scale.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidScale);
        }
        Ok(())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    InvalidTranslation,
    InvalidRotation,
    InvalidScale,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Geometry {
    /// Transform-only hierarchy node with no drawable geometry.
    Group,
    Cube,
    Sphere,
    Quad,
    Point,
    Line {
        a: [f32; 3],
        b: [f32; 3],
    },
}

impl Geometry {
    fn validate(self) -> Result<(), NodeError> {
        match self {
            Self::Line { a, b }
                if !a
                    .into_iter()
                    .chain(b)
                    .all(|component| component.is_finite()) =>
            {
                Err(NodeError::InvalidGeometry)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Material {
    pub color: [f32; 4],
    pub wireframe: bool,
}

impl Material {
    pub const DEFAULT: Self = Self {
        color: [1.0, 1.0, 1.0, 1.0],
        wireframe: false,
    };

    fn validate(self) -> Result<(), NodeError> {
        self.color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .then_some(())
            .ok_or(NodeError::InvalidMaterial)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderLayer {
    #[default]
    Scene,
    Debug,
    Ui,
    Viewmodel,
    /// Drawn behind the world by the backdrop camera (`SetBackdrop`), in
    /// backdrop coordinates: no shadows, probes or world lights but the
    /// directional, ambient and hemisphere ones.
    Backdrop,
}

impl RenderLayer {
    pub fn is_scene(&self) -> bool {
        *self == Self::Scene
    }
}

/// Whether a part casts shadows. Receiving is unchanged; `None` keeps a
/// water plane, glass or a fog card out of every shadow layer and saves the
/// caster cost of small clutter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShadowCasting {
    #[default]
    Cast,
    None,
}

impl ShadowCasting {
    pub fn is_cast(&self) -> bool {
        *self == Self::Cast
    }
}

/// Authority provenance remains raw identity data at this border. The renderer
/// may report it in a pick, but cannot turn it into gameplay authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderMetadata {
    pub source_entity: Option<u64>,
    pub source_scene_node: Option<u64>,
    pub tags: Vec<String>,
    pub label: Option<String>,
}

impl RenderMetadata {
    pub fn validate(&self) -> Result<(), NodeError> {
        if self
            .source_entity
            .into_iter()
            .chain(self.source_scene_node)
            .any(|value| value > JSON_SAFE_U64_MAX)
        {
            return Err(NodeError::UnsafeSourceIdentity);
        }
        if self.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(NodeError::EmptyTag);
        }
        if self.tags.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(NodeError::TagsNotCanonical);
        }
        if self
            .label
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(NodeError::EmptyLabel);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderNode {
    pub geometry: Geometry,
    pub material: Material,
    pub transform: Transform,
    pub visible: bool,
    pub layer: RenderLayer,
    /// Whether the node's parts cast shadows (`Cast` by default).
    #[serde(default, skip_serializing_if = "ShadowCasting::is_cast")]
    pub shadow_casting: ShadowCasting,
    pub metadata: RenderMetadata,
}

impl RenderNode {
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            material: Material::DEFAULT,
            transform: Transform::IDENTITY,
            visible: true,
            layer: RenderLayer::Scene,
            shadow_casting: ShadowCasting::Cast,
            metadata: RenderMetadata::default(),
        }
    }

    pub fn validate(&self) -> Result<(), NodeError> {
        self.geometry.validate()?;
        self.material.validate()?;
        self.transform.validate().map_err(NodeError::Transform)?;
        self.metadata.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeError {
    InvalidGeometry,
    InvalidMaterial,
    Transform(TransformError),
    EmptyTag,
    TagsNotCanonical,
    EmptyLabel,
    UnsafeSourceIdentity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
// A retained op is as large as a material definition; the ops are applied
// and retained, not kept in bulk, so the size is not worth an indirection.
#[allow(clippy::large_enum_variant)]
pub enum RenderDiff {
    /// Select a named joint within this node's retained animated-mesh parent.
    /// Local TRS remains relative to that joint; None restores the parent root.
    SetParentJoint {
        handle: RenderHandle,
        joint: Option<String>,
    },
    Create {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        node: RenderNode,
    },
    Update {
        handle: RenderHandle,
        transform: Option<Transform>,
        material: Option<Material>,
        visible: Option<bool>,
        metadata: Option<RenderMetadata>,
    },
    Destroy {
        handle: RenderHandle,
    },
    ReplaceMeshPayload {
        handle: RenderHandle,
        payload: MeshPayloadDescriptor,
    },
    /// Replace only the distance field of a primitive's replaced payload: a
    /// voxel chunk whose neighbour changed within the field's reach keeps
    /// its mesh.
    /// Replaces a payload mesh's distance field alone; `None` drops it (the
    /// scene stopped building fields).
    ReplaceMeshDistanceField {
        handle: RenderHandle,
        field: Option<crate::MeshDistanceField>,
    },
    CreateLight {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        light: LightDescriptor,
    },
    UpdateLight {
        handle: RenderHandle,
        light: LightDescriptor,
    },
    DefineMaterial {
        material: RenderMaterialDescriptor,
    },
    ReleaseMaterial {
        id: String,
    },
    SetMaterialInstanceParameters {
        handle: RenderHandle,
        slot: u16,
        parameters: Option<MaterialInstanceParameters>,
    },
    DefineTexture {
        texture: TextureDescriptor,
    },
    ReleaseTexture {
        id: String,
    },
    DefineShader {
        shader: ShaderDescriptor,
    },
    ReleaseShader {
        id: String,
    },
    SetSkyBackground {
        background: Option<SkyBackgroundDescriptor>,
    },
    /// Selects one opaque retained viewport clear color and clears any selected sky.
    SetBackgroundColor {
        color: [f32; 4],
    },
    /// Selects the world's distance fog; None turns it off.
    SetFog {
        fog: Option<FogDescriptor>,
    },
    SetToneMapping {
        tone_mapping: ToneMappingDescriptor,
    },
    /// Selects the world's bloom; None turns it off.
    SetBloom {
        bloom: Option<BloomDescriptor>,
    },
    /// Selects auto exposure; None turns it off.
    SetAutoExposure {
        auto_exposure: Option<AutoExposureDescriptor>,
    },
    /// Selects the world's colour grading; None turns it off.
    SetColorGrading {
        color_grading: Option<ColorGradingDescriptor>,
    },
    /// Selects the atmosphere: height fog, sun haze and the sun in the sky;
    /// None turns it off.
    SetAtmosphere {
        atmosphere: Option<AtmosphereDescriptor>,
    },
    /// Selects sun shafts; None turns them off.
    SetSunShafts {
        sun_shafts: Option<SunShaftsDescriptor>,
    },
    /// Selects the scene's wind; None stills it.
    SetWind {
        wind: Option<WindDescriptor>,
    },
    /// Selects the sky's cloud layer; None clears the sky of clouds.
    SetClouds {
        clouds: Option<CloudsDescriptor>,
    },
    /// Selects how wet the scene's surfaces are; None dries them.
    SetWetness {
        wetness: Option<WetnessDescriptor>,
    },
    /// Links the `Backdrop` layer to the world views of camera `camera`, or
    /// with None to every view without a link of its own; a None link
    /// removes it.
    SetBackdrop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<String>,
        backdrop: Option<BackdropDescriptor>,
    },
    /// Places or replaces cloud region `id`.
    SetCloudRegion {
        id: u32,
        region: CloudRegionDescriptor,
    },
    /// Removes cloud region `id`.
    RemoveCloudRegion {
        id: u32,
    },
    /// Selects the volumetric fog's medium.
    SetVolumetricFog {
        fog: VolumetricFogDescriptor,
    },
    /// Places or replaces fog volume `id`.
    SetFogVolume {
        id: u32,
        volume: FogVolumeDescriptor,
    },
    /// Removes fog volume `id`.
    RemoveFogVolume {
        id: u32,
    },
    /// Selects the precipitation around the camera; None stops it.
    SetPrecipitation {
        precipitation: Option<PrecipitationDescriptor>,
    },
    /// Selects the image effect over each primary view's finished picture;
    /// None removes it.
    SetImageEffect {
        effect: Option<ImageEffectDescriptor>,
    },
    /// The indirect light volume; `None` turns it off.
    SetIndirectLight {
        indirect_light: Option<IndirectLightDescriptor>,
    },
    /// Selects the sky's light; None turns it off.
    SetSkyLight {
        sky_light: Option<SkyLightDescriptor>,
    },
    /// Selects the renderer's settings: its pipeline features and quality.
    SetRendererSettings {
        settings: RendererSettingsDescriptor,
    },
    DefineSpriteAtlas {
        atlas: SpriteAtlasDescriptor,
    },
    ReleaseSpriteAtlas {
        id: String,
    },
    DefineStaticMesh {
        asset: StaticMeshAsset,
    },
    ReleaseStaticMesh {
        asset: String,
    },
    DefineAnimatedMesh {
        asset: AnimatedMeshAsset,
    },
    ReleaseAnimatedMesh {
        asset: String,
    },
    DefineVoxelObject {
        asset: VoxelObjectRenderAsset,
    },
    ReleaseVoxelObject {
        asset: String,
    },
    CreateStaticMeshInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: StaticMeshInstanceDescriptor,
    },
    CreateAnimatedMeshInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: crate::AnimatedMeshInstanceDescriptor,
    },
    /// Many copies of a static mesh drawn as one node (`scatter`): grass,
    /// stones and flowers the Engine placed over a piece of ground.
    CreateScatterPatch {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        patch: crate::ScatterPatchDescriptor,
    },
    SetAnimatedMeshInspection {
        handle: RenderHandle,
        inspection: crate::AnimatedMeshInspection,
    },
    SetAnimatedMeshPlayback {
        handle: RenderHandle,
        playback: AnimatedMeshPlaybackCommand,
    },
    /// Replaces the instance's pose controls (`AnimatedMeshPose`); the
    /// default value clears them.
    SetAnimatedMeshPose {
        handle: RenderHandle,
        pose: crate::AnimatedMeshPose,
    },
    CreateVoxelObjectInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: VoxelObjectInstanceDescriptor,
    },
    SetVoxelObjectFrame {
        handle: RenderHandle,
        frame: u32,
    },
    CreateSprite {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        sprite: SpriteInstanceDescriptor,
    },
    UpdateSprite {
        handle: RenderHandle,
        frame: Option<u32>,
        tint: Option<[f32; 4]>,
        render_order: Option<i32>,
        visible: Option<bool>,
    },
}

impl RenderDiff {
    pub fn validate(&self) -> Result<(), RenderOperationError> {
        self.validate_handles()
            .map_err(RenderOperationError::Handle)?;
        match self {
            Self::Create { node, .. } => node.validate().map_err(RenderOperationError::Node),
            Self::Update {
                transform,
                material,
                metadata,
                ..
            } => {
                if let Some(value) = transform {
                    value.validate().map_err(RenderOperationError::Transform)?;
                }
                if let Some(value) = material {
                    value.validate().map_err(RenderOperationError::Node)?;
                }
                if let Some(value) = metadata {
                    value.validate().map_err(RenderOperationError::Node)?;
                }
                Ok(())
            }
            Self::SetParentJoint { joint, .. } => {
                if joint.as_ref().is_some_and(|name| name.trim().is_empty()) {
                    Err(RenderOperationError::InvalidParentJoint)
                } else {
                    Ok(())
                }
            }
            Self::Destroy { .. }
            | Self::SetMaterialInstanceParameters {
                parameters: None, ..
            } => Ok(()),
            Self::ReplaceMeshPayload { payload, .. } => {
                payload.validate().map_err(RenderOperationError::Mesh)
            }
            Self::ReplaceMeshDistanceField {
                field: Some(field), ..
            } => field.validate().map_err(RenderOperationError::Mesh),
            Self::ReplaceMeshDistanceField { field: None, .. } => Ok(()),
            Self::CreateLight { light, .. } | Self::UpdateLight { light, .. } => {
                light.validate().map_err(RenderOperationError::Light)
            }
            Self::DefineMaterial { material } => material
                .validate()
                .map_err(RenderOperationError::MaterialDescriptor),
            Self::SetMaterialInstanceParameters {
                parameters: Some(parameters),
                ..
            } => parameters
                .validate()
                .map_err(RenderOperationError::MaterialParameters),
            Self::DefineTexture { texture } => {
                texture.validate().map_err(RenderOperationError::Texture)
            }
            Self::DefineShader { shader } => shader.validate().map_err(RenderOperationError::Asset),
            Self::SetSkyBackground {
                background: Some(background),
            } => background
                .validate()
                .map_err(RenderOperationError::SkyBackground),
            Self::SetSkyBackground { background: None } => Ok(()),
            Self::SetBackgroundColor { color } if valid_color(*color) && color[3] == 1.0 => Ok(()),
            Self::SetBackgroundColor { .. } => Err(RenderOperationError::BackgroundColor),
            Self::SetFog { fog: Some(fog) } => fog.validate(),
            Self::SetFog { fog: None } => Ok(()),
            Self::SetToneMapping { tone_mapping }
                if tone_mapping.exposure.is_finite() && tone_mapping.exposure >= 0.0 =>
            {
                Ok(())
            }
            Self::SetToneMapping { .. } => Err(RenderOperationError::ToneMapping),
            Self::SetBloom { bloom: Some(bloom) } if !bloom.valid() => {
                Err(RenderOperationError::Bloom)
            }
            Self::SetBloom { .. } => Ok(()),
            Self::SetAutoExposure {
                auto_exposure: Some(auto_exposure),
            } if !auto_exposure.valid() => Err(RenderOperationError::AutoExposure),
            Self::SetAutoExposure { .. } => Ok(()),
            Self::SetColorGrading {
                color_grading: Some(color_grading),
            } if !color_grading.valid() => Err(RenderOperationError::ColorGrading),
            Self::SetColorGrading { .. } => Ok(()),
            Self::SetAtmosphere {
                atmosphere: Some(atmosphere),
            } if !atmosphere.valid() => Err(RenderOperationError::Atmosphere),
            Self::SetAtmosphere { .. } => Ok(()),
            Self::SetSunShafts {
                sun_shafts: Some(sun_shafts),
            } if !sun_shafts.valid() => Err(RenderOperationError::SunShafts),
            Self::SetSunShafts { .. } => Ok(()),
            Self::SetWind { wind: Some(wind) } if !wind.valid() => Err(RenderOperationError::Wind),
            Self::SetWind { .. } => Ok(()),
            Self::SetClouds {
                clouds: Some(clouds),
            } if !clouds.valid() => Err(RenderOperationError::Clouds),
            Self::SetClouds { .. } => Ok(()),
            Self::SetWetness {
                wetness: Some(wetness),
            } if !wetness.valid() => Err(RenderOperationError::Wetness),
            Self::SetWetness { .. } => Ok(()),
            Self::SetBackdrop {
                backdrop: Some(backdrop),
                ..
            } if !backdrop.valid() => Err(RenderOperationError::Backdrop),
            Self::SetBackdrop { .. } => Ok(()),
            Self::SetCloudRegion { region, .. } if !region.valid() => {
                Err(RenderOperationError::CloudRegion)
            }
            Self::SetCloudRegion { .. } | Self::RemoveCloudRegion { .. } => Ok(()),
            Self::SetVolumetricFog { fog } if !fog.valid() => {
                Err(RenderOperationError::VolumetricFog)
            }
            Self::SetVolumetricFog { .. } => Ok(()),
            Self::SetFogVolume { volume, .. } if !volume.valid() => {
                Err(RenderOperationError::FogVolume)
            }
            Self::SetFogVolume { .. } | Self::RemoveFogVolume { .. } => Ok(()),
            Self::SetPrecipitation {
                precipitation: Some(precipitation),
            } if !precipitation.valid() => Err(RenderOperationError::Precipitation),
            Self::SetPrecipitation { .. } => Ok(()),
            Self::SetImageEffect {
                effect: Some(effect),
            } if !effect.valid() => Err(RenderOperationError::ImageEffect),
            Self::SetImageEffect { .. } => Ok(()),
            Self::SetIndirectLight {
                indirect_light: Some(indirect_light),
            } if !indirect_light.valid() => Err(RenderOperationError::IndirectLight),
            Self::SetIndirectLight { .. } => Ok(()),
            Self::SetSkyLight {
                sky_light: Some(sky_light),
            } if !sky_light.valid() => Err(RenderOperationError::SkyLight),
            Self::SetSkyLight { .. } => Ok(()),
            Self::SetRendererSettings { settings } if !settings.valid() => {
                Err(RenderOperationError::RendererSettings)
            }
            Self::SetRendererSettings { .. } => Ok(()),
            Self::DefineSpriteAtlas { atlas } => {
                atlas.validate().map_err(RenderOperationError::SpriteAtlas)
            }
            Self::DefineStaticMesh { asset } => {
                asset.validate().map_err(RenderOperationError::StaticMesh)
            }
            Self::ReleaseMaterial { id } => {
                crate::validate_asset_id(id, crate::RenderAssetKind::Material)
                    .map_err(RenderOperationError::Asset)
            }
            Self::ReleaseTexture { id } => {
                crate::validate_asset_id(id, crate::RenderAssetKind::Texture)
                    .map_err(RenderOperationError::Asset)
            }
            Self::ReleaseShader { id } => {
                crate::validate_asset_id(id, crate::RenderAssetKind::Shader)
                    .map_err(RenderOperationError::Asset)
            }
            Self::ReleaseSpriteAtlas { id } => {
                crate::validate_asset_id(id, crate::RenderAssetKind::SpriteAtlas)
                    .map_err(RenderOperationError::Asset)
            }
            Self::ReleaseAnimatedMesh { asset } => {
                crate::validate_asset_id(asset, crate::RenderAssetKind::AnimatedMesh)
                    .map_err(RenderOperationError::Asset)
            }
            Self::ReleaseStaticMesh { asset } => {
                crate::validate_asset_id(asset, crate::RenderAssetKind::StaticMesh)
                    .map_err(RenderOperationError::Asset)
            }
            Self::DefineAnimatedMesh { asset } => {
                asset.validate().map_err(RenderOperationError::AnimatedMesh)
            }
            Self::DefineVoxelObject { asset } => {
                asset.validate().map_err(RenderOperationError::VoxelObject)
            }
            Self::ReleaseVoxelObject { asset } => {
                crate::validate_asset_id(asset, crate::RenderAssetKind::VoxelObject)
                    .map_err(RenderOperationError::Asset)
            }
            Self::CreateStaticMeshInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::StaticMeshInstance),
            Self::CreateAnimatedMeshInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::AnimatedMeshInstance),
            Self::CreateScatterPatch { patch, .. } => {
                patch.validate().map_err(RenderOperationError::ScatterPatch)
            }
            Self::SetAnimatedMeshPlayback { playback, .. } => playback
                .validate()
                .map_err(RenderOperationError::AnimatedPlayback),
            Self::SetAnimatedMeshPose { pose, .. } => {
                pose.validate().map_err(RenderOperationError::AnimatedPose)
            }
            Self::CreateVoxelObjectInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::VoxelObjectInstance),
            Self::SetVoxelObjectFrame { .. } | Self::SetAnimatedMeshInspection { .. } => Ok(()),
            Self::CreateSprite { sprite, .. } => {
                sprite.validate().map_err(RenderOperationError::Sprite)
            }
            Self::UpdateSprite { tint, .. } => {
                if tint.is_some_and(|color| !valid_color(color)) {
                    return Err(RenderOperationError::InvalidSpriteTint);
                }
                Ok(())
            }
        }
    }

    fn validate_handles(&self) -> Result<(), RenderHandleError> {
        match self {
            Self::Create { handle, parent, .. }
            | Self::CreateLight { handle, parent, .. }
            | Self::CreateStaticMeshInstance { handle, parent, .. }
            | Self::CreateAnimatedMeshInstance { handle, parent, .. }
            | Self::CreateScatterPatch { handle, parent, .. }
            | Self::CreateVoxelObjectInstance { handle, parent, .. }
            | Self::CreateSprite { handle, parent, .. } => {
                handle.validate()?;
                if let Some(parent) = parent {
                    parent.validate()?;
                }
            }
            Self::Update { handle, .. }
            | Self::SetParentJoint { handle, .. }
            | Self::Destroy { handle }
            | Self::ReplaceMeshPayload { handle, .. }
            | Self::ReplaceMeshDistanceField { handle, .. }
            | Self::UpdateLight { handle, .. }
            | Self::SetMaterialInstanceParameters { handle, .. }
            | Self::SetAnimatedMeshInspection { handle, .. }
            | Self::SetAnimatedMeshPlayback { handle, .. }
            | Self::SetAnimatedMeshPose { handle, .. }
            | Self::SetVoxelObjectFrame { handle, .. }
            | Self::UpdateSprite { handle, .. } => handle.validate()?,
            Self::DefineMaterial { .. }
            | Self::DefineTexture { .. }
            | Self::DefineShader { .. }
            | Self::ReleaseShader { .. }
            | Self::SetSkyBackground { .. }
            | Self::SetBackgroundColor { .. }
            | Self::SetFog { .. }
            | Self::SetToneMapping { .. }
            | Self::SetBloom { .. }
            | Self::SetAutoExposure { .. }
            | Self::SetColorGrading { .. }
            | Self::SetAtmosphere { .. }
            | Self::SetSunShafts { .. }
            | Self::SetWind { .. }
            | Self::SetClouds { .. }
            | Self::SetWetness { .. }
            | Self::SetBackdrop { .. }
            | Self::SetCloudRegion { .. }
            | Self::RemoveCloudRegion { .. }
            | Self::SetVolumetricFog { .. }
            | Self::SetFogVolume { .. }
            | Self::RemoveFogVolume { .. }
            | Self::SetPrecipitation { .. }
            | Self::SetImageEffect { .. }
            | Self::SetIndirectLight { .. }
            | Self::SetSkyLight { .. }
            | Self::SetRendererSettings { .. }
            | Self::DefineSpriteAtlas { .. }
            | Self::DefineStaticMesh { .. }
            | Self::ReleaseMaterial { .. }
            | Self::ReleaseTexture { .. }
            | Self::ReleaseSpriteAtlas { .. }
            | Self::ReleaseAnimatedMesh { .. }
            | Self::ReleaseStaticMesh { .. }
            | Self::DefineAnimatedMesh { .. }
            | Self::DefineVoxelObject { .. }
            | Self::ReleaseVoxelObject { .. } => {}
        }
        Ok(())
    }
}

fn valid_color<const N: usize>(color: [f32; N]) -> bool {
    color
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderOperationError {
    InvalidParentJoint,
    Handle(RenderHandleError),
    Node(NodeError),
    Transform(TransformError),
    Mesh(crate::MeshDescriptorError),
    Light(crate::LightDescriptorError),
    MaterialDescriptor(crate::MaterialDescriptorError),
    MaterialParameters(crate::MaterialParametersError),
    Texture(crate::TextureError),
    SkyBackground(crate::RenderAssetError),
    BackgroundColor,
    Fog,
    ToneMapping,
    Bloom,
    AutoExposure,
    ColorGrading,
    Atmosphere,
    SunShafts,
    Wind,
    Clouds,
    Wetness,
    Backdrop,
    VolumetricFog,
    FogVolume,
    CloudRegion,
    Precipitation,
    ImageEffect,
    IndirectLight,
    SkyLight,
    RendererSettings,
    SpriteAtlas(crate::SpriteAtlasError),
    StaticMesh(crate::StaticMeshError),
    StaticMeshInstance(crate::StaticMeshInstanceError),
    AnimatedMesh(crate::AnimatedMeshAssetError),
    AnimatedMeshInstance(crate::AnimatedMeshInstanceError),
    ScatterPatch(crate::ScatterPatchError),
    AnimatedPlayback(crate::AnimatedMeshPlaybackError),
    AnimatedPose(crate::AnimatedMeshPoseError),
    VoxelObject(crate::VoxelObjectRenderAssetError),
    VoxelObjectInstance(crate::VoxelObjectInstanceError),
    Asset(crate::RenderAssetError),
    Sprite(crate::SpriteError),
    InvalidSpriteTint,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderFrameDiff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<RenderFramePublication>,
    pub ops: Vec<RenderDiff>,
}

/// Optional monotonic publication identity for one independently ordered
/// retained stream. The operation count makes a clipped chunk update reject
/// before any renderer state changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderFramePublication {
    pub stream: String,
    pub base_revision: u64,
    pub revision: u64,
    pub operation_count: u32,
}

impl RenderFrameDiff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every mesh payload the frame carries, to move inline streams into
    /// resources (`pack_mesh_resources`).
    pub fn mesh_payloads_mut(&mut self) -> impl Iterator<Item = &mut crate::MeshPayloadDescriptor> {
        self.ops.iter_mut().flat_map(|op| {
            let payloads: Vec<&mut crate::MeshPayloadDescriptor> = match op {
                RenderDiff::ReplaceMeshPayload { payload, .. } => vec![payload],
                RenderDiff::DefineStaticMesh { asset } => vec![&mut asset.payload],
                RenderDiff::DefineVoxelObject { asset } => asset
                    .meshes
                    .iter_mut()
                    .map(|mesh| &mut mesh.payload)
                    .collect(),
                // Listed so that a new op carrying a payload is added here.
                RenderDiff::SetParentJoint { .. }
                | RenderDiff::ReplaceMeshDistanceField { .. }
                | RenderDiff::Create { .. }
                | RenderDiff::Update { .. }
                | RenderDiff::Destroy { .. }
                | RenderDiff::CreateLight { .. }
                | RenderDiff::UpdateLight { .. }
                | RenderDiff::DefineMaterial { .. }
                | RenderDiff::ReleaseMaterial { .. }
                | RenderDiff::SetMaterialInstanceParameters { .. }
                | RenderDiff::DefineTexture { .. }
                | RenderDiff::ReleaseTexture { .. }
                | RenderDiff::DefineShader { .. }
                | RenderDiff::ReleaseShader { .. }
                | RenderDiff::SetSkyBackground { .. }
                | RenderDiff::SetBackgroundColor { .. }
                | RenderDiff::SetFog { .. }
                | RenderDiff::SetToneMapping { .. }
                | RenderDiff::SetBloom { .. }
                | RenderDiff::SetAutoExposure { .. }
                | RenderDiff::SetColorGrading { .. }
                | RenderDiff::SetAtmosphere { .. }
                | RenderDiff::SetSunShafts { .. }
                | RenderDiff::SetWind { .. }
                | RenderDiff::SetClouds { .. }
                | RenderDiff::SetWetness { .. }
                | RenderDiff::SetBackdrop { .. }
                | RenderDiff::SetCloudRegion { .. }
                | RenderDiff::RemoveCloudRegion { .. }
                | RenderDiff::SetVolumetricFog { .. }
                | RenderDiff::SetFogVolume { .. }
                | RenderDiff::RemoveFogVolume { .. }
                | RenderDiff::SetPrecipitation { .. }
                | RenderDiff::SetImageEffect { .. }
                | RenderDiff::SetIndirectLight { .. }
                | RenderDiff::SetSkyLight { .. }
                | RenderDiff::SetRendererSettings { .. }
                | RenderDiff::DefineSpriteAtlas { .. }
                | RenderDiff::ReleaseSpriteAtlas { .. }
                | RenderDiff::ReleaseStaticMesh { .. }
                | RenderDiff::DefineAnimatedMesh { .. }
                | RenderDiff::ReleaseAnimatedMesh { .. }
                | RenderDiff::ReleaseVoxelObject { .. }
                | RenderDiff::CreateStaticMeshInstance { .. }
                | RenderDiff::CreateAnimatedMeshInstance { .. }
                | RenderDiff::CreateScatterPatch { .. }
                | RenderDiff::SetAnimatedMeshInspection { .. }
                | RenderDiff::SetAnimatedMeshPlayback { .. }
                | RenderDiff::SetAnimatedMeshPose { .. }
                | RenderDiff::CreateVoxelObjectInstance { .. }
                | RenderDiff::SetVoxelObjectFrame { .. }
                | RenderDiff::CreateSprite { .. }
                | RenderDiff::UpdateSprite { .. } => Vec::new(),
            };
            payloads
        })
    }

    pub fn try_from_ops(ops: Vec<RenderDiff>) -> Result<Self, RenderFrameError> {
        let frame = Self {
            publication: None,
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn try_from_published_ops(
        stream: impl Into<String>,
        base_revision: u64,
        revision: u64,
        ops: Vec<RenderDiff>,
    ) -> Result<Self, RenderFrameError> {
        let operation_count =
            u32::try_from(ops.len()).map_err(|_| RenderFrameError::PublicationOperationCount {
                expected: u32::MAX,
                actual: ops.len(),
            })?;
        let frame = Self {
            publication: Some(RenderFramePublication {
                stream: stream.into(),
                base_revision,
                revision,
                operation_count,
            }),
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), RenderFrameError> {
        if let Some(publication) = &self.publication {
            if publication.stream.trim().is_empty() || publication.stream.len() > 256 {
                return Err(RenderFrameError::InvalidPublicationStream);
            }
            if publication.base_revision > JSON_SAFE_U64_MAX {
                return Err(RenderFrameError::PublicationRevisionNotJsonSafe {
                    revision: publication.base_revision,
                });
            }
            if publication.revision > JSON_SAFE_U64_MAX {
                return Err(RenderFrameError::PublicationRevisionNotJsonSafe {
                    revision: publication.revision,
                });
            }
            if publication.base_revision.checked_add(1) != Some(publication.revision) {
                return Err(RenderFrameError::InvalidPublicationRevisionStep {
                    base_revision: publication.base_revision,
                    revision: publication.revision,
                });
            }
            if publication.operation_count as usize != self.ops.len() {
                return Err(RenderFrameError::PublicationOperationCount {
                    expected: publication.operation_count,
                    actual: self.ops.len(),
                });
            }
        }
        for (index, operation) in self.ops.iter().enumerate() {
            operation
                .validate()
                .map_err(|source| RenderFrameError::Operation { index, source })?;
        }
        Ok(())
    }

    pub fn push(&mut self, operation: RenderDiff) {
        self.ops.push(operation);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderFrameError {
    Operation {
        index: usize,
        source: RenderOperationError,
    },
    InvalidPublicationStream,
    PublicationRevisionNotJsonSafe {
        revision: u64,
    },
    InvalidPublicationRevisionStep {
        base_revision: u64,
        revision: u64,
    },
    PublicationOperationCount {
        expected: u32,
        actual: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_frame_requires_one_exact_revision_step() {
        let frame = RenderFrameDiff::try_from_published_ops("voxel:test", 4, 5, Vec::new())
            .expect("one exact step");
        assert_eq!(frame.publication.unwrap().base_revision, 4);
        assert!(matches!(
            RenderFrameDiff::try_from_published_ops("voxel:test", 4, 6, Vec::new()),
            Err(RenderFrameError::InvalidPublicationRevisionStep {
                base_revision: 4,
                revision: 6,
            })
        ));
    }

    #[test]
    fn camera_relative_viewmodel_layer_round_trips_without_backend_vocabulary() {
        let mut node = RenderNode::new(Geometry::Group);
        node.layer = RenderLayer::Viewmodel;
        let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::Create {
            handle: RenderHandle::new(8),
            parent: None,
            node,
        }])
        .unwrap();

        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"layer\":\"viewmodel\""));
        assert_eq!(
            serde_json::from_str::<RenderFrameDiff>(&json).unwrap(),
            frame
        );
    }

    #[test]
    fn invalid_operation_rejects_the_whole_frame() {
        let invalid = RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::Create {
                handle: RenderHandle::new(1),
                parent: None,
                node: RenderNode {
                    material: Material {
                        color: [2.0, 0.0, 0.0, 1.0],
                        wireframe: false,
                    },
                    ..RenderNode::new(Geometry::Cube)
                },
            }],
        };
        assert!(matches!(
            invalid.validate(),
            Err(RenderFrameError::Operation { index: 0, .. })
        ));
    }

    #[test]
    fn frame_rejects_handles_that_javascript_cannot_represent_exactly() {
        let invalid = RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::Create {
                handle: RenderHandle::new(JSON_SAFE_U64_MAX + 1),
                parent: None,
                node: RenderNode::new(Geometry::Cube),
            }],
        };
        assert!(matches!(
            invalid.validate(),
            Err(RenderFrameError::Operation {
                index: 0,
                source: RenderOperationError::Handle(RenderHandleError::OutsideJsonSafeRange(_))
            })
        ));
    }
}
