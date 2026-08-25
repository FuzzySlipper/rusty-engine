use std::collections::BTreeMap;

use crate::RuntimeInputError;
use product_model::{InputTrigger, IntentValueKind, LinkedProductComposition};
use serde_json::Value;

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
