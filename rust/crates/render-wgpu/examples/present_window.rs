//! Present the retained scene to a native window through `WindowSurface`.
//!
//! ```text
//! cargo run -p render-wgpu --example present_window
//! ```
//!
//! The desktop shell (#8790) owns the real window; this shows the calls it
//! makes: `Gpu::for_window`, `WindowSurface::resize` and
//! `Renderer::render_to_surface`. It closes after `FRAMES` presented frames.

use std::sync::Arc;

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
};
use render_model::{
    Geometry, Material, RenderDiff, RenderFrameDiff, RenderHandle, RenderNode, Transform,
};
use render_wgpu::{Gpu, NoResources, Renderer, RendererOptions, WindowSurface};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const FRAMES: u32 = 120;

struct State {
    window: Arc<Window>,
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    presented: u32,
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

fn camera() -> RendererCompositionCamera {
    RendererCompositionCamera {
        id: "window".to_owned(),
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("render-wgpu"))
                .expect("create window"),
        );
        let size = window.inner_size();
        let (gpu, surface) =
            Gpu::for_window(window.clone(), size.width, size.height).expect("window gpu");
        eprintln!("adapter: {:?}", gpu.adapter_summary());
        let mut renderer = Renderer::new(&gpu, RendererOptions::default());
        let mut cube = RenderNode::new(Geometry::Cube);
        cube.material = Material {
            color: [0.9, 0.3, 0.2, 1.0],
            wireframe: false,
        };
        cube.transform = Transform {
            rotation: [0.2, 0.4, 0.0, 0.89],
            ..Transform::IDENTITY
        };
        let issues = renderer.apply(
            &RenderFrameDiff {
                ops: vec![
                    RenderDiff::SetBackgroundColor {
                        color: [0.05, 0.08, 0.12, 1.0],
                    },
                    RenderDiff::Create {
                        handle: RenderHandle::new(1),
                        parent: None,
                        node: cube,
                    },
                ],
                ..RenderFrameDiff::new()
            },
            &NoResources,
        );
        assert!(issues.is_empty(), "{issues:?}");
        self.state = Some(State {
            window,
            gpu,
            surface,
            renderer,
            presented: 0,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.surface.resize(&state.gpu, size.width, size.height),
            WindowEvent::RedrawRequested => {
                if state
                    .renderer
                    .render_to_surface(&camera(), &mut state.surface)
                    .is_ok()
                {
                    state.presented += 1;
                }
                if state.presented >= FRAMES {
                    eprintln!("presented {} frames", state.presented);
                    event_loop.exit();
                } else {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::default()).expect("run");
}
