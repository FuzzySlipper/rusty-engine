use std::ffi::c_void;

use csharp_engine_abi::*;
use runtime_input::{
    CompiledInputMappings, ControllerAxis, ControllerButton, DirectInputIntentDescriptor,
    InputAxis, InputContext, InputEdge, KeyboardControl, PointerButton, RuntimeInputMapping,
    RuntimeInputTrigger,
};

use crate::composition::{borrowed_slice, ABI_OK};

/// The C# service owns only a callback-local compiled replacement. The loaded
/// product runtime is the one owner that can settle it onto its input lane.
pub(crate) struct RuntimeInputBridge {
    direct_intents: Vec<DirectInputIntentDescriptor>,
    accepts_replacement: bool,
    staged: Option<CompiledInputMappings>,
}

impl RuntimeInputBridge {
    pub(crate) fn new(direct_intents: Vec<DirectInputIntentDescriptor>) -> Self {
        Self {
            direct_intents,
            accepts_replacement: false,
            staged: None,
        }
    }

    pub(crate) fn begin_call(&mut self, accepts_replacement: bool) {
        self.accepts_replacement = accepts_replacement;
        self.staged = None;
    }

    pub(crate) fn discard_call(&mut self) {
        self.accepts_replacement = false;
        self.staged = None;
    }

    pub(crate) fn take_call(&mut self) -> Option<CompiledInputMappings> {
        self.accepts_replacement = false;
        self.staged.take()
    }
}

pub(crate) fn api(bridge: &mut RuntimeInputBridge) -> NativeInputApi {
    NativeInputApi {
        context: (bridge as *mut RuntimeInputBridge).cast(),
        replace_physical_mappings,
    }
}

unsafe extern "C" fn replace_physical_mappings(
    context: *mut c_void,
    mappings: *const NativeInputMapping,
    mappings_len: usize,
    outcome: *mut NativeInputMappingReplacementOutcome,
) -> i32 {
    if context.is_null() || outcome.is_null() {
        return 0;
    }
    let mappings = match unsafe { borrowed_slice(mappings, mappings_len, "input mappings") } {
        Ok(mappings) => mappings,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeInputBridge>() };
    if !bridge.accepts_replacement {
        unsafe { *outcome = NativeInputMappingReplacementOutcome::Unavailable };
        return ABI_OK;
    }
    let candidate = mappings
        .iter()
        .map(decode_mapping)
        .collect::<Result<Vec<_>, _>>()
        .and_then(|mappings| {
            CompiledInputMappings::standard(bridge.direct_intents.clone(), mappings).map_err(|_| ())
        });
    match candidate {
        Ok(candidate) => {
            // A later valid request deliberately supersedes an earlier valid
            // request in this one callback. Invalid requests do not erase it.
            bridge.staged = Some(candidate);
            unsafe { *outcome = NativeInputMappingReplacementOutcome::Staged };
        }
        Err(()) => unsafe { *outcome = NativeInputMappingReplacementOutcome::InvalidMappings },
    }
    ABI_OK
}

fn decode_mapping(value: &NativeInputMapping) -> Result<RuntimeInputMapping, ()> {
    let id = text(value.id, value.id_len)?;
    let intent = text(value.intent, value.intent_len)?;
    let context = optional_context(value.context, value.context_len)?;
    let trigger = match value.trigger_kind {
        NativeInputTriggerKind::Key => RuntimeInputTrigger::Key {
            code: keyboard(value.keyboard).ok_or(())?,
            edge: edge(value.edge).ok_or(())?,
            chord: unsafe { borrowed_slice(value.chord, value.chord_len, "input mapping chord") }
                .map_err(|_| ())?
                .iter()
                .map(|key| keyboard(*key).ok_or(()))
                .collect::<Result<Vec<_>, _>>()?,
            context,
        },
        NativeInputTriggerKind::PointerButton => RuntimeInputTrigger::PointerButton {
            button: pointer_button(value.pointer_button).ok_or(())?,
            edge: edge(value.edge).ok_or(())?,
            context,
        },
        NativeInputTriggerKind::PointerAxis => RuntimeInputTrigger::PointerAxis {
            axis: axis(value.axis).ok_or(())?,
            context,
        },
        NativeInputTriggerKind::Wheel => RuntimeInputTrigger::Wheel {
            axis: axis(value.axis).ok_or(())?,
            context,
        },
        NativeInputTriggerKind::ControllerButton => RuntimeInputTrigger::ControllerButton {
            button: controller_button(value.controller_button).ok_or(())?,
            edge: edge(value.edge).ok_or(())?,
            context,
        },
        NativeInputTriggerKind::ControllerAxis => RuntimeInputTrigger::ControllerAxis {
            axis: controller_axis(value.controller_axis).ok_or(())?,
            context,
        },
        NativeInputTriggerKind::ControllerButtonValue => {
            RuntimeInputTrigger::ControllerButtonValue {
                button: controller_button(value.controller_button).ok_or(())?,
                context,
            }
        }
    };
    RuntimeInputMapping::new(id, intent, trigger).map_err(|_| ())
}

fn text(pointer: *const u8, len: usize) -> Result<String, ()> {
    let bytes = unsafe { borrowed_slice(pointer, len, "input mapping text") }.map_err(|_| ())?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ())
}

fn optional_context(pointer: *const u8, len: usize) -> Result<Option<InputContext>, ()> {
    if len == 0 {
        return Ok(None);
    }
    InputContext::new(text(pointer, len)?)
        .map(Some)
        .map_err(|_| ())
}

fn edge(value: NativeInputEdge) -> Option<InputEdge> {
    match value {
        NativeInputEdge::Held => Some(InputEdge::Held),
        NativeInputEdge::Pressed => Some(InputEdge::Pressed),
        NativeInputEdge::Released => Some(InputEdge::Released),
        NativeInputEdge::None => None,
    }
}

fn axis(value: NativeInputAxis) -> Option<InputAxis> {
    match value {
        NativeInputAxis::X => Some(InputAxis::X),
        NativeInputAxis::Y => Some(InputAxis::Y),
        NativeInputAxis::None => None,
    }
}

fn keyboard(value: NativeKeyboardControl) -> Option<KeyboardControl> {
    const VALUES: [KeyboardControl; 45] = [
        KeyboardControl::KeyA,
        KeyboardControl::KeyB,
        KeyboardControl::KeyC,
        KeyboardControl::KeyD,
        KeyboardControl::KeyE,
        KeyboardControl::KeyF,
        KeyboardControl::KeyG,
        KeyboardControl::KeyH,
        KeyboardControl::KeyI,
        KeyboardControl::KeyJ,
        KeyboardControl::KeyK,
        KeyboardControl::KeyL,
        KeyboardControl::KeyM,
        KeyboardControl::KeyN,
        KeyboardControl::KeyO,
        KeyboardControl::KeyP,
        KeyboardControl::KeyQ,
        KeyboardControl::KeyR,
        KeyboardControl::KeyS,
        KeyboardControl::KeyT,
        KeyboardControl::KeyU,
        KeyboardControl::KeyV,
        KeyboardControl::KeyW,
        KeyboardControl::KeyX,
        KeyboardControl::KeyY,
        KeyboardControl::KeyZ,
        KeyboardControl::Digit0,
        KeyboardControl::Digit1,
        KeyboardControl::Digit2,
        KeyboardControl::Digit3,
        KeyboardControl::Digit4,
        KeyboardControl::Digit5,
        KeyboardControl::Digit6,
        KeyboardControl::Digit7,
        KeyboardControl::Digit8,
        KeyboardControl::Digit9,
        KeyboardControl::Space,
        KeyboardControl::Enter,
        KeyboardControl::Escape,
        KeyboardControl::ShiftLeft,
        KeyboardControl::ShiftRight,
        KeyboardControl::ControlLeft,
        KeyboardControl::ControlRight,
        KeyboardControl::AltLeft,
        KeyboardControl::AltRight,
    ];
    let index = (value as u32).checked_sub(1)? as usize;
    VALUES.get(index).copied()
}

fn pointer_button(value: NativePointerButton) -> Option<PointerButton> {
    match value {
        NativePointerButton::Primary => Some(PointerButton::Primary),
        NativePointerButton::Secondary => Some(PointerButton::Secondary),
        NativePointerButton::Middle => Some(PointerButton::Middle),
        NativePointerButton::None => None,
    }
}

fn controller_button(value: NativeControllerButton) -> Option<ControllerButton> {
    const VALUES: [ControllerButton; 16] = [
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
    let index = (value as u32).checked_sub(1)? as usize;
    VALUES.get(index).copied()
}

fn controller_axis(value: NativeControllerAxis) -> Option<ControllerAxis> {
    const VALUES: [ControllerAxis; 4] = [
        ControllerAxis::Axis0,
        ControllerAxis::Axis1,
        ControllerAxis::Axis2,
        ControllerAxis::Axis3,
    ];
    let index = (value as u32).checked_sub(1)? as usize;
    VALUES.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_input::IntentValueKind;

    fn mapping(
        id: &'static [u8],
        intent: &'static [u8],
        key: NativeKeyboardControl,
    ) -> NativeInputMapping {
        NativeInputMapping {
            id: id.as_ptr(),
            id_len: id.len(),
            intent: intent.as_ptr(),
            intent_len: intent.len(),
            trigger_kind: NativeInputTriggerKind::Key,
            edge: NativeInputEdge::Pressed,
            axis: NativeInputAxis::None,
            keyboard: key,
            pointer_button: NativePointerButton::None,
            controller_button: NativeControllerButton::None,
            controller_axis: NativeControllerAxis::None,
            chord: std::ptr::null(),
            chord_len: 0,
            context: std::ptr::null(),
            context_len: 0,
        }
    }

    #[test]
    fn replacement_stages_last_valid_candidate_and_preserves_it_on_invalid_request() {
        let descriptor =
            DirectInputIntentDescriptor::new("fixture.attack", IntentValueKind::Digital).unwrap();
        let mut bridge = RuntimeInputBridge::new(vec![descriptor]);
        bridge.begin_call(true);
        let api = api(&mut bridge);
        let valid = [mapping(
            b"attack-f",
            b"fixture.attack",
            NativeKeyboardControl::KeyF,
        )];
        let mut outcome = NativeInputMappingReplacementOutcome::Unavailable;
        assert_eq!(
            unsafe {
                (api.replace_physical_mappings)(
                    api.context,
                    valid.as_ptr(),
                    valid.len(),
                    &mut outcome,
                )
            },
            ABI_OK
        );
        assert_eq!(outcome, NativeInputMappingReplacementOutcome::Staged);

        let duplicate = [
            mapping(b"attack-f", b"fixture.attack", NativeKeyboardControl::KeyF),
            mapping(b"attack-f", b"fixture.attack", NativeKeyboardControl::KeyE),
        ];
        assert_eq!(
            unsafe {
                (api.replace_physical_mappings)(
                    api.context,
                    duplicate.as_ptr(),
                    duplicate.len(),
                    &mut outcome,
                )
            },
            ABI_OK
        );
        assert_eq!(
            outcome,
            NativeInputMappingReplacementOutcome::InvalidMappings
        );
        let staged = bridge.take_call().expect("valid candidate remains staged");
        assert_eq!(staged.mappings()[0].id(), "attack-f");
        assert_eq!(
            staged.mappings()[0].trigger(),
            &RuntimeInputTrigger::Key {
                code: KeyboardControl::KeyF,
                edge: InputEdge::Pressed,
                chord: Vec::new(),
                context: None,
            }
        );
    }

    #[test]
    fn replacement_is_unavailable_outside_create_update_or_lifecycle_calls() {
        let mut bridge = RuntimeInputBridge::new(Vec::new());
        bridge.begin_call(false);
        let api = api(&mut bridge);
        let mut outcome = NativeInputMappingReplacementOutcome::Staged;
        assert_eq!(
            unsafe {
                (api.replace_physical_mappings)(api.context, std::ptr::null(), 0, &mut outcome)
            },
            ABI_OK
        );
        assert_eq!(outcome, NativeInputMappingReplacementOutcome::Unavailable);
        assert!(bridge.take_call().is_none());
    }
}
