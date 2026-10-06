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
}

impl RenderLayer {
    pub fn is_scene(&self) -> bool {
        *self == Self::Scene
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
    SetAnimatedMeshInspection {
        handle: RenderHandle,
        inspection: crate::AnimatedMeshInspection,
    },
    SetAnimatedMeshPlayback {
        handle: RenderHandle,
        playback: AnimatedMeshPlaybackCommand,
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
            Self::SetAnimatedMeshPlayback { playback, .. } => playback
                .validate()
                .map_err(RenderOperationError::AnimatedPlayback),
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
    RendererSettings,
    SpriteAtlas(crate::SpriteAtlasError),
    StaticMesh(crate::StaticMeshError),
    StaticMeshInstance(crate::StaticMeshInstanceError),
    AnimatedMesh(crate::AnimatedMeshAssetError),
    AnimatedMeshInstance(crate::AnimatedMeshInstanceError),
    AnimatedPlayback(crate::AnimatedMeshPlaybackError),
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
                | RenderDiff::SetRendererSettings { .. }
                | RenderDiff::DefineSpriteAtlas { .. }
                | RenderDiff::ReleaseSpriteAtlas { .. }
                | RenderDiff::ReleaseStaticMesh { .. }
                | RenderDiff::DefineAnimatedMesh { .. }
                | RenderDiff::ReleaseAnimatedMesh { .. }
                | RenderDiff::ReleaseVoxelObject { .. }
                | RenderDiff::CreateStaticMeshInstance { .. }
                | RenderDiff::CreateAnimatedMeshInstance { .. }
                | RenderDiff::SetAnimatedMeshInspection { .. }
                | RenderDiff::SetAnimatedMeshPlayback { .. }
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
