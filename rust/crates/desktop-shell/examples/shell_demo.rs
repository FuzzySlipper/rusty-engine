//! The desktop shell over a small retained scene, with a UI page.
//!
//! ```text
//! CEF_PATH=<cef dir> cargo run -p desktop-shell --features web-overlay \
//!     --example shell_demo -- <cef dir> <ui url> [seconds]
//! ```
//!
//! A spinning cube stands in for a product's scene; the page is any URL,
//! normally a UI probe. The shell report prints on exit.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use desktop_shell::{
    DesktopScene, DesktopShell, DesktopShellConfig, Gpu, Renderer, RendererOptions,
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
        }],
        presentations: Vec::new(),
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let cef_dir = std::path::PathBuf::from(args.next().ok_or("usage: <cef dir> <url> [seconds]")?);
    if let Some(code) = desktop_shell::run_web_subprocess(&cef_dir)? {
        std::process::exit(code);
    }
    let url = args.next().ok_or("missing UI url")?;
    let seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let switches = std::env::var("DEMO_SWITCHES")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|switch| match switch.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (switch.to_owned(), None),
        })
        .collect();
    let shell = DesktopShell::open()?;
    let scene = Arc::new(SpinningCube::build(shell.gpu(), seconds));
    let report = shell.run(
        DesktopShellConfig {
            title: "desktop shell demo".to_owned(),
            width: 1280,
            height: 720,
            ui_url: Some(url),
            placement_file: None,
            web: WebRuntimeConfig {
                cef_dir,
                cache_dir: None,
                switches,
            },
        },
        scene,
    )?;
    println!("{report:?}");
    Ok(())
}
