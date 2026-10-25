// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A GL colour target whose memory a wgpu device imports without a copy, for
//! a GL renderer (an embedded web engine) drawing frames a wgpu host
//! composites.
//!
//! The target is a ring of colour buffers allocated for sharing:
//!
//! - **Linux:** linear ABGR8888 GBM buffers on the render node of the wgpu
//!   adapter, bound as GL textures through `EGL_EXT_image_dma_buf_import`,
//!   with the GL context on an EGL display of that same GBM device. Each
//!   buffer exports as a [`NativeFrame::DmaBufImage`].
//!
//! The renderer draws into the back buffer ([`GlSharedTarget::bind_back`]);
//! [`GlSharedTarget::finish`] waits for GL to finish it and makes it the
//! front buffer. The host imports each buffer once per
//! [`GlSharedTarget::generation`] and samples the front one. It must not let
//! the renderer draw into a buffer the host's GPU work may still read: GL
//! rows run bottom-up, so the host flips rows when it samples.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;

use crate::native_frame::NativeFrame;

/// The number of colour buffers in the ring: the renderer draws into one
/// while the host samples the other.
pub const GL_TARGET_BUFFERS: usize = 2;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GlTargetError(pub String);

impl GlTargetError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// A GL context and its shared colour buffers. It lives on the thread that
/// made it, which is the thread that renders with it.
pub struct GlSharedTarget {
    inner: platform::Target,
}

impl GlSharedTarget {
    /// A context on the GPU behind `adapter` (the wgpu adapter that will
    /// import the buffers), with buffers of `size`, current on this thread.
    pub fn new(adapter: &wgpu::AdapterInfo, size: PhysicalSize<u32>) -> Result<Self, GlTargetError> {
        Ok(Self {
            inner: platform::Target::new(adapter, clamp(size))?,
        })
    }

    pub fn make_current(&self) -> Result<(), GlTargetError> {
        self.inner.make_current()
    }

    /// The GL API for `gleam` users.
    pub fn gleam(&self) -> Rc<dyn gleam::gl::Gl> {
        self.inner.gleam()
    }

    /// The GL API for `glow` users.
    pub fn glow(&self) -> Arc<glow::Context> {
        self.inner.glow()
    }

    /// The GL renderer and version strings.
    pub fn renderer(&self) -> String {
        self.inner.renderer()
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.inner.size()
    }

    /// Replace the buffers with ones of `size`, starting a new
    /// [`GlSharedTarget::generation`]. Buffers the host already imported
    /// stay valid for it until it drops them.
    pub fn resize(&self, size: PhysicalSize<u32>) -> Result<(), GlTargetError> {
        self.inner.resize(clamp(size))
    }

    /// Changes whenever the buffers are replaced, so the host imports them
    /// again.
    pub fn generation(&self) -> u64 {
        self.inner.generation()
    }

    /// Bind the back buffer's framebuffer for drawing.
    pub fn bind_back(&self) {
        self.inner.bind_back();
    }

    /// The buffer the renderer draws into next.
    pub fn back(&self) -> usize {
        self.inner.back()
    }

    /// The last finished buffer, `None` before the first.
    pub fn front(&self) -> Option<usize> {
        self.inner.front()
    }

    /// Wait for GL to finish drawing the back buffer, then make it the front
    /// buffer and the next one the back.
    pub fn finish(&self) {
        self.inner.finish();
    }

    /// Buffer `index` of this generation, for the host to import. Each call
    /// hands out its own handle.
    pub fn export(&self, index: usize) -> Result<NativeFrame, GlTargetError> {
        self.inner.export(index)
    }
}

fn clamp(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    PhysicalSize::new(size.width.max(1), size.height.max(1))
}
