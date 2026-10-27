//! The gamepad, read by the window while it holds gameplay input. The page
//! samples `navigator.getGamepads()` on its own schedule, so a UI page
//! running long tasks delayed presses and missed short ones. This reads the
//! first connected pad through gilrs on a thread of its own and hands the
//! event loop snapshots in Chromium's standard mapping;
//! [`crate::gamepad_state::changes`] turns two snapshots into the facts the
//! page's capture makes from the same pad.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use gilrs::ev::filter::{axis_dpad_to_button, FilterFn};
use gilrs::{Axis, Button, Event, EventType, Gilrs, GilrsBuilder};

pub(crate) use crate::gamepad_state::PadState;

/// Chromium's standard gamepad buttons 0 to 15, the ones the page reads.
/// Keep this order aligned with `gamepad_state::BUTTONS`.
const BUTTONS: [Button; 16] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];
/// Chromium counts an analog trigger pressed past 30/255; gilrs's default is
/// 0.75.
const TRIGGER_PRESSED: f32 = 30.0 / 255.0;
const TRIGGER_RELEASED: f32 = 25.0 / 255.0;
/// How often the reader looks at its stop flag while no pad event arrives.
const IDLE_WAKE: Duration = Duration::from_millis(100);

/// The gilrs thread. Dropping it stops the thread.
pub(crate) struct GamepadReader {
    states: mpsc::Receiver<PadState>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GamepadReader {
    /// Start reading, calling `wake` after each new state; `None` where
    /// gilrs cannot read gamepads, and the page keeps the controller.
    pub(crate) fn start(wake: impl Fn() + Send + 'static) -> Option<Self> {
        let (deliver, states) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("desktop-shell-gamepad".to_owned())
            .spawn(move || {
                // The page's values are the browser's, unfiltered: no dead
                // zone or jitter filter rescales them here either.
                let gilrs = GilrsBuilder::new()
                    .with_default_filters(false)
                    .set_axis_to_btn(TRIGGER_PRESSED, TRIGGER_RELEASED)
                    .build();
                let mut gilrs = match gilrs {
                    Ok(gilrs) => {
                        let _ = ready.send(true);
                        gilrs
                    }
                    Err(error) => {
                        eprintln!("desktop-shell: gamepads stay with the page: {error}");
                        let _ = ready.send(false);
                        return;
                    }
                };
                let mut last = PadState::default();
                while !stopping.load(Ordering::Relaxed) {
                    // Events update gilrs's state; the state is what counts.
                    if let Some(event) = gilrs.next_event_blocking(Some(IDLE_WAKE)) {
                        dpad_buttons(event, &mut gilrs);
                        while let Some(event) = gilrs.next_event() {
                            dpad_buttons(event, &mut gilrs);
                        }
                    }
                    let state = gilrs
                        .gamepads()
                        .next()
                        .map_or_else(PadState::default, |(_, pad)| read(&pad));
                    if state != last {
                        last = state;
                        if deliver.send(state).is_err() {
                            return;
                        }
                        wake();
                    }
                }
            })
            .ok()?;
        if started.recv() != Ok(true) {
            let _ = thread.join();
            return None;
        }
        Some(Self {
            states,
            stop,
            thread: Some(thread),
        })
    }

    /// The newest state since the last call, if the pad changed.
    pub(crate) fn latest(&self) -> Option<PadState> {
        self.states.try_iter().last()
    }
}

impl Drop for GamepadReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A hat's D-pad as buttons 12 to 15, as the standard mapping has it,
/// through the one filter kept from gilrs's defaults: it queues each
/// button's value change, read as events later, and returns its press or
/// release, which gilrs applies only to the events it returns itself.
fn dpad_buttons(event: Event, gilrs: &mut Gilrs) {
    if let EventType::AxisChanged(Axis::DPadX | Axis::DPadY, ..) = event.event {
        if let Some(edge) = axis_dpad_to_button.filter(Some(event), gilrs) {
            gilrs.update(&edge);
        }
    }
}

fn read(pad: &gilrs::Gamepad<'_>) -> PadState {
    let mut state = PadState {
        connected: true,
        // gilrs's sticks are up positive; the page's are down positive.
        axes: [
            pad.value(Axis::LeftStickX),
            -pad.value(Axis::LeftStickY),
            pad.value(Axis::RightStickX),
            -pad.value(Axis::RightStickY),
        ],
        ..PadState::default()
    };
    for (index, button) in BUTTONS.into_iter().enumerate() {
        state.values[index] = pad.button_data(button).map_or(0.0, |data| data.value());
        state.pressed[index] = pad.is_pressed(button);
    }
    // A stick at rest reads -0.0 after negation; the page sends no change.
    for axis in &mut state.axes {
        if *axis == 0.0 {
            *axis = 0.0;
        }
    }
    state
}
