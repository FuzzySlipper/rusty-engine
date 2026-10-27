//! Gameplay input the window hands the runtime itself while it holds the
//! pointer lock, so a busy UI page cannot delay movement and look. The page
//! still receives every event, for its UI; it sends no gameplay input while
//! the window holds it. Facts are the ones the page's input capture makes
//! from the same events. The gamepad, read by the window too
//! ([`crate::gamepad`]), goes the same way.
//!
//! A key or pad button already down when the window takes input belongs to
//! whoever saw it go down, usually the page's UI closing a panel: gameplay
//! gets only a release and a new press, as the page's capture does across a
//! change of owner. A held stick is a position, not a press, and carries over.

use std::collections::BTreeSet;
use std::sync::Arc;

use runtime_input::{
    AxisValue, InputClearReason, KeyboardControl, PhysicalEdge, PointerButton, RuntimeInputFact,
};
use winit::keyboard::KeyCode;

use crate::gamepad_state::{self, PadState};
use crate::{keys, DesktopScene};

/// The page's input capture's bound on one wheel event's delta.
const WHEEL_DELTA_MAXIMUM: f64 = 256.0;

pub(crate) struct GameplayRoute {
    scene: Arc<dyn DesktopScene>,
    /// The window holds gameplay input now.
    routed: bool,
    /// Keys down in the window, held input or not.
    down: BTreeSet<KeyboardControl>,
    /// Keys that were down when the window took input: their release is not
    /// gameplay's.
    withheld: BTreeSet<KeyboardControl>,
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
            withheld: BTreeSet::new(),
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
        self.withheld = self.down.clone();
        if self.controller {
            let sticks = PadState {
                axes: [0.0; 4],
                ..self.pad
            };
            let held = gamepad_state::changes(&sticks, &self.pad);
            self.sent = self.pad;
            if !held.is_empty() {
                self.scene.gameplay(held);
            }
        }
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
        let facts = gamepad_state::changes(&self.sent, &state);
        self.sent = state;
        if !facts.is_empty() {
            self.scene.gameplay(facts);
        }
    }

    /// Keys the window lost track of (another window has focus).
    pub(crate) fn forget_keys(&mut self) {
        self.down.clear();
        self.withheld.clear();
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
        if !pressed && self.withheld.remove(&control) {
            return;
        }
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
    fn a_key_held_when_input_is_taken_waits_for_a_new_press_and_giving_back_clears() {
        let (mut route, recorder) = route();
        // Before the lock the page has the keys; the window only keeps track.
        // Escape is the key whose press closed the page's menu and took the
        // lock: it was the page's, not gameplay's.
        route.key(KeyCode::Escape, true);
        route.key(KeyCode::KeyW, true);
        assert_eq!(taken(&recorder), []);
        route.take();
        assert_eq!(taken(&recorder), [Handed::Route(true, false)]);
        // Neither its repeat nor its release is gameplay's.
        route.key(KeyCode::Escape, true);
        route.key(KeyCode::Escape, false);
        route.key(KeyCode::KeyW, false);
        assert_eq!(taken(&recorder), []);
        // A new press is; a repeat is no new press; a release ends it.
        route.key(KeyCode::KeyW, true);
        route.key(KeyCode::KeyW, true);
        route.key(KeyCode::KeyW, false);
        assert_eq!(
            taken(&recorder),
            [
                key_fact(KeyboardControl::KeyW, PhysicalEdge::Pressed),
                key_fact(KeyboardControl::KeyW, PhysicalEdge::Released),
            ]
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
    fn a_key_after_focus_loss_is_a_new_press() {
        let (mut route, recorder) = route();
        route.take();
        taken(&recorder);
        route.key(KeyCode::KeyW, true);
        assert_eq!(
            taken(&recorder),
            [key_fact(KeyboardControl::KeyW, PhysicalEdge::Pressed)]
        );

        route.give_back(InputClearReason::FocusLoss);
        taken(&recorder);
        route.forget_keys();
        route.take();
        taken(&recorder);
        route.key(KeyCode::KeyW, true);
        assert_eq!(
            taken(&recorder),
            [key_fact(KeyboardControl::KeyW, PhysicalEdge::Pressed)]
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
        held.axes[1] = -1.0;
        held.values[1] = 1.0;
        held.pressed[1] = true;
        let stick = |state: &PadState| PadState {
            axes: state.axes,
            ..PadState::default()
        };
        // Before the lock the page has the pad. Taking input carries the
        // stick held then; the button held then was the page's (B closing a
        // panel), so gameplay gets nothing of it.
        route.pad(held);
        assert_eq!(taken(&recorder), []);
        route.take();
        assert_eq!(
            taken(&recorder),
            [
                Handed::Route(true, true),
                Handed::Facts(gamepad_state::changes(&PadState::default(), &stick(&held),)),
            ]
        );
        // Each change goes as it is read; giving back stops it.
        let mut moved = held;
        moved.axes[1] = 0.0;
        route.pad(moved);
        assert_eq!(
            taken(&recorder),
            [Handed::Facts(gamepad_state::changes(&held, &moved))]
        );
        route.give_back(InputClearReason::FocusLoss);
        taken(&recorder);
        route.pad(held);
        assert_eq!(taken(&recorder), []);
        // Taken again, the window starts from the pad as it is.
        route.take();
        assert_eq!(
            taken(&recorder).last(),
            Some(&Handed::Facts(gamepad_state::changes(
                &PadState::default(),
                &stick(&held)
            )))
        );
        // A button pressed while the window holds input goes as it is.
        let mut pressed = held;
        pressed.pressed[0] = true;
        pressed.values[0] = 1.0;
        route.pad(pressed);
        assert_eq!(
            taken(&recorder),
            [Handed::Facts(gamepad_state::changes(&held, &pressed))]
        );
    }
}
