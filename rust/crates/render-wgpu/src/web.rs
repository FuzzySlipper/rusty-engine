//! The desktop shell's UI overlay: the product's web UI, rendered off-screen
//! by Chromium (CEF), imported into the renderer's device whenever it
//! repaints, and composited over the presented frame in gamma space, as a
//! browser composites its page.
//!
//! Only built with the `web-overlay` feature. CEF re-executes the host
//! binary for its subprocesses, so the host calls
//! [`WebRuntime::run_subprocess`] first in `main`. The overlay runs Chromium
//! unsandboxed: it shows the product's own first-party UI, never arbitrary
//! pages.

use std::path::{Path, PathBuf};
use std::time::Instant;

use welding::{CefSurfaceProducer, HostWgpuContext, ImportedTexture};
pub use welding::{
    CursorShape, EventModifiers, KeyEvent, KeyEventKind, MouseAction, MouseButton, MouseEvent,
    NavigationEvent,
};

use crate::{FrameStats, Gpu, Renderer, SurfaceFrame, WindowSurface};

const SANDBOX: welding::CefSandboxMode = welding::CefSandboxMode::UnsandboxedTrustedContent;

/// Where CEF's binary distribution lives and how Chromium is started.
#[derive(Debug, Clone)]
pub struct WebRuntimeConfig {
    /// The directory holding `libcef`, its `.pak` files, `icudtl.dat` and
    /// `locales/`.
    pub cef_dir: PathBuf,
    /// Chromium's profile root: the page's local storage and cookies persist
    /// under it, as a browser keeps them per origin. Absolute; `None` keeps
    /// them in memory.
    pub cache_dir: Option<PathBuf>,
    /// Extra Chromium switches, `(name, value)`.
    pub switches: Vec<(String, Option<String>)>,
}

/// The process-wide Chromium runtime. Dropping it shuts CEF down; drop every
/// [`WebOverlay`] first.
pub struct WebRuntime {
    runtime: welding::CefRuntime,
    cache_dir: Option<PathBuf>,
}

impl WebRuntime {
    /// Run this process as a CEF subprocess if CEF started it as one.
    /// Returns the exit code to leave with, or `None` in the host process.
    pub fn run_subprocess(cef_dir: &Path) -> Result<Option<i32>, String> {
        welding::CefRuntime::execute_process_from(cef_dir, SANDBOX).map_err(|e| e.to_string())
    }

    pub fn initialize(config: WebRuntimeConfig) -> Result<Self, String> {
        let mut runtime = welding::CefRuntimeConfig::new(config.cef_dir, SANDBOX);
        runtime.cache_path = config.cache_dir.clone();
        // The page is the product's own UI: no first-run prompts, no
        // component downloads or background requests of a browser, and no
        // desktop keyring. Keyring access goes over D-Bus to KWallet or
        // libsecret and blocks page loading while that session is locked.
        runtime.command_line_switches = [
            ("no-first-run", None),
            ("no-default-browser-check", None),
            ("disable-component-update", None),
            ("disable-background-networking", None),
            ("password-store", Some("basic")),
        ]
        .into_iter()
        .map(|(switch, value)| (switch.to_owned(), value.map(str::to_owned)))
        .collect();
        runtime.command_line_switches.extend(config.switches);
        let runtime = welding::CefRuntime::initialize(runtime).map_err(|e| e.to_string())?;
        Ok(Self {
            runtime,
            cache_dir: config.cache_dir,
        })
    }

    /// Let Chromium do its pending work. Call once per host frame.
    pub fn pump(&self) {
        self.runtime.do_message_loop_work();
    }
}

/// What the overlay has imported, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WebOverlayStats {
    /// UI repaints imported since the overlay started.
    pub imports: u64,
    /// How long the last import took on the host thread.
    pub last_import_ms: f64,
    pub import_errors: u64,
}

/// One page drawn by Chromium into a texture of the window's size.
pub struct WebOverlay {
    producer: welding::PlatformCefProducer,
    host: HostWgpuContext,
    frame: Option<ImportedTexture>,
    bind_group: Option<wgpu::BindGroup>,
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    pipelines: Vec<((wgpu::TextureFormat, bool), wgpu::RenderPipeline)>,
    stats: WebOverlayStats,
    last_error: Option<String>,
}

impl WebOverlay {
    /// Load `url` into a transparent page of `width`×`height` physical pixels
    /// at `scale_factor` physical pixels per CSS pixel.
    pub fn new(
        runtime: &WebRuntime,
        gpu: &Gpu,
        url: &str,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Result<Self, String> {
        let host = HostWgpuContext::new(gpu.device.clone(), gpu.queue.clone());
        let producer = welding::PlatformCefProducer::new(
            &runtime.runtime,
            welding::PlatformCefConfig {
                surface: welding::CefSurfaceConfig {
                    initial_url: url.to_owned(),
                    initial_size: dpi::PhysicalSize::new(width.max(1), height.max(1)),
                    background_color: None,
                    prefer_accelerated: true,
                    // Chromium creates the profile directory itself.
                    user_data_dir: runtime.cache_dir.as_ref().map(|dir| dir.join("page")),
                    scale_factor,
                    ..Default::default()
                },
            },
            // Windows copies Chromium's D3D11 frames into textures shared
            // with this device.
            #[cfg(windows)]
            &host,
        )
        .map_err(|e| e.to_string())?;
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu web overlay"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu web overlay"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        Ok(Self {
            producer,
            host,
            frame: None,
            bind_group: None,
            layout,
            shader,
            pipelines: Vec::new(),
            stats: WebOverlayStats::default(),
            last_error: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.producer
            .resize(dpi::PhysicalSize::new(width.max(1), height.max(1)))
            .map_err(|e| e.to_string())
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) -> Result<(), String> {
        self.producer
            .set_scale_factor(scale_factor)
            .map_err(|e| e.to_string())
    }

    pub fn send_mouse(&mut self, event: MouseEvent) -> Result<(), String> {
        self.producer
            .send_mouse_input(event)
            .map_err(|e| e.to_string())
    }

    pub fn send_key(&mut self, event: KeyEvent) -> Result<(), String> {
        self.producer
            .send_keyboard_input(event)
            .map_err(|e| e.to_string())
    }

    /// Give the page keyboard focus.
    pub fn focus(&mut self) -> Result<(), String> {
        self.producer
            .move_focus(welding::FocusDirection::Forward)
            .map_err(|e| e.to_string())
    }

    pub fn execute_script(&mut self, script: &str) -> Result<(), String> {
        self.producer
            .execute_script(script, "rusty-desktop-shell")
            .map_err(|e| e.to_string())
    }

    /// Navigation, title and console events, oldest first.
    pub fn poll_event(&mut self) -> Option<NavigationEvent> {
        self.producer.poll_navigation_event()
    }

    /// The next cursor shape the page asks for.
    pub fn poll_cursor(&mut self) -> Option<CursorShape> {
        self.producer.poll_cursor_shape()
    }

    pub fn stats(&self) -> WebOverlayStats {
        self.stats
    }

    /// The last import failure, if the most recent repaint could not be
    /// imported. The previous frame stays on screen.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Import the page's latest repaint, if it repainted. It touches no
    /// scene state, so do it before drawing.
    pub fn import_repaint(&mut self) {
        let started = Instant::now();
        match self.producer.acquire_frame(&self.host) {
            Ok(Some(frame)) => {
                self.stats.imports += 1;
                self.stats.last_import_ms = started.elapsed().as_secs_f64() * 1e3;
                self.last_error = None;
                self.bind_group = Some(self.host.device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        label: Some("render-wgpu web overlay"),
                        layout: &self.layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&frame.view),
                        }],
                    },
                ));
                self.frame = Some(frame);
            }
            Ok(None) => {}
            Err(error) => {
                self.stats.import_errors += 1;
                self.last_error = Some(error.to_string());
            }
        }
    }

    /// Composite the page over `view`, a non-sRGB view of the presented
    /// image, with premultiplied alpha.
    fn draw(&mut self, view: &wgpu::TextureView, format: wgpu::TextureFormat) {
        let (Some(bind_group), Some(frame)) = (&self.bind_group, &self.frame) else {
            return;
        };
        let key = (format, frame.format.is_srgb());
        let device = &self.host.device;
        let index = match self.pipelines.iter().position(|(k, _)| *k == key) {
            Some(index) => index,
            None => {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render-wgpu web overlay"),
                    bind_group_layouts: &[Some(&self.layout)],
                    immediate_size: 0,
                });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu web overlay"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("vs_overlay"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some(if key.1 {
                            "fs_overlay_srgb"
                        } else {
                            "fs_overlay"
                        }),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.pipelines.push((key, pipeline));
                self.pipelines.len() - 1
            }
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu web overlay"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu web overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines[index].1);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.host.queue.submit([encoder.finish()]);
    }
}

impl Renderer {
    /// As [`Renderer::render_view_composition_to_frame`], with `overlay`'s
    /// last imported page composited over the frame
    /// ([`WebOverlay::import_repaint`]). A playing video covers both.
    pub fn render_view_composition_to_frame_with_overlay(
        &mut self,
        surface: &WindowSurface,
        frame: &SurfaceFrame,
        time_seconds: f64,
        overlay: &mut WebOverlay,
    ) -> FrameStats {
        let uploaded = self.prepare();
        self.surface_size = Some(surface.size());
        let (view, finished) = surface.views(frame);
        let mut stats = self.render_composition(view, time_seconds);
        overlay.draw(finished.color, finished.format);
        stats.video = self.draw_video(&finished);
        stats.parts_uploaded = uploaded;
        stats
    }
}

/// A full-window triangle that copies the page's pixels one to one. When the
/// imported texture is sRGB-tagged, loading decodes it; encoding again
/// restores Chromium's premultiplied sRGB bytes for the gamma-space blend.
const SHADER: &str = r#"
@group(0) @binding(0) var page: texture_2d<f32>;

@vertex
fn vs_overlay(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn load(position: vec4<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(page));
    let texel = vec2<i32>(position.xy);
    if (texel.x >= size.x || texel.y >= size.y) {
        return vec4<f32>(0.0);
    }
    return textureLoad(page, texel, 0);
}

@fragment
fn fs_overlay(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return load(position);
}

@fragment
fn fs_overlay_srgb(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = load(position);
    return vec4<f32>(encode(texel.rgb), texel.a);
}
"#;
