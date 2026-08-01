use core_assets::AssetReference;

pub const MAX_TEXTURE_DIMENSION: u32 = 4_096;
pub const MAX_TEXTURE_TEXELS: u64 = 16_777_216;
pub const MAX_ATLAS_REGIONS: usize = 1_024;
pub const MAX_AGGREGATE_ATLAS_REGIONS: usize = 4_096;
pub const MAX_ATLAS_PADDING: u16 = 32;
pub const MIN_TILE_SCALE_CELLS: f32 = 1.0 / 256.0;
pub const MAX_TILE_SCALE_CELLS: f32 = 4_096.0;
pub const MAX_TILE_ORIGIN_CELLS: f32 = 16_777_216.0;

/// Authority-relevant occupancy classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralClass {
    Decorative,
    Solid,
    Structural,
}

/// Authority projection. It deliberately contains no visual fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialAuthority {
    pub solid: bool,
    pub collidable: bool,
    pub occludes: bool,
    pub structural_class: StructuralClass,
}

impl MaterialAuthority {
    pub const DECORATIVE: Self = Self {
        solid: false,
        collidable: false,
        occludes: false,
        structural_class: StructuralClass::Decorative,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvStrategy {
    Flat,
    Planar,
    Atlas,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextureFilter {
    #[default]
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextureWrap {
    Clamp,
    #[default]
    Repeat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureDefinition {
    pub width: u32,
    pub height: u32,
    pub filter: TextureFilter,
    pub wrap: TextureWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasPadding {
    pub left: u16,
    pub right: u16,
    pub bottom: u16,
    pub top: u16,
}

impl AtlasPadding {
    pub const ZERO: Self = Self {
        left: 0,
        right: 0,
        bottom: 0,
        top: 0,
    };

    pub const ONE: Self = Self {
        left: 1,
        right: 1,
        bottom: 1,
        top: 1,
    };
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AtlasInset {
    #[default]
    HalfTexel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasRegionDefinition {
    pub id: String,
    pub content_min: [u32; 2],
    pub content_extent: [u32; 2],
    pub padding: AtlasPadding,
    pub inset: AtlasInset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAtlasDefinition {
    pub schema_version: u32,
    pub texture: AssetReference,
    pub regions: Vec<AtlasRegionDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoxelAlphaMode {
    Opaque,
    Mask { cutoff: f32 },
    Blend,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelSurfaceMapping {
    Repeat {
        texture: AssetReference,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
    Atlas {
        atlas: AssetReference,
        region: String,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelSurfaceBinding {
    pub schema_version: u32,
    pub mapping: VoxelSurfaceMapping,
    pub alpha_mode: VoxelAlphaMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedVoxelSurfaceMapping {
    Repeat {
        texture: AssetReference,
        texture_version: u32,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
    Atlas {
        atlas: AssetReference,
        atlas_version: u32,
        texture: AssetReference,
        texture_version: u32,
        region: AtlasRegionDefinition,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedVoxelSurface {
    pub schema_version: u32,
    pub filter: TextureFilter,
    pub wrap: TextureWrap,
    pub alpha_mode: VoxelAlphaMode,
    pub mapping: ResolvedVoxelSurfaceMapping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelSurfaceResolutionError {
    MissingAsset,
    StaleReference,
    MissingTextureDefinition,
    MissingAtlasDefinition,
    MissingAtlasRegion,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const DEBUG_GREY: Self = Self {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    pub const DEBUG_MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
}

/// Renderer-facing material data. It deliberately contains no authority flags.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialStyle {
    pub color: Rgba,
    pub texture: Option<AssetReference>,
    pub roughness: f32,
    pub texture_tint: Rgba,
    pub emission_color: Rgba,
    pub emissive: f32,
    pub uv_strategy: UvStrategy,
    pub voxel_surface: Option<VoxelSurfaceBinding>,
}

impl MaterialStyle {
    pub fn flat(color: Rgba) -> Self {
        Self {
            color,
            texture: None,
            roughness: 1.0,
            texture_tint: Rgba::WHITE,
            emission_color: color,
            emissive: 0.0,
            uv_strategy: UvStrategy::Flat,
            voxel_surface: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDefinition {
    pub authority: MaterialAuthority,
    pub style: MaterialStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderMaterial {
    pub color: Rgba,
    pub texture: Option<AssetReference>,
    pub roughness: f32,
    pub texture_tint: Rgba,
    pub emission_color: Rgba,
    pub emissive: f32,
    pub uv_strategy: UvStrategy,
    pub voxel_surface: Option<ResolvedVoxelSurface>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionMaterial {
    pub solid: bool,
    pub collidable: bool,
    pub occludes: bool,
    pub structural_class: StructuralClass,
}

impl MaterialDefinition {
    pub fn render_projection(&self) -> RenderMaterial {
        RenderMaterial {
            color: self.style.color,
            texture: self.style.texture.clone(),
            roughness: self.style.roughness,
            texture_tint: self.style.texture_tint,
            emission_color: self.style.emission_color,
            emissive: self.style.emissive,
            uv_strategy: self.style.uv_strategy,
            voxel_surface: None,
        }
    }

    pub fn render_projection_with_surface(
        &self,
        voxel_surface: ResolvedVoxelSurface,
    ) -> RenderMaterial {
        let mut material = self.render_projection();
        material.voxel_surface = Some(voxel_surface);
        material
    }

    pub fn collision_projection(&self) -> CollisionMaterial {
        CollisionMaterial {
            solid: self.authority.solid,
            collidable: self.authority.collidable,
            occludes: self.authority.occludes,
            structural_class: self.authority.structural_class,
        }
    }
}
