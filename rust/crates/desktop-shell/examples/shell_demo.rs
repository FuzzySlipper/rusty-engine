//! The desktop shell over a small retained scene, with a UI page.
//!
//! ```text
//! CEF_PATH=<cef dir> cargo run -p desktop-shell --features web-overlay \
//!     --example shell_demo -- <cef dir> <ui url> [seconds]
//! cargo run -p desktop-shell --features servo-overlay \
//!     --example shell_demo -- servo <ui url> [seconds]
//! ```
//!
//! A spinning cube stands in for a product's scene; the page is any URL,
//! normally a UI probe, drawn by Chromium from `<cef dir>` or by Servo. The
//! shell report prints on exit.
//!
//! Linux only. On Windows libcef stops at a breakpoint on one of its own
//! threads shortly after the browser starts, whatever the build profile,
//! page, GPU mode or CEF copy, while the packaged desktop host runs; use
//! `rusty dev --output window` there.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use desktop_shell::{
    DesktopScene, DesktopShell, DesktopShellConfig, Gpu, Renderer, RendererOptions, WebEngine,
    WebRuntimeConfig,
};
use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
};
use render_model::{
    Geometry, Material, RenderDiff, RenderFrameDiff, RenderHandle, RenderNode, Transform,
};
use render_wgpu::NoResources;

const CUBE: RenderHandle = RenderHandle::new(1);

struct SpinningCube {
    renderer: Mutex<Renderer>,
    started: Instant,
    seconds: u64,
    closed: AtomicBool,
}

impl SpinningCube {
    fn build(gpu: &Gpu, seconds: u64) -> Self {
        let mut renderer = Renderer::new(gpu, RendererOptions::default());
        let mut cube = RenderNode::new(Geometry::Cube);
        cube.material = Material {
            color: [0.9, 0.3, 0.2, 1.0],
            wireframe: false,
        };
        renderer.apply(
            &RenderFrameDiff {
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
            },
            &NoResources,
        );
        renderer.set_view_composition(&composition(), 0.0);
        Self {
            renderer: Mutex::new(renderer),
            started: Instant::now(),
            seconds,
            closed: AtomicBool::new(false),
        }
    }
}

impl DesktopScene for SpinningCube {
    fn draw(&self, draw: &mut dyn FnMut(&mut Renderer, f64)) -> u64 {
        let mut renderer = self.renderer.lock().unwrap();
        let (s, c) = (self.started.elapsed().as_secs_f32() * 0.5).sin_cos();
        renderer.apply(
            &RenderFrameDiff {
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
            },
            &NoResources,
        );
        draw(&mut renderer, self.started.elapsed().as_secs_f64());
        0
    }

    fn presented(&self, _: desktop_shell::PresentedFrame) {}

    fn input_received(&self, _: std::time::SystemTime) {}

    fn stopped(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
            || self.started.elapsed() > Duration::from_secs(self.seconds)
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

fn composition() -> RendererViewComposition {
    RendererViewComposition {
        cameras: vec![RendererCompositionCamera {
            id: "demo".to_owned(),
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
            viewmodel_fov_y_degrees: 0.0,
        }],
        targets: Vec::new(),
        views: vec![RendererCompositionView {
            id: "main".to_owned(),
            camera_id: "demo".to_owned(),
            target: RendererViewTarget::Primary,
            viewport: RendererViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            order: 0,
            viewport_anchor: None,
        }],
        presentations: Vec::new(),
    }
}

fn main() -> Result<(), String> {
    if cfg!(windows) {
        return Err(
            "shell_demo is Linux-only; on Windows use rusty dev --output window".to_owned(),
        );
    }
    let mut args = std::env::args().skip(1);
    let engine = args
        .next()
        .ok_or("usage: <cef dir | servo> <url> [seconds]")?;
    let engine = if engine == "servo" {
        servo()?
    } else {
        chromium(std::path::PathBuf::from(engine))?
    };
    let url = args.next().ok_or("missing UI url")?;
    let seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let shell = DesktopShell::open()?;
    let scene = Arc::new(SpinningCube::build(shell.gpu(), seconds));
    let report = shell.run(
        DesktopShellConfig {
            title: "desktop shell demo".to_owned(),
            width: 1280,
            height: 720,
            maximized: false,
            ui_url: Some(url),
            placement_file: None,
            web: WebRuntimeConfig {
                engine,
                cache_dir: None,
            },
        },
        scene,
    )?;
    println!("{report:?}");
    Ok(())
}

#[cfg(feature = "web-overlay")]
fn chromium(cef_dir: std::path::PathBuf) -> Result<WebEngine, String> {
    if let Some(code) = desktop_shell::run_web_subprocess(&cef_dir)? {
        std::process::exit(code);
    }
    let switches = std::env::var("DEMO_SWITCHES")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|switch| match switch.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (switch.to_owned(), None),
        })
        .collect();
    Ok(WebEngine::Chromium { cef_dir, switches })
}

#[cfg(not(feature = "web-overlay"))]
fn chromium(_: std::path::PathBuf) -> Result<WebEngine, String> {
    Err("built without web-overlay: pass servo".to_owned())
}

#[cfg(feature = "servo-overlay")]
fn servo() -> Result<WebEngine, String> {
    Ok(WebEngine::Servo {
        preferences: Vec::new(),
    })
}

#[cfg(not(feature = "servo-overlay"))]
fn servo() -> Result<WebEngine, String> {
    Err("built without servo-overlay".to_owned())
}
