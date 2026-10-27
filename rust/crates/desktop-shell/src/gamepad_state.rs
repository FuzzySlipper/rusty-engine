//! A native gamepad snapshot in the browser's standard mapping, and the
//! runtime facts that take one snapshot to another.

use runtime_input::{AxisValue, ControllerAxis, ControllerButton, PhysicalEdge, RuntimeInputFact};

const AXES: [ControllerAxis; 4] = [
    ControllerAxis::Axis0,
    ControllerAxis::Axis1,
    ControllerAxis::Axis2,
    ControllerAxis::Axis3,
];
const BUTTONS: [ControllerButton; 16] = [
    ControllerButton::Button0,
    ControllerButton::Button1,
    ControllerButton::Button2,
    ControllerButton::Button3,
    ControllerButton::Button4,
    ControllerButton::Button5,
    ControllerButton::Button6,
    ControllerButton::Button7,
    ControllerButton::Button8,
    ControllerButton::Button9,
    ControllerButton::Button10,
    ControllerButton::Button11,
    ControllerButton::Button12,
    ControllerButton::Button13,
    ControllerButton::Button14,
    ControllerButton::Button15,
];

/// Chromium's standard gamepad state: left x, left y, right x, right y
/// (down positive), and each button's value and press. With no pad connected
/// it is at rest.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PadState {
    pub(crate) connected: bool,
    pub(crate) axes: [f32; 4],
    pub(crate) values: [f32; 16],
    pub(crate) pressed: [bool; 16],
}

/// The facts that take `from` to `to`, in the page's order: axes, then each
/// button's value and its press or release.
pub(crate) fn changes(from: &PadState, to: &PadState) -> Vec<RuntimeInputFact> {
    let mut facts = Vec::new();
    for (index, axis) in AXES.into_iter().enumerate() {
        if to.axes[index] != from.axes[index] {
            if let Ok(value) = AxisValue::new(to.axes[index]) {
                facts.push(RuntimeInputFact::ControllerAxis { axis, value });
            }
        }
    }
    for (index, button) in BUTTONS.into_iter().enumerate() {
        if to.values[index] != from.values[index] {
            if let Ok(value) = AxisValue::new(to.values[index]) {
                facts.push(RuntimeInputFact::ControllerButtonValue { button, value });
            }
        }
        if to.pressed[index] != from.pressed[index] {
            let edge = if to.pressed[index] {
                PhysicalEdge::Pressed
            } else {
                PhysicalEdge::Released
            };
            facts.push(RuntimeInputFact::ControllerButton { button, edge });
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_follow_the_pages_order_and_report_only_what_moved() {
        let rest = PadState::default();
        let mut pressed = rest;
        pressed.axes[1] = 0.5;
        pressed.values[0] = 1.0;
        pressed.pressed[0] = true;
        pressed.values[7] = 0.25;
        let value = |v| AxisValue::new(v).unwrap();
        assert_eq!(
            changes(&rest, &pressed),
            [
                RuntimeInputFact::ControllerAxis {
                    axis: ControllerAxis::Axis1,
                    value: value(0.5),
                },
                RuntimeInputFact::ControllerButtonValue {
                    button: ControllerButton::Button0,
                    value: value(1.0),
                },
                RuntimeInputFact::ControllerButton {
                    button: ControllerButton::Button0,
                    edge: PhysicalEdge::Pressed,
                },
                RuntimeInputFact::ControllerButtonValue {
                    button: ControllerButton::Button7,
                    value: value(0.25),
                },
            ]
        );
        assert!(changes(&pressed, &pressed).is_empty());
        assert_eq!(
            changes(&pressed, &rest).len(),
            4,
            "back to rest undoes each"
        );
    }
}
