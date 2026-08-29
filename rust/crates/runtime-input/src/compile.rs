use std::collections::BTreeMap;

use crate::{
    model::is_identity, ControllerAxis, ControllerButton, InputAxis, InputContext, InputEdge,
    IntentValueKind, KeyboardControl, PointerButton, RuntimeInputError,
};
use serde_json::Value;

/// A typed physical trigger owned by the standard runtime input lane.
///
/// This is deliberately a runtime vocabulary rather than a product
/// configuration format. A product host may construct these values directly
/// from generated/native configuration without loading or assembling a larger
/// product description.
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
/// product composition. It authorizes only the named direct value shape;
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
    pub fn new(
        id: impl Into<String>,
        target: impl Into<String>,
        binding_index: usize,
    ) -> Result<Self, RuntimeInputError> {
        let id = id.into();
        let target = target.into();
        if !is_identity(&id) || !is_identity(&target) {
            return Err(RuntimeInputError::InvalidMapping);
        }
        Ok(Self {
            id,
            target,
            binding_index,
        })
    }

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
    /// Builds one neutral compiled intent from already-converted descriptor
    /// facts. Product configuration conversion belongs at its owning assembly
    /// edge; this crate retains only the values the input lane consumes.
    pub fn new(
        descriptor_index: usize,
        id: impl Into<String>,
        value_kind: IntentValueKind,
        payload_contract: Option<String>,
        capability: Option<CompiledInputCapabilityLink>,
        payload: Value,
    ) -> Result<Self, RuntimeInputError> {
        let id = id.into();
        if !is_identity(&id) {
            return Err(RuntimeInputError::InvalidIntent);
        }
        if payload_contract
            .as_deref()
            .is_some_and(|contract| !is_identity(contract))
        {
            return Err(RuntimeInputError::InvalidProductPayloadContract);
        }
        Ok(Self {
            descriptor_index,
            id,
            value_kind,
            payload_contract,
            capability,
            payload,
        })
    }

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
        // The owning configuration edge has already admitted this opaque
        // value. Runtime Input never interprets it.
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
    /// held-input ownership. It does not need a product description's intent
    /// mappings merely to retain those infrastructure guarantees.
    pub fn empty() -> Self {
        Self {
            intents: BTreeMap::new(),
            mappings: Vec::new(),
        }
    }

    /// Compiles direct descriptors and typed physical mappings for a standard
    /// runtime without loading an older product composition. The
    /// returned lane owns the normal direct-claim binding, sequence,
    /// context, held-state, clear, and rebind rules.
    pub fn direct_intents(
        descriptors: impl IntoIterator<Item = DirectInputIntentDescriptor>,
    ) -> Result<Self, RuntimeInputError> {
        Self::standard(descriptors, std::iter::empty())
    }

    /// Compiles the standard runtime's direct descriptors and physical mapping
    /// configuration. This path has no product composition input and
    /// performs all mapping identity/value-kind checks before runtime use.
    pub fn standard(
        descriptors: impl IntoIterator<Item = DirectInputIntentDescriptor>,
        mappings: impl IntoIterator<Item = RuntimeInputMapping>,
    ) -> Result<Self, RuntimeInputError> {
        let intents = descriptors
            .into_iter()
            .enumerate()
            .map(|(descriptor_index, descriptor)| {
                CompiledInputIntent::new(
                    descriptor_index,
                    descriptor.id,
                    descriptor.value_kind,
                    descriptor.payload_contract,
                    None,
                    Value::Null,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mappings = mappings
            .into_iter()
            .map(|mapping| CompiledInputMapping::new(mapping.id, mapping.intent, mapping.trigger))
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_parts(intents, mappings)
    }

    /// Builds a neutral mapping set from already-converted runtime values.
    /// Legacy product configuration and any future configuration format should
    /// perform its one-way conversion before calling this boundary.
    pub fn from_parts(
        intents: impl IntoIterator<Item = CompiledInputIntent>,
        mappings: impl IntoIterator<Item = CompiledInputMapping>,
    ) -> Result<Self, RuntimeInputError> {
        let mut intent_map = BTreeMap::new();
        for intent in intents {
            if intent_map.contains_key(intent.id()) {
                return Err(RuntimeInputError::DuplicateIntent);
            }
            intent_map.insert(intent.id.clone(), intent);
        }
        let mut compiled_mappings = Vec::new();
        for mapping in mappings {
            if compiled_mappings
                .iter()
                .any(|existing: &CompiledInputMapping| existing.id == mapping.id)
            {
                return Err(RuntimeInputError::DuplicateMapping);
            }
            let intent = intent_map
                .get(mapping.intent())
                .ok_or(RuntimeInputError::UnknownIntent)?;
            if intent.value_kind() != mapping.trigger().value_kind() {
                return Err(RuntimeInputError::IntentValueKindMismatch);
            }
            compiled_mappings.push(mapping);
        }
        Ok(Self {
            intents: intent_map,
            mappings: compiled_mappings,
        })
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
