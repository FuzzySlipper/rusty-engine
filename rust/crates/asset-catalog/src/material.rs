use core_assets::AssetReference;

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
        }
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
