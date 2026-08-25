use std::collections::BTreeMap;

use product_model::{InputTrigger, IntentValueKind, LinkedProductComposition};
use serde_json::Value;

use crate::RuntimeInputError;

/// One linked product intent available to both physical mappings and direct UI
/// claims. Capability linkage was completed by Product Model before this
/// compiler accepts it; this crate retains only descriptive readouts.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledInputIntent {
    descriptor_index: usize,
    id: String,
    value_kind: IntentValueKind,
    payload_contract: Option<String>,
    capability_id: String,
    capability_target: String,
    capability_binding_index: usize,
    capability_payload: Value,
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
    pub fn capability_id(&self) -> &str {
        &self.capability_id
    }
    pub fn capability_target(&self) -> &str {
        &self.capability_target
    }
    pub const fn capability_binding_index(&self) -> usize {
        self.capability_binding_index
    }
    /// Immutable intent-specific capability data admitted with the descriptor.
    /// It is descriptive readout only; the input lane never interprets it.
    pub fn capability_payload(&self) -> &Value {
        &self.capability_payload
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledInputMapping {
    id: String,
    intent: String,
    trigger: InputTrigger,
}

impl CompiledInputMapping {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn intent(&self) -> &str {
        &self.intent
    }
    pub fn trigger(&self) -> &InputTrigger {
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
    pub fn compile(linked: &LinkedProductComposition) -> Result<Self, RuntimeInputError> {
        let mut intents = BTreeMap::new();
        for descriptor in linked.admitted().intent_descriptors() {
            let capability = linked
                .capability_binding(descriptor.capability().binding_index())
                .ok_or(RuntimeInputError::BindingMismatch)?;
            if capability.id() != descriptor.capability().id()
                || capability.target() != descriptor.capability().target()
            {
                return Err(RuntimeInputError::BindingMismatch);
            }
            intents.insert(
                descriptor.id().to_owned(),
                CompiledInputIntent {
                    descriptor_index: descriptor.index(),
                    id: descriptor.id().to_owned(),
                    value_kind: descriptor.value_kind(),
                    payload_contract: descriptor.payload_contract().map(str::to_owned),
                    capability_id: descriptor.capability().id().to_owned(),
                    capability_target: descriptor.capability().target().to_owned(),
                    capability_binding_index: descriptor.capability().binding_index(),
                    capability_payload: descriptor.payload().clone(),
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
                trigger: mapping.trigger().clone(),
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
