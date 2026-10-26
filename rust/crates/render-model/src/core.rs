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

/// Environment values feed state the renderer keeps across frames (fog and
/// cloud histories, auto exposure), where one NaN would persist after the
/// product fixed it, so they are finite. Their upper bounds are a look the
/// product chooses, not a limit.
fn non_negative(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
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

    pub fn valid(&self) -> bool {
        let color = self.color();
        color.iter().all(|value| value.is_finite() && *value >= 0.0)
            && match *self {
                Self::Linear { start, end, .. } => {
                    start.is_finite() && end.is_finite() && start >= 0.0 && end > start
                }
                Self::Exponential { density, .. } | Self::ExponentialSquared { density, .. } => {
                    density.is_finite() && density > 0.0
                }
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
/// spreads into a glow added at `intensity` (0 or more) before exposure and
/// tone mapping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BloomDescriptor {
    pub threshold: f32,
    pub intensity: f32,
}

impl BloomDescriptor {
    /// A finite non-negative threshold and intensity.
    pub fn valid(&self) -> bool {
        non_negative(self.threshold) && non_negative(self.intensity)
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
/// sun, and the sun drawn in the sky. The sky's sun (its disc and halo, the
/// haze toward it, sun shafts and the light on the clouds) is the brightest
/// enabled directional light of the world, unless `sky_sun_direction` places
/// it. Zero leaves each part out.
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
    /// The disc's radiance in the sun's colour, linear HDR (0 or more),
    /// whatever the light's intensity. 0 draws the disc and halo at the
    /// light's intensity, at most 1, and bloom does not see
    /// them; above 0 the sky shows them at up to full brightness, and bloom
    /// spreads the disc at this radiance and the halo at `sun_halo` where
    /// nothing in the world or the clouds covers them.
    #[serde(default)]
    pub sun_radiance: f32,
    /// Toward the sun in the sky, in world space (any length); zero takes
    /// the brightest enabled directional light's direction, colour and
    /// intensity. A product whose ground light differs from the sun it
    /// shows (a twilight light kept above the horizon) places the sky's sun
    /// here, below the horizon if need be.
    #[serde(default)]
    pub sky_sun_direction: [f32; 3],
    /// The sky's sun's linear RGB, when `sky_sun_direction` places it.
    #[serde(default)]
    pub sky_sun_color: [f32; 3],
    /// The sky's sun's intensity, when `sky_sun_direction` places it.
    #[serde(default)]
    pub sky_sun_intensity: f32,
}

impl AtmosphereDescriptor {
    /// Finite values, all but the fog base height and the sky sun
    /// direction non-negative.
    pub fn valid(&self) -> bool {
        self.fog_base_height.is_finite()
            && non_negative(self.fog_falloff_height)
            && self.haze_color.iter().all(|&value| non_negative(value))
            && non_negative(self.haze_exponent)
            && non_negative(self.sun_radius_degrees)
            && non_negative(self.sun_halo)
            && non_negative(self.sun_radiance)
            && self.sky_sun_direction.iter().all(|value| value.is_finite())
            && self.sky_sun_color.iter().all(|&value| non_negative(value))
            && non_negative(self.sky_sun_intensity)
    }
}

/// Sun shafts: the sky around the sun, blurred along rays from it, so light
/// streams past whatever stands in front of it. `intensity` scales the
/// light added (0 or more); `length` is how far the rays reach from the sun,
/// as a fraction of the way to each pixel (0 takes the Engine's default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SunShaftsDescriptor {
    pub intensity: f32,
    pub length: f32,
}

impl SunShaftsDescriptor {
    /// A finite non-negative intensity and a length within `0..=1`.
    pub fn valid(&self) -> bool {
        non_negative(self.intensity) && (0.0..=1.0).contains(&self.length)
    }
}

/// The moon in the sky, apart from the sun: a disc of `radius_degrees`
/// toward `direction` (world space, any length), its lit part `radiance`
/// times `color` (linear HDR). `phase` is the lit fraction, 0 (new) to 1
/// (full); the lit limb turns toward the sky's sun. `earthshine` (0 to 1)
/// lights the dark part by that share of the radiance, and `halo` adds a
/// glow a few degrees wide of that strength. `light_intensity` lights the
/// clouds from the moon in its colour, a second light beside the sky's sun;
/// 0 lights nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoonDescriptor {
    pub direction: [f32; 3],
    pub radius_degrees: f32,
    pub color: [f32; 3],
    pub radiance: f32,
    pub phase: f32,
    pub earthshine: f32,
    pub halo: f32,
    #[serde(default)]
    pub light_intensity: f32,
}

impl MoonDescriptor {
    /// A finite direction with some length, a phase and earthshine within
    /// `0..=1`, and every other value finite and non-negative.
    pub fn valid(&self) -> bool {
        let [x, y, z] = self.direction;
        let unit = 0.0..=1.0;
        [x, y, z].iter().all(|value| value.is_finite())
            && x * x + y * y + z * z > 0.0
            && non_negative(self.radius_degrees)
            && self.color.iter().all(|&channel| non_negative(channel))
            && non_negative(self.radiance)
            && unit.contains(&self.phase)
            && unit.contains(&self.earthshine)
            && non_negative(self.halo)
            && non_negative(self.light_intensity)
    }
}

/// The scene's wind, which materials with the wind feature
/// (`RenderMaterialDescriptor::wind`) and product displace stages sway in.
/// `direction` is over the ground (world x, z; any length, normalized when
/// read); `strength` (0 or more) scales every material's bend and flutter;
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
    /// A finite direction with some length, a finite non-negative strength
    /// and a gust share within `0..=1`.
    pub fn valid(&self) -> bool {
        let [x, z] = self.direction;
        x.is_finite()
            && z.is_finite()
            && (x * x + z * z) > 0.0
            && non_negative(self.strength)
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
/// gathers standing water in patches on flat ground as the surfaces wet;
/// `frozen` (0 to 1) turns the puddles to ice from their edges in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WetnessDescriptor {
    pub wetness: f32,
    pub puddles: f32,
    #[serde(default)]
    pub frozen: f32,
}

/// Snow lying on the scene's surfaces (`CameraView.SetSnow`): `amount`, from
/// 0 to 1, covers lit surfaces that face up and stand under the open sky in
/// `color` (linear), its edge broken by world-space noise. A
/// `texels_per_metre` above 0 steps the cover's edge on a world grid of that
/// density, so it reads as painted pixels; 0 blends it smoothly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnowDescriptor {
    pub amount: f32,
    pub color: [f32; 3],
    pub texels_per_metre: f32,
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
    /// Wetness, puddles and frozen within `0..=1`.
    pub fn valid(&self) -> bool {
        [self.wetness, self.puddles, self.frozen]
            .iter()
            .all(|value| (0.0..=1.0).contains(value))
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

    /// A finite centre and drift, a finite radius above 0, a coverage and
    /// darkness within `0..=1` and a finite non-negative thickness.
    pub fn valid(&self) -> bool {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        finite(&self.center)
            && finite(&self.drift)
            && positive(self.radius)
            && (0.0..=1.0).contains(&self.coverage)
            && (0.0..=1.0).contains(&self.darkness)
            && non_negative(self.thickness)
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
    /// Extinction per metre at `base_height` (0 or more): 0 fills no air,
    /// so only fog volumes draw.
    pub density: f32,
    /// The colour the fog scatters, linear (0 or more a channel).
    pub albedo: [f32; 3],
    /// Henyey–Greenstein anisotropy (-0.9 to 0.9): positive scatters forward,
    /// so fog glows looking toward a light and shafts read against it.
    pub anisotropy: f32,
    /// The height `density` holds at, in world units.
    pub base_height: f32,
    /// Density falls by e every this many metres up; 0 is one density
    /// everywhere.
    pub falloff_height: f32,
    /// How far from the camera the fog grid reaches, metres (above 0).
    pub distance: f32,
    /// How much of the ambient and sky light the fog scatters (0 or more).
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

    /// Finite values: a non-negative density, albedo, falloff and ambient,
    /// an anisotropy within `-0.9..=0.9` (Henyey–Greenstein's domain) and a
    /// distance above 0 (the history reprojection divides by it).
    pub fn valid(&self) -> bool {
        non_negative(self.density)
            && self.albedo.iter().all(|&channel| non_negative(channel))
            && (-0.9..=0.9).contains(&self.anisotropy)
            && self.base_height.is_finite()
            && non_negative(self.falloff_height)
            && positive(self.distance)
            && non_negative(self.ambient)
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

/// A projected decal (`CameraView.SetDecal`): a mark the world pass lays
/// over the opaque surfaces inside a box, projected down its own -y axis.
/// Positions are in the renderer's world space, as a fog volume's. The box
/// stands at `center`, turned by `rotation` (a unit quaternion x, y, z, w),
/// `size` metres along its own x, y and z; its x and z span the region
/// `uv_rect` (u0, v0, u1, v1) of the decal atlas (`SetDecalAtlas`), v
/// growing toward the box's +z.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecalDescriptor {
    pub center: [f32; 3],
    pub rotation: [f32; 4],
    pub size: [f32; 3],
    pub uv_rect: [f32; 4],
    /// Linear tint (rgb) times the atlas, and its opacity (a).
    pub color: [f32; 4],
    /// The roughness where the decal covers (above 0), or 0 to keep the
    /// surface's.
    pub roughness: f32,
    /// How strongly the normal atlas bends the surface (0: not at all).
    pub normal_strength: f32,
    /// Linear light it emits where it covers (may exceed 1), times the
    /// emission atlas or, without one, the colour atlas; added after the
    /// surface is lit. Zero for a decal that emits none.
    #[serde(default)]
    pub emission: [f32; 3],
    /// Fade out surfaces that turn away from the box's +y, so steep faces
    /// do not show the projection smeared along them.
    pub angle_falloff: bool,
    /// Also lay over vegetation copies, and over animated meshes.
    pub on_vegetation: bool,
    pub on_animated: bool,
    /// The Engine presentation seconds between which it fades out, and
    /// after which it is gone; both 0 for a decal that stays.
    pub fade_start: f64,
    pub fade_end: f64,
}

impl DecalDescriptor {
    /// The most decals a scene holds.
    pub const MAX_DECALS: usize = 4096;
}

/// The textures decals sample (`CameraView.SetDecalAtlas`): the atlas their
/// regions come from, by texture id, and a normal atlas and an emission
/// atlas laid out the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecalAtlasDescriptor {
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normals: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emission: Option<String>,
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
    /// Half the size along each axis, metres (above 0).
    pub half_extents: [f32; 3],
    /// Turn about the vertical axis, degrees.
    pub yaw_degrees: f32,
    /// Extinction per metre at its heart (0 or more).
    pub density: f32,
    /// The colour it scatters, linear (0 or more a channel).
    pub albedo: [f32; 3],
    /// Light it gives off per unit of density, linear (0 or more a
    /// channel): a glowing storm or an arcane haze.
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

    /// Finite values: half extents above 0, a non-negative density, albedo,
    /// emission and noise scale, and an edge and noise strength within
    /// `0..=1`.
    pub fn valid(&self) -> bool {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        finite(&self.center)
            && finite(&self.noise_velocity)
            && self.yaw_degrees.is_finite()
            && self.half_extents.iter().all(|&half| positive(half))
            && non_negative(self.density)
            && self.albedo.iter().all(|&channel| non_negative(channel))
            && self.emission.iter().all(|&channel| non_negative(channel))
            && (0.0..=1.0).contains(&self.edge)
            && non_negative(self.noise_scale)
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
/// or more a channel) and alpha (0 to 1), added to the frame when
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
    /// The most drops one volume draws: the draw's instance count.
    pub const MAX_DROPS: u32 = 200_000;

    /// Drops within `MAX_DROPS`; a finite velocity; a finite size above 0;
    /// a finite non-negative streak time and colour channels with alpha
    /// within `0..=1`; and a finite radius and height of at least 1 m.
    pub fn valid(&self) -> bool {
        let extent = |value: f32| value.is_finite() && value >= 1.0;
        self.drops <= Self::MAX_DROPS
            && self.velocity.iter().all(|value| value.is_finite())
            && positive(self.size)
            && non_negative(self.streak_seconds)
            && self.color[..3].iter().all(|&channel| non_negative(channel))
            && (0.0..=1.0).contains(&self.color[3])
            && extent(self.radius)
            && extent(self.height)
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
/// horizon; `color` (linear, 0 or more a channel) tints the light they
/// take. The volumetric clouds stand from `altitude` up by `thickness`
/// metres (0: six tenths of the altitude), shaped by `kind`.
/// `twilight_degrees` (0 or more) is how far below the horizon the sky's
/// sun still lights the clouds
/// overhead, from below and reddening as it sinks; 0 lights them only while
/// the sun is up.
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
    #[serde(default)]
    pub twilight_degrees: f32,
}

impl CloudsDescriptor {
    /// Coverage within `0..=1`, a finite drift, a finite altitude and scale
    /// above 0, and a finite non-negative thickness, colour and twilight.
    pub fn valid(&self) -> bool {
        (0.0..=1.0).contains(&self.coverage)
            && self.drift.iter().all(|value| value.is_finite())
            && positive(self.altitude)
            && positive(self.scale)
            && non_negative(self.thickness)
            && self.color.iter().all(|&channel| non_negative(channel))
            && non_negative(self.twilight_degrees)
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
/// `intensity` (0 or more).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkyLightDescriptor {
    pub intensity: f32,
}

impl SkyLightDescriptor {
    /// A finite non-negative intensity.
    pub fn valid(&self) -> bool {
        non_negative(self.intensity)
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

/// How water reflects the scene (`RendererSettingsDescriptor`): the sky's
/// light and the probes alone, or also what the view shows, found by
/// marching the reflected ray across the view's depth at low or high
/// quality, the sky and probes taking over where the ray leaves the view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaterReflectionsQuality {
    #[default]
    Off,
    Low,
    High,
}

/// How the primary destination smooths its edges
/// (`RendererSettingsDescriptor::antialiasing`): several samples a pixel
/// (MSAA), or a temporal upscale stage that gathers jittered frames at the
/// render scale into the output (render-wgpu `upscale.rs`): the Engine's
/// TAAU (`temporal`) on every GPU, or NVIDIA DLSS (`dlss`) on RTX cards in
/// builds with it, which draws TAAU where refused. One setting holds them
/// all, so a temporal stage and MSAA exclude each other. Written as its
/// text form (`off`, `2x`, `4x`, `temporal`, `dlss`); settings written when
/// it was a sample count read as that count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Antialiasing {
    Off,
    Msaa2,
    #[default]
    Msaa4,
    Temporal,
    Dlss,
}

impl Antialiasing {
    /// Every value, in a menu's order.
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Msaa2,
        Self::Msaa4,
        Self::Temporal,
        Self::Dlss,
    ];

    /// Samples per pixel of the primary destination: one under a temporal
    /// stage.
    pub const fn samples(self) -> u32 {
        match self {
            Self::Off | Self::Temporal | Self::Dlss => 1,
            Self::Msaa2 => 2,
            Self::Msaa4 => 4,
        }
    }

    /// Whether a temporal stage draws it (`temporal` or `dlss`); the render
    /// scale then picks its mode (`TEMPORAL_MODES`).
    pub const fn temporal(self) -> bool {
        matches!(self, Self::Temporal | Self::Dlss)
    }

    /// The text form: `off`, `2x`, `4x`, `temporal` or `dlss`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Msaa2 => "2x",
            Self::Msaa4 => "4x",
            Self::Temporal => "temporal",
            Self::Dlss => "dlss",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.name() == name)
    }

    /// The MSAA value drawing `samples` a pixel.
    pub fn from_samples(samples: u32) -> Option<Self> {
        match samples {
            1 => Some(Self::Off),
            2 => Some(Self::Msaa2),
            4 => Some(Self::Msaa4),
            _ => None,
        }
    }
}

impl Serialize for Antialiasing {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Antialiasing {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Written {
            Name(String),
            Samples(u32),
        }
        match Written::deserialize(deserializer)? {
            Written::Name(name) => Self::from_name(&name).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "antialiasing {name}: expected off, 2x, 4x, temporal or dlss"
                ))
            }),
            Written::Samples(samples) => Self::from_samples(samples).ok_or_else(|| {
                serde::de::Error::custom(format!("antialiasing {samples}: expected 1, 2 or 4"))
            }),
        }
    }
}

/// The temporal stages' named render scales
/// (`RendererSettingsDescriptor::render_scale` under `Antialiasing::Temporal`
/// or `Antialiasing::Dlss`): native-resolution anti-aliasing (DLAA under
/// DLSS), then the usual upscaling quality modes. Any scale within range
/// draws; these are the ones a menu names.
pub const TEMPORAL_MODES: [(&str, f32); 5] = [
    ("native", 1.0),
    ("quality", 0.67),
    ("balanced", 0.58),
    ("performance", 0.5),
    ("ultraPerformance", 0.33),
];

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
    /// With `shadows`, the brightest casting sun also shadows the backdrop
    /// (`SetBackdrop`) through cascades of its own. Settings written before
    /// it existed have it on.
    #[serde(default = "enabled")]
    pub backdrop_shadows: bool,
    /// At most this many shadow layers at once, the requesting lights
    /// chosen by priority then distance from the camera; `None` for no
    /// limit.
    pub shadow_budget: Option<u32>,
    pub ambient_occlusion: AmbientOcclusionSettings,
    /// How the primary destination smooths its edges: 1, 2 or 4 samples a
    /// pixel, or a temporal stage (TAAU or DLSS).
    pub antialiasing: Antialiasing,
    /// The fraction of the primary destination's size the world, viewmodel,
    /// labels and effects draw at before being upscaled into it: 0.33 to 1
    /// (`TEMPORAL_MODES` names the temporal stages').
    pub render_scale: f32,
    /// Window output waits for the display's refresh before presenting.
    pub vsync: bool,
    /// Bin each world view's lights into view-frustum clusters before
    /// shading, instead of shading every light per fragment.
    pub clustered_lighting: bool,
    /// Test each view's opaque parts against its frustum on the GPU and
    /// draw them indirectly, instead of building the draw list on the CPU.
    pub gpu_culling: bool,
    /// Drop scattered copies (grass, plants, trees) hidden behind a world
    /// view's opaque depth before they draw. Settings written before it
    /// existed have it off.
    #[serde(default)]
    pub occlusion_culling: bool,
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
    /// Water materials reflect what the view shows, in screen space.
    /// Settings written before it existed have it off.
    #[serde(default)]
    pub water_reflections: WaterReflectionsQuality,
}

fn enabled() -> bool {
    true
}

impl RendererSettingsDescriptor {
    /// The smallest render scale: the temporal stage's ultra-performance
    /// mode.
    pub const MIN_RENDER_SCALE: f32 = 0.33;

    pub const DEFAULT: Self = Self {
        shadows: false,
        backdrop_shadows: true,
        shadow_budget: None,
        ambient_occlusion: AmbientOcclusionSettings::DEFAULT,
        antialiasing: Antialiasing::Msaa4,
        render_scale: 1.0,
        vsync: true,
        clustered_lighting: false,
        gpu_culling: false,
        occlusion_culling: false,
        volumetric_fog: VolumetricFogQuality::Off,
        volumetric_clouds: VolumetricCloudsQuality::Off,
        water_reflections: WaterReflectionsQuality::Off,
    };

    /// Valid occlusion values and a render scale within range.
    pub fn valid(&self) -> bool {
        self.ambient_occlusion.valid()
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
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
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
    /// Selects the moon in the sky; None takes it away.
    SetMoon {
        moon: Option<MoonDescriptor>,
    },
    /// Selects the star field in the sky; None takes it away.
    SetStars {
        stars: Option<crate::StarsDescriptor>,
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
    /// Selects the snow lying on the scene's surfaces; None clears it.
    SetSnow {
        snow: Option<SnowDescriptor>,
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
    /// Places or replaces decal `id`.
    SetDecal {
        id: u64,
        decal: DecalDescriptor,
    },
    /// Removes decal `id`.
    RemoveDecal {
        id: u64,
    },
    /// Selects the textures decals sample; None draws each decal in its
    /// tint.
    SetDecalAtlas {
        atlas: Option<DecalAtlasDescriptor>,
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
    /// Replaces the named overlay set; an empty set removes it.
    SetOverlaySet {
        name: String,
        set: crate::OverlaySetDescriptor,
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
    /// The focus `handle`'s child nodes morph their meshes about
    /// (`MeshMorph`): a point (x, z) in `handle`'s own space, such as where
    /// a heightfield's rings centre. None draws them unmorphed.
    SetMorphFocus {
        handle: RenderHandle,
        focus: Option<[f64; 2]>,
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
    /// The pose the Engine evaluated for the instance, replacing the last
    /// one: what realizers draw it in.
    SetEvaluatedPose {
        handle: RenderHandle,
        pose: crate::EvaluatedPose,
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
    /// Many sprites from one atlas drawn as one node (`sprite_batch`):
    /// vegetation or crowds a product places as one unit.
    CreateSpriteBatch {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        batch: crate::SpriteBatchDescriptor,
    },
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
                | RenderDiff::SetMorphFocus { .. }
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
                | RenderDiff::SetMoon { .. }
                | RenderDiff::SetStars { .. }
                | RenderDiff::SetWind { .. }
                | RenderDiff::SetClouds { .. }
                | RenderDiff::SetWetness { .. }
                | RenderDiff::SetSnow { .. }
                | RenderDiff::SetBackdrop { .. }
                | RenderDiff::SetCloudRegion { .. }
                | RenderDiff::RemoveCloudRegion { .. }
                | RenderDiff::SetVolumetricFog { .. }
                | RenderDiff::SetFogVolume { .. }
                | RenderDiff::RemoveFogVolume { .. }
                | RenderDiff::SetDecal { .. }
                | RenderDiff::RemoveDecal { .. }
                | RenderDiff::SetDecalAtlas { .. }
                | RenderDiff::SetPrecipitation { .. }
                | RenderDiff::SetImageEffect { .. }
                | RenderDiff::SetIndirectLight { .. }
                | RenderDiff::SetSkyLight { .. }
                | RenderDiff::SetRendererSettings { .. }
                | RenderDiff::SetOverlaySet { .. }
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
                | RenderDiff::SetEvaluatedPose { .. }
                | RenderDiff::CreateVoxelObjectInstance { .. }
                | RenderDiff::SetVoxelObjectFrame { .. }
                | RenderDiff::CreateSprite { .. }
                | RenderDiff::CreateSpriteBatch { .. }
                | RenderDiff::UpdateSprite { .. } => Vec::new(),
            };
            payloads
        })
    }

    /// A frame of operations the Engine built from admitted values. Each
    /// value was validated where it entered the Engine; the frame is not
    /// validated again on its way to the renderer.
    pub fn from_ops(ops: Vec<RenderDiff>) -> Self {
        Self {
            publication: None,
            ops,
        }
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
    PublicationOperationCount { expected: u32, actual: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_relative_viewmodel_layer_round_trips_without_backend_vocabulary() {
        let mut node = RenderNode::new(Geometry::Group);
        node.layer = RenderLayer::Viewmodel;
        let frame = RenderFrameDiff::from_ops(vec![RenderDiff::Create {
            handle: RenderHandle::new(8),
            parent: None,
            node,
        }]);

        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"layer\":\"viewmodel\""));
        assert_eq!(
            serde_json::from_str::<RenderFrameDiff>(&json).unwrap(),
            frame
        );
    }
}
