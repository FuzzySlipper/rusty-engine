//! The standard shader family (`render-shaders`) as wgpu shader modules.

pub(crate) use render_shaders::{Entry, Features, ProductShader};

pub(crate) struct Shaders(render_shaders::Shaders);

impl Shaders {
    pub fn new() -> Self {
        Self(render_shaders::Shaders::new())
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
