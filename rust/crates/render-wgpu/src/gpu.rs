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
            // Timestamp queries time the renderer's GPU passes
            // (`timing.rs`); an adapter without them draws untimed. Indirect
            // first instance lets GPU-culled batches draw from their runs
            // (`culling.rs`); without it the CPU draw list stays.
            // Adapter-specific format features let the primary destination
            // multisample at every count the adapter supports (2 is not
            // guaranteed by WebGPU); without them `samples_supported` keeps
            // to the guaranteed counts.
            required_features: adapter.features()
                & (wgpu::Features::TIMESTAMP_QUERY
                    | wgpu::Features::INDIRECT_FIRST_INSTANCE
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
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

    /// Whether the adapter is a software rasterizer (llvmpipe, WARP): it
    /// starts, draws correctly and refuses expensive features cleanly, but
    /// its look and cost are not evidence for real GPUs.
    pub fn is_software(&self) -> bool {
        self.adapter.get_info().device_type == wgpu::DeviceType::Cpu
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

    /// The adapter's compute limits, for diagnostics. The device takes wgpu's
    /// default limits (`from_instance`); a kernel needing more raises them
    /// there.
    pub fn compute_limits(&self) -> ComputeLimits {
        ComputeLimits::of(&self.adapter.limits())
    }

    /// Whether the primary destination's colour, HDR and depth formats can
    /// all be multisampled at `samples` on this device: as wgpu validates a
    /// texture, by the adapter's format features when the device has them,
    /// else by the counts WebGPU guarantees.
    pub(crate) fn samples_supported(&self, samples: u32) -> bool {
        let adapter_specific = self
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        samples == 1
            || [
                crate::target::OFFSCREEN_FORMAT,
                crate::finish::HDR_FORMAT,
                crate::target::DEPTH_FORMAT,
            ]
            .into_iter()
            .all(|format| {
                let flags = if adapter_specific {
                    self.adapter.get_texture_format_features(format).flags
                } else {
                    format
                        .guaranteed_format_features(self.device.features())
                        .flags
                };
                flags.sample_count_supported(samples)
            })
    }

    /// Why this device cannot run a compute kernel of `workgroup` invocations
    /// using `shared_bytes` of workgroup storage, if it cannot. A kernel's
    /// owner takes its raster path or skips the work when refused.
    pub(crate) fn compute_refusal(&self, workgroup: [u32; 3], shared_bytes: u32) -> Option<String> {
        if !self
            .adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            return Some("the adapter has no compute shaders".to_owned());
        }
        let granted = ComputeLimits::of(&self.device.limits());
        if (0..3).any(|axis| granted.workgroup_size[axis] < workgroup[axis])
            || granted.invocations_per_workgroup < workgroup.iter().product::<u32>()
        {
            return Some(format!(
                "the device allows {} invocations per workgroup ({:?} by axis); the kernel needs {workgroup:?}",
                granted.invocations_per_workgroup, granted.workgroup_size
            ));
        }
        if granted.workgroup_storage_bytes < shared_bytes {
            return Some(format!(
                "the device allows {} bytes of workgroup storage; the kernel needs {shared_bytes}",
                granted.workgroup_storage_bytes
            ));
        }
        None
    }
}

/// Compute limits of an adapter or device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeLimits {
    pub workgroup_size: [u32; 3],
    pub invocations_per_workgroup: u32,
    pub workgroups_per_dimension: u32,
    pub workgroup_storage_bytes: u32,
    pub storage_buffer_binding_bytes: u64,
}

impl ComputeLimits {
    fn of(limits: &wgpu::Limits) -> Self {
        Self {
            workgroup_size: [
                limits.max_compute_workgroup_size_x,
                limits.max_compute_workgroup_size_y,
                limits.max_compute_workgroup_size_z,
            ],
            invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            workgroup_storage_bytes: limits.max_compute_workgroup_storage_size,
            storage_buffer_binding_bytes: limits.max_storage_buffer_binding_size,
        }
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
