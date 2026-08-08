use std::time::{Duration, Instant};

use render_host_contracts::RendererCameraPose;
use renderer_webview_host::{
    RendererWebviewAdapter, RendererWebviewObservation, RendererWebviewOptions,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Default)]
struct SmokeApplication {
    window: Option<Window>,
    renderer: Option<RendererWebviewAdapter>,
    started_at: Option<Instant>,
    render_request: Option<u64>,
    frame_request: Option<u64>,
    presentation_request: Option<u64>,
    camera_request: Option<u64>,
    state_request: Option<u64>,
    dispose_request: Option<u64>,
    document_loaded: bool,
    completed: bool,
}

impl ApplicationHandler for SmokeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Rusty Engine Rust-only renderer smoke")
                    .with_inner_size(winit::dpi::LogicalSize::new(320, 240)),
            )
            .expect("create smoke window");
        let renderer = RendererWebviewAdapter::mount(
            &window,
            RendererWebviewOptions {
                auto_start: false,
                bounds: renderer_webview_host::RendererWebviewBounds {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                },
                ..RendererWebviewOptions::default()
            },
        )
        .expect("mount renderer webview");
        self.started_at = Some(Instant::now());
        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "linux")]
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }

        if self
            .started_at
            .is_some_and(|started| started.elapsed() > Duration::from_secs(30))
        {
            panic!(
                "renderer webview smoke timed out (document_loaded={})",
                self.document_loaded
            );
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        for observation in renderer.drain_observations() {
            match observation.expect("decode renderer observation") {
                RendererWebviewObservation::DocumentLoaded => {
                    self.document_loaded = true;
                }
                RendererWebviewObservation::Ready(_) if self.frame_request.is_none() => {
                    self.frame_request = Some(
                        renderer
                            .submit_frame(&render_model::RenderFrameDiff::new())
                            .expect("submit Rust retained frame"),
                    );
                }
                RendererWebviewObservation::FrameApplied { request_id, .. }
                    if Some(request_id) == self.frame_request =>
                {
                    self.presentation_request = Some(
                        renderer
                            .submit_presentation(&render_presentation::PresentationFrameDiff::new())
                            .expect("submit Rust presentation frame"),
                    );
                }
                RendererWebviewObservation::PresentationApplied { request_id, .. }
                    if Some(request_id) == self.presentation_request =>
                {
                    self.camera_request = Some(
                        renderer
                            .set_camera_pose(
                                RendererCameraPose {
                                    position: [0.0, 1.62, 8.0],
                                    pitch_degrees: 0.0,
                                    yaw_degrees: 0.0,
                                },
                                None,
                            )
                            .expect("set Rust camera pose"),
                    );
                }
                RendererWebviewObservation::CameraUpdated { request_id, .. }
                    if Some(request_id) == self.camera_request =>
                {
                    self.state_request = Some(renderer.read_state().expect("read mounted state"));
                }
                RendererWebviewObservation::StateRead { request_id, .. }
                    if Some(request_id) == self.state_request =>
                {
                    if self.render_request.is_none() {
                        self.render_request =
                            Some(renderer.render_once(Some(1.0)).expect("render once"));
                    }
                }
                RendererWebviewObservation::FrameRendered {
                    request_id,
                    readout,
                } if Some(request_id) == self.render_request => {
                    assert!(readout.render_sequence > 0);
                    self.dispose_request = Some(renderer.dispose().expect("dispose renderer"));
                }
                RendererWebviewObservation::Disposed { request_id }
                    if Some(request_id) == self.dispose_request =>
                {
                    println!("RUST_ONLY_WEBVIEW_RENDERER_OK");
                    self.completed = true;
                    event_loop.exit();
                }
                RendererWebviewObservation::MountFailed { message } => {
                    panic!("renderer mount failed: {message}");
                }
                RendererWebviewObservation::OperationFailed {
                    operation, message, ..
                } => {
                    panic!("renderer operation {operation:?} failed: {message}");
                }
                _ => {}
            }
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    gtk::init().expect("initialize GTK for the X11 webview host");
    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut application = SmokeApplication::default();
    let result = event_loop.run_app(&mut application);
    if !application.completed {
        result.expect("run renderer smoke application");
        panic!("renderer smoke event loop exited before completion");
    }
}
