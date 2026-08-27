use std::collections::BTreeMap;

use crate::{model::is_identity, InputContext, RuntimeInputError};
use product_model::{
    ControllerAxis, ControllerButton, InputAxis, InputEdge, InputTrigger, IntentValueKind,
    KeyboardControl, LinkedProductComposition, PointerButton,
};
use serde_json::Value;

/// A typed physical trigger owned by the standard runtime input lane.
///
/// This is deliberately a runtime vocabulary rather than a Product Model
/// composition.  A product host may construct these values directly from
/// generated/native configuration and never needs to load or assemble a
/// `LinkedProductComposition`.  The control enums are shared host-neutral
/// vocabulary; product-model composition, capability, and payload data are not
/// part of this configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInputTrigger {
    Key {
        code: KeyboardControl,
        edge: InputEdge,
        chord: Vec<KeyboardControl>,
        context: Option<InputContext>,
    },
    PointerButton {
        button: PointerButton,
        edge: InputEdge,
        context: Option<InputContext>,
    },
    PointerAxis {
        axis: InputAxis,
        context: Option<InputContext>,
    },
    Wheel {
        axis: InputAxis,
        context: Option<InputContext>,
    },
    ControllerButton {
        button: ControllerButton,
        edge: InputEdge,
        context: Option<InputContext>,
    },
    ControllerAxis {
        axis: ControllerAxis,
        context: Option<InputContext>,
    },
}

impl RuntimeInputTrigger {
    pub const fn value_kind(&self) -> IntentValueKind {
        match self {
            Self::Key { .. } | Self::PointerButton { .. } | Self::ControllerButton { .. } => {
                IntentValueKind::Digital
            }
            Self::PointerAxis { .. } | Self::Wheel { .. } | Self::ControllerAxis { .. } => {
                IntentValueKind::Axis
            }
        }
    }

    pub fn context(&self) -> Option<&InputContext> {
        match self {
            Self::Key { context, .. }
            | Self::PointerButton { context, .. }
            | Self::PointerAxis { context, .. }
            | Self::Wheel { context, .. }
            | Self::ControllerButton { context, .. }
            | Self::ControllerAxis { context, .. } => context.as_ref(),
        }
    }
}

/// One explicit standard-runtime physical mapping.
///
/// Mapping identity is stable product data, while the runtime owns when the
/// mapping is evaluated and which lifecycle step receives its envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInputMapping {
    id: String,
    intent: String,
    trigger: RuntimeInputTrigger,
}

impl RuntimeInputMapping {
    pub fn new(
        id: impl Into<String>,
        intent: impl Into<String>,
        trigger: RuntimeInputTrigger,
    ) -> Result<Self, RuntimeInputError> {
        let id = id.into();
        let intent = intent.into();
        if !is_identity(&id) || !is_identity(&intent) {
            return Err(RuntimeInputError::InvalidMapping);
        }
        Ok(Self {
            id,
            intent,
            trigger,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn intent(&self) -> &str {
        &self.intent
    }

    pub fn trigger(&self) -> &RuntimeInputTrigger {
        &self.trigger
    }
}

/// One purpose-neutral direct intent admitted by a standard runtime without a
/// Product Model composition. It authorizes only the named direct value shape;
/// it does not register a callback, route a command, or define product policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectInputIntentDescriptor {
    id: String,
    value_kind: IntentValueKind,
    payload_contract: Option<String>,
}

impl DirectInputIntentDescriptor {
    pub fn new(
        id: impl Into<String>,
        value_kind: IntentValueKind,
    ) -> Result<Self, RuntimeInputError> {
        let id = id.into();
        if !is_identity(&id) {
            return Err(RuntimeInputError::InvalidIntent);
        }
        if value_kind == IntentValueKind::ProductPayload {
            return Err(RuntimeInputError::DirectIntentPayloadUnsupported);
        }
        Ok(Self {
            id,
            value_kind,
            payload_contract: None,
        })
    }

    pub fn product_payload(
        id: impl Into<String>,
        payload_contract: impl Into<String>,
    ) -> Result<Self, RuntimeInputError> {
        let id = id.into();
        let payload_contract = payload_contract.into();
        if !is_identity(&id) {
            return Err(RuntimeInputError::InvalidIntent);
        }
        if !is_identity(&payload_contract) {
            return Err(RuntimeInputError::InvalidProductPayloadContract);
        }
        Ok(Self {
            id,
            value_kind: IntentValueKind::ProductPayload,
            payload_contract: Some(payload_contract),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn value_kind(&self) -> IntentValueKind {
        self.value_kind
    }

    pub fn payload_contract(&self) -> Option<&str> {
        self.payload_contract.as_deref()
    }
}

/// One linked product intent available to both physical mappings and direct UI
/// claims. It retains one optional capability linkage for legacy Product
/// Kernel execution; VM-local intent descriptors omit it entirely.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledInputIntent {
    descriptor_index: usize,
    id: String,
    value_kind: IntentValueKind,
    payload_contract: Option<String>,
    capability: Option<CompiledInputCapabilityLink>,
    payload: Value,
}

/// The one optional static capability linkage retained by a compiled input
/// intent. It is descriptive only: Runtime Input neither invokes it nor turns
/// it into a service route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledInputCapabilityLink {
    id: String,
    target: String,
    binding_index: usize,
}

impl CompiledInputCapabilityLink {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }
}

impl CompiledInputIntent {
    pub const fn descriptor_index(&self) -> usize {
        self.descriptor_index
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn value_kind(&self) -> IntentValueKind {
        self.value_kind
    }
    /// Descriptor-owned stable contract for a direct product payload intent.
    pub fn payload_contract(&self) -> Option<&str> {
        self.payload_contract.as_deref()
    }
    pub fn capability(&self) -> Option<&CompiledInputCapabilityLink> {
        self.capability.as_ref()
    }
    /// Immutable descriptor payload. It remains data only whether or not this
    /// intent retains a legacy capability link.
    pub fn payload(&self) -> &Value {
        // Product Model already admitted this opaque value before input
        // compilation. Runtime Input never interprets it.
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledInputMapping {
    id: String,
    intent: String,
    trigger: RuntimeInputTrigger,
}

impl CompiledInputMapping {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn intent(&self) -> &str {
        &self.intent
    }
    pub fn trigger(&self) -> &RuntimeInputTrigger {
        &self.trigger
    }
}

/// An immutable, pre-runtime view of all typed input mappings. It neither
/// registers callbacks nor resolves a service; linked capability descriptors
/// are only checked before the lane is made available.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledInputMappings {
    intents: BTreeMap<String, CompiledInputIntent>,
    mappings: Vec<CompiledInputMapping>,
}

impl CompiledInputMappings {
    /// Builds a deliberately unmapped physical-input lane.
    ///
    /// A product runtime that forwards normalized physical observations to a
    /// downstream application still needs the lane's binding, sequence, and
    /// held-input ownership. It does not need Product Model intent mappings
    /// merely to retain those infrastructure guarantees.
    pub fn empty() -> Self {
        Self {
            intents: BTreeMap::new(),
            mappings: Vec::new(),
        }
    }

    /// Compiles direct descriptors and typed physical mappings for a standard
    /// runtime without loading an older Product Model composition. The
    /// returned lane owns the normal direct-claim binding, sequence,
    /// context, held-state, clear, and rebind rules.
    pub fn direct_intents(
        descriptors: impl IntoIterator<Item = DirectInputIntentDescriptor>,
    ) -> Result<Self, RuntimeInputError> {
        Self::standard(descriptors, std::iter::empty())
    }

    /// Compiles the standard runtime's direct descriptors and physical mapping
    /// configuration. This path has no Product Model composition input and
    /// performs all mapping identity/value-kind checks before runtime use.
    pub fn standard(
        descriptors: impl IntoIterator<Item = DirectInputIntentDescriptor>,
        mappings: impl IntoIterator<Item = RuntimeInputMapping>,
    ) -> Result<Self, RuntimeInputError> {
        let mut intents = BTreeMap::new();
        for (descriptor_index, descriptor) in descriptors.into_iter().enumerate() {
            if intents.contains_key(descriptor.id()) {
                return Err(RuntimeInputError::DuplicateIntent);
            }
            intents.insert(
                descriptor.id.clone(),
                CompiledInputIntent {
                    descriptor_index,
                    id: descriptor.id,
                    value_kind: descriptor.value_kind,
                    payload_contract: descriptor.payload_contract,
                    capability: None,
                    payload: Value::Null,
                },
            );
        }
        let mut compiled_mappings = Vec::new();
        for mapping in mappings {
            if compiled_mappings
                .iter()
                .any(|existing: &CompiledInputMapping| existing.id == mapping.id)
            {
                return Err(RuntimeInputError::DuplicateMapping);
            }
            let intent = intents
                .get(mapping.intent())
                .ok_or(RuntimeInputError::UnknownIntent)?;
            if intent.value_kind() != mapping.trigger().value_kind() {
                return Err(RuntimeInputError::IntentValueKindMismatch);
            }
            compiled_mappings.push(CompiledInputMapping {
                id: mapping.id,
                intent: mapping.intent,
                trigger: mapping.trigger,
            });
        }
        Ok(Self {
            intents,
            mappings: compiled_mappings,
        })
    }

    pub fn compile(linked: &LinkedProductComposition) -> Result<Self, RuntimeInputError> {
        let mut intents = BTreeMap::new();
        for descriptor in linked.admitted().intent_descriptors() {
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
                    Ok(CompiledInputCapabilityLink {
                        id: reference.id().to_owned(),
                        target: reference.target().to_owned(),
                        binding_index: reference.binding_index(),
                    })
                })
                .transpose()?;
            intents.insert(
                descriptor.id().to_owned(),
                CompiledInputIntent {
                    descriptor_index: descriptor.index(),
                    id: descriptor.id().to_owned(),
                    value_kind: descriptor.value_kind(),
                    payload_contract: descriptor.payload_contract().map(str::to_owned),
                    capability,
                    payload: descriptor.payload().clone(),
                },
            );
        }
        let mut mappings = Vec::with_capacity(linked.admitted().input_map().len());
        for mapping in linked.admitted().input_map() {
            let intent = intents
                .get(mapping.intent())
                .ok_or(RuntimeInputError::UnknownIntent)?;
            if intent.value_kind() != mapping.intent_descriptor().value_kind() {
                return Err(RuntimeInputError::IntentValueKindMismatch);
            }
            mappings.push(CompiledInputMapping {
                id: mapping.id().to_owned(),
                intent: mapping.intent().to_owned(),
                trigger: runtime_trigger(mapping.trigger())?,
            });
        }
        Ok(Self { intents, mappings })
    }

    pub fn intent(&self, id: &str) -> Option<&CompiledInputIntent> {
        self.intents.get(id)
    }
    pub fn intents(&self) -> impl Iterator<Item = &CompiledInputIntent> {
        self.intents.values()
    }
    pub fn mappings(&self) -> &[CompiledInputMapping] {
        &self.mappings
    }
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
            code: *code,
            edge: *edge,
            chord: chord.clone(),
            context: context(trigger_context)?,
        },
        InputTrigger::PointerButton {
            button,
            edge,
            context: trigger_context,
        } => RuntimeInputTrigger::PointerButton {
            button: *button,
            edge: *edge,
            context: context(trigger_context)?,
        },
        InputTrigger::PointerAxis {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::PointerAxis {
            axis: *axis,
            context: context(trigger_context)?,
        },
        InputTrigger::Wheel {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::Wheel {
            axis: *axis,
            context: context(trigger_context)?,
        },
        InputTrigger::ControllerButton {
            button,
            edge,
            context: trigger_context,
        } => RuntimeInputTrigger::ControllerButton {
            button: *button,
            edge: *edge,
            context: context(trigger_context)?,
        },
        InputTrigger::ControllerAxis {
            axis,
            context: trigger_context,
        } => RuntimeInputTrigger::ControllerAxis {
            axis: *axis,
            context: context(trigger_context)?,
        },
    })
}
