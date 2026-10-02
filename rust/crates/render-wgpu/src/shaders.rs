//! The standard shader family (`src/shaders`): WGSL modules composed with
//! naga_oil, and the features a material compiles into its variant.
//!
//! `types`, `view`, `material`, `surface`, `lighting`, `tonemap` and `finish`
//! are importable modules (`#import rusty::lighting::standard_radiance`);
//! `world`, `sky`, `shadow`, `effects`, `ghost` and `compose` are the entry
//! shaders built from them.
//! A material's features are shader defs, so a variant only carries the
//! samples and branches its material uses.

use std::collections::HashMap;
use std::ops::BitOr;

use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage,
    ShaderType,
};

/// The standard shader features a material compiles in, derived from what it
/// contains (`MaterialParams::features`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Features(u8);

impl Features {
    pub const UNLIT: Self = Self(1);
    pub const MASK: Self = Self(2);
    pub const VOXEL_SURFACE: Self = Self(4);
    pub const NORMAL_MAP: Self = Self(8);
    pub const EMISSIVE_MAP: Self = Self(16);
    pub const OCCLUSION_MAP: Self = Self(32);
    /// The mesh's second stream: tangents and a second uv set (a mesh
    /// feature, not a material one).
    pub const VERTEX_TANGENTS: Self = Self(64);
    pub const TRIPLANAR: Self = Self(128);

    const DEFS: [(Self, &'static str); 8] = [
        (Self::UNLIT, "UNLIT"),
        (Self::MASK, "MASK"),
        (Self::VOXEL_SURFACE, "VOXEL_SURFACE"),
        (Self::NORMAL_MAP, "NORMAL_MAP"),
        (Self::EMISSIVE_MAP, "EMISSIVE_MAP"),
        (Self::OCCLUSION_MAP, "OCCLUSION_MAP"),
        (Self::VERTEX_TANGENTS, "VERTEX_TANGENTS"),
        (Self::TRIPLANAR, "TRIPLANAR"),
    ];

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// `self` with `other` added when `on`.
    pub fn with(self, other: Self, on: bool) -> Self {
        if on {
            self | other
        } else {
            self
        }
    }

    /// What the shadow caster pass compiles: only the alpha mask and the
    /// voxel uv remap and triplanar planes it samples through.
    pub fn caster(self) -> Self {
        Self(self.0 & (Self::MASK.0 | Self::VOXEL_SURFACE.0 | Self::TRIPLANAR.0))
    }

    fn defs(self) -> HashMap<String, ShaderDefValue> {
        Self::DEFS
            .iter()
            .filter(|(feature, _)| self.contains(*feature))
            .map(|(_, name)| (name.to_string(), ShaderDefValue::Bool(true)))
            .collect()
    }
}

impl BitOr for Features {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Importable modules, each after the modules it imports.
const MODULES: [(&str, &str); 7] = [
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
];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Entry {
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

pub(crate) struct Shaders {
    composer: Composer,
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
        Self { composer }
    }

    /// An entry shader compiled with `features`.
    pub fn module(
        &mut self,
        device: &wgpu::Device,
        entry: Entry,
        features: Features,
    ) -> wgpu::ShaderModule {
        let (path, source) = entry.source();
        let module = self
            .composer
            .make_naga_module(NagaModuleDescriptor {
                source,
                file_path: path,
                shader_type: ShaderType::Wgsl,
                shader_defs: features.defs(),
                ..Default::default()
            })
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&self.composer)));
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(path),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry composes and validates under every feature set it can be
    /// compiled with, without a device.
    #[test]
    fn every_entry_composes_under_every_feature_set() {
        let mut shaders = Shaders::new();
        let all = (0..=u8::MAX).map(Features);
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
}
