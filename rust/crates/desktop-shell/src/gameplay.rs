//! Gameplay input the window hands the runtime itself while it holds the
//! pointer lock, so a busy UI page cannot delay movement and look. The page
//! still receives every event, for its UI; it sends no gameplay input while
//! the window holds it. Facts are the ones the page's input capture makes
//! from the same events. The gamepad, read by the window too
//! ([`crate::gamepad`]), goes the same way.

use std::collections::BTreeSet;
use std::sync::Arc;

use runtime_input::{
    AxisValue, InputClearReason, KeyboardControl, PhysicalEdge, PointerButton, RuntimeInputFact,
};
use winit::keyboard::KeyCode;

use crate::gamepad::{self, PadState};
use crate::{keys, DesktopScene};

/// The page's input capture's bound on one wheel event's delta.
const WHEEL_DELTA_MAXIMUM: f64 = 256.0;

pub(crate) struct GameplayRoute {
    scene: Arc<dyn DesktopScene>,
    /// The window holds gameplay input now.
    routed: bool,
    /// Keys down in the window, held input or not: taking input presses the
    /// ones held then, as the page's capture keeps them across a lock.
    down: BTreeSet<KeyboardControl>,
    /// Buttons whose press the runtime had from the window. The press that
    /// took the lock is not one, as the page's capture leaves it out.
    pressed: BTreeSet<PointerButton>,
    motion: (f64, f64),
    /// The window reads the gamepad, so the page's controller facts no
    /// longer count while it holds input.
    controller: bool,
    /// The pad as last read, held input or not, and what the runtime has
    /// of it from the window.
    pad: PadState,
    sent: PadState,
}

impl GameplayRoute {
    /// With `controller`, the window reads the gamepad too.
    pub(crate) fn new(scene: Arc<dyn DesktopScene>, controller: bool) -> Self {
        Self {
            scene,
            routed: false,
            down: BTreeSet::new(),
            pressed: BTreeSet::new(),
            motion: (0.0, 0.0),
            controller,
            pad: PadState::default(),
            sent: PadState::default(),
        }
    }

    /// The window took the pointer lock.
    pub(crate) fn take(&mut self) {
        if self.routed {
            return;
        }
        self.routed = true;
        self.scene.route_gameplay(true, self.controller);
        let mut held: Vec<_> = self
            .down
            .iter()
            .map(|&code| key(code, PhysicalEdge::Pressed))
            .collect();
        if self.controller {
            held.extend(gamepad::changes(&PadState::default(), &self.pad));
            self.sent = self.pad;
        }
        self.scene.gameplay(held);
    }

    /// The lock ended for `reason`: the page takes input back.
    pub(crate) fn give_back(&mut self, reason: InputClearReason) {
        if !self.routed {
            return;
        }
        self.routed = false;
        self.pressed.clear();
        self.motion = (0.0, 0.0);
        self.sent = PadState::default();
        self.scene
            .gameplay(vec![RuntimeInputFact::Clear { reason }]);
        self.scene.route_gameplay(false, self.controller);
    }

    /// The gamepad's state changed.
    pub(crate) fn pad(&mut self, state: PadState) {
        self.pad = state;
        if !self.routed || !self.controller {
            return;
        }
        let facts = gamepad::changes(&self.sent, &state);
        self.sent = state;
        if !facts.is_empty() {
            self.scene.gameplay(facts);
        }
    }

    /// Keys the window lost track of (another window has focus).
    pub(crate) fn forget_keys(&mut self) {
        self.down.clear();
    }

    pub(crate) fn key(&mut self, code: KeyCode, pressed: bool) {
        let Some(control) = keys::gameplay_key(code) else {
            return;
        };
        // A repeat is no new press, and a release without a press is none.
        let changed = if pressed {
            self.down.insert(control)
        } else {
            self.down.remove(&control)
        };
        if self.routed && changed {
            self.scene.gameplay(vec![key(control, edge(pressed))]);
        }
    }

    /// Left, middle or right (`index` 0, 1, 2), as DOM `button` numbers them.
    pub(crate) fn button(&mut self, index: usize, pressed: bool) {
        let button = match index {
            0 => PointerButton::Primary,
            1 => PointerButton::Middle,
            2 => PointerButton::Secondary,
            _ => return,
        };
        if !self.routed {
            return;
        }
        let changed = if pressed {
            self.pressed.insert(button)
        } else {
            self.pressed.remove(&button)
        };
        if changed {
            self.scene.gameplay(vec![RuntimeInputFact::PointerButton {
                button,
                edge: edge(pressed),
                position: None,
            }]);
        }
    }

    /// Raw mouse motion, rightward and downward positive like the page's
    /// `movementX`/`movementY`.
    pub(crate) fn motion(&mut self, x: f64, y: f64) {
        if self.routed {
            self.motion.0 += x;
            self.motion.1 += y;
        }
    }

    /// Hand the motion since the last flush over as one delta.
    pub(crate) fn flush_motion(&mut self) {
        let (x, y) = std::mem::take(&mut self.motion);
        if !self.routed || (x == 0.0 && y == 0.0) {
            return;
        }
        if let (Ok(x), Ok(y)) = (AxisValue::new(x as f32), AxisValue::new(y as f32)) {
            self.scene
                .gameplay(vec![RuntimeInputFact::PointerDelta { x, y }]);
        }
    }

    /// A wheel event the page receives as `delta_x`/`delta_y` (positive up
    /// and left, in pixels): `WheelEvent.deltaX`/`deltaY` are their negation.
    pub(crate) fn wheel(&mut self, delta_x: i32, delta_y: i32) {
        if !self.routed {
            return;
        }
        let dom = |delta: i32| (-f64::from(delta)).clamp(-WHEEL_DELTA_MAXIMUM, WHEEL_DELTA_MAXIMUM);
        let (x, y) = (dom(delta_x), dom(delta_y));
        if x == 0.0 && y == 0.0 {
            return;
        }
        if let (Ok(x), Ok(y)) = (AxisValue::new(x as f32), AxisValue::new(y as f32)) {
            self.scene.gameplay(vec![RuntimeInputFact::Wheel { x, y }]);
        }
    }
}

fn edge(pressed: bool) -> PhysicalEdge {
    if pressed {
        PhysicalEdge::Pressed
    } else {
        PhysicalEdge::Released
    }
}

fn key(code: KeyboardControl, edge: PhysicalEdge) -> RuntimeInputFact {
    RuntimeInputFact::Key { code, edge }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::SystemTime;

    use render_wgpu::Renderer;

    use super::*;

    /// What the window handed the runtime, in order.
    #[derive(Debug, Clone, PartialEq)]
    enum Handed {
        /// Routed, and whether the window reads the controller.
        Route(bool, bool),
        Facts(Vec<RuntimeInputFact>),
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Handed>>);

    impl DesktopScene for Recorder {
        fn draw(&self, _: &mut dyn FnMut(&mut Renderer, f64)) -> u64 {
            0
        }
        fn presented(&self, _: crate::PresentedFrame) {}
        fn input_received(&self, _: SystemTime) {}
        fn route_gameplay(&self, routed: bool, controller: bool) {
            self.0
                .lock()
                .unwrap()
                .push(Handed::Route(routed, controller));
        }
        fn gameplay(&self, facts: Vec<RuntimeInputFact>) {
            self.0.lock().unwrap().push(Handed::Facts(facts));
        }
        fn stopped(&self) -> bool {
            false
        }
        fn close(&self) {}
    }

    fn route() -> (GameplayRoute, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        (
            GameplayRoute::new(Arc::clone(&recorder) as _, false),
            recorder,
        )
    }

    fn taken(recorder: &Recorder) -> Vec<Handed> {
        std::mem::take(&mut *recorder.0.lock().unwrap())
    }

    fn key_fact(code: KeyboardControl, edge: PhysicalEdge) -> Handed {
        Handed::Facts(vec![key(code, edge)])
    }

    fn axis(value: f32) -> AxisValue {
        AxisValue::new(value).unwrap()
    }

    #[test]
    fn taking_input_presses_the_keys_held_and_giving_it_back_clears() {
        let (mut route, recorder) = route();
        // Before the lock the page has the keys; the window only keeps track.
        route.key(KeyCode::KeyW, true);
        route.key(KeyCode::Tab, true);
        assert_eq!(taken(&recorder), []);
        route.take();
        assert_eq!(
            taken(&recorder),
            [
                Handed::Route(true, false),
                key_fact(KeyboardControl::KeyW, PhysicalEdge::Pressed),
            ]
        );
        // A repeat is no new press; a release ends the held key.
        route.key(KeyCode::KeyW, true);
        route.key(KeyCode::KeyW, false);
        assert_eq!(
            taken(&recorder),
            [key_fact(KeyboardControl::KeyW, PhysicalEdge::Released)]
        );
        route.give_back(InputClearReason::PointerLockLoss);
        assert_eq!(
            taken(&recorder),
            [
                Handed::Facts(vec![RuntimeInputFact::Clear {
                    reason: InputClearReason::PointerLockLoss,
                }]),
                Handed::Route(false, false),
            ]
        );
        // Given back, nothing more goes to the runtime.
        route.key(KeyCode::KeyS, true);
        route.button(0, true);
        route.motion(3.0, 0.0);
        route.flush_motion();
        assert_eq!(taken(&recorder), []);
    }

    #[test]
    fn the_press_that_took_the_lock_never_reaches_gameplay() {
        let (mut route, recorder) = route();
        // The click requests the lock; the window takes input before its
        // release arrives.
        route.button(0, true);
        route.take();
        taken(&recorder);
        route.button(0, false);
        assert_eq!(taken(&recorder), []);
        route.button(2, true);
        route.button(2, false);
        assert_eq!(
            taken(&recorder),
            [
                Handed::Facts(vec![RuntimeInputFact::PointerButton {
                    button: PointerButton::Secondary,
                    edge: PhysicalEdge::Pressed,
                    position: None,
                }]),
                Handed::Facts(vec![RuntimeInputFact::PointerButton {
                    button: PointerButton::Secondary,
                    edge: PhysicalEdge::Released,
                    position: None,
                }]),
            ]
        );
    }

    #[test]
    fn motion_is_handed_over_whole_and_the_wheel_as_the_page_reads_it() {
        let (mut route, recorder) = route();
        route.take();
        taken(&recorder);
        route.motion(2.0, -1.0);
        route.motion(3.5, 0.0);
        route.flush_motion();
        route.flush_motion();
        // One line down, as winit reports it: `deltaY` 53 in the page.
        route.wheel(0, -53);
        route.wheel(0, 0);
        route.wheel(-1000, 0);
        assert_eq!(
            taken(&recorder),
            [
                Handed::Facts(vec![RuntimeInputFact::PointerDelta {
                    x: axis(5.5),
                    y: axis(-1.0),
                }]),
                Handed::Facts(vec![RuntimeInputFact::Wheel {
                    x: axis(0.0),
                    y: axis(53.0),
                }]),
                Handed::Facts(vec![RuntimeInputFact::Wheel {
                    x: axis(256.0),
                    y: axis(0.0),
                }]),
            ]
        );
    }

    #[test]
    fn the_window_hands_over_the_pad_it_reads_while_it_holds_input() {
        let recorder = Arc::new(Recorder::default());
        let mut route = GameplayRoute::new(Arc::clone(&recorder) as _, true);
        let mut held = PadState::default();
        held.values[0] = 1.0;
        held.pressed[0] = true;
        // Before the lock the page has the pad; taking input presses what
        // is held then, after the keys.
        route.key(KeyCode::KeyW, true);
        route.pad(held);
        assert_eq!(taken(&recorder), []);
        route.take();
        let mut facts = vec![key(KeyboardControl::KeyW, PhysicalEdge::Pressed)];
        facts.extend(gamepad::changes(&PadState::default(), &held));
        assert_eq!(
            taken(&recorder),
            [Handed::Route(true, true), Handed::Facts(facts)]
        );
        // Each change goes as it is read; giving back stops it.
        route.pad(PadState::default());
        assert_eq!(
            taken(&recorder),
            [Handed::Facts(gamepad::changes(&held, &PadState::default()))]
        );
        route.give_back(InputClearReason::FocusLoss);
        taken(&recorder);
        route.pad(held);
        assert_eq!(taken(&recorder), []);
        // Taken again, the window starts from the pad as it is.
        route.take();
        let handed = taken(&recorder);
        assert_eq!(
            handed.last(),
            Some(&Handed::Facts(
                std::iter::once(key(KeyboardControl::KeyW, PhysicalEdge::Pressed))
                    .chain(gamepad::changes(&PadState::default(), &held))
                    .collect()
            ))
        );
    }
}
