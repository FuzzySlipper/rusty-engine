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
    pub(crate) instance: wgpu::Instance,
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
        #[allow(unused_mut)]
        let mut descriptor =
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone()));
        // Chromium hands the web overlay's frames over as D3D shared textures,
        // which only a DX12 device can import.
        #[cfg(all(feature = "web-overlay", windows))]
        if std::env::var_os("WGPU_BACKEND").is_none() {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window)
            .map_err(|error| GpuError::Surface(error.to_string()))?;
        let gpu = Self::from_instance(instance, Some(&surface))?;
        let surface = crate::WindowSurface::new(&gpu, surface, width, height)?;
        Ok((gpu, surface))
    }

    /// A device for windows on `display`, before any window exists. Windows
    /// get their surfaces later from [`crate::WindowSurface::create`], so a
    /// renderer can be built on this device first.
    pub fn for_display<D>(display: D) -> Result<Self, GpuError>
    where
        D: wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Send + Sync + 'static,
    {
        #[allow(unused_mut)]
        let mut descriptor =
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display));
        #[cfg(all(feature = "web-overlay", windows))]
        if std::env::var_os("WGPU_BACKEND").is_none() {
            descriptor.backends = wgpu::Backends::DX12;
        }
        Self::from_instance(wgpu::Instance::new(descriptor), None)
    }

    fn from_instance(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = pollster::block_on(wgpu::util::initialize_adapter_from_env_or_default(
            &instance, surface,
        ))
        .map_err(|error| GpuError::NoAdapter(error.to_string()))?;
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("render-wgpu"),
            // Timestamp queries time the compute pass (`compute.rs`); an
            // adapter without them draws untimed.
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        };
        let (device, queue) = request_device(&adapter, descriptor)?;
        Ok(Self {
            instance,
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

#[cfg(not(all(feature = "web-overlay", target_os = "linux")))]
fn request_device(
    adapter: &wgpu::Adapter,
    descriptor: wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    pollster::block_on(adapter.request_device(&descriptor))
        .map_err(|error| GpuError::Device(error.to_string()))
}

/// The web overlay imports Chromium's frames as DMA-BUFs. Chromium may leave
/// the modifier implicit (it does under X11), which only a device created with
/// the DMA-BUF extensions and `VULKAN_EXTERNAL_MEMORY_DMA_BUF` can import.
#[cfg(all(feature = "web-overlay", target_os = "linux"))]
fn request_device(
    adapter: &wgpu::Adapter,
    mut descriptor: wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    if adapter.get_info().backend != wgpu::Backend::Vulkan {
        return pollster::block_on(adapter.request_device(&descriptor))
            .map_err(|error| GpuError::Device(error.to_string()));
    }
    descriptor.required_features |=
        adapter.features() & wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF;
    welding::build_dmabuf_capable_device(adapter, &descriptor)
        .map_err(|error| GpuError::Device(error.to_string()))
}
