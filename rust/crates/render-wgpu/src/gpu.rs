//! Device and adapter selection. The runtime owns one `Gpu`; the renderer,
//! offscreen targets and window surfaces borrow it.

use std::sync::{Arc, Mutex};

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
/// `WGPU_BACKEND=vulkan` and the llvmpipe (lvp) ICD. A window with the web
/// overlay on Windows always draws with Vulkan (`Gpu::window_refusal`).
///
/// Built with `dlss`, a Vulkan instance and an NVIDIA adapter's device also
/// take NGX's Vulkan extensions (`dlss.rs`); where NGX cannot name them,
/// the device is made without them and DLSS is refused.
#[derive(Clone)]
pub struct Gpu {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// The device has NGX's Vulkan extensions for DLSS.
    #[cfg(feature = "dlss")]
    pub(crate) dlss: bool,
    /// NGX on this device, shared by every renderer on it: NGX starts and
    /// shuts down once per device (`dlss.rs`).
    #[cfg(feature = "dlss")]
    pub(crate) ngx: crate::dlss::Ngx,
    /// Why the device was lost, once it has been. wgpu reports a loss only
    /// through its callback; after it every use of the device fails.
    lost: Arc<Mutex<Option<String>>>,
}

impl Gpu {
    /// A device with no presentation surface, for offscreen rendering.
    pub fn headless() -> Result<Self, GpuError> {
        let instance = instance(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::from_instance(instance, None)
    }

    /// A headless device whose storage bindings and buffers hold at most
    /// `bytes`: for tests of what passes a device's binding limit.
    pub fn headless_with_binding_limit(bytes: u64) -> Result<Self, GpuError> {
        let instance = instance(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::from_instance_with(instance, None, Some(bytes), None)
    }

    /// A headless device that binds at most `textures` sampled textures per
    /// shader stage: for tests of what a device at WebGPU's guarantee (16)
    /// refuses.
    pub fn headless_with_sampled_texture_limit(textures: u32) -> Result<Self, GpuError> {
        let instance = instance(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::from_instance_with(instance, None, None, Some(textures))
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
        #[cfg(all(feature = "web-overlay", windows))]
        {
            descriptor.backends = wgpu::Backends::VULKAN;
        }
        let instance = instance(descriptor);
        let surface = instance
            .create_surface(window)
            .map_err(|error| GpuError::Surface(error.to_string()))?;
        let gpu = Self::from_instance(instance, Some(&surface))?;
        #[cfg(all(feature = "web-overlay", windows))]
        gpu.window_refusal()?;
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
        {
            descriptor.backends = wgpu::Backends::VULKAN;
        }
        let gpu = Self::from_instance(instance(descriptor), None)?;
        #[cfg(all(feature = "web-overlay", windows))]
        gpu.window_refusal()?;
        Ok(gpu)
    }

    /// Refuses, naming the reason, a device that cannot draw a window with
    /// the web overlay on Windows. The overlay imports Chromium's frames,
    /// D3D11 textures, into Vulkan with `VK_KHR_external_memory_win32`
    /// (`welding`); a software adapter is refused too.
    #[cfg(all(feature = "web-overlay", windows))]
    fn window_refusal(&self) -> Result<(), GpuError> {
        let info = self.adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err(GpuError::NoAdapter(format!(
                "{} is a software adapter; the desktop window needs a Vulkan GPU",
                info.name
            )));
        }
        if !self
            .device
            .features()
            .contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_WIN32)
        {
            return Err(GpuError::NoAdapter(format!(
                "{} ({}) cannot import the UI's D3D11 textures: the desktop window needs VK_KHR_external_memory_win32",
                info.name, info.driver
            )));
        }
        Ok(())
    }

    fn from_instance(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        Self::from_instance_with(instance, surface, None, None)
    }

    fn from_instance_with(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        binding_limit: Option<u64>,
        texture_limit: Option<u32>,
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
            required_limits: limits(&adapter.limits(), binding_limit, texture_limit),
            ..Default::default()
        };
        #[cfg_attr(not(feature = "dlss"), allow(unused_variables))]
        let (device, queue, dlss) = request_device(&adapter, descriptor)?;
        let lost = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            let cause = format!("{reason:?}: {message}");
            eprintln!("render-wgpu: the wgpu device was lost ({cause})");
            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(cause);
        });
        // A lost device's resources are all invalid, so every later use of
        // one raises a validation error: those follow from the loss, which
        // is reported, and the renderer goes on without drawing. Any other
        // error is a renderer bug and panics, as wgpu's default handler does.
        let after_loss = Arc::clone(&lost);
        device.on_uncaptured_error(Arc::new(move |error| {
            if after_loss
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_none()
            {
                panic!("wgpu error: {error}");
            }
        }));
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            #[cfg(feature = "dlss")]
            dlss,
            #[cfg(feature = "dlss")]
            ngx: Default::default(),
            lost,
        })
    }

    /// Destroy the device, as a driver reset loses one: for tests of what a
    /// lost device leaves running. wgpu reports a destroyed device lost on
    /// the next poll, where a driver's loss is reported as it is found.
    #[doc(hidden)]
    pub fn destroy_device(&self) {
        self.device.destroy();
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Why the device was lost (its `DeviceLostReason` and the driver's
    /// message), if it has been. Nothing draws after that.
    pub fn lost(&self) -> Option<String> {
        self.lost
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
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

    /// The most bytes one storage buffer binding (and its buffer) holds on
    /// this device: what bounds the scatter copies and the selection's
    /// entries.
    pub(crate) fn storage_binding_bytes(&self) -> u64 {
        let limits = self.device.limits();
        limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size)
    }

    /// The most bytes one buffer holds on this device (a capture's readback
    /// among them).
    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size
    }

    /// Why water on this device cannot read the opaque scene's colour
    /// (`water.rs`), if it cannot: the water pass binds one more texture
    /// than WebGPU guarantees.
    pub(crate) fn scene_color_refusal(&self) -> Option<String> {
        let granted = self.device.limits().max_sampled_textures_per_shader_stage;
        (granted < crate::water::SCENE_SAMPLED_TEXTURES).then(|| {
            format!(
                "the device binds {granted} sampled textures per shader stage; water with the scene's colour binds {}",
                crate::water::SCENE_SAMPLED_TEXTURES
            )
        })
    }

    /// Why this device cannot lay decals, if it cannot: the world pass binds
    /// two more textures with them than WebGPU guarantees.
    pub(crate) fn decals_refusal(&self) -> Option<String> {
        let granted = self.device.limits().max_sampled_textures_per_shader_stage;
        (granted < crate::decals::WORLD_SAMPLED_TEXTURES).then(|| {
            format!(
                "the device binds {granted} sampled textures per shader stage; the world pass with decals binds {}",
                crate::decals::WORLD_SAMPLED_TEXTURES
            )
        })
    }

    pub fn max_texture_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// The adapter's compute limits, for diagnostics. The device takes wgpu's
    /// default limits but the adapter's buffer sizes (`limits`); a kernel
    /// needing more raises them there.
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

/// wgpu's default limits at the adapter's resolution, with the adapter's
/// largest storage binding and buffer, so dense scatter holds as many
/// copies as the device can bind (at most `binding_limit` bytes), and the
/// sampled textures a world pass with decals binds, when the adapter has
/// them (at most `texture_limit`).
fn limits(
    adapter: &wgpu::Limits,
    binding_limit: Option<u64>,
    texture_limit: Option<u32>,
) -> wgpu::Limits {
    let cap = binding_limit.unwrap_or(u64::MAX);
    let defaults = wgpu::Limits::default();
    wgpu::Limits {
        max_storage_buffer_binding_size: adapter.max_storage_buffer_binding_size.min(cap),
        max_buffer_size: adapter.max_buffer_size.min(cap),
        max_sampled_textures_per_shader_stage: adapter
            .max_sampled_textures_per_shader_stage
            .min(crate::decals::WORLD_SAMPLED_TEXTURES)
            .max(defaults.max_sampled_textures_per_shader_stage)
            .min(texture_limit.unwrap_or(u32::MAX)),
        ..defaults.using_resolution(adapter.clone())
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

/// The instance `descriptor` describes. Built with `dlss`, a Vulkan
/// instance also takes NGX's instance extensions, and is Vulkan alone; if
/// NGX cannot name them, it is made without them.
fn instance(descriptor: wgpu::InstanceDescriptor) -> wgpu::Instance {
    #[cfg(feature = "dlss")]
    if descriptor.backends.contains(wgpu::Backends::VULKAN) {
        let mut support = dlss_wgpu::FeatureSupport::default();
        if let Ok(instance) =
            dlss_wgpu::create_instance(crate::dlss::PROJECT_ID, &descriptor, &mut support)
        {
            return instance;
        }
    }
    wgpu::Instance::new(descriptor)
}

/// A device and queue on `adapter`, and whether it has NGX's Vulkan
/// extensions for DLSS. The web overlay adds what importing Chromium's
/// frames needs.
fn request_device(
    adapter: &wgpu::Adapter,
    #[allow(unused_mut)] mut descriptor: wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue, bool), GpuError> {
    // The web overlay imports Chromium's frames, D3D11 textures, with
    // `VK_KHR_external_memory_win32` (`Gpu::window_refusal`).
    #[cfg(all(feature = "web-overlay", windows))]
    {
        descriptor.required_features |=
            adapter.features() & wgpu::Features::VULKAN_EXTERNAL_MEMORY_WIN32;
    }
    // On Linux it imports DMA-BUFs. Chromium may leave the modifier
    // implicit (it does under X11), which only a device created with the
    // DMA-BUF extensions and `VULKAN_EXTERNAL_MEMORY_DMA_BUF` can import.
    #[cfg(all(feature = "web-overlay", target_os = "linux"))]
    let dmabuf = adapter.get_info().backend == wgpu::Backend::Vulkan;
    #[cfg(all(feature = "web-overlay", target_os = "linux"))]
    if dmabuf {
        descriptor.required_features |=
            adapter.features() & wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF;
    }
    #[cfg(feature = "dlss")]
    {
        let info = adapter.get_info();
        if info.backend == wgpu::Backend::Vulkan && info.vendor == crate::dlss::NVIDIA {
            let mut support = dlss_wgpu::FeatureSupport::default();
            #[cfg(all(feature = "web-overlay", target_os = "linux"))]
            let made = welding::build_dmabuf_capable_device_with(
                adapter,
                &descriptor,
                |args, raw_adapter| {
                    let registered = dlss_wgpu::register_device_extensions(
                        crate::dlss::PROJECT_ID,
                        args,
                        raw_adapter,
                        &mut support,
                    );
                    if registered.is_err() {
                        support.super_resolution_supported = false;
                    }
                },
            )
            .map_err(|error| error.to_string());
            #[cfg(not(all(feature = "web-overlay", target_os = "linux")))]
            let made = dlss_wgpu::request_device(
                crate::dlss::PROJECT_ID,
                adapter,
                &descriptor,
                &mut support,
                Some(descriptor.required_limits.clone()),
            )
            .map_err(|error| error.to_string());
            // Without DLSS's extensions the device is made as usual.
            if let Ok((device, queue)) = made {
                return Ok((device, queue, support.super_resolution_supported));
            }
        }
    }
    #[cfg(all(feature = "web-overlay", target_os = "linux"))]
    if dmabuf {
        return welding::build_dmabuf_capable_device(adapter, &descriptor)
            .map(|(device, queue)| (device, queue, false))
            .map_err(|error| GpuError::Device(error.to_string()));
    }
    pollster::block_on(adapter.request_device(&descriptor))
        .map(|(device, queue)| (device, queue, false))
        .map_err(|error| GpuError::Device(error.to_string()))
}

/// A buffer holding `contents`, as `DeviceExt::create_buffer_init` makes
/// it, that does not panic on a lost device. There `create_buffer` returns
/// an invalid buffer without raising an error, and wgpu's helper panics
/// mapping it; this one returns it unwritten, as a lost device leaves every
/// other resource ([`Gpu::lost`] says why).
pub(crate) fn buffer_init(
    device: &wgpu::Device,
    descriptor: &wgpu::util::BufferInitDescriptor<'_>,
) -> wgpu::Buffer {
    // `create_buffer_init`'s padding: a mapped buffer's size is a nonzero
    // multiple of `COPY_BUFFER_ALIGNMENT`.
    let size = descriptor.contents.len() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: descriptor.label,
        size: size
            .next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            .max(wgpu::COPY_BUFFER_ALIGNMENT),
        usage: descriptor.usage,
        mapped_at_creation: true,
    });
    if let Ok(mut mapped) = buffer.get_mapped_range_mut(..) {
        mapped
            .slice(..descriptor.contents.len())
            .copy_from_slice(descriptor.contents);
        drop(mapped);
        buffer.unmap();
    }
    buffer
}
