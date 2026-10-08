//! The standard shader family (`src/shaders`): WGSL modules composed with
//! naga_oil, the features a material compiles into its variant, and product
//! shaders that take over its last stage.
//!
//! `types`, `view`, `material`, `surface`, `lighting`, `tonemap`, `finish`
//! and `shade` are importable modules (`#import rusty::lighting::standard_radiance`);
//! `world`, `sky`, `shadow`, `effects`, `ghost`, `compose`,
//! `ambient_occlusion`, `finish_pass`, `post`, `sky_light`, `light_clusters`,
//! `cull` and `distance_field` are the entry shaders built from them.
//! A material's features are shader defs, so a variant only carries the
//! samples and branches its material uses.
//!
//! A product shader is one WGSL module imported as `rusty::product`. It
//! defines `fn shade(surface: Surface) -> vec4<f32>`, which the world pass
//! calls in place of `rusty::shade::standard_shade` with the surface the
//! standard stages computed; may define `fn cast_shadow(caster: Caster)`,
//! which the shadow pass calls (and which may discard); and may define
//! `fn displace(vertex: Vertex) -> vec3<f32>`, which the world and shadow
//! passes' vertex stages call for the vertex's world position. It may
//! import any module above, and read the standard feature defs
//! (`#ifdef NORMAL_MAP`) and its own keywords, chosen when it is opened.

use std::collections::HashMap;
use std::ops::BitOr;

use naga_oil::compose::{
    comment_strip_iter::CommentReplaceExt, preprocess::Preprocessor, tokenizer::Tokenizer,
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage,
    ShaderType,
};

/// The standard shader features a material compiles in, derived from what it
/// contains, and the product shader (if any) that shades it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Features {
    bits: u16,
    /// `Shaders::product` id; 0 for the standard shade stage.
    product: u32,
}

impl Features {
    pub const UNLIT: Self = Self::bit(1);
    pub const MASK: Self = Self::bit(2);
    pub const VOXEL_SURFACE: Self = Self::bit(4);
    pub const NORMAL_MAP: Self = Self::bit(8);
    pub const EMISSIVE_MAP: Self = Self::bit(16);
    pub const OCCLUSION_MAP: Self = Self::bit(32);
    /// The mesh's second stream: tangents and a second uv set (a mesh
    /// feature, not a material one).
    pub const VERTEX_TANGENTS: Self = Self::bit(64);
    pub const TRIPLANAR: Self = Self::bit(128);
    /// Blend up to three more base textures and normal maps over the
    /// material's own by the mesh's layer weights.
    pub const TERRAIN_LAYERS: Self = Self::bit(256);
    /// The mesh's vertex colours are terrain layer weights, not a tint (a
    /// mesh feature, not a material one).
    pub const LAYER_WEIGHTS: Self = Self::bit(512);
    /// Sample the base texture and normal map as blended, randomly offset
    /// and rotated hexagonal tiles (`rusty::surface::hex_tiles`).
    pub const STOCHASTIC_TILING: Self = Self::bit(1024);
    /// The occlusion map is packed occlusion, roughness, metalness (R, G,
    /// B): its green and blue multiply the material's roughness and
    /// metalness. Replaces OCCLUSION_MAP for that material.
    pub const ORM_MAP: Self = Self::bit(2048);
    /// Shade each triangle flat: the normal comes from the world position's
    /// screen derivatives, the mesh's normals and any normal map ignored.
    pub const FLAT_SHADING: Self = Self::bit(4096);
    /// Sway in the scene's wind (`rusty::wind`): the part bends with height
    /// above its origin and its vertices flutter by their colour's alpha,
    /// in the world and shadow passes alike.
    pub const WIND: Self = Self::bit(8192);
    /// A water surface (`world.wgsl` `water_surface`): tinted by the depth
    /// of the scene behind it (the blend pass's copy of the opaque depth,
    /// group 3), foam along the shore, scrolling normal maps or procedural
    /// ripples, and a Fresnel alpha. Only blended materials have it.
    pub const WATER: Self = Self::bit(16384);
    /// The mesh's vertex colour alpha is ambient occlusion from the voxels
    /// around each vertex (a mesh feature, not a material one), scaling the
    /// ambient, hemisphere and sky light; with LAYER_WEIGHTS the fourth
    /// layer's weight is the remainder of the first three.
    pub const VERTEX_OCCLUSION: Self = Self::bit(32768);
    /// Every standard feature's bit.
    #[cfg(test)]
    const ALL_BITS: u16 = 65535;

    const DEFS: [(Self, &'static str); 16] = [
        (Self::UNLIT, "UNLIT"),
        (Self::MASK, "MASK"),
        (Self::VOXEL_SURFACE, "VOXEL_SURFACE"),
        (Self::NORMAL_MAP, "NORMAL_MAP"),
        (Self::EMISSIVE_MAP, "EMISSIVE_MAP"),
        (Self::OCCLUSION_MAP, "OCCLUSION_MAP"),
        (Self::VERTEX_TANGENTS, "VERTEX_TANGENTS"),
        (Self::TRIPLANAR, "TRIPLANAR"),
        (Self::TERRAIN_LAYERS, "TERRAIN_LAYERS"),
        (Self::LAYER_WEIGHTS, "LAYER_WEIGHTS"),
        (Self::STOCHASTIC_TILING, "STOCHASTIC_TILING"),
        (Self::ORM_MAP, "ORM_MAP"),
        (Self::FLAT_SHADING, "FLAT_SHADING"),
        (Self::WIND, "WIND"),
        (Self::WATER, "WATER"),
        (Self::VERTEX_OCCLUSION, "VERTEX_OCCLUSION"),
    ];

    const fn bit(bits: u16) -> Self {
        Self { bits, product: 0 }
    }

    pub fn contains(self, other: Self) -> bool {
        self.bits & other.bits == other.bits
    }

    /// `self` with `other` added when `on`.
    pub fn with(self, other: Self, on: bool) -> Self {
        if on {
            self | other
        } else {
            self
        }
    }

    /// `self` shaded by product shader `product` (`Shaders::product`), or by
    /// the standard stage for 0.
    pub fn with_product(self, product: u32) -> Self {
        Self { product, ..self }
    }

    pub fn product(self) -> u32 {
        self.product
    }

    /// What the shadow caster pass compiles: only the alpha mask, the voxel
    /// uv remap, triplanar planes and hex tiles it samples through, whether
    /// vertex colours are layer weights or occlusion rather than alpha, the
    /// wind that moves the vertices, and a product shader that defines a
    /// caster or a displace stage.
    pub fn caster(self) -> Self {
        Self {
            bits: self.bits
                & (Self::MASK.bits
                    | Self::VOXEL_SURFACE.bits
                    | Self::TRIPLANAR.bits
                    | Self::STOCHASTIC_TILING.bits
                    | Self::LAYER_WEIGHTS.bits
                    | Self::VERTEX_OCCLUSION.bits
                    | Self::WIND.bits),
            product: if self.product & (PRODUCT_CASTS | PRODUCT_DISPLACES) != 0 {
                self.product
            } else {
                0
            },
        }
    }

    /// Whether the caster pass has a fragment stage: the alpha mask, or a
    /// product shader's caster stage, to discard.
    pub fn caster_fragment(self) -> bool {
        self.contains(Self::MASK) || self.product & PRODUCT_CASTS != 0
    }

    /// Whether the vertices move with presentation time, so shadow maps
    /// redraw as it moves: the wind, or a product shader's displace stage.
    pub fn moves_with_time(self) -> bool {
        self.contains(Self::WIND) || self.product & PRODUCT_DISPLACES != 0
    }

    /// Whether a material can compile this set (render-wgpu `apply.rs`
    /// `MaterialParams::features` and the mesh features): the exhaustive
    /// tests compose every set that can occur. An unlit material reads no
    /// map and shades neither flat nor layered; a material has an occlusion
    /// map or an ORM map; flat shading drops the normal map; terrain layers
    /// need a voxel surface; water is blended (never masked) and reads its
    /// normal maps itself.
    #[cfg(test)]
    fn occurs(self) -> bool {
        let unlit_never = Self::NORMAL_MAP
            | Self::EMISSIVE_MAP
            | Self::OCCLUSION_MAP
            | Self::ORM_MAP
            | Self::FLAT_SHADING
            | Self::TERRAIN_LAYERS;
        // `feature` rules `never` out.
        let excluded =
            |feature: Self, never: Self| self.contains(feature) && self.bits & never.bits != 0;
        let layers_without_surface =
            self.contains(Self::TERRAIN_LAYERS) && !self.contains(Self::VOXEL_SURFACE);
        let exclusions = [
            excluded(Self::UNLIT, unlit_never),
            excluded(Self::OCCLUSION_MAP, Self::ORM_MAP),
            excluded(Self::FLAT_SHADING, Self::NORMAL_MAP),
            layers_without_surface,
            excluded(Self::WATER, Self::MASK | Self::NORMAL_MAP),
        ];
        !exclusions.contains(&true)
    }

    fn defs(self) -> HashMap<String, ShaderDefValue> {
        let mut defs: HashMap<String, ShaderDefValue> = Self::DEFS
            .iter()
            .filter(|(feature, _)| self.contains(*feature))
            .map(|(_, name)| (name.to_string(), ShaderDefValue::Bool(true)))
            .collect();
        if self.product != 0 {
            defs.insert("PRODUCT_SHADER".to_string(), ShaderDefValue::Bool(true));
        }
        if self.product & PRODUCT_CASTS != 0 {
            defs.insert("PRODUCT_CASTS".to_string(), ShaderDefValue::Bool(true));
        }
        if self.product & PRODUCT_DISPLACES != 0 {
            defs.insert("PRODUCT_DISPLACES".to_string(), ShaderDefValue::Bool(true));
        }
        // The shadow pass has a fragment stage only to discard.
        if self.caster_fragment() {
            defs.insert("CASTER_FRAGMENT".to_string(), ShaderDefValue::Bool(true));
        }
        defs
    }
}

impl BitOr for Features {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
            product: self.product.max(other.product),
        }
    }
}

/// Importable modules, each after the modules it imports.
const MODULES: [(&str, &str); 10] = [
    ("shaders/types.wgsl", include_str!("shaders/types.wgsl")),
    ("shaders/view.wgsl", include_str!("shaders/view.wgsl")),
    ("shaders/clouds.wgsl", include_str!("shaders/clouds.wgsl")),
    (
        "shaders/material.wgsl",
        include_str!("shaders/material.wgsl"),
    ),
    ("shaders/wind.wgsl", include_str!("shaders/wind.wgsl")),
    ("shaders/surface.wgsl", include_str!("shaders/surface.wgsl")),
    (
        "shaders/lighting.wgsl",
        include_str!("shaders/lighting.wgsl"),
    ),
    ("shaders/tonemap.wgsl", include_str!("shaders/tonemap.wgsl")),
    ("shaders/finish.wgsl", include_str!("shaders/finish.wgsl")),
    ("shaders/shade.wgsl", include_str!("shaders/shade.wgsl")),
];

/// The import path a product shader's module takes.
const PRODUCT_MODULE: &str = "rusty::product";

/// Set in a product id whose shader defines `cast_shadow`, so `Features::caster`
/// keeps it.
const PRODUCT_CASTS: u32 = 1 << 31;
/// Set in a product id whose shader defines `displace`, so `Features::caster`
/// keeps it and the shadow maps follow its vertices.
const PRODUCT_DISPLACES: u32 = 1 << 30;
/// The product id's flags; the rest is its index + 1.
const PRODUCT_FLAGS: u32 = PRODUCT_CASTS | PRODUCT_DISPLACES;

/// Shader defs a product keyword may not take.
const RESERVED_DEFS: [&str; 4] = [
    "PRODUCT_SHADER",
    "PRODUCT_CASTS",
    "PRODUCT_DISPLACES",
    "CASTER_FRAGMENT",
];

#[derive(Clone, Copy, Debug)]
pub enum Entry {
    World,
    Sky,
    Shadow,
    Effects,
    Ghost,
    Compose,
    /// Screen-space ambient occlusion over a view's depth: compute and
    /// raster paths, and the blur.
    AmbientOcclusion,
    /// The finish pass over a view's HDR world (`finish_pass.wgsl`).
    Finish,
    /// Bloom and auto exposure from a view's HDR world (`post.wgsl`).
    Post,
    /// The sky's light: the background prefiltered into a cube and its
    /// irradiance harmonics (`sky_light.wgsl`, compute).
    SkyLight,
    /// Light rows binned into a view's cluster grid.
    LightClusters,
    /// A view's opaque candidates culled into indirect draws.
    Cull,
    /// Distance-field ambient occlusion: cone traces through the chunk
    /// field atlas over a view's depth.
    DistanceField,
    /// The opaque pass's depth copied for the blend pass's water surfaces
    /// (`water_depth.wgsl`).
    WaterDepth,
    /// A shadow layer's static depth restored from its cache
    /// (`shadow_restore.wgsl`).
    ShadowRestore,
}

impl Entry {
    fn source(self) -> (&'static str, &'static str) {
        match self {
            Self::World => ("shaders/world.wgsl", include_str!("shaders/world.wgsl")),
            Self::Sky => ("shaders/sky.wgsl", include_str!("shaders/sky.wgsl")),
            Self::Shadow => ("shaders/shadow.wgsl", include_str!("shaders/shadow.wgsl")),
            Self::Effects => ("shaders/effects.wgsl", include_str!("shaders/effects.wgsl")),
            Self::Ghost => ("shaders/ghost.wgsl", include_str!("shaders/ghost.wgsl")),
            Self::Compose => ("shaders/compose.wgsl", include_str!("shaders/compose.wgsl")),
            Self::AmbientOcclusion => (
                "shaders/ambient_occlusion.wgsl",
                include_str!("shaders/ambient_occlusion.wgsl"),
            ),
            Self::Finish => (
                "shaders/finish_pass.wgsl",
                include_str!("shaders/finish_pass.wgsl"),
            ),
            Self::Post => ("shaders/post.wgsl", include_str!("shaders/post.wgsl")),
            Self::SkyLight => (
                "shaders/sky_light.wgsl",
                include_str!("shaders/sky_light.wgsl"),
            ),
            Self::LightClusters => (
                "shaders/light_clusters.wgsl",
                include_str!("shaders/light_clusters.wgsl"),
            ),
            Self::Cull => ("shaders/cull.wgsl", include_str!("shaders/cull.wgsl")),
            Self::WaterDepth => (
                "shaders/water_depth.wgsl",
                include_str!("shaders/water_depth.wgsl"),
            ),
            Self::ShadowRestore => (
                "shaders/shadow_restore.wgsl",
                include_str!("shaders/shadow_restore.wgsl"),
            ),
            Self::DistanceField => (
                "shaders/distance_field.wgsl",
                include_str!("shaders/distance_field.wgsl"),
            ),
        }
    }
}

/// A product shader's WGSL, named by the content path its errors cite, and
/// the keywords (its own shader defs) it is compiled with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductShader {
    pub path: String,
    pub source: String,
    pub keywords: Vec<String>,
}

impl ProductShader {
    /// Whether it defines a caster stage, `fn cast_shadow`, under its
    /// keywords.
    fn casts(&self) -> bool {
        self.defines("cast_shadow")
    }

    /// Whether it defines a displace stage, `fn displace`, under its
    /// keywords.
    fn displaces(&self) -> bool {
        self.defines("displace")
    }

    /// Whether it defines `fn name` under its keywords: naga_oil's own
    /// comment stripping, preprocessor and tokenizer, so any legal spelling
    /// counts. A shader that does not preprocess fails composition instead.
    fn defines(&self, name: &str) -> bool {
        let mut lines = self.source.lines();
        let code: Vec<_> = lines.replace_comments().collect();
        let defs = self
            .keywords
            .iter()
            .map(|keyword| (keyword.clone(), ShaderDefValue::Bool(true)))
            .collect();
        let Ok(output) = Preprocessor::default().preprocess(&code.join("\n"), &defs) else {
            return false;
        };
        let tokens: Vec<_> = Tokenizer::new(&output.preprocessed_source, false).collect();
        tokens
            .windows(2)
            .any(|pair| pair[0].identifier() == Some("fn") && pair[1].identifier() == Some(name))
    }
}

/// A keyword is an upper-case shader def name the standard family does not
/// use: `[A-Z_][A-Z0-9_]*`.
pub fn check_keyword(keyword: &str) -> Result<(), String> {
    let mut characters = keyword.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase() || first == '_')
        && characters.all(|rest| rest.is_ascii_uppercase() || rest.is_ascii_digit() || rest == '_');
    if !valid {
        return Err(format!(
            "shader keyword `{keyword}` must be upper case: [A-Z_][A-Z0-9_]*"
        ));
    }
    if Features::DEFS.iter().any(|(_, name)| *name == keyword) || RESERVED_DEFS.contains(&keyword) {
        return Err(format!(
            "shader keyword `{keyword}` is a standard shader feature"
        ));
    }
    Ok(())
}

pub struct Shaders {
    composer: Composer,
    /// Product shaders by id - 1; the module last added as `rusty::product`.
    products: Vec<ProductShader>,
    current: Option<u32>,
}

impl Default for Shaders {
    fn default() -> Self {
        Self::new()
    }
}

impl Shaders {
    pub fn new() -> Self {
        let mut composer = Composer::default();
        for (path, source) in MODULES {
            if let Err(error) = composer.add_composable_module(ComposableModuleDescriptor {
                source,
                file_path: path,
                language: ShaderLanguage::Wgsl,
                ..Default::default()
            }) {
                panic!("{}", error.emit_to_string(&composer));
            }
        }
        Self {
            composer,
            products: Vec::new(),
            current: None,
        }
    }

    /// The id `Features::with_product` takes for `shader`: the same id for
    /// the same path, source and keywords, so materials sharing a shader
    /// batch.
    pub fn product(&mut self, shader: ProductShader) -> u32 {
        let flags = if shader.casts() { PRODUCT_CASTS } else { 0 }
            | if shader.displaces() {
                PRODUCT_DISPLACES
            } else {
                0
            };
        let index = match self.products.iter().position(|known| *known == shader) {
            Some(index) => index,
            None => {
                self.products.push(shader);
                self.products.len() - 1
            }
        };
        (index as u32 + 1) | flags
    }

    /// An entry shader compiled with `features`. A product shader that does
    /// not compose under them is an error naming its file and line; the
    /// standard family always composes.
    pub fn compose(&mut self, entry: Entry, features: Features) -> Result<naga::Module, String> {
        let index = (features.product & !PRODUCT_FLAGS) as usize;
        if features.product != 0 && self.current != Some(features.product) {
            let shader = &self.products[index - 1];
            self.current = None;
            let added = self
                .composer
                .add_composable_module(ComposableModuleDescriptor {
                    source: &shader.source,
                    file_path: &shader.path,
                    language: ShaderLanguage::Wgsl,
                    as_name: Some(PRODUCT_MODULE.to_string()),
                    ..Default::default()
                })
                .map(|_| ());
            added.map_err(|error| error.emit_to_string(&self.composer))?;
            self.current = Some(features.product);
        }
        let (path, source) = entry.source();
        let mut defs = features.defs();
        if index != 0 {
            for keyword in &self.products[index - 1].keywords {
                defs.insert(keyword.clone(), ShaderDefValue::Bool(true));
            }
        }
        let module = self
            .composer
            .make_naga_module(NagaModuleDescriptor {
                source,
                file_path: path,
                shader_type: ShaderType::Wgsl,
                shader_defs: defs,
                ..Default::default()
            })
            .map_err(|error| error.emit_to_string(&self.composer));
        if features.product == 0 {
            return Ok(module.unwrap_or_else(|error| panic!("{error}")));
        }
        module
    }
}

/// Check a product shader before it is admitted: its keywords, and that it
/// composes into the world pass (and the shadow pass, if it defines a
/// caster) with the standard features a plain material has. The error names
/// its file and line.
pub fn check_product_shader(path: &str, source: &str, keywords: &[String]) -> Result<(), String> {
    for keyword in keywords {
        check_keyword(keyword)?;
    }
    let mut shaders = Shaders::new();
    let product = shaders.product(ProductShader {
        path: path.to_string(),
        source: source.to_string(),
        keywords: keywords.to_vec(),
    });
    let features = Features::default().with_product(product);
    shaders.compose(Entry::World, features)?;
    if features.caster().product() != 0 {
        shaders.compose(Entry::Shadow, features.caster())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `check` over every standard feature set that can occur
    /// (`Features::occurs`), the sets split across the cores, each thread
    /// with its own `Shaders` that `prepare` readies (registering the
    /// product shaders the check composes with).
    fn every_feature_set<T: Send>(
        prepare: impl Fn(&mut Shaders) -> T + Sync,
        check: impl Fn(&mut Shaders, &T, Features) + Sync,
    ) {
        let sets = Features::ALL_BITS as usize + 1;
        let threads = std::thread::available_parallelism()
            .map_or(1, |cores| cores.get())
            .min(16);
        let per_thread = sets.div_ceil(threads);
        std::thread::scope(|scope| {
            for chunk in 0..threads {
                let (prepare, check) = (&prepare, &check);
                scope.spawn(move || {
                    let mut shaders = Shaders::new();
                    let state = prepare(&mut shaders);
                    let start = chunk * per_thread;
                    for bits in start..(start + per_thread).min(sets) {
                        let features = Features::bit(bits as u16);
                        if features.occurs() {
                            check(&mut shaders, &state, features);
                        }
                    }
                });
            }
        });
    }

    /// The struct members and array elements of `module` that are 2-row
    /// matrices. naga's HLSL backend wraps those in a struct, which FXC (the
    /// DX12 compiler) will not convert to and from the matrix (#9674), so a
    /// shader that compiles on Vulkan fails on Windows.
    fn fxc_wrapped_matrices(module: &naga::Module) -> Vec<String> {
        let two_rows = |ty: naga::Handle<naga::Type>| {
            matches!(
                module.types[ty].inner,
                naga::TypeInner::Matrix {
                    rows: naga::VectorSize::Bi,
                    ..
                }
            )
        };
        let mut found = Vec::new();
        for (_, ty) in module.types.iter() {
            let name = ty.name.clone().unwrap_or_default();
            match &ty.inner {
                naga::TypeInner::Struct { members, .. } => {
                    for member in members.iter().filter(|member| two_rows(member.ty)) {
                        found.push(format!(
                            "{name}.{}",
                            member.name.clone().unwrap_or_default()
                        ));
                    }
                }
                naga::TypeInner::Array { base, .. } if two_rows(*base) => {
                    found.push(format!("an array of 2-row matrices {name}"));
                }
                _ => {}
            }
        }
        found
    }

    /// `module` written through naga's HLSL backend, as wgpu's DX12 backend
    /// writes it before FXC compiles it.
    fn write_hlsl(module: &naga::Module) -> Result<(), String> {
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(module)
        .map_err(|error| format!("{error:?}"))?;
        // Overridable constants take their defaults, as a pipeline that
        // sets none of them gets.
        let (module, info) = naga::back::pipeline_constants::process_overrides(
            module,
            &info,
            None,
            &Default::default(),
        )
        .map_err(|error| error.to_string())?;
        let mut hlsl = String::new();
        naga::back::hlsl::Writer::new(
            &mut hlsl,
            &naga::back::hlsl::Options::default(),
            &naga::back::hlsl::PipelineOptions::default(),
        )
        .write(&module, &info, None)
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    /// Every entry composes and validates under every feature set it can be
    /// compiled with, without a device, writes as HLSL (the DX12 path), and
    /// holds no type FXC rejects.
    #[test]
    fn every_entry_composes_under_every_feature_set() {
        let compose = |shaders: &mut Shaders, entry: Entry, features: Features| {
            let (path, source) = entry.source();
            match shaders.composer.make_naga_module(NagaModuleDescriptor {
                source,
                file_path: path,
                shader_type: ShaderType::Wgsl,
                shader_defs: features.defs(),
                ..Default::default()
            }) {
                Ok(module) => {
                    let wrapped = fxc_wrapped_matrices(&module);
                    assert!(
                        wrapped.is_empty(),
                        "{entry:?} {features:?}: FXC cannot compile {wrapped:?}"
                    );
                    write_hlsl(&module)
                        .unwrap_or_else(|error| panic!("{entry:?} {features:?}: HLSL: {error}"));
                }
                Err(error) => panic!(
                    "{entry:?} {features:?}: {}",
                    error.emit_to_string(&shaders.composer)
                ),
            }
        };
        every_feature_set(
            |_| (),
            |shaders, (), features| {
                compose(shaders, Entry::World, features);
                compose(shaders, Entry::Shadow, features.caster());
            },
        );
        let mut shaders = Shaders::new();
        for entry in [
            Entry::Sky,
            Entry::Effects,
            Entry::Ghost,
            Entry::Compose,
            Entry::AmbientOcclusion,
            Entry::Finish,
            Entry::Post,
            Entry::SkyLight,
            Entry::LightClusters,
            Entry::Cull,
            Entry::DistanceField,
            Entry::WaterDepth,
            Entry::ShadowRestore,
        ] {
            compose(&mut shaders, entry, Features::default());
        }
        assert!(Features::WATER.occurs() && Features::default().occurs());
        assert!(!(Features::WATER | Features::MASK).occurs());
        assert!(!(Features::UNLIT | Features::NORMAL_MAP).occurs());
        assert!((Features::UNLIT | Features::VERTEX_TANGENTS | Features::WIND).occurs());
    }

    fn rim_shader() -> ProductShader {
        ProductShader {
            path: "shaders/rim.wgsl".to_string(),
            source: RIM.to_string(),
            keywords: Vec::new(),
        }
    }

    const RIM: &str = "#import rusty::types::Surface
#import rusty::shade::standard_shade
#import rusty::view::frame
#import rusty::material::material

fn shade(surface: Surface) -> vec4<f32> {
    let view = normalize(frame.camera.xyz - surface.world_position);
    let rim = pow(1.0 - max(dot(surface.normal, view), 0.0), 3.0);
    let shaded = standard_shade(surface);
#ifdef NORMAL_MAP
    let strength = material.parameters[0].w;
#else
    let strength = 1.0;
#endif
    return vec4<f32>(shaded.rgb + material.parameters[0].rgb * rim * strength, shaded.a);
}
";

    #[test]
    fn a_product_shader_composes_under_the_standard_features_and_its_errors_name_its_line() {
        check_product_shader("shaders/rim.wgsl", RIM, &[]).unwrap();
        let mut shaders = Shaders::new();
        let rim = shaders.product(rim_shader());
        assert_eq!(shaders.product(rim_shader()), rim, "one id for one shader");
        every_feature_set(
            |shaders| shaders.product(rim_shader()),
            |shaders, rim, features| {
                if let Err(error) = shaders.compose(Entry::World, features.with_product(*rim)) {
                    panic!("{features:?}: {error}");
                }
            },
        );
        // The standard family still composes after a product module.
        shaders.compose(Entry::World, Features::NORMAL_MAP).unwrap();

        let broken = RIM.replace("pow(1.0 -", "pow(missing -");
        let error = check_product_shader("shaders/broken.wgsl", &broken, &[]).unwrap_err();
        assert!(error.contains("shaders/broken.wgsl"), "{error}");
        assert!(error.contains(":8:"), "{error}");
        let error = check_product_shader("shaders/empty.wgsl", "fn other() {}", &[]).unwrap_err();
        assert!(error.contains("shade"), "{error}");
    }

    const DISSOLVE: &str = "#import rusty::types::{Surface, Caster}
#import rusty::material::{material, product_map_a, product_sampler_a}
#import rusty::view::frame
#import rusty::shade::standard_shade

fn dissolved(uv: vec2<f32>) -> bool {
    let noise = textureSampleLevel(product_map_a, product_sampler_a, uv, 0.0).r;
    return noise < fract(frame.time.x * material.parameters[0].x);
}

fn shade(surface: Surface) -> vec4<f32> {
#ifdef DISSOLVE
    if dissolved(surface.uv) {
        discard;
    }
#endif
#ifdef BROKEN
    return missing;
#else
    return standard_shade(surface);
#endif
}

// A shadow that follows the discard.
fn cast_shadow(caster: Caster) {
#ifdef DISSOLVE
    if dissolved(caster.uv) {
        discard;
    }
#endif
}
";

    #[test]
    fn keywords_select_a_product_variant_and_a_caster_stage_composes_into_the_shadow_pass() {
        let keywords = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        check_product_shader("shaders/dissolve.wgsl", DISSOLVE, &keywords(&["DISSOLVE"])).unwrap();
        check_product_shader("shaders/dissolve.wgsl", DISSOLVE, &[]).unwrap();
        let error = check_product_shader("shaders/dissolve.wgsl", DISSOLVE, &keywords(&["BROKEN"]))
            .unwrap_err();
        assert!(error.contains("shaders/dissolve.wgsl:18:"), "{error}");
        for refused in ["NORMAL_MAP", "PRODUCT_SHADER", "lower", "1ST", ""] {
            assert!(check_keyword(refused).is_err(), "{refused}");
        }

        let mut shaders = Shaders::new();
        let dissolve = shaders.product(ProductShader {
            path: "shaders/dissolve.wgsl".to_string(),
            source: DISSOLVE.to_string(),
            keywords: keywords(&["DISSOLVE"]),
        });
        let plain = shaders.product(ProductShader {
            path: "shaders/dissolve.wgsl".to_string(),
            source: DISSOLVE.to_string(),
            keywords: Vec::new(),
        });
        assert_ne!(dissolve, plain, "keywords make a separate variant");
        let features = Features::MASK.with_product(dissolve);
        assert_eq!(
            features.caster().product(),
            dissolve,
            "a caster stage keeps the product"
        );
        let rim = shaders.product(rim_shader());
        assert_eq!(
            Features::MASK.with_product(rim).caster(),
            Features::MASK,
            "no caster stage"
        );
        for names in [&["DISSOLVE"][..], &[]] {
            every_feature_set(
                |shaders| {
                    shaders.product(ProductShader {
                        path: "shaders/dissolve.wgsl".to_string(),
                        source: DISSOLVE.to_string(),
                        keywords: keywords(names),
                    })
                },
                |shaders, product, bits| {
                    let features = bits.with_product(*product);
                    if let Err(error) = shaders.compose(Entry::World, features) {
                        panic!("{features:?}: {error}");
                    }
                    if let Err(error) = shaders.compose(Entry::Shadow, features.caster()) {
                        panic!("{features:?}: {error}");
                    }
                },
            );
        }

        // Any legal spelling of the declaration is a caster stage; one in a
        // comment, or under a keyword not given, is not.
        let casts = |source: &str, names: &[&str]| {
            let product = Shaders::new().product(ProductShader {
                path: "shaders/dissolve.wgsl".to_string(),
                source: source.to_string(),
                keywords: keywords(names),
            });
            Features::default().with_product(product).caster().product() != 0
        };
        assert!(casts(
            &DISSOLVE.replace("fn cast_shadow(", "fn\n    cast_shadow ("),
            &[]
        ));
        assert!(!casts(
            &format!("{RIM}/*\nfn cast_shadow(caster: Caster) {{}}\n*/\n"),
            &[]
        ));
        let optional =
            DISSOLVE.replace("fn cast_shadow(", "#ifdef CASTS\nfn cast_shadow(") + "#endif\n";
        assert!(casts(&optional, &["CASTS"]));
        assert!(!casts(&optional, &[]));
        check_product_shader("shaders/dissolve.wgsl", &optional, &[]).unwrap();
    }

    const WAVING: &str = "#import rusty::types::{Surface, Vertex}
#import rusty::view::frame
#import rusty::material::material
#import rusty::shade::standard_shade

fn displace(vertex: Vertex) -> vec3<f32> {
    let lift = sin(frame.time.x * material.parameters[0].x + vertex.uv.x) * vertex.color.a;
    return vertex.world_position + vertex.world_normal * lift;
}

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}
";

    /// A displace stage keeps its product in the caster pass (the shadow
    /// follows the vertices) without a caster fragment stage, composes in
    /// the world and shadow passes under every feature set, and moves with
    /// time.
    #[test]
    fn a_displace_stage_places_vertices_in_both_passes() {
        let mut shaders = Shaders::new();
        let waving = shaders.product(ProductShader {
            path: "shaders/waving.wgsl".to_string(),
            source: WAVING.to_string(),
            keywords: Vec::new(),
        });
        let features = Features::MASK.with_product(waving);
        assert_eq!(
            features.caster().product(),
            waving,
            "kept for the shadow pass"
        );
        assert!(features.caster().moves_with_time());
        assert!(features.caster().caster_fragment(), "the mask discards");
        let plain = Features::default().with_product(waving);
        assert!(!plain.caster().caster_fragment(), "nothing to discard");
        assert!(plain.caster().moves_with_time());
        assert!(
            !Features::default().caster().moves_with_time(),
            "a standard material stands"
        );
        assert!(Features::WIND.caster().moves_with_time(), "the wind moves");
        every_feature_set(
            |shaders| {
                shaders.product(ProductShader {
                    path: "shaders/waving.wgsl".to_string(),
                    source: WAVING.to_string(),
                    keywords: Vec::new(),
                })
            },
            |shaders, waving, bits| {
                let features = bits.with_product(*waving);
                if let Err(error) = shaders.compose(Entry::World, features) {
                    panic!("{features:?}: {error}");
                }
                if let Err(error) = shaders.compose(Entry::Shadow, features.caster()) {
                    panic!("{features:?}: {error}");
                }
            },
        );
        check_product_shader("shaders/waving.wgsl", WAVING, &[]).unwrap();
        let both = format!("{WAVING}\nfn cast_shadow(caster: Caster) {{}}\n")
            .replace("{Surface, Vertex}", "{Surface, Vertex, Caster}");
        check_product_shader("shaders/both.wgsl", &both, &[]).unwrap();
        for reserved in ["PRODUCT_CASTS", "PRODUCT_DISPLACES"] {
            assert!(check_keyword(reserved).is_err(), "{reserved} is reserved");
        }
    }
}
