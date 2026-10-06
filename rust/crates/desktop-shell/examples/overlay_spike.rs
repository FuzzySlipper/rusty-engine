//! Overlay pattern spike for #8790: measure the two in-process patterns.
//!
//! ```text
//! cargo run -p desktop-shell --example overlay_spike -- child  [seconds]
//! cargo run -p desktop-shell --example overlay_spike -- canvas [seconds]
//! ```
//!
//! - `child`: wgpu presents to the winit window's surface; a transparent wry
//!   child webview covers it with a DOM HUD (pattern 1).
//! - `canvas`: wgpu renders offscreen and reads back; the webview fetches each
//!   frame over a custom protocol and draws it into a canvas under the same
//!   DOM HUD (pattern 2).
//!
//! It prints one JSON line of measurements and exits after `seconds`.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
};
use render_model::{
    Geometry, Material, RenderDiff, RenderFrameDiff, RenderHandle, RenderNode, Transform,
};
use render_wgpu::{Gpu, NoResources, OffscreenTarget, Renderer, RendererOptions, WindowSurface};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use wry::{
    dpi::{LogicalPosition, LogicalSize},
    http::{header::CONTENT_TYPE, Request, Response},
    Rect, WebView, WebViewBuilder,
};

const HUD: &str = r#"
<div id="hud" style="position:absolute;left:12px;top:12px;font:16px sans-serif;color:#fff;
  background:rgba(0,0,0,.45);padding:6px 10px;border-radius:4px">HUD <span id="fps">-</span></div>
<button id="btn" style="position:absolute;left:100px;top:100px;width:160px;height:40px">Menu</button>
"#;

fn page(mode: &str) -> String {
    let canvas = if mode == "canvas" {
        r#"<canvas id="frame" style="position:absolute;inset:0;width:100%;height:100%"></canvas>"#
    } else {
        ""
    };
    let bg = if mode == "canvas" {
        "#000"
    } else {
        "transparent"
    };
    PAGE.replace("__BG__", bg)
        .replace("__CANVAS__", canvas)
        .replace("__HUD__", HUD)
        .replace("__MODE__", mode)
}

const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;width:100%;height:100%;overflow:hidden;background:__BG__}</style></head>
<body>__CANVAS____HUD__<script>
const post = (m) => window.ipc.postMessage(m);
let raf = 0, t0 = performance.now();
document.addEventListener('pointerdown', e => post('pointer:' + e.target.id + ':' + e.clientX + ',' + e.clientY));
document.addEventListener('keydown', e => post('key:' + e.code));
const mode = '__MODE__';
const canvas = document.getElementById('frame');
const ctx = canvas ? canvas.getContext('webgl2') : null;
let tex = null;
if (ctx) {
  const gl = ctx;
  const vs = gl.createShader(gl.VERTEX_SHADER);
  gl.shaderSource(vs, `#version 300 es
  out vec2 uv; void main(){ vec2 p = vec2(float((gl_VertexID<<1)&2), float(gl_VertexID&2)); uv = vec2(p.x, 1.0-p.y) ; gl_Position = vec4(p*2.0-1.0,0,1);} `);
  gl.compileShader(vs);
  const fs = gl.createShader(gl.FRAGMENT_SHADER);
  gl.shaderSource(fs, `#version 300 es
  precision mediump float; in vec2 uv; uniform sampler2D t; out vec4 c; void main(){ c = texture(t, uv);} `);
  gl.compileShader(fs);
  const prog = gl.createProgram(); gl.attachShader(prog, vs); gl.attachShader(prog, fs); gl.linkProgram(prog); gl.useProgram(prog);
  tex = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
}
let busy = false, shown = 0, lastSeq = -1;
async function pull() {
  if (busy) return; busy = true;
  try {
    const r = await fetch('rusty://localhost/frame');
    const buf = await r.arrayBuffer();
    const head = new DataView(buf, 0, 16);
    const w = head.getUint32(0, true), h = head.getUint32(4, true), seq = head.getUint32(8, true);
    if (w > 0 && seq !== lastSeq) {
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; ctx.viewport(0,0,w,h); }
      ctx.texImage2D(ctx.TEXTURE_2D, 0, ctx.RGBA, w, h, 0, ctx.RGBA, ctx.UNSIGNED_BYTE, new Uint8Array(buf, 16, w * h * 4));
      ctx.drawArrays(ctx.TRIANGLES, 0, 3);
      lastSeq = seq; shown++; post('shown:' + seq);
    }
  } finally { busy = false; }
}
function tick() {
  raf++;
  if (ctx) pull();
  const t = performance.now();
  if (t - t0 >= 1000) { document.getElementById('fps').textContent = raf + ' raf/s'; post('raf:' + raf + ':' + shown); raf = 0; shown = 0; t0 = t; }
  requestAnimationFrame(tick);
}
requestAnimationFrame(tick);
post('loaded');
</script></body></html>"#;

fn camera() -> RendererCompositionCamera {
    RendererCompositionCamera {
        id: "spike".to_owned(),
        pose: RendererCameraPose {
            position: [0.0, 0.0, 3.0],
            pitch_degrees: 0.0,
            yaw_degrees: 0.0,
        },
        basis: None,
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        },
        motion: None,
    }
}

const CUBE: RenderHandle = RenderHandle::new(1);

fn scene() -> RenderFrameDiff {
    let mut cube = RenderNode::new(Geometry::Cube);
    cube.material = Material {
        color: [0.9, 0.3, 0.2, 1.0],
        wireframe: false,
    };
    RenderFrameDiff {
        ops: vec![
            RenderDiff::SetBackgroundColor {
                color: [0.05, 0.3, 0.12, 1.0],
            },
            RenderDiff::Create {
                handle: CUBE,
                parent: None,
                node: cube,
            },
        ],
        ..RenderFrameDiff::new()
    }
}

fn spin(angle: f32) -> RenderFrameDiff {
    let (s, c) = (angle * 0.5).sin_cos();
    RenderFrameDiff {
        ops: vec![RenderDiff::Update {
            handle: CUBE,
            transform: Some(Transform {
                rotation: [0.3 * s, s, 0.0, c],
                ..Transform::IDENTITY
            }),
            material: None,
            visible: None,
            metadata: None,
        }],
        ..RenderFrameDiff::new()
    }
}

/// The latest read-back frame: 16-byte header (width, height, seq, 0) then RGBA.
type Shared = Arc<Mutex<Arc<Vec<u8>>>>;

#[derive(Default)]
struct Metrics {
    rendered: u32,
    render_ms: Vec<f64>,
    readback_ms: Vec<f64>,
    raf: Vec<u32>,
    shown: Vec<u32>,
    latency_ms: Vec<f64>,
    ipc: Vec<String>,
    window_events: Vec<String>,
    loaded_after_ms: Option<f64>,
}

struct State {
    window: Arc<Window>,
    gpu: Gpu,
    surface: Option<WindowSurface>,
    target: Option<OffscreenTarget>,
    renderer: Renderer,
    _webview: WebView,
    shared: Shared,
    seq_times: HashMap<u32, Instant>,
    pixels: Vec<u8>,
}

struct App {
    mode: String,
    seconds: u64,
    started: Instant,
    last_render: Instant,
    size: (u32, u32),
    state: Option<State>,
    metrics: Rc<RefCell<Metrics>>,
}

impl App {
    fn render(&mut self) {
        let Some(state) = &mut self.state else { return };
        let angle = self.started.elapsed().as_secs_f32();
        state.renderer.apply(&spin(angle), &NoResources);
        let mut metrics = self.metrics.borrow_mut();
        let t = Instant::now();
        if let Some(surface) = &mut state.surface {
            if state.renderer.render_to_surface(&camera(), surface).is_ok() {
                metrics.rendered += 1;
            }
            metrics.render_ms.push(t.elapsed().as_secs_f64() * 1e3);
        } else if let Some(target) = &state.target {
            state.renderer.render_offscreen(&camera(), target);
            let rendered = Instant::now();
            state
                .target
                .as_ref()
                .unwrap()
                .read_rgba_into(&state.gpu, &mut state.pixels);
            metrics.render_ms.push((rendered - t).as_secs_f64() * 1e3);
            metrics
                .readback_ms
                .push(rendered.elapsed().as_secs_f64() * 1e3);
            metrics.rendered += 1;
            let seq = metrics.rendered;
            let (w, h) = target.size();
            let mut frame = Vec::with_capacity(16 + state.pixels.len());
            for word in [w, h, seq, 0] {
                frame.extend_from_slice(&word.to_le_bytes());
            }
            frame.extend_from_slice(&state.pixels);
            *state.shared.lock().unwrap() = Arc::new(frame);
            self.state.as_mut().unwrap().seq_times.insert(seq, t);
        }
    }

    fn drain_ipc(&mut self) {
        let Some(state) = &mut self.state else { return };
        let mut metrics = self.metrics.borrow_mut();
        let messages = std::mem::take(&mut metrics.ipc);
        for message in messages {
            if let Some(seq) = message.strip_prefix("shown:") {
                if let Some(at) = seq
                    .parse()
                    .ok()
                    .and_then(|s: u32| state.seq_times.remove(&s))
                {
                    metrics.latency_ms.push(at.elapsed().as_secs_f64() * 1e3);
                }
            } else if let Some(rest) = message.strip_prefix("raf:") {
                let mut parts = rest.split(':');
                metrics.raf.push(parts.next().unwrap().parse().unwrap());
                metrics.shown.push(parts.next().unwrap().parse().unwrap());
            } else if message == "loaded" {
                metrics.loaded_after_ms = Some(self.started.elapsed().as_secs_f64() * 1e3);
            } else {
                metrics.window_events.push(format!("dom {message}"));
            }
        }
        state
            .seq_times
            .retain(|_, at| at.elapsed() < Duration::from_secs(2));
    }

    fn report(&self) {
        let m = self.metrics.borrow();
        let avg = |v: &[f64]| {
            if v.is_empty() {
                0.0
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };
        let p = |v: &[f64], q: f64| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            v.get(((v.len() as f64 - 1.0) * q) as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let secs = self.started.elapsed().as_secs_f64();
        println!(
            "{{\"mode\":\"{}\",\"seconds\":{secs:.1},\"rendered_fps\":{:.1},\"render_ms_avg\":{:.2},\"readback_ms_avg\":{:.2},\"raf_per_s\":{:?},\"shown_per_s\":{:?},\"latency_ms_p50\":{:.1},\"latency_ms_p95\":{:.1},\"page_loaded_ms\":{:?},\"events\":{:?}}}",
            self.mode,
            m.rendered as f64 / secs,
            avg(&m.render_ms),
            avg(&m.readback_ms),
            m.raf,
            m.shown,
            p(&m.latency_ms, 0.5),
            p(&m.latency_ms, 0.95),
            m.loaded_after_ms,
            m.window_events,
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(format!("overlay spike: {}", self.mode))
                        .with_inner_size(winit::dpi::PhysicalSize::new(self.size.0, self.size.1)),
                )
                .expect("create window"),
        );
        let size = window.inner_size();
        let (gpu, surface, target) = if self.mode == "child" {
            let (gpu, surface) =
                Gpu::for_window(window.clone(), size.width, size.height).expect("window gpu");
            (gpu, Some(surface), None)
        } else {
            let gpu = Gpu::headless().expect("headless gpu");
            let target = OffscreenTarget::new(&gpu, size.width, size.height, 4);
            (gpu, None, Some(target))
        };
        eprintln!("adapter: {:?}", gpu.adapter_summary());
        let mut renderer = Renderer::new(&gpu, RendererOptions::default());
        renderer.apply(&scene(), &NoResources);

        let shared: Shared = Arc::new(Mutex::new(Arc::new(vec![0; 16])));
        let frame = shared.clone();
        let html = page(&self.mode);
        let metrics = self.metrics.clone();
        let webview = WebViewBuilder::new()
            .with_transparent(self.mode == "child")
            .with_bounds(Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: LogicalSize::new(size.width, size.height).into(),
            })
            .with_custom_protocol("rusty".into(), move |_, request: Request<Vec<u8>>| {
                let (body, kind): (Cow<'static, [u8]>, _) = match request.uri().path() {
                    "/frame" => {
                        let latest = frame.lock().unwrap().clone();
                        (
                            Cow::Owned(latest.as_ref().clone()),
                            "application/octet-stream",
                        )
                    }
                    _ => (Cow::Owned(html.clone().into_bytes()), "text/html"),
                };
                Response::builder()
                    .header(CONTENT_TYPE, kind)
                    .body(body)
                    .unwrap()
            })
            .with_url("rusty://localhost/index.html")
            .with_ipc_handler(move |request: Request<String>| {
                metrics.borrow_mut().ipc.push(request.body().clone());
            })
            .build_as_child(&window)
            .expect("child webview");
        let pixels = Vec::new();
        self.state = Some(State {
            window,
            gpu,
            surface,
            target,
            renderer,
            _webview: webview,
            shared,
            seq_times: HashMap::new(),
            pixels,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    if let Some(surface) = &mut state.surface {
                        surface.resize(&state.gpu, size.width, size.height);
                    }
                }
            }
            WindowEvent::RedrawRequested => {}
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.metrics
                    .borrow_mut()
                    .window_events
                    .push(format!("winit key {:?}", event.physical_key));
            }
            WindowEvent::MouseInput { state, button, .. } if *state == ElementState::Pressed => {
                self.metrics
                    .borrow_mut()
                    .window_events
                    .push(format!("winit mouse {button:?}"));
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "linux")]
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        if self.last_render.elapsed() >= Duration::from_micros(16_600) || self.mode == "child" {
            self.last_render = Instant::now();
            self.render();
        }
        self.drain_ipc();
        if self.started.elapsed() > Duration::from_secs(self.seconds) {
            self.report();
            event_loop.exit();
        }
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
        if self.mode != "child" {
            std::thread::sleep(Duration::from_micros(500));
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "child".to_owned());
    let seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    #[cfg(target_os = "linux")]
    gtk::init().expect("initialize GTK");
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        mode,
        seconds,
        started: Instant::now(),
        last_render: Instant::now(),
        size: (
            std::env::var("SPIKE_W")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1280),
            std::env::var("SPIKE_H")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(720),
        ),
        state: None,
        metrics: Rc::default(),
    };
    event_loop.run_app(&mut app).expect("run");
}
