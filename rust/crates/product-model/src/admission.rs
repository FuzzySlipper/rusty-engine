//! Immutable admission of one checked Compiled Composition for one Product Layout.
//!
//! Admission establishes a validated product/lifecycle linkage and resolves
//! composition-local references into stable declaration readouts. It neither
//! resolves runtime targets nor evaluates schedules, timelines, inputs, or
//! payloads.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::{
    diagnostic::failure, validate_compiled_composition, CompiledComposition,
    CompiledCompositionCandidate, LifecycleMode, ProductManifest, ProductModelError, RealtimeClock,
};

const SOURCE: &str = "product-composition-admission";

/// A composition-local capability binding with its stable admitted index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedCapabilityBinding {
    index: usize,
    id: String,
    target: String,
}

impl AdmittedCapabilityBinding {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The declared Engine/Kernel target name. This is not a live resolution.
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// A checked reference to one declared capability binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedCapabilityReference {
    binding_index: usize,
    id: String,
    target: String,
}

impl AdmittedCapabilityReference {
    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

/// A composition-local gameplay definition with its stable admitted index.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedGameplayDefinition {
    index: usize,
    id: String,
    payload: Value,
}

impl AdmittedGameplayDefinition {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// A checked reference to one declared gameplay definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedDefinitionReference {
    definition_index: usize,
    id: String,
}

impl AdmittedDefinitionReference {
    pub const fn definition_index(&self) -> usize {
        self.definition_index
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// One ordered input declaration with a resolved capability reference.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedInputMapEntry {
    id: String,
    intent: String,
    capability: AdmittedCapabilityReference,
    payload: Value,
}

impl AdmittedInputMapEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn intent(&self) -> &str {
        &self.intent
    }

    pub fn capability(&self) -> &AdmittedCapabilityReference {
        &self.capability
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One ordered schedule declaration. Read/write declarations remain data only;
/// this type deliberately assigns neither conflict nor execution semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedScheduleFragment {
    id: String,
    phase: String,
    capability: AdmittedCapabilityReference,
    definition: Option<AdmittedDefinitionReference>,
    reads: Vec<String>,
    writes: Vec<String>,
    payload: Value,
}

impl AdmittedScheduleFragment {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn phase(&self) -> &str {
        &self.phase
    }

    pub fn capability(&self) -> &AdmittedCapabilityReference {
        &self.capability
    }

    pub fn definition(&self) -> Option<&AdmittedDefinitionReference> {
        self.definition.as_ref()
    }

    pub fn reads(&self) -> &[String] {
        &self.reads
    }

    pub fn writes(&self) -> &[String] {
        &self.writes
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One ordered timeline declaration with already checked capability references.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedTimeline {
    id: String,
    steps: Vec<AdmittedTimelineStep>,
}

impl AdmittedTimeline {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn steps(&self) -> &[AdmittedTimelineStep] {
        &self.steps
    }
}

/// One ordered timeline step with a resolved capability reference.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedTimelineStep {
    id: String,
    capability: AdmittedCapabilityReference,
    payload: Value,
}

impl AdmittedTimelineStep {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn capability(&self) -> &AdmittedCapabilityReference {
        &self.capability
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// A composition admitted for one Product Layout. Its declarations are fixed
/// after construction and are exposed only as inspection readouts.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedProductComposition {
    product_id: String,
    lifecycle: LifecycleMode,
    realtime: Option<RealtimeClock>,
    composition: CompiledComposition,
    input_map: Vec<AdmittedInputMapEntry>,
    schedule: Vec<AdmittedScheduleFragment>,
    gameplay_definitions: Vec<AdmittedGameplayDefinition>,
    timelines: Vec<AdmittedTimeline>,
    capability_bindings: Vec<AdmittedCapabilityBinding>,
}

impl AdmittedProductComposition {
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    pub const fn lifecycle(&self) -> LifecycleMode {
        self.lifecycle
    }

    pub const fn realtime(&self) -> Option<RealtimeClock> {
        self.realtime
    }

    /// Returns the checked source artifact without granting mutation of the
    /// admitted value. Runtime-facing consumers should use the resolved slices.
    pub fn composition(&self) -> &CompiledComposition {
        &self.composition
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        self.composition.canonical_bytes()
    }

    pub fn input_map(&self) -> &[AdmittedInputMapEntry] {
        &self.input_map
    }

    pub fn schedule(&self) -> &[AdmittedScheduleFragment] {
        &self.schedule
    }

    pub fn gameplay_definitions(&self) -> &[AdmittedGameplayDefinition] {
        &self.gameplay_definitions
    }

    pub fn timelines(&self) -> &[AdmittedTimeline] {
        &self.timelines
    }

    pub fn capability_bindings(&self) -> &[AdmittedCapabilityBinding] {
        &self.capability_bindings
    }
}

/// Validates a direct composition candidate, then admits it for the product.
/// This has the same result as admitting its checked-artifact counterpart.
pub fn admit_product_composition(
    manifest: &ProductManifest,
    candidate: CompiledCompositionCandidate,
) -> Result<AdmittedProductComposition, ProductModelError> {
    let composition = validate_compiled_composition(candidate)?;
    admit_checked_product_composition(manifest, composition)
}

/// Admits an already checked artifact for a product. No value is returned until
/// product linkage and every local reference have been resolved.
pub fn admit_checked_product_composition(
    manifest: &ProductManifest,
    composition: CompiledComposition,
) -> Result<AdmittedProductComposition, ProductModelError> {
    let candidate = composition.candidate();
    if candidate.product != manifest.product_id() {
        return Err(failure(
            "PRODUCT_COMPOSITION_PRODUCT_MISMATCH",
            SOURCE,
            "product",
            format!(
                "compiled composition product `{}` does not match Product Layout product `{}`",
                candidate.product,
                manifest.product_id()
            ),
        ));
    }

    let capability_bindings = candidate
        .capability_bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| AdmittedCapabilityBinding {
            index,
            id: binding.id.clone(),
            target: binding.target.clone(),
        })
        .collect::<Vec<_>>();
    let capability_indices = capability_bindings
        .iter()
        .map(|binding| (binding.id.clone(), binding.index))
        .collect::<BTreeMap<_, _>>();

    let gameplay_definitions = candidate
        .gameplay_definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| AdmittedGameplayDefinition {
            index,
            id: definition.id.clone(),
            payload: definition.payload.clone(),
        })
        .collect::<Vec<_>>();
    let definition_indices = gameplay_definitions
        .iter()
        .map(|definition| (definition.id.clone(), definition.index))
        .collect::<BTreeMap<_, _>>();

    let input_map = candidate
        .input_map
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            Ok(AdmittedInputMapEntry {
                id: entry.id.clone(),
                intent: entry.intent.clone(),
                capability: resolve_capability(
                    &entry.capability,
                    &capability_indices,
                    &capability_bindings,
                    &format!("inputMap[{index}].capability"),
                )?,
                payload: entry.payload.clone(),
            })
        })
        .collect::<Result<Vec<_>, ProductModelError>>()?;

    let schedule = candidate
        .schedule
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            Ok(AdmittedScheduleFragment {
                id: entry.id.clone(),
                phase: entry.phase.clone(),
                capability: resolve_capability(
                    &entry.capability,
                    &capability_indices,
                    &capability_bindings,
                    &format!("schedule[{index}].capability"),
                )?,
                definition: entry
                    .definition
                    .as_deref()
                    .map(|id| {
                        resolve_definition(
                            id,
                            &definition_indices,
                            &gameplay_definitions,
                            &format!("schedule[{index}].definition"),
                        )
                    })
                    .transpose()?,
                reads: entry.reads.clone(),
                writes: entry.writes.clone(),
                payload: entry.payload.clone(),
            })
        })
        .collect::<Result<Vec<_>, ProductModelError>>()?;

    let timelines = candidate
        .timelines
        .iter()
        .enumerate()
        .map(|(timeline_index, timeline)| {
            let steps = timeline
                .steps
                .iter()
                .enumerate()
                .map(|(step_index, step)| {
                    Ok(AdmittedTimelineStep {
                        id: step.id.clone(),
                        capability: resolve_capability(
                            &step.capability,
                            &capability_indices,
                            &capability_bindings,
                            &format!("timelines[{timeline_index}].steps[{step_index}].capability"),
                        )?,
                        payload: step.payload.clone(),
                    })
                })
                .collect::<Result<Vec<_>, ProductModelError>>()?;
            Ok(AdmittedTimeline {
                id: timeline.id.clone(),
                steps,
            })
        })
        .collect::<Result<Vec<_>, ProductModelError>>()?;

    Ok(AdmittedProductComposition {
        product_id: manifest.product_id().to_owned(),
        lifecycle: manifest.lifecycle(),
        realtime: manifest.realtime(),
        composition,
        input_map,
        schedule,
        gameplay_definitions,
        timelines,
        capability_bindings,
    })
}

fn resolve_capability(
    id: &str,
    indices: &BTreeMap<String, usize>,
    bindings: &[AdmittedCapabilityBinding],
    path: &str,
) -> Result<AdmittedCapabilityReference, ProductModelError> {
    let index = indices.get(id).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_UNRESOLVED_CAPABILITY",
            SOURCE,
            path,
            format!("checked composition capability `{id}` was not available for admission"),
        )
    })?;
    let binding = bindings.get(*index).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_CAPABILITY_INDEX",
            SOURCE,
            path,
            format!("checked composition capability `{id}` had an invalid admitted index {index}"),
        )
    })?;
    Ok(AdmittedCapabilityReference {
        binding_index: binding.index,
        id: binding.id.clone(),
        target: binding.target.clone(),
    })
}

fn resolve_definition(
    id: &str,
    indices: &BTreeMap<String, usize>,
    definitions: &[AdmittedGameplayDefinition],
    path: &str,
) -> Result<AdmittedDefinitionReference, ProductModelError> {
    let index = indices.get(id).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_UNRESOLVED_DEFINITION",
            SOURCE,
            path,
            format!("checked composition definition `{id}` was not available for admission"),
        )
    })?;
    let definition = definitions.get(*index).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_DEFINITION_INDEX",
            SOURCE,
            path,
            format!("checked composition definition `{id}` had an invalid admitted index {index}"),
        )
    })?;
    Ok(AdmittedDefinitionReference {
        definition_index: definition.index,
        id: definition.id.clone(),
    })
}
