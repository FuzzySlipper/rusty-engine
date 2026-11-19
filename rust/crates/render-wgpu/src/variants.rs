//! A product shader's variants made on the preparation workers (#10913).
//! Composing a product shader into the world and caster entries and making
//! their pipelines costs tens of milliseconds a variant with a warm driver
//! cache and up to seconds with a cold one, so it never runs where a call
//! is applied or a frame drawn: a material or part that needs a variant not
//! yet made asks for it here (`Renderer::request_variant`), and what draws
//! it draws nothing until a later committed call takes it in
//! (`prepare.rs`). The worker composes with its own family, as the
//! renderer's composes; the modules and pipelines it makes are those
//! `Layouts` would have made, and are kept as if it had.

use std::cell::RefCell;

use crate::batch::Pass;
use crate::pipelines::{compiled, Layouts, Pipelines, ShadowPipelines, VariantLayouts};
use crate::shaders::{Composing, Entry, Features, ProductShader, Shaders};
use crate::target::ColorTarget;

/// What a variant is wanted for. Either makes its pipelines on each target
/// the world draws into, its casters when shadows may draw, and its
/// pre-pass pipelines once that draws, so a part drawing a material made
/// ahead of it draws at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wanted {
    /// A material defined with these features (`Renderer::prepare_material`):
    /// its world entry too, so its error is known.
    Material,
    /// A part drawing them (`Layouts::note_variant`).
    Part,
}

/// One module or pipeline a variant still needs.
enum Need {
    /// The world entry compiled with these features.
    Module(Features),
    World {
        set: usize,
        target: ColorTarget,
        features: Features,
        pass: Pass,
    },
    Caster {
        features: Features,
        cull: Option<wgpu::Face>,
    },
    DepthOnly {
        features: Features,
        pass: Pass,
        samples: u32,
    },
}

/// What a worker makes a variant from: what it needs, the modules made
/// already that it reuses, and the shader its features name.
pub(crate) struct VariantWork {
    needs: Vec<Need>,
    world_shaders: Vec<(Features, wgpu::ShaderModule)>,
    shadow_shaders: Vec<(Features, wgpu::ShaderModule)>,
    layouts: VariantLayouts,
    composing: Composing,
    product: ProductShader,
}

/// What a worker made, to keep as `Layouts` keeps what it makes.
#[derive(Default)]
pub(crate) struct MadeVariant {
    world_shaders: Vec<(Features, wgpu::ShaderModule)>,
    shadow_shaders: Vec<(Features, wgpu::ShaderModule)>,
    world: Vec<(usize, ColorTarget, (Features, Pass), wgpu::RenderPipeline)>,
    casters: Vec<((Features, Option<wgpu::Face>), wgpu::RenderPipeline)>,
    depth_only: Vec<((Features, Pass, u32), wgpu::RenderPipeline)>,
    /// The shader did not compose under some of the features; those draw
    /// with the standard shade stage. Each names its file and line.
    pub errors: Vec<String>,
}

impl Layouts {
    /// What drawing `features` in `passes` for `wanted` needs that is not
    /// made yet; nothing for a standard variant, which is made where it is
    /// needed.
    pub(crate) fn variant_work(
        &self,
        sets: &[Pipelines],
        features: Features,
        passes: &[Pass],
        wanted: Wanted,
        shadows: bool,
    ) -> Option<VariantWork> {
        if features.product() == 0 {
            return None;
        }
        let mut needs = Vec::new();
        if wanted == Wanted::Material && !self.world_shaders.contains_key(&features) {
            needs.push(Need::Module(features));
        }
        let (casters, prepass) = (shadows || self.variants.casters, self.variants.prepass);
        for &pass in passes {
            for (set, pipelines) in sets.iter().enumerate() {
                if pipelines.draws_world && !pipelines.world.contains_key(&(features, pass)) {
                    needs.push(Need::World {
                        set,
                        target: pipelines.target,
                        features,
                        pass,
                    });
                }
            }
            let caster = features.caster();
            let cull = ShadowPipelines::cull(pass);
            if casters && !self.shadow.pipelines.contains_key(&(caster, cull)) {
                needs.push(Need::Caster {
                    features: caster,
                    cull,
                });
            }
            if prepass && pass < Pass::Lines {
                for samples in
                    std::iter::once(1).chain(self.variants.occluder_samples.iter().copied())
                {
                    if !self
                        .prepass
                        .pipelines
                        .contains_key(&(caster, pass, samples))
                    {
                        needs.push(Need::DepthOnly {
                            features: caster,
                            pass,
                            samples,
                        });
                    }
                }
            }
        }
        if needs.is_empty() {
            return None;
        }
        // The modules made already that the needs compile with.
        let mut world_shaders = Vec::new();
        let mut shadow_shaders = Vec::new();
        for need in &needs {
            match *need {
                Need::Module(_) => {}
                Need::World {
                    target,
                    features,
                    pass,
                    ..
                } => {
                    let compiled = compiled(target, features, pass);
                    if let Some(module) = self.world_shaders.get(&compiled) {
                        world_shaders.push((compiled, module.clone()));
                    }
                }
                Need::Caster { features, .. } | Need::DepthOnly { features, .. } => {
                    if let Some(module) = self.shadow_shaders.get(&features) {
                        shadow_shaders.push((features, module.clone()));
                    }
                }
            }
        }
        Some(VariantWork {
            needs,
            world_shaders,
            shadow_shaders,
            layouts: self.variant_layouts.clone(),
            composing: self.shaders.composing(),
            product: self.shaders.product_shader(features.product())?.clone(),
        })
    }

    /// Keep what a worker made, as if it had been made here; what was made
    /// here meanwhile is kept instead, and a pipeline for a target set that
    /// changed is dropped.
    pub(crate) fn install_variant(&mut self, sets: &mut [Pipelines], made: MadeVariant) {
        for (features, module) in made.world_shaders {
            self.world_shaders.entry(features).or_insert(module);
        }
        for (features, module) in made.shadow_shaders {
            self.shadow_shaders.entry(features).or_insert(module);
        }
        for (set, target, key, pipeline) in made.world {
            if let Some(pipelines) = sets.get_mut(set).filter(|set| set.target == target) {
                pipelines.world.entry(key).or_insert(pipeline);
            }
        }
        for (key, pipeline) in made.casters {
            self.shadow.pipelines.entry(key).or_insert(pipeline);
        }
        for (key, pipeline) in made.depth_only {
            self.prepass.pipelines.entry(key).or_insert(pipeline);
        }
    }
}

thread_local! {
    /// Each worker's own families, by what they compose: made on a worker's
    /// first variant, as a renderer makes its own.
    static FAMILIES: RefCell<Vec<Shaders>> = const { RefCell::new(Vec::new()) };
}

impl VariantWork {
    /// Compose and make everything the variant needs. A world or caster
    /// entry the shader does not compose under is made with the standard
    /// stages instead, as `Layouts::world_shader` does, and its error kept.
    pub(crate) fn make(self, device: &wgpu::Device) -> MadeVariant {
        FAMILIES.with(|families| {
            let mut families = families.borrow_mut();
            let index = match families
                .iter()
                .position(|family| family.composing() == self.composing)
            {
                Some(index) => index,
                None => {
                    families.push(Shaders::composing_as(self.composing));
                    families.len() - 1
                }
            };
            self.make_with(&mut families[index], device)
        })
    }

    fn make_with(self, family: &mut Shaders, device: &wgpu::Device) -> MadeVariant {
        let product = family.product(self.product);
        // The worker's family numbers the shader its own way; the features
        // made here are keyed as the renderer's.
        let local = |features: Features| match features.product() {
            0 => features,
            _ => features.with_product(product),
        };
        let mut made = MadeVariant {
            world_shaders: self.world_shaders,
            shadow_shaders: self.shadow_shaders,
            ..MadeVariant::default()
        };
        let mut module = |made: &mut MadeVariant, entry: Entry, features: Features| {
            let known = match entry {
                Entry::Shadow => &made.shadow_shaders,
                _ => &made.world_shaders,
            };
            if let Some((_, module)) = known.iter().find(|(known, _)| *known == features) {
                return module.clone();
            }
            let module = family
                .module(device, entry, local(features))
                .unwrap_or_else(|error| {
                    made.errors.push(error);
                    family
                        .module(device, entry, features.with_product(0))
                        .unwrap_or_else(|error| panic!("{error}"))
                });
            match entry {
                Entry::Shadow => made.shadow_shaders.push((features, module.clone())),
                _ => made.world_shaders.push((features, module.clone())),
            }
            module
        };
        for need in self.needs {
            match need {
                Need::Module(features) => {
                    module(&mut made, Entry::World, features);
                }
                Need::World {
                    set,
                    target,
                    features,
                    pass,
                } => {
                    let shader = module(&mut made, Entry::World, compiled(target, features, pass));
                    let pipeline = self
                        .layouts
                        .world_pipeline(device, target, &shader, features, pass);
                    made.world.push((set, target, (features, pass), pipeline));
                }
                Need::Caster { features, cull } => {
                    let shader = module(&mut made, Entry::Shadow, features);
                    let pipeline = self
                        .layouts
                        .caster_pipeline(device, &shader, features, cull);
                    made.casters.push(((features, cull), pipeline));
                }
                Need::DepthOnly {
                    features,
                    pass,
                    samples,
                } => {
                    let shader = module(&mut made, Entry::Shadow, features);
                    let pipeline = self
                        .layouts
                        .depth_only_pipeline(device, &shader, features, pass, samples);
                    made.depth_only.push(((features, pass, samples), pipeline));
                }
            }
        }
        made
    }
}
