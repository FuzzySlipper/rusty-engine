// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Windows: callback-time D3D11 copy of CEF's pooled shared texture into a
//! weld-owned NT-handle texture, then its import into the host's Vulkan
//! device (`VK_KHR_external_memory_win32`, D3D11 texture handles).
//!
//! Split out of `native_frame/mod.rs`.

use super::*;

/// Copies callback-scoped CEF D3D11 textures into weld-owned shared textures
/// on the GPU the host's Vulkan device draws with.
pub struct D3d11CallbackFrameCopier {
    device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    device1: windows::Win32::Graphics::Direct3D11::ID3D11Device1,
    context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
}

impl D3d11CallbackFrameCopier {
    pub fn new(ctx: &HostWgpuContext) -> Result<Self, ImportError> {
        use windows::{
            Win32::{
                Foundation::HMODULE,
                Graphics::{
                    Direct3D::{
                        D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                        D3D_FEATURE_LEVEL_11_1,
                    },
                    Direct3D11::{
                        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                        ID3D11Device, ID3D11Device1, ID3D11DeviceContext,
                    },
                    Dxgi::{CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4},
                },
            },
            core::Interface,
        };

        let luid = vulkan_device_luid(ctx)?;
        let adapter = unsafe {
            let factory = CreateDXGIFactory1::<IDXGIFactory4>()
                .map_err(|err| ImportError::Hal(format!("CreateDXGIFactory1 failed: {err}")))?;
            factory
                .EnumAdapterByLuid::<IDXGIAdapter>(luid)
                .map_err(|err| ImportError::Hal(format!("EnumAdapterByLuid failed: {err}")))?
        };

        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();
        let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        unsafe {
            D3D11CreateDevice(
                Some(&adapter),
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )
        }
        .map_err(|err| ImportError::Hal(format!("D3D11CreateDevice failed: {err}")))?;
        let device = device
            .ok_or_else(|| ImportError::Hal("D3D11CreateDevice returned no device".into()))?;
        let context = context
            .ok_or_else(|| ImportError::Hal("D3D11CreateDevice returned no context".into()))?;
        let device1 = device
            .cast::<ID3D11Device1>()
            .map_err(|err| ImportError::Hal(format!("ID3D11Device1 cast failed: {err}")))?;

        Ok(Self {
            device,
            device1,
            context,
        })
    }

    fn copy_to_owned_shared_frame(
        &self,
        frame: Dx12SharedTexture,
    ) -> Result<
        (
            Dx12SharedTexture,
            windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
        ),
        ImportError,
    > {
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        use windows::{
            Win32::{
                Foundation::{GENERIC_ALL, HANDLE},
                Graphics::{
                    Direct3D11::{
                        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
                        D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
                        D3D11_USAGE_DEFAULT, ID3D11Texture2D,
                    },
                    Dxgi::IDXGIResource1,
                },
            },
            core::{Interface, PCWSTR},
        };

        let size = frame.size;
        let format = frame.format;
        let generation = frame.generation;
        let source_handle = frame.into_owned_handle()?;
        let source = unsafe {
            self.device1
                .OpenSharedResource1::<ID3D11Texture2D>(HANDLE(source_handle.as_raw_handle()))
        }
        .map_err(|err| ImportError::D3d11OpenShared(err.to_string()))?;
        let mut desc = Default::default();
        unsafe { source.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_DEFAULT;
        desc.BindFlags = (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32;
        desc.CPUAccessFlags = 0;
        // No keyed mutex: the copy has finished on the GPU
        // (`flush_and_wait_for_gpu`) before the handle leaves this callback,
        // and nothing writes the texture after it.
        desc.MiscFlags =
            (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0) as u32;

        let mut target = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut target)) }
            .map_err(|err| ImportError::Hal(format!("D3D11 CreateTexture2D failed: {err}")))?;
        let target =
            target.ok_or_else(|| ImportError::Hal("D3D11 CreateTexture2D returned null".into()))?;
        unsafe {
            self.context.CopyResource(&target, &source);
        }
        self.flush_and_wait_for_gpu()?;

        let dxgi_resource = target
            .cast::<IDXGIResource1>()
            .map_err(|err| ImportError::Hal(format!("IDXGIResource1 cast failed: {err}")))?;
        let target_handle = unsafe {
            dxgi_resource
                .CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null())
                .map_err(|err| ImportError::Hal(format!("DXGI CreateSharedHandle failed: {err}")))?
        };
        let target_handle =
            unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(target_handle.0) };
        Ok((
            Dx12SharedTexture::from_owned_handle(target_handle, size, format, generation)?,
            target,
        ))
    }

    fn flush_and_wait_for_gpu(&self) -> Result<(), ImportError> {
        use windows::Win32::Graphics::Direct3D11::{D3D11_QUERY_DESC, D3D11_QUERY_EVENT};

        let mut query = None;
        unsafe {
            self.device
                .CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )
                .map_err(|err| ImportError::Hal(format!("D3D11 CreateQuery failed: {err}")))?;
        }
        let query =
            query.ok_or_else(|| ImportError::Hal("D3D11 CreateQuery returned null".into()))?;
        unsafe {
            self.context.End(&query);
            self.context.Flush();
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let mut data: u32 = 0;
            let result = unsafe {
                self.context.GetData(
                    &query,
                    Some(&mut data as *mut _ as *mut std::ffi::c_void),
                    std::mem::size_of::<u32>() as u32,
                    0,
                )
            };
            if d3d11_event_reached(result, data)? {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                return Err(ImportError::Hal(
                    "D3D11 GPU copy timed out after 2 seconds".into(),
                ));
            }
            std::thread::yield_now();
        }
    }
}

impl WgpuTextureImporter {
    /// Copy a callback-scoped Windows CEF frame into an application-owned
    /// texture before returning the CEF surface to its pool.
    ///
    /// CEF permits opening the shared handle inside `OnAcceleratedPaint`, but
    /// the opened resource must not escape that callback. Copying through D3D11
    /// matches CEF's native Windows sharing path and keeps the pooled source
    /// inside the callback. [`WgpuTextureImporter::import`] takes the copy.
    pub fn copy_d3d11_callback_frame_to_owned(
        frame: Dx12SharedTexture,
        copier: &D3d11CallbackFrameCopier,
    ) -> Result<Dx12SharedTexture, ImportError> {
        let (copied_frame, _d3d11_target) = copier.copy_to_owned_shared_frame(frame)?;
        Ok(copied_frame)
    }
}

/// The LUID of the GPU behind the host's Vulkan device, which names the same
/// adapter to DXGI.
fn vulkan_device_luid(
    ctx: &HostWgpuContext,
) -> Result<windows::Win32::Foundation::LUID, ImportError> {
    use ash::vk;

    if ctx.backend != InteropBackend::Vulkan {
        return Err(ImportError::BackendMismatch {
            frame: NativeFrameKind::Dx12SharedTexture,
            wgpu: ctx.backend,
        });
    }
    let mut id = vk::PhysicalDeviceIDProperties::default();
    unsafe {
        let hal_device = ctx.device.as_hal::<wgpu::wgc::api::Vulkan>().ok_or(
            ImportError::BackendMismatch {
                frame: NativeFrameKind::Dx12SharedTexture,
                wgpu: ctx.backend,
            },
        )?;
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
        hal_device
            .shared_instance()
            .raw_instance()
            .get_physical_device_properties2(hal_device.raw_physical_device(), &mut properties);
    }
    if id.device_luid_valid == vk::FALSE {
        return Err(ImportError::Hal(
            "the Vulkan device reports no LUID to find its DXGI adapter by".into(),
        ));
    }
    let [l0, l1, l2, l3, h0, h1, h2, h3] = id.device_luid;
    Ok(windows::Win32::Foundation::LUID {
        LowPart: u32::from_le_bytes([l0, l1, l2, l3]),
        HighPart: i32::from_le_bytes([h0, h1, h2, h3]),
    })
}

/// The Windows half of [`WgpuTextureImporter::import`] for
/// [`NativeFrame::Dx12SharedTexture`] frames: the copier's NT-handle D3D11
/// texture, imported into the host's Vulkan device and acquired from
/// `VK_QUEUE_FAMILY_EXTERNAL`, shader-readable.
pub(super) fn import_d3d11(
    frame: Dx12SharedTexture,
    ctx: &HostWgpuContext,
) -> Result<ImportedTexture, ImportError> {
    use std::os::windows::io::AsRawHandle;

    let frame_size = frame.size();
    if frame_size.width == 0 || frame_size.height == 0 {
        return Err(ImportError::InvalidFrame(
            "D3D shared texture has zero size",
        ));
    }
    let mismatch = ImportError::BackendMismatch {
        frame: NativeFrameKind::Dx12SharedTexture,
        wgpu: ctx.backend,
    };
    if ctx.backend != InteropBackend::Vulkan {
        return Err(mismatch);
    }
    let format = frame.format();
    let generation = frame.generation();
    let handle = frame.into_owned_handle()?;
    let size = wgpu::Extent3d {
        width: frame_size.width,
        height: frame_size.height,
        depth_or_array_layers: 1,
    };
    let (hal_texture, queue_family, vk_device) = unsafe {
        let hal_device = ctx
            .device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .ok_or(mismatch)?;
        let hal_texture = hal_device
            .texture_from_d3d11_shared_handle(
                windows::Win32::Foundation::HANDLE(handle.as_raw_handle()),
                &wgpu_hal::TextureDescriptor {
                    label: Some("weld-cef-d3d11-import"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
                    memory_flags: wgpu_hal::MemoryFlags::empty(),
                    view_formats: Vec::new(),
                },
            )
            .map_err(|err| ImportError::VulkanImport(err.to_string()))?;
        (
            hal_texture,
            hal_device.queue_family_index(),
            hal_device.raw_device().clone(),
        )
    };
    // The imported memory holds its own reference to the D3D11 texture.
    drop(handle);
    let image = unsafe { hal_texture.raw_handle() };
    let texture = unsafe {
        ctx.device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("weld-cef-d3d11-import"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            // The acquire below leaves it shader-readable.
            wgpu::TextureUses::RESOURCE,
        )
    };
    acquire_from_external_queue(ctx, &vk_device, queue_family, image)?;
    // wgpu does not see the acquire use the texture: keep it until that
    // submission completes.
    let kept = texture.clone();
    ctx.queue.on_submitted_work_done(move || drop(kept));

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok(ImportedTexture {
        texture,
        view,
        size,
        format,
        generation,
    })
}

/// Take `image` from `VK_QUEUE_FAMILY_EXTERNAL` (D3D11 wrote it) to the
/// host's queue family in the shader-read layout, in a submission of its own
/// ahead of any that draws it. It waits for nothing: the copier's
/// flush-and-wait finished the D3D11 write.
fn acquire_from_external_queue(
    ctx: &HostWgpuContext,
    vk_device: &ash::Device,
    queue_family: u32,
    image: ash::vk::Image,
) -> Result<(), ImportError> {
    use ash::vk;

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("weld-cef-d3d11-acquire"),
        });
    let recorded = unsafe {
        encoder.as_hal_mut::<wgpu::wgc::api::Vulkan, _, _>(|hal_encoder| {
            let Some(hal_encoder) = hal_encoder else {
                return false;
            };
            let barrier = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
                .dst_queue_family_index(queue_family)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            vk_device.cmd_pipeline_barrier(
                hal_encoder.raw_handle(),
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            true
        })
    };
    if !recorded {
        return Err(ImportError::VulkanImport(
            "the host's command encoder has no Vulkan encoder to acquire the frame in".into(),
        ));
    }
    ctx.queue.submit([encoder.finish()]);
    Ok(())
}
