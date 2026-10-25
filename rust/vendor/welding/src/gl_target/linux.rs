// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Linux: an EGL context on a GBM device of the wgpu adapter's render node,
//! drawing into linear GBM buffers imported as GL textures.
//!
//! The buffers are linear because Mesa's GL and Vulkan drivers need not agree
//! on a tiled layout: a texture exported with an implicit modifier imports
//! into Vulkan as scrambled tiles.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use gleam::gl::{self, Gl};
use khronos_egl as egl;

use super::{GL_TARGET_BUFFERS, GlTargetError};
use crate::native_frame::{DmaBufImage, DmaBufPlane, HostWgpuContext, NativeFrame};

const EGL_PLATFORM_GBM_KHR: egl::Enum = 0x31D7;
const EGL_LINUX_DMA_BUF_EXT: egl::Enum = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: egl::Attrib = 0x3271;
const EGL_DMA_BUF_PLANE0_FD_EXT: egl::Attrib = 0x3272;
const EGL_DMA_BUF_PLANE0_OFFSET_EXT: egl::Attrib = 0x3273;
const EGL_DMA_BUF_PLANE0_PITCH_EXT: egl::Attrib = 0x3274;
const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: egl::Attrib = 0x3443;
const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: egl::Attrib = 0x3444;

/// `DRM_FORMAT_ABGR8888`: R, G, B, A bytes in memory, wgpu's `Rgba8Unorm`.
const DRM_FORMAT_ABGR8888: u32 = 0x3432_4241;
const DRM_FORMAT_MOD_LINEAR: u64 = 0;
const GBM_BO_USE_RENDERING: u32 = 1 << 2;
const GBM_BO_USE_LINEAR: u32 = 1 << 4;

type ImageTargetTexture = unsafe extern "C" fn(u32, *mut c_void);

/// libgbm's entry points, loaded at run time so hosts that never make a
/// target do not link it.
struct Gbm {
    _library: gbm_loader::Library,
    create_device: unsafe extern "C" fn(i32) -> *mut c_void,
    device_destroy: unsafe extern "C" fn(*mut c_void),
    bo_create: unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32) -> *mut c_void,
    bo_destroy: unsafe extern "C" fn(*mut c_void),
    bo_get_fd: unsafe extern "C" fn(*mut c_void) -> i32,
    bo_get_stride: unsafe extern "C" fn(*mut c_void) -> u32,
    bo_get_offset: unsafe extern "C" fn(*mut c_void, i32) -> u32,
}

impl Gbm {
    fn load() -> Result<Self, GlTargetError> {
        // SAFETY: libgbm is a C library with no initialisers that run on
        // load; each symbol is read with the signature gbm.h declares.
        unsafe {
            let library = gbm_loader::Library::new("libgbm.so.1")
                .map_err(|e| GlTargetError::new(format!("libgbm.so.1: {e}")))?;
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| GlTargetError::new(format!("libgbm {}: {e}", $name)))?
                };
            }
            Ok(Self {
                create_device: symbol!("gbm_create_device"),
                device_destroy: symbol!("gbm_device_destroy"),
                bo_create: symbol!("gbm_bo_create"),
                bo_destroy: symbol!("gbm_bo_destroy"),
                bo_get_fd: symbol!("gbm_bo_get_fd"),
                bo_get_stride: symbol!("gbm_bo_get_stride"),
                bo_get_offset: symbol!("gbm_bo_get_offset"),
                _library: library,
            })
        }
    }
}

/// One shared colour buffer and the GL objects drawing into it.
struct Buffer {
    bo: *mut c_void,
    fd: OwnedFd,
    stride: u32,
    offset: u32,
    image: egl::Image,
    texture: u32,
    depth: u32,
    framebuffer: u32,
}

struct Buffers {
    buffers: Vec<Buffer>,
    size: PhysicalSize<u32>,
    generation: u64,
}

pub(super) struct Target {
    // Field order is drop order for what has no Drop of its own; `Drop`
    // releases the buffers, context and display first.
    buffers: RefCell<Buffers>,
    back: Cell<usize>,
    front: Cell<Option<usize>>,
    gleam: Rc<dyn Gl>,
    glow: Arc<glow::Context>,
    image_target_texture: ImageTargetTexture,
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    gbm: Gbm,
    device: *mut c_void,
    _drm: File,
}

impl Target {
    pub(super) fn new(
        _host: &HostWgpuContext,
        adapter: &wgpu::AdapterInfo,
        size: PhysicalSize<u32>,
    ) -> Result<Self, GlTargetError> {
        let node = render_node(adapter)?;
        let drm = File::options()
            .read(true)
            .write(true)
            .open(&node)
            .map_err(|e| GlTargetError::new(format!("{}: {e}", node.display())))?;
        let gbm = Gbm::load()?;
        // SAFETY: the descriptor stays open in `_drm` for the device's life.
        let device = unsafe { (gbm.create_device)(drm.as_raw_fd()) };
        if device.is_null() {
            return Err(GlTargetError::new(format!(
                "gbm_create_device failed on {}",
                node.display()
            )));
        }
        let (egl, display, context, gleam, glow, image_target_texture) =
            match open_context(device) {
                Ok(context) => context,
                Err(error) => {
                    // SAFETY: nothing else holds the device.
                    unsafe { (gbm.device_destroy)(device) };
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
            egl,
            display,
            context,
            gbm,
            device,
            _drm: drm,
        };
        // On failure `Drop` releases the context, display and device.
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
        let fd = buffer
            .fd
            .try_clone()
            .map_err(|e| GlTargetError::new(format!("dup the buffer's descriptor: {e}")))?;
        let size = buffers.size;
        let image = DmaBufImage::from_owned_buffers(
            vec![fd],
            vec![DmaBufPlane::new(
                0,
                u64::from(buffer.offset),
                u64::from(buffer.stride) * u64::from(size.height),
                buffer.stride,
            )],
            size,
            wgpu::TextureFormat::Rgba8Unorm,
            DRM_FORMAT_ABGR8888,
            DRM_FORMAT_MOD_LINEAR,
            buffers.generation,
        )
        .map_err(|e| GlTargetError::new(e.to_string()))?;
        Ok(NativeFrame::DmaBufImage(image))
    }

    /// A linear buffer of `size` and its framebuffer. The context is current.
    fn allocate(&self, size: PhysicalSize<u32>) -> Result<Buffer, GlTargetError> {
        // SAFETY: `device` is the live GBM device; the buffer object is
        // destroyed in `release` or below on failure.
        let bo = unsafe {
            (self.gbm.bo_create)(
                self.device,
                size.width,
                size.height,
                DRM_FORMAT_ABGR8888,
                GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR,
            )
        };
        if bo.is_null() {
            return Err(GlTargetError::new(format!(
                "gbm_bo_create {}x{} linear ABGR8888 failed",
                size.width, size.height
            )));
        }
        // SAFETY: `bo` is live; gbm_bo_get_fd returns a new descriptor the
        // caller owns, or -1.
        let (raw_fd, stride, offset) = unsafe {
            (
                (self.gbm.bo_get_fd)(bo),
                (self.gbm.bo_get_stride)(bo),
                (self.gbm.bo_get_offset)(bo, 0),
            )
        };
        if raw_fd < 0 {
            // SAFETY: `bo` is live and owned here.
            unsafe { (self.gbm.bo_destroy)(bo) };
            return Err(GlTargetError::new("gbm_bo_get_fd failed"));
        }
        // SAFETY: `raw_fd` is a fresh descriptor nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        let attributes = [
            egl::WIDTH as egl::Attrib,
            size.width as egl::Attrib,
            egl::HEIGHT as egl::Attrib,
            size.height as egl::Attrib,
            EGL_LINUX_DRM_FOURCC_EXT,
            DRM_FORMAT_ABGR8888 as egl::Attrib,
            EGL_DMA_BUF_PLANE0_FD_EXT,
            raw_fd as egl::Attrib,
            EGL_DMA_BUF_PLANE0_OFFSET_EXT,
            offset as egl::Attrib,
            EGL_DMA_BUF_PLANE0_PITCH_EXT,
            stride as egl::Attrib,
            EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
            (DRM_FORMAT_MOD_LINEAR & 0xffff_ffff) as egl::Attrib,
            EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            (DRM_FORMAT_MOD_LINEAR >> 32) as egl::Attrib,
            egl::ATTRIB_NONE,
        ];
        // SAFETY: a DMA-BUF import takes no client buffer and duplicates the
        // descriptor it is given; `fd` stays owned here.
        let image = unsafe {
            self.egl.create_image(
                self.display,
                egl::Context::from_ptr(egl::NO_CONTEXT),
                EGL_LINUX_DMA_BUF_EXT,
                egl::ClientBuffer::from_ptr(std::ptr::null_mut()),
                &attributes,
            )
        };
        let image = match image {
            Ok(image) => image,
            Err(error) => {
                // SAFETY: `bo` is live and owned here.
                unsafe { (self.gbm.bo_destroy)(bo) };
                return Err(GlTargetError::new(format!(
                    "import the buffer into EGL: {error}"
                )));
            }
        };
        let gl = &self.gleam;
        let texture = gl.gen_textures(1)[0];
        gl.bind_texture(gl::TEXTURE_2D, texture);
        // SAFETY: the context is current, `texture` is bound to TEXTURE_2D
        // and `image` is a live EGL image of this display.
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
            texture,
            0,
        );
        gl.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::DEPTH_STENCIL_ATTACHMENT,
            gl::RENDERBUFFER,
            depth,
        );
        let buffer = Buffer {
            bo,
            fd,
            stride,
            offset,
            image,
            texture,
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

    /// Delete `buffer`'s GL objects and free its memory. A host's import of
    /// the memory keeps its own reference to it.
    fn release(&self, buffer: Buffer) {
        let _ = self.make_current();
        let gl = &self.gleam;
        gl.delete_framebuffers(&[buffer.framebuffer]);
        gl.delete_renderbuffers(&[buffer.depth]);
        gl.delete_textures(&[buffer.texture]);
        let _ = self.egl.destroy_image(self.display, buffer.image);
        // SAFETY: `bo` is live and nothing references it after this.
        unsafe { (self.gbm.bo_destroy)(buffer.bo) };
        drop(buffer.fd);
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
        if !self.device.is_null() {
            // SAFETY: every buffer object of the device is destroyed above.
            unsafe { (self.gbm.device_destroy)(self.device) };
        }
    }
}

#[allow(clippy::type_complexity)]
fn open_context(
    device: *mut c_void,
) -> Result<
    (
        egl::DynamicInstance<egl::EGL1_5>,
        egl::Display,
        egl::Context,
        Rc<dyn Gl>,
        Arc<glow::Context>,
        ImageTargetTexture,
    ),
    GlTargetError,
> {
    // SAFETY: loading libEGL runs no code beyond its initialisers.
    let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
        .map_err(|e| GlTargetError::new(format!("libEGL: {e}")))?;
    // SAFETY: `device` is a live GBM device, the native display type of
    // EGL_PLATFORM_GBM_KHR.
    let display = unsafe {
        egl.get_platform_display(EGL_PLATFORM_GBM_KHR, device, &[egl::ATTRIB_NONE])
    }
    .map_err(|e| GlTargetError::new(format!("EGL display on the GBM device: {e}")))?;
    egl.initialize(display)
        .map_err(|e| GlTargetError::new(format!("eglInitialize: {e}")))?;
    // Desktop GL where the driver has it, else GLES 3.
    let attempts: [(egl::Enum, egl::Int, &[egl::Int], bool); 2] = [
        (
            egl::OPENGL_API,
            egl::OPENGL_BIT,
            &[
                egl::CONTEXT_MAJOR_VERSION,
                3,
                egl::CONTEXT_MINOR_VERSION,
                2,
                egl::CONTEXT_OPENGL_PROFILE_MASK,
                egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
                egl::NONE,
            ],
            false,
        ),
        (
            egl::OPENGL_ES_API,
            egl::OPENGL_ES3_BIT,
            &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE],
            true,
        ),
    ];
    let mut failures = Vec::new();
    for (api, renderable, context_attributes, gles) in attempts {
        let made = egl
            .bind_api(api)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                egl.choose_first_config(
                    display,
                    &[egl::RENDERABLE_TYPE, renderable, egl::SURFACE_TYPE, 0, egl::NONE],
                )
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "no config".to_owned())
            })
            .and_then(|config| {
                egl.create_context(display, config, None, context_attributes)
                    .map_err(|e| e.to_string())
            });
        let context = match made {
            Ok(context) => context,
            Err(error) => {
                failures.push(format!("{}: {error}", if gles { "GLES 3" } else { "GL 3.2" }));
                continue;
            }
        };
        egl.make_current(display, None, None, Some(context))
            .map_err(|e| GlTargetError::new(format!("surfaceless eglMakeCurrent: {e}")))?;
        let loader = |name: &str| -> *const c_void {
            egl.get_proc_address(name)
                .map_or(std::ptr::null(), |f| f as *const c_void)
        };
        // SAFETY: the loader returns the current context's entry points.
        let gleam = unsafe {
            if gles {
                gl::GlesFns::load_with(loader)
            } else {
                gl::GlFns::load_with(loader)
            }
        };
        // SAFETY: as above.
        let glow = Arc::new(unsafe { glow::Context::from_loader_function(loader) });
        let image_target = egl
            .get_proc_address("glEGLImageTargetTexture2DOES")
            .ok_or_else(|| GlTargetError::new("no glEGLImageTargetTexture2DOES"))?;
        // SAFETY: the extension function has this signature (OES_EGL_image).
        let image_target_texture: ImageTargetTexture =
            unsafe { std::mem::transmute::<extern "system" fn(), ImageTargetTexture>(image_target) };
        return Ok((egl, display, context, gleam, glow, image_target_texture));
    }
    let _ = egl.terminate(display);
    Err(GlTargetError::new(format!(
        "no GL context on the GPU: {}",
        failures.join("; ")
    )))
}

/// The render node of the GPU behind `adapter`: by PCI bus address where
/// wgpu reports one, else by PCI vendor and device.
fn render_node(adapter: &wgpu::AdapterInfo) -> Result<PathBuf, GlTargetError> {
    let read = |path: &Path| std::fs::read_to_string(path).ok();
    let hex = |text: Option<String>| {
        text.and_then(|text| u32::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok())
    };
    let mut by_id = None;
    let entries = std::fs::read_dir("/sys/class/drm")
        .map_err(|e| GlTargetError::new(format!("/sys/class/drm: {e}")))?;
    let mut nodes: Vec<_> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("renderD"))
        .collect();
    nodes.sort();
    for name in nodes {
        let device = Path::new("/sys/class/drm").join(&name).join("device");
        let bus = std::fs::read_link(&device)
            .ok()
            .and_then(|link| link.file_name().map(|n| n.to_string_lossy().into_owned()));
        let path = Path::new("/dev/dri").join(&name);
        if !adapter.device_pci_bus_id.is_empty()
            && bus.as_deref().is_some_and(|bus| bus.eq_ignore_ascii_case(&adapter.device_pci_bus_id))
        {
            return Ok(path);
        }
        if by_id.is_none()
            && hex(read(&device.join("vendor"))) == Some(adapter.vendor)
            && hex(read(&device.join("device"))) == Some(adapter.device)
        {
            by_id = Some(path);
        }
    }
    by_id.ok_or_else(|| {
        GlTargetError::new(format!(
            "no render node for {} ({:04x}:{:04x})",
            adapter.name, adapter.vendor, adapter.device
        ))
    })
}
