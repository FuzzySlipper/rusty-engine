//! Print every DeviceEvent::MouseMotion winit delivers while the cursor is
//! grabbed, to count raw motion per physical motion.
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(event_loop.create_window(Window::default_attributes().with_title("winitprobe")).unwrap());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } => {
                let window = self.window.as_ref().unwrap();
                if std::env::var_os("NO_GRAB").is_some() {
                    window.set_cursor_visible(false);
                    println!("hidden, no grab");
                    return;
                }
                let grab = window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .map(|_| "locked")
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined).map(|_| "confined"));
                println!("grab {grab:?}");
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, device: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            println!("motion {device:?} {delta:?}");
        }
    }
}

fn main() {
    EventLoop::new().unwrap().run_app(&mut App::default()).unwrap();
}
