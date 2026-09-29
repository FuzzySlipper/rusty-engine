//! #8790 pattern 3 spike: CEF off-screen (accelerated) UI composited by wgpu
//! over a wgpu-rendered scene in a winit window.
//!
//! CEF_PATH=<cef dist> LD_LIBRARY_PATH=$CEF_PATH cargo run -- [seconds] [hud.html]

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use welding::{
    wgpu, CefRuntime, CefRuntimeConfig, CefSandboxMode, CefSurfaceConfig, CefSurfaceProducer,
    EventModifiers, HostWgpuContext, ImportedTexture, KeyEvent, KeyEventKind, PlatformCefConfig,
    MouseAction, MouseButton, MouseEvent, NavigationEvent, PlatformCefProducer,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const SHADER: &str = r#"
struct U { time: f32, aspect: f32, _p0: f32, _p1: f32 };
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var ui_tex: texture_2d<f32>;
@group(1) @binding(1) var ui_samp: sampler;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  var o: V; o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0); o.uv = vec2<f32>(p.x, 1.0 - p.y); return o;
}
@fragment fn fs_scene(v: V) -> @location(0) vec4<f32> {
  let c = (v.uv - 0.5) * vec2<f32>(u.aspect, 1.0);
  let a = u.time;
  let r = vec2<f32>(c.x * cos(a) - c.y * sin(a), c.x * sin(a) + c.y * cos(a));
  let inside = step(max(abs(r.x), abs(r.y)), 0.2);
  let bg = vec3<f32>(0.05, 0.3, 0.12);
  return vec4<f32>(mix(bg, vec3<f32>(0.9, 0.3, 0.2), inside), 1.0);
}
@fragment fn fs_ui(v: V) -> @location(0) vec4<f32> {
  return textureSample(ui_tex, ui_samp, v.uv);
}
"#;

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    host: HostWgpuContext,
    scene: wgpu::RenderPipeline,
    ui: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    ui_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    ui_group: Option<wgpu::BindGroup>,
    ui_frame: Option<ImportedTexture>,
}

#[derive(Default)]
struct Metrics {
    presented: u32,
    ui_frames: u32,
    first_ui_frame_ms: Option<f64>,
    import_ms: Vec<f64>,
    frame_cpu_ms: Vec<f64>,
    pump_ms: Vec<f64>,
    acquire_ms: Vec<f64>,
    present_ms: Vec<f64>,
    script_to_frame_ms: Vec<f64>,
    titles: Vec<String>,
    ui_per_s: Vec<u32>,
    game_per_s: Vec<u32>,
}

struct App {
    seconds: u64,
    hud_url: String,
    started: Instant,
    runtime: Option<CefRuntime>,
    gfx: Option<Gfx>,
    producer: Option<PlatformCefProducer>,
    metrics: Metrics,
    script_sent: Option<Instant>,
    next_ping: Instant,
    ping: u32,
    scripted_click: bool,
    scripted_key: bool,
    second_start: Instant,
    ui_this_second: u32,
    game_this_second: u32,
    cursor: (i32, i32),
}

impl App {
    fn init_gfx(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("overlay spike: cef")
                        .with_inner_size(PhysicalSize::new(1280, 720)),
                )
                .unwrap(),
        );
        let size = window.inner_size();
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(wgpu::util::initialize_adapter_from_env_or_default(
            &instance,
            Some(&surface),
        ))
        .unwrap();
        eprintln!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = welding::build_dmabuf_capable_device(
            &adapter,
            &wgpu::DeviceDescriptor {
                label: Some("spike"),
                required_features: adapter.features() & wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF,
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            },
        )
        .expect("dmabuf-capable device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: match std::env::var("SPIKE_PRESENT").as_deref() {
                Ok("mailbox") => wgpu::PresentMode::Mailbox,
                Ok("immediate") => wgpu::PresentMode::Immediate,
                _ => wgpu::PresentMode::AutoVsync,
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let u_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let ui_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &u_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline = |layouts: &[Option<&wgpu::BindGroupLayout>], fs: &str, blend| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: layouts,
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let scene = pipeline(&[Some(&u_layout)], "fs_scene", None);
        let ui = pipeline(
            &[Some(&u_layout), Some(&ui_layout)],
            "fs_ui",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        self.gfx = Some(Gfx {
            window,
            surface,
            config,
            host: HostWgpuContext::new(device, queue),
            scene,
            ui,
            uniform,
            uniform_group,
            ui_layout,
            sampler,
            ui_group: None,
            ui_frame: None,
        });
    }

    fn init_cef(&mut self) {
        let gfx = self.gfx.as_ref().unwrap();
        let cef_path = std::env::var("CEF_PATH").expect("CEF_PATH");
        let mut config = CefRuntimeConfig::new(cef_path, CefSandboxMode::UnsandboxedTrustedContent);
        config.command_line_switches = vec![
            ("no-first-run".into(), None),
            ("no-default-browser-check".into(), None),
        ];
        if let Ok(extra) = std::env::var("SPIKE_SWITCHES") {
            for switch in extra.split(',').filter(|s| !s.is_empty()) {
                let (name, value) = match switch.split_once('=') {
                    Some((n, v)) => (n.to_owned(), Some(v.to_owned())),
                    None => (switch.to_owned(), None),
                };
                config.command_line_switches.push((name, value));
            }
        }
        let t = Instant::now();
        let runtime = CefRuntime::initialize(config).expect("cef initialize");
        eprintln!("cef initialize: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        let producer = PlatformCefProducer::new(
            &runtime,
            PlatformCefConfig {
                surface: CefSurfaceConfig {
                    initial_url: self.hud_url.clone(),
                    initial_size: gfx.window.inner_size(),
                    background_color: None,
                    prefer_accelerated: true,
                    ..Default::default()
                },
            },
        )
        .expect("cef producer");
        self.runtime = Some(runtime);
        self.producer = Some(producer);
    }

    fn frame(&mut self) {
        let t = Instant::now();
        if let Some(runtime) = &self.runtime {
            let pump = Instant::now();
            runtime.do_message_loop_work();
            self.metrics.pump_ms.push(pump.elapsed().as_secs_f64() * 1e3);
        }
        let (Some(gfx), Some(producer)) = (&mut self.gfx, &mut self.producer) else {
            return;
        };
        while let Some(event) = producer.poll_navigation_event() {
            if let NavigationEvent::TitleChanged { title } = event {
                self.metrics.titles.push(format!(
                    "{:.0}ms {title}",
                    self.started.elapsed().as_secs_f64() * 1e3
                ));
            }
        }
        let import = Instant::now();
        match producer.acquire_frame(&gfx.host) {
            Ok(Some(frame)) => {
                self.metrics.import_ms.push(import.elapsed().as_secs_f64() * 1e3);
                self.metrics.ui_frames += 1;
                self.ui_this_second += 1;
                if self.metrics.first_ui_frame_ms.is_none() {
                    self.metrics.first_ui_frame_ms =
                        Some(self.started.elapsed().as_secs_f64() * 1e3);
                    eprintln!("ui frame format {:?} size {:?}", frame.format, frame.size);
                }
                if let Some(sent) = self.script_sent.take() {
                    self.metrics
                        .script_to_frame_ms
                        .push(sent.elapsed().as_secs_f64() * 1e3);
                }
                gfx.ui_group = Some(gfx.host.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &gfx.ui_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&frame.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&gfx.sampler),
                        },
                    ],
                }));
                gfx.ui_frame = Some(frame);
            }
            Ok(None) => {}
            Err(error) => eprintln!("acquire_frame: {error}"),
        }
        let time = self.started.elapsed().as_secs_f32();
        let aspect = gfx.config.width as f32 / gfx.config.height as f32;
        let mut bytes = Vec::new();
        for v in [time, aspect, 0.0, 0.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        gfx.host.queue.write_buffer(&gfx.uniform, 0, &bytes);
        let acquire = Instant::now();
        let surface_texture = match gfx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                gfx.surface.configure(&gfx.host.device, &gfx.config);
                return;
            }
        };
        self.metrics.acquire_ms.push(acquire.elapsed().as_secs_f64() * 1e3);
        let view = surface_texture.texture.create_view(&Default::default());
        let mut encoder = gfx.host.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gfx.scene);
            pass.set_bind_group(0, &gfx.uniform_group, &[]);
            pass.draw(0..3, 0..1);
            if let Some(group) = &gfx.ui_group {
                pass.set_pipeline(&gfx.ui);
                pass.set_bind_group(1, group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        let present = Instant::now();
        gfx.host.queue.submit([encoder.finish()]);
        gfx.host.queue.present(surface_texture);
        self.metrics.present_ms.push(present.elapsed().as_secs_f64() * 1e3);
        self.metrics.presented += 1;
        self.game_this_second += 1;
        self.metrics.frame_cpu_ms.push(t.elapsed().as_secs_f64() * 1e3);
        gfx.window.request_redraw();
    }

    fn script(&mut self) {
        let Some(producer) = &mut self.producer else { return };
        let elapsed = self.started.elapsed();
        if self.metrics.first_ui_frame_ms.is_none() {
            return;
        }
        if elapsed > Duration::from_secs(3) && !self.scripted_click {
            self.scripted_click = true;
            for action in [MouseAction::Moved, MouseAction::Pressed, MouseAction::Released] {
                producer
                    .send_mouse_input(MouseEvent {
                        x: 180,
                        y: 120,
                        button: MouseButton::Left,
                        action,
                        modifiers: EventModifiers::default(),
                    })
                    .unwrap();
            }
        }
        if elapsed > Duration::from_millis(3300) && elapsed < Duration::from_millis(3400) {
            let x = 180 + ((elapsed.as_millis() % 100) as i32);
            let _ = producer.send_mouse_input(MouseEvent { x, y: 120, button: MouseButton::Left, action: MouseAction::Moved, modifiers: EventModifiers::default() });
        }
        if elapsed > Duration::from_millis(3500) && !self.scripted_key {
            self.scripted_key = true;
            for kind in [KeyEventKind::RawKeyDown, KeyEventKind::Char, KeyEventKind::KeyUp] {
                producer
                    .send_keyboard_input(KeyEvent {
                        kind,
                        windows_key_code: if kind == KeyEventKind::Char { 'k' as i32 } else { 0x4B },
                        native_key_code: 0,
                        character: Some('k'),
                        modifiers: EventModifiers::default(),
                    })
                    .unwrap();
            }
        }
        // Latency probe: a DOM change from a script, to the imported frame.
        if self.script_sent.is_none() && Instant::now() >= self.next_ping && elapsed > Duration::from_secs(4) {
            self.ping += 1;
            producer
                .execute_script(
                    &format!("document.getElementById('ping').textContent = 'ping {}';", self.ping),
                    "spike",
                )
                .unwrap();
            self.script_sent = Some(Instant::now());
            self.next_ping = Instant::now() + Duration::from_millis(250);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_none() {
            self.init_gfx(event_loop);
            self.init_cef();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.config.width = size.width.max(1);
                    gfx.config.height = size.height.max(1);
                    gfx.surface.configure(&gfx.host.device, &gfx.config);
                }
                if let Some(producer) = &mut self.producer {
                    let _ = producer.resize(size);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if let Some(producer) = &mut self.producer {
                    let _ = producer.send_mouse_input(MouseEvent {
                        x: self.cursor.0,
                        y: self.cursor.1,
                        button: MouseButton::Left,
                        action: MouseAction::Moved,
                        modifiers: EventModifiers::default(),
                    });
                }
            }
            WindowEvent::MouseInput { state, .. } => {
                self.metrics.titles.push(format!("winit mouse {state:?}"));
                if let Some(producer) = &mut self.producer {
                    let _ = producer.send_mouse_input(MouseEvent {
                        x: self.cursor.0,
                        y: self.cursor.1,
                        button: MouseButton::Left,
                        action: if state == ElementState::Pressed {
                            MouseAction::Pressed
                        } else {
                            MouseAction::Released
                        },
                        modifiers: EventModifiers::default(),
                    });
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.script();
        self.frame();
        if self.second_start.elapsed() >= Duration::from_secs(1) {
            self.metrics.ui_per_s.push(std::mem::take(&mut self.ui_this_second));
            self.metrics.game_per_s.push(std::mem::take(&mut self.game_this_second));
            self.second_start = Instant::now();
        }
        if self.started.elapsed() > Duration::from_secs(self.seconds) {
            let m = &self.metrics;
            let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
            let mut s2f = m.script_to_frame_ms.clone();
            s2f.sort_by(f64::total_cmp);
            let pct = |q: f64| s2f.get(((s2f.len() as f64 - 1.0) * q) as usize).copied().unwrap_or(0.0);
            println!(
                "{{\"mode\":\"cef\",\"presented\":{},\"ui_frames\":{},\"first_ui_frame_ms\":{:?},\"import_ms_avg\":{:.3},\"frame_cpu_ms_avg\":{:.2},\"pump_ms_avg\":{:.2},\"acquire_ms_avg\":{:.2},\"present_ms_avg\":{:.2},\"script_to_frame_ms_p50\":{:.1},\"script_to_frame_ms_p95\":{:.1},\"game_per_s\":{:?},\"ui_per_s\":{:?},\"titles\":{:?}}}",
                m.presented, m.ui_frames, m.first_ui_frame_ms, avg(&m.import_ms), avg(&m.frame_cpu_ms), avg(&m.pump_ms), avg(&m.acquire_ms), avg(&m.present_ms), pct(0.5), pct(0.95), m.game_per_s, m.ui_per_s, m.titles
            );
            self.producer = None;
            self.gfx = None;
            event_loop.exit();
        }
    }
}

fn main() {
    let cef_path = std::env::var("CEF_PATH").expect("CEF_PATH");
    if let Some(code) =
        CefRuntime::execute_process_from(cef_path.as_ref(), CefSandboxMode::UnsandboxedTrustedContent)
            .expect("probe subprocess role")
    {
        std::process::exit(code);
    }
    let mut args = std::env::args().skip(1);
    let seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let hud = args
        .next()
        .unwrap_or_else(|| format!("file://{}/hud.html", env!("CARGO_MANIFEST_DIR")));
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let now = Instant::now();
    let mut app = App {
        seconds,
        hud_url: hud,
        started: now,
        runtime: None,
        gfx: None,
        producer: None,
        metrics: Metrics::default(),
        script_sent: None,
        next_ping: now,
        ping: 0,
        scripted_click: false,
        scripted_key: false,
        second_start: now,
        ui_this_second: 0,
        game_this_second: 0,
        cursor: (0, 0),
    };
    event_loop.run_app(&mut app).unwrap();
    drop(app.runtime.take());
}
