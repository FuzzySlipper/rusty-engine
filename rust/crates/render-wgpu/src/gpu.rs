//! Device and adapter selection. The runtime owns one `Gpu`; the renderer,
//! offscreen targets and window surfaces borrow it.

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    NoAdapter(String),
    Device(String),
    Surface(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter(detail) => write!(f, "no wgpu adapter: {detail}"),
            Self::Device(detail) => write!(f, "wgpu device request failed: {detail}"),
            Self::Surface(detail) => write!(f, "wgpu surface creation failed: {detail}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Adapter facts for diagnostics and evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterSummary {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
}

/// The runtime's wgpu adapter, device and queue.
///
/// `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and `WGPU_POWER_PREF` select the adapter
/// as wgpu documents them. CI selects software Vulkan with
/// `WGPU_BACKEND=vulkan` and the llvmpipe (lvp) ICD.
#[derive(Clone)]
pub struct Gpu {
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
}

impl Gpu {
    /// A device with no presentation surface, for offscreen rendering.
    pub fn headless() -> Result<Self, GpuError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::from_instance(instance, None)
    }

    /// A device and a presentation surface for one window, compatible with
    /// each other. The window handle traits come from `raw-window-handle`,
    /// which winit implements, so the shell never names a wgpu type.
    pub fn for_window<W>(
        window: Arc<W>,
        width: u32,
        height: u32,
    ) -> Result<(Self, crate::WindowSurface), GpuError>
    where
        W: wgpu::rwh::HasWindowHandle
            + wgpu::rwh::HasDisplayHandle
            + std::fmt::Debug
            + Send
            + Sync
            + 'static,
    {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance
            .create_surface(window)
            .map_err(|error| GpuError::Surface(error.to_string()))?;
        let gpu = Self::from_instance(instance, Some(&surface))?;
        let surface = crate::WindowSurface::new(&gpu, surface, width, height)?;
        Ok((gpu, surface))
    }

    fn from_instance(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = pollster::block_on(wgpu::util::initialize_adapter_from_env_or_default(
            &instance, surface,
        ))
        .map_err(|error| GpuError::NoAdapter(error.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|error| GpuError::Device(error.to_string()))?;
        Ok(Self {
            adapter,
            device,
            queue,
        })
    }

    pub fn adapter_summary(&self) -> AdapterSummary {
        let info = self.adapter.get_info();
        AdapterSummary {
            name: info.name,
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver: info.driver,
        }
    }

    pub fn max_texture_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }
}
