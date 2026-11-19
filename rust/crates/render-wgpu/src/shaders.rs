//! The standard shader family (`render-shaders`) as wgpu shader modules.

pub(crate) use render_shaders::{Composing, Entry, Features, ProductShader};

pub(crate) struct Shaders(render_shaders::Shaders);

impl Shaders {
    pub fn new() -> Self {
        Self(render_shaders::Shaders::new())
    }

    /// The world pass without decals (`render_shaders::Shaders::without_decals`).
    pub fn without_decals(self) -> Self {
        Self(self.0.without_decals())
    }

    /// Water without the scene's colour
    /// (`render_shaders::Shaders::without_scene_color`).
    pub fn without_scene_color(self) -> Self {
        Self(self.0.without_scene_color())
    }

    /// What it composes beside the standard modules.
    pub fn composing(&self) -> Composing {
        self.0.composing()
    }

    /// A new family composing as `composing`
    /// (`render_shaders::Shaders::composing_as`).
    pub fn composing_as(composing: Composing) -> Self {
        Self(render_shaders::Shaders::composing_as(composing))
    }

    /// The product shader a `Features::product` id names.
    pub fn product_shader(&self, product: u32) -> Option<&ProductShader> {
        self.0.product_shader(product)
    }

    /// The id a material shaded by `shader` adds to its features.
    pub fn product(&mut self, shader: ProductShader) -> u32 {
        self.0.product(shader)
    }

    /// An entry shader compiled with `features`. Only a product shader can
    /// fail to compose; the error names its file and line.
    pub fn module(
        &mut self,
        device: &wgpu::Device,
        entry: Entry,
        features: Features,
    ) -> Result<wgpu::ShaderModule, String> {
        let module = self.0.compose(entry, features)?;
        Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("render-wgpu {entry:?}")),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        }))
    }
}
