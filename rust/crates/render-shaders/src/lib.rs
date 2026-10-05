//! The standard shader family (`src/shaders`): WGSL modules composed with
//! naga_oil, the features a material compiles into its variant, and product
//! shaders that take over its last stage.
//!
//! `types`, `view`, `material`, `surface`, `lighting`, `tonemap`, `finish`
//! and `shade` are importable modules (`#import rusty::lighting::standard_radiance`);
//! `world`, `sky`, `shadow`, `effects`, `ghost` and `compose` are the entry
//! shaders built from them.
//! A material's features are shader defs, so a variant only carries the
//! samples and branches its material uses.
//!
//! A product shader is one WGSL module imported as `rusty::product`. It
//! defines `fn shade(surface: Surface) -> vec4<f32>`, which the world pass
//! calls in place of `rusty::shade::standard_shade` with the surface the
//! standard stages computed, and may define `fn cast_shadow(caster: Caster)`, which
//! the shadow pass calls (and which may discard). It may import any module
//! above, and read the standard feature defs (`#ifdef NORMAL_MAP`) and its
//! own keywords, chosen when it is opened.

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
    /// Every standard feature's bit.
    #[cfg(test)]
    const ALL_BITS: u16 = 2047;

    const DEFS: [(Self, &'static str); 11] = [
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
    /// uv remap, triplanar planes and hex tiles it samples through and whether vertex
    /// colours are layer weights rather than alpha, and a product shader that
    /// defines a caster stage.
    pub fn caster(self) -> Self {
        Self {
            bits: self.bits
                & (Self::MASK.bits
                    | Self::VOXEL_SURFACE.bits
                    | Self::TRIPLANAR.bits
                    | Self::STOCHASTIC_TILING.bits
                    | Self::LAYER_WEIGHTS.bits),
            product: if self.product & PRODUCT_CASTS != 0 {
                self.product
            } else {
                0
            },
        }
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
        // The shadow pass has a fragment stage only to discard.
        if self.contains(Self::MASK) || self.product != 0 {
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
const MODULES: [(&str, &str); 8] = [
    ("shaders/types.wgsl", include_str!("shaders/types.wgsl")),
    ("shaders/view.wgsl", include_str!("shaders/view.wgsl")),
    (
        "shaders/material.wgsl",
        include_str!("shaders/material.wgsl"),
    ),
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

/// Shader defs a product keyword may not take.
const RESERVED_DEFS: [&str; 2] = ["PRODUCT_SHADER", "CASTER_FRAGMENT"];

#[derive(Clone, Copy, Debug)]
pub enum Entry {
    World,
    Sky,
    Shadow,
    Effects,
    Ghost,
    Compose,
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
    /// keywords: naga_oil's own comment stripping, preprocessor and
    /// tokenizer, so any legal spelling counts. A shader that does not
    /// preprocess fails composition instead.
    fn casts(&self) -> bool {
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
        tokens.windows(2).any(|pair| {
            pair[0].identifier() == Some("fn") && pair[1].identifier() == Some("cast_shadow")
        })
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
        let casts = shader.casts();
        let index = match self.products.iter().position(|known| *known == shader) {
            Some(index) => index,
            None => {
                self.products.push(shader);
                self.products.len() - 1
            }
        };
        (index as u32 + 1) | if casts { PRODUCT_CASTS } else { 0 }
    }

    /// An entry shader compiled with `features`. A product shader that does
    /// not compose under them is an error naming its file and line; the
    /// standard family always composes.
    pub fn compose(&mut self, entry: Entry, features: Features) -> Result<naga::Module, String> {
        let index = (features.product & !PRODUCT_CASTS) as usize;
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

    /// Every entry composes and validates under every feature set it can be
    /// compiled with, without a device.
    #[test]
    fn every_entry_composes_under_every_feature_set() {
        let mut shaders = Shaders::new();
        let all = (0..=Features::ALL_BITS).map(Features::bit);
        let mut compose = |entry: Entry, features: Features| {
            let (path, source) = entry.source();
            if let Err(error) = shaders.composer.make_naga_module(NagaModuleDescriptor {
                source,
                file_path: path,
                shader_type: ShaderType::Wgsl,
                shader_defs: features.defs(),
                ..Default::default()
            }) {
                panic!(
                    "{entry:?} {features:?}: {}",
                    error.emit_to_string(&shaders.composer)
                );
            }
        };
        for features in all {
            compose(Entry::World, features);
            compose(Entry::Shadow, features.caster());
        }
        for entry in [Entry::Sky, Entry::Effects, Entry::Ghost, Entry::Compose] {
            compose(entry, Features::default());
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
        let rim = shaders.product(ProductShader {
            path: "shaders/rim.wgsl".to_string(),
            source: RIM.to_string(),
            keywords: Vec::new(),
        });
        assert_eq!(
            shaders.product(ProductShader {
                path: "shaders/rim.wgsl".to_string(),
                source: RIM.to_string(),
                keywords: Vec::new(),
            }),
            rim,
            "one id for one shader"
        );
        for features in (0..=Features::ALL_BITS).map(Features::bit) {
            if let Err(error) = shaders.compose(Entry::World, features.with_product(rim)) {
                panic!("{features:?}: {error}");
            }
        }
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
        let rim = shaders.product(ProductShader {
            path: "shaders/rim.wgsl".to_string(),
            source: RIM.to_string(),
            keywords: Vec::new(),
        });
        assert_eq!(
            Features::MASK.with_product(rim).caster(),
            Features::MASK,
            "no caster stage"
        );
        for bits in (0..=Features::ALL_BITS).map(Features::bit) {
            for product in [dissolve, plain] {
                let features = bits.with_product(product);
                if let Err(error) = shaders.compose(Entry::World, features) {
                    panic!("{features:?}: {error}");
                }
                if let Err(error) = shaders.compose(Entry::Shadow, features.caster()) {
                    panic!("{features:?}: {error}");
                }
            }
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
}
