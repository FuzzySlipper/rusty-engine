//! One-way adapter from the retired product-composition vocabulary into the
//! neutral runtime input lane.
//!
//! `runtime-input` intentionally does not know how a product description is
//! authored or linked. This module is the remaining assembly-edge bridge for
//! products that still arrive as a linked composition; direct/native products
//! can construct `runtime_input` descriptors without this dependency.

use product_model::{InputTrigger, LinkedProductComposition};
use runtime_input::{
    CompiledInputCapabilityLink, CompiledInputIntent, CompiledInputMapping, CompiledInputMappings,
    ControllerAxis, ControllerButton, InputAxis, InputContext, InputEdge, IntentValueKind,
    KeyboardControl, PointerButton, RuntimeInputError, RuntimeInputTrigger,
};

/// Converts one linked legacy product composition into neutral runtime input
/// descriptors and mappings. Product Model admission/linkage remains owned by
/// this assembly edge; the returned runtime values contain no Product Model
/// types.
pub fn compile_input_mappings(
    linked: &LinkedProductComposition,
) -> Result<CompiledInputMappings, RuntimeInputError> {
    let intents = linked
        .admitted()
        .intent_descriptors()
        .iter()
        .map(|descriptor| {
            let capability = descriptor
                .capability()
                .map(|reference| {
                    let capability = linked
                        .capability_binding(reference.binding_index())
                        .ok_or(RuntimeInputError::BindingMismatch)?;
                    if capability.id() != reference.id()
                        || capability.target() != reference.target()
                    {
                        return Err(RuntimeInputError::BindingMismatch);
                    }
                    CompiledInputCapabilityLink::new(
                        reference.id().to_owned(),
                        reference.target().to_owned(),
                        reference.binding_index(),
                    )
                })
                .transpose()?;
            CompiledInputIntent::new(
                descriptor.index(),
                descriptor.id().to_owned(),
                runtime_intent_value_kind(descriptor.value_kind()),
                descriptor.payload_contract().map(str::to_owned),
                capability,
                descriptor.payload().clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mappings = linked
        .admitted()
        .input_map()
        .iter()
        .map(|mapping| {
            CompiledInputMapping::new(
                mapping.id().to_owned(),
                mapping.intent().to_owned(),
                runtime_trigger(mapping.trigger())?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    CompiledInputMappings::from_parts(intents, mappings)
}

fn runtime_trigger(trigger: &InputTrigger) -> Result<RuntimeInputTrigger, RuntimeInputError> {
    let context = |value: &Option<String>| value.as_deref().map(InputContext::new).transpose();
    Ok(match trigger {
        InputTrigger::Key {
            code,
            edge,
            chord,
            context: trigger_context,
        } => RuntimeInputTrigger::Key {
            code: runtime_keyboard_control(*code),
            edge: runtime_input_edge(*edge),
            chord: chord
                .iter()
                .copied()
                .map(runtime_keyboard_control)
                .collect(),
            context: context(trigger_context)?,
        },
        InputTrigger::PointerButton {
            button,
            edge,
            context: trigger_context,
        } => RuntimeInputTrigger::PointerButton {
            button: runtime_pointer_button(*button),
            edge: runtime_input_edge(*edge),
            context: context(trigger_context)?,
        },
        InputTrigger::PointerAxis {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::PointerAxis {
            axis: runtime_input_axis(*axis),
            context: context(trigger_context)?,
        },
        InputTrigger::Wheel {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::Wheel {
            axis: runtime_input_axis(*axis),
            context: context(trigger_context)?,
        },
        InputTrigger::ControllerButton {
            button,
            edge,
            context: trigger_context,
        } => RuntimeInputTrigger::ControllerButton {
            button: runtime_controller_button(*button),
            edge: runtime_input_edge(*edge),
            context: context(trigger_context)?,
        },
        InputTrigger::ControllerAxis {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::ControllerAxis {
            axis: runtime_controller_axis(*axis),
            context: context(trigger_context)?,
        },
    })
}

fn runtime_intent_value_kind(value: product_model::IntentValueKind) -> IntentValueKind {
    match value {
        product_model::IntentValueKind::Digital => IntentValueKind::Digital,
        product_model::IntentValueKind::Axis => IntentValueKind::Axis,
        product_model::IntentValueKind::ProductPayload => IntentValueKind::ProductPayload,
    }
}

fn runtime_input_edge(value: product_model::InputEdge) -> InputEdge {
    match value {
        product_model::InputEdge::Held => InputEdge::Held,
        product_model::InputEdge::Pressed => InputEdge::Pressed,
        product_model::InputEdge::Released => InputEdge::Released,
    }
}

fn runtime_keyboard_control(value: product_model::KeyboardControl) -> KeyboardControl {
    match value {
        product_model::KeyboardControl::KeyA => KeyboardControl::KeyA,
        product_model::KeyboardControl::KeyB => KeyboardControl::KeyB,
        product_model::KeyboardControl::KeyC => KeyboardControl::KeyC,
        product_model::KeyboardControl::KeyD => KeyboardControl::KeyD,
        product_model::KeyboardControl::KeyE => KeyboardControl::KeyE,
        product_model::KeyboardControl::KeyF => KeyboardControl::KeyF,
        product_model::KeyboardControl::KeyG => KeyboardControl::KeyG,
        product_model::KeyboardControl::KeyH => KeyboardControl::KeyH,
        product_model::KeyboardControl::KeyI => KeyboardControl::KeyI,
        product_model::KeyboardControl::KeyJ => KeyboardControl::KeyJ,
        product_model::KeyboardControl::KeyK => KeyboardControl::KeyK,
        product_model::KeyboardControl::KeyL => KeyboardControl::KeyL,
        product_model::KeyboardControl::KeyM => KeyboardControl::KeyM,
        product_model::KeyboardControl::KeyN => KeyboardControl::KeyN,
        product_model::KeyboardControl::KeyO => KeyboardControl::KeyO,
        product_model::KeyboardControl::KeyP => KeyboardControl::KeyP,
        product_model::KeyboardControl::KeyQ => KeyboardControl::KeyQ,
        product_model::KeyboardControl::KeyR => KeyboardControl::KeyR,
        product_model::KeyboardControl::KeyS => KeyboardControl::KeyS,
        product_model::KeyboardControl::KeyT => KeyboardControl::KeyT,
        product_model::KeyboardControl::KeyU => KeyboardControl::KeyU,
        product_model::KeyboardControl::KeyV => KeyboardControl::KeyV,
        product_model::KeyboardControl::KeyW => KeyboardControl::KeyW,
        product_model::KeyboardControl::KeyX => KeyboardControl::KeyX,
        product_model::KeyboardControl::KeyY => KeyboardControl::KeyY,
        product_model::KeyboardControl::KeyZ => KeyboardControl::KeyZ,
        product_model::KeyboardControl::Digit0 => KeyboardControl::Digit0,
        product_model::KeyboardControl::Digit1 => KeyboardControl::Digit1,
        product_model::KeyboardControl::Digit2 => KeyboardControl::Digit2,
        product_model::KeyboardControl::Digit3 => KeyboardControl::Digit3,
        product_model::KeyboardControl::Digit4 => KeyboardControl::Digit4,
        product_model::KeyboardControl::Digit5 => KeyboardControl::Digit5,
        product_model::KeyboardControl::Digit6 => KeyboardControl::Digit6,
        product_model::KeyboardControl::Digit7 => KeyboardControl::Digit7,
        product_model::KeyboardControl::Digit8 => KeyboardControl::Digit8,
        product_model::KeyboardControl::Digit9 => KeyboardControl::Digit9,
        product_model::KeyboardControl::Space => KeyboardControl::Space,
        product_model::KeyboardControl::Enter => KeyboardControl::Enter,
        product_model::KeyboardControl::Escape => KeyboardControl::Escape,
        product_model::KeyboardControl::ShiftLeft => KeyboardControl::ShiftLeft,
        product_model::KeyboardControl::ShiftRight => KeyboardControl::ShiftRight,
        product_model::KeyboardControl::ControlLeft => KeyboardControl::ControlLeft,
        product_model::KeyboardControl::ControlRight => KeyboardControl::ControlRight,
        product_model::KeyboardControl::AltLeft => KeyboardControl::AltLeft,
        product_model::KeyboardControl::AltRight => KeyboardControl::AltRight,
    }
}

fn runtime_pointer_button(value: product_model::PointerButton) -> PointerButton {
    match value {
        product_model::PointerButton::Primary => PointerButton::Primary,
        product_model::PointerButton::Secondary => PointerButton::Secondary,
        product_model::PointerButton::Middle => PointerButton::Middle,
    }
}

fn runtime_input_axis(value: product_model::InputAxis) -> InputAxis {
    match value {
        product_model::InputAxis::X => InputAxis::X,
        product_model::InputAxis::Y => InputAxis::Y,
    }
}

fn runtime_controller_button(value: product_model::ControllerButton) -> ControllerButton {
    match value {
        product_model::ControllerButton::Button0 => ControllerButton::Button0,
        product_model::ControllerButton::Button1 => ControllerButton::Button1,
        product_model::ControllerButton::Button2 => ControllerButton::Button2,
        product_model::ControllerButton::Button3 => ControllerButton::Button3,
        product_model::ControllerButton::Button4 => ControllerButton::Button4,
        product_model::ControllerButton::Button5 => ControllerButton::Button5,
        product_model::ControllerButton::Button6 => ControllerButton::Button6,
        product_model::ControllerButton::Button7 => ControllerButton::Button7,
        product_model::ControllerButton::Button8 => ControllerButton::Button8,
        product_model::ControllerButton::Button9 => ControllerButton::Button9,
        product_model::ControllerButton::Button10 => ControllerButton::Button10,
        product_model::ControllerButton::Button11 => ControllerButton::Button11,
        product_model::ControllerButton::Button12 => ControllerButton::Button12,
        product_model::ControllerButton::Button13 => ControllerButton::Button13,
        product_model::ControllerButton::Button14 => ControllerButton::Button14,
        product_model::ControllerButton::Button15 => ControllerButton::Button15,
    }
}

fn runtime_controller_axis(value: product_model::ControllerAxis) -> ControllerAxis {
    match value {
        product_model::ControllerAxis::Axis0 => ControllerAxis::Axis0,
        product_model::ControllerAxis::Axis1 => ControllerAxis::Axis1,
        product_model::ControllerAxis::Axis2 => ControllerAxis::Axis2,
        product_model::ControllerAxis::Axis3 => ControllerAxis::Axis3,
    }
}
