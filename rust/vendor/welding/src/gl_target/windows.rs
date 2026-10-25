// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Windows: ANGLE's GLES over D3D11 on the adapter of the host's Vulkan
//! device (by LUID), drawing into D3D11 textures with NT shared handles
//! bound as GL textures (`EGL_ANGLE_image_d3d11_texture`). ANGLE's
//! `libEGL.dll` and `libGLESv2.dll` load from beside the executable.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use gleam::gl::{self, Gl};
use khronos_egl as egl;
use windows::Win32::Foundation::{GENERIC_ALL, LUID};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED,
    D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIResource1};
use windows::core::{Interface, PCWSTR};

use super::{GL_TARGET_BUFFERS, GlTargetError};
use crate::native_frame::{Dx12SharedTexture, HostWgpuContext, NativeFrame};

const EGL_PLATFORM_ANGLE_ANGLE: egl::Enum = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: egl::Attrib = 0x3203;
const EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE: egl::Attrib = 0x3208;
const EGL_PLATFORM_ANGLE_D3D_LUID_HIGH_ANGLE: egl::Attrib = 0x34A0;
const EGL_PLATFORM_ANGLE_D3D_LUID_LOW_ANGLE: egl::Attrib = 0x34A1;
const EGL_DEVICE_EXT: i32 = 0x322C;
const EGL_D3D11_DEVICE_ANGLE: i32 = 0x33A1;
const EGL_D3D11_TEXTURE_ANGLE: egl::Enum = 0x3484;

type QueryDisplayAttrib = unsafe extern "C" fn(egl::EGLDisplay, i32, *mut isize) -> egl::Boolean;
type QueryDeviceAttrib = unsafe extern "C" fn(*mut c_void, i32, *mut isize) -> egl::Boolean;
type ImageTargetTexture = unsafe extern "C" fn(u32, *mut c_void);

/// One shared colour buffer and the GL objects drawing into it.
struct Buffer {
    texture: ID3D11Texture2D,
    image: egl::Image,
    gl_texture: u32,
    depth: u32,
    framebuffer: u32,
}

struct Buffers {
    buffers: Vec<Buffer>,
    size: PhysicalSize<u32>,
    generation: u64,
}

pub(super) struct Target {
    buffers: RefCell<Buffers>,
    back: Cell<usize>,
    front: Cell<Option<usize>>,
    gleam: Rc<dyn Gl>,
    glow: Arc<glow::Context>,
    image_target_texture: ImageTargetTexture,
    device: ID3D11Device,
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
}

impl Target {
    pub(super) fn new(
        host: &HostWgpuContext,
        _adapter: &wgpu::AdapterInfo,
        size: PhysicalSize<u32>,
    ) -> Result<Self, GlTargetError> {
        let luid = crate::native_frame::d3d11::vulkan_device_luid(host)
            .map_err(|e| GlTargetError::new(e.to_string()))?;
        // SAFETY: loading ANGLE's libEGL runs no code beyond its initialisers.
        let egl = unsafe {
            egl::DynamicInstance::<egl::EGL1_5>::load_required_from_filename("libEGL.dll")
        }
        .map_err(|e| GlTargetError::new(format!("ANGLE's libEGL.dll: {e}")))?;
        // SAFETY: the default display with ANGLE's documented attributes.
        let display = unsafe {
            egl.get_platform_display(
                EGL_PLATFORM_ANGLE_ANGLE,
                egl::DEFAULT_DISPLAY,
                &[
                    EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                    EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE,
                    EGL_PLATFORM_ANGLE_D3D_LUID_HIGH_ANGLE,
                    luid.HighPart as egl::Attrib,
                    EGL_PLATFORM_ANGLE_D3D_LUID_LOW_ANGLE,
                    luid.LowPart as egl::Attrib,
                    egl::ATTRIB_NONE,
                ],
            )
        }
        .map_err(|e| GlTargetError::new(format!("ANGLE's D3D11 display: {e}")))?;
        egl.initialize(display)
            .map_err(|e| GlTargetError::new(format!("eglInitialize (ANGLE): {e}")))?;
        let opened = open_context(&egl, display, luid);
        let (context, gleam, glow, image_target_texture, device) = match opened {
            Ok(opened) => opened,
            Err(error) => {
                let _ = egl.terminate(display);
                return Err(error);
            }
        };
        let target = Self {
            buffers: RefCell::new(Buffers {
                buffers: Vec::new(),
                size,
                generation: 0,
            }),
            back: Cell::new(0),
            front: Cell::new(None),
            gleam,
            glow,
            image_target_texture,
            device,
            egl,
            display,
            context,
        };
        // On failure `Drop` releases the context and display.
        target.resize(size)?;
        Ok(target)
    }

    pub(super) fn make_current(&self) -> Result<(), GlTargetError> {
        self.egl
            .make_current(self.display, None, None, Some(self.context))
            .map_err(|e| GlTargetError::new(format!("eglMakeCurrent: {e}")))
    }

    pub(super) fn gleam(&self) -> Rc<dyn Gl> {
        self.gleam.clone()
    }

    pub(super) fn glow(&self) -> Arc<glow::Context> {
        self.glow.clone()
    }

    pub(super) fn renderer(&self) -> String {
        format!(
            "{} | {}",
            self.gleam.get_string(gl::RENDERER),
            self.gleam.get_string(gl::VERSION)
        )
    }

    pub(super) fn size(&self) -> PhysicalSize<u32> {
        self.buffers.borrow().size
    }

    pub(super) fn generation(&self) -> u64 {
        self.buffers.borrow().generation
    }

    pub(super) fn resize(&self, size: PhysicalSize<u32>) -> Result<(), GlTargetError> {
        self.make_current()?;
        let mut fresh = Vec::with_capacity(GL_TARGET_BUFFERS);
        for _ in 0..GL_TARGET_BUFFERS {
            match self.allocate(size) {
                Ok(buffer) => fresh.push(buffer),
                Err(error) => {
                    for buffer in fresh {
                        self.release(buffer);
                    }
                    return Err(error);
                }
            }
        }
        let old = {
            let mut buffers = self.buffers.borrow_mut();
            buffers.size = size;
            buffers.generation += 1;
            std::mem::replace(&mut buffers.buffers, fresh)
        };
        for buffer in old {
            self.release(buffer);
        }
        self.back.set(0);
        self.front.set(None);
        Ok(())
    }

    pub(super) fn bind_back(&self) {
        let buffers = self.buffers.borrow();
        if let Some(buffer) = buffers.buffers.get(self.back.get()) {
            self.gleam
                .bind_framebuffer(gl::FRAMEBUFFER, buffer.framebuffer);
        }
    }

    pub(super) fn back(&self) -> usize {
        self.back.get()
    }

    pub(super) fn front(&self) -> Option<usize> {
        self.front.get()
    }

    pub(super) fn finish(&self) {
        self.gleam.finish();
        let back = self.back.get();
        self.front.set(Some(back));
        self.back.set((back + 1) % GL_TARGET_BUFFERS);
    }

    pub(super) fn export(&self, index: usize) -> Result<NativeFrame, GlTargetError> {
        let buffers = self.buffers.borrow();
        let buffer = buffers
            .buffers
            .get(index)
            .ok_or_else(|| GlTargetError::new(format!("no buffer {index}")))?;
        // SAFETY: the texture was made with SHARED_NTHANDLE; the new handle
        // is owned by the frame, which closes it.
        let frame = unsafe {
            let resource: IDXGIResource1 = buffer
                .texture
                .cast()
                .map_err(|e| GlTargetError::new(e.to_string()))?;
            let handle = resource
                .CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null())
                .map_err(|e| GlTargetError::new(format!("CreateSharedHandle: {e}")))?;
            Dx12SharedTexture::from_owned_raw_handle(
                handle.0,
                buffers.size,
                wgpu::TextureFormat::Rgba8Unorm,
                buffers.generation,
            )
        }
        .map_err(|e| GlTargetError::new(e.to_string()))?;
        Ok(NativeFrame::Dx12SharedTexture(frame))
    }

    /// A shared texture of `size` and its framebuffer. The context is current.
    fn allocate(&self, size: PhysicalSize<u32>) -> Result<Buffer, GlTargetError> {
        let description = D3D11_TEXTURE2D_DESC {
            Width: size.width,
            Height: size.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture = None;
        // SAFETY: a plain texture description on ANGLE's live device.
        unsafe {
            self.device
                .CreateTexture2D(&description, None, Some(&mut texture))
        }
        .map_err(|e| GlTargetError::new(format!("CreateTexture2D: {e}")))?;
        let texture = texture.ok_or_else(|| GlTargetError::new("CreateTexture2D gave no texture"))?;
        // SAFETY: the client buffer is a live D3D11 texture of ANGLE's
        // device, which EGL_ANGLE_image_d3d11_texture takes.
        let image = unsafe {
            self.egl.create_image(
                self.display,
                egl::Context::from_ptr(egl::NO_CONTEXT),
                EGL_D3D11_TEXTURE_ANGLE,
                egl::ClientBuffer::from_ptr(texture.as_raw()),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(|e| GlTargetError::new(format!("eglCreateImage (D3D11 texture): {e}")))?;
        let gl = &self.gleam;
        let gl_texture = gl.gen_textures(1)[0];
        gl.bind_texture(gl::TEXTURE_2D, gl_texture);
        // SAFETY: the context is current, `gl_texture` is bound to
        // TEXTURE_2D and `image` is a live EGL image of this display.
        unsafe { (self.image_target_texture)(gl::TEXTURE_2D, image.as_ptr()) };
        let depth = gl.gen_renderbuffers(1)[0];
        gl.bind_renderbuffer(gl::RENDERBUFFER, depth);
        gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH24_STENCIL8,
            size.width as i32,
            size.height as i32,
        );
        let framebuffer = gl.gen_framebuffers(1)[0];
        gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
        gl.framebuffer_texture_2d(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            gl_texture,
            0,
        );
        gl.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::DEPTH_STENCIL_ATTACHMENT,
            gl::RENDERBUFFER,
            depth,
        );
        let buffer = Buffer {
            texture,
            image,
            gl_texture,
            depth,
            framebuffer,
        };
        let status = gl.check_frame_buffer_status(gl::FRAMEBUFFER);
        if status != gl::FRAMEBUFFER_COMPLETE {
            self.release(buffer);
            return Err(GlTargetError::new(format!(
                "the shared framebuffer is incomplete: {status:#x}"
            )));
        }
        Ok(buffer)
    }

    /// Delete `buffer`'s GL objects and its texture. A host's import of the
    /// texture keeps its own reference to it.
    fn release(&self, buffer: Buffer) {
        let _ = self.make_current();
        let gl = &self.gleam;
        gl.delete_framebuffers(&[buffer.framebuffer]);
        gl.delete_renderbuffers(&[buffer.depth]);
        gl.delete_textures(&[buffer.gl_texture]);
        let _ = self.egl.destroy_image(self.display, buffer.image);
        drop(buffer.texture);
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        let buffers = std::mem::take(&mut self.buffers.get_mut().buffers);
        for buffer in buffers {
            self.release(buffer);
        }
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_context(self.display, self.context);
        let _ = self.egl.terminate(self.display);
    }
}

/// A GLES 3 context on `display`, current, and ANGLE's D3D11 device, which
/// must be the adapter `luid` names.
#[allow(clippy::type_complexity)]
fn open_context(
    egl: &egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    luid: LUID,
) -> Result<
    (
        egl::Context,
        Rc<dyn Gl>,
        Arc<glow::Context>,
        ImageTargetTexture,
        ID3D11Device,
    ),
    GlTargetError,
> {
    egl.bind_api(egl::OPENGL_ES_API)
        .map_err(|e| GlTargetError::new(e.to_string()))?;
    let config = egl
        .choose_first_config(
            display,
            &[
                egl::RENDERABLE_TYPE,
                egl::OPENGL_ES3_BIT,
                egl::RED_SIZE,
                8,
                egl::GREEN_SIZE,
                8,
                egl::BLUE_SIZE,
                8,
                egl::ALPHA_SIZE,
                8,
                egl::NONE,
            ],
        )
        .map_err(|e| GlTargetError::new(e.to_string()))?
        .ok_or_else(|| GlTargetError::new("ANGLE has no GLES 3 config"))?;
    let context = egl
        .create_context(display, config, None, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE])
        .map_err(|e| GlTargetError::new(format!("eglCreateContext (ANGLE): {e}")))?;
    egl.make_current(display, None, None, Some(context))
        .map_err(|e| GlTargetError::new(format!("surfaceless eglMakeCurrent: {e}")))?;
    let loader = |name: &str| -> *const c_void {
        egl.get_proc_address(name)
            .map_or(std::ptr::null(), |f| f as *const c_void)
    };
    // SAFETY: the loader returns the current context's entry points.
    let gleam = unsafe { gl::GlesFns::load_with(loader) };
    // SAFETY: as above.
    let glow = Arc::new(unsafe { glow::Context::from_loader_function(loader) });
    let address = |name: &str| {
        egl.get_proc_address(name)
            .ok_or_else(|| GlTargetError::new(format!("ANGLE has no {name}")))
    };
    // SAFETY: the extension functions have these signatures
    // (EGL_EXT_device_query, OES_EGL_image).
    let (query_display, query_device, image_target_texture) = unsafe {
        (
            std::mem::transmute::<extern "system" fn(), QueryDisplayAttrib>(address(
                "eglQueryDisplayAttribEXT",
            )?),
            std::mem::transmute::<extern "system" fn(), QueryDeviceAttrib>(address(
                "eglQueryDeviceAttribEXT",
            )?),
            std::mem::transmute::<extern "system" fn(), ImageTargetTexture>(address(
                "glEGLImageTargetTexture2DOES",
            )?),
        )
    };
    // SAFETY: EGL_EXT_device_query on a live display; the D3D11 device is
    // borrowed from ANGLE and cloned (AddRef) to keep it.
    let device = unsafe {
        let mut egl_device = 0isize;
        if query_display(display.as_ptr(), EGL_DEVICE_EXT, &mut egl_device) == 0 {
            return Err(GlTargetError::new("eglQueryDisplayAttribEXT(EGL_DEVICE_EXT) failed"));
        }
        let mut d3d = 0isize;
        if query_device(egl_device as *mut c_void, EGL_D3D11_DEVICE_ANGLE, &mut d3d) == 0 {
            return Err(GlTargetError::new(
                "eglQueryDeviceAttribEXT(EGL_D3D11_DEVICE_ANGLE) failed",
            ));
        }
        ID3D11Device::from_raw_borrowed(&(d3d as *mut c_void))
            .ok_or_else(|| GlTargetError::new("ANGLE gave a null D3D11 device"))?
            .clone()
    };
    // SAFETY: plain queries on a live device.
    let adapter = unsafe {
        device
            .cast::<IDXGIDevice>()
            .and_then(|dxgi| dxgi.GetAdapter())
            .and_then(|adapter| adapter.GetDesc())
    }
    .map_err(|e| GlTargetError::new(format!("ANGLE's adapter: {e}")))?;
    if adapter.AdapterLuid.LowPart != luid.LowPart || adapter.AdapterLuid.HighPart != luid.HighPart
    {
        return Err(GlTargetError::new(format!(
            "ANGLE draws on {}, not the window's Vulkan GPU",
            String::from_utf16_lossy(&adapter.Description).trim_end_matches('\0')
        )));
    }
    Ok((context, gleam, glow, image_target_texture, device))
}
