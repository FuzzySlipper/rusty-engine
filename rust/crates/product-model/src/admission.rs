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
    CompiledCompositionCandidate, InputTrigger, IntentValueKind, LifecycleMode, ProductManifest,
    ProductModelError, RealtimeClock, ScheduleCadence, ScheduleComposition,
    ScheduleCompositionMode, SchedulePhase, SchedulePlacement,
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

/// One descriptor for a semantic product intent with an optional resolved
/// capability. VM-local intents retain no Product Kernel linkage.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedIntentDescriptor {
    index: usize,
    id: String,
    value_kind: IntentValueKind,
    payload_contract: Option<String>,
    capability: Option<AdmittedCapabilityReference>,
    payload: Value,
}

impl AdmittedIntentDescriptor {
    pub const fn index(&self) -> usize {
        self.index
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn value_kind(&self) -> IntentValueKind {
        self.value_kind
    }
    /// Stable contract selected by the descriptor for direct product payloads.
    /// The UI-provided wire label is checked against this value before a
    /// runtime envelope is emitted.
    pub fn payload_contract(&self) -> Option<&str> {
        self.payload_contract.as_deref()
    }
    pub fn capability(&self) -> Option<&AdmittedCapabilityReference> {
        self.capability.as_ref()
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// A checked reference to a product intent descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedIntentReference {
    descriptor_index: usize,
    id: String,
    value_kind: IntentValueKind,
}

impl AdmittedIntentReference {
    pub const fn descriptor_index(&self) -> usize {
        self.descriptor_index
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn value_kind(&self) -> IntentValueKind {
        self.value_kind
    }
}

/// One ordered physical mapping into a resolved product intent descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedInputMapEntry {
    id: String,
    intent: AdmittedIntentReference,
    trigger: InputTrigger,
}

impl AdmittedInputMapEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn intent(&self) -> &str {
        self.intent.id()
    }

    pub fn intent_descriptor(&self) -> &AdmittedIntentReference {
        &self.intent
    }

    pub fn trigger(&self) -> &InputTrigger {
        &self.trigger
    }
}

/// One admitted schedule system. Placement is derived from the authored phase
/// composition so runtime inspection can explain where the system entered the
/// final schedule without adding a second wire-level provenance field.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedScheduleSystem {
    id: String,
    capability: AdmittedCapabilityReference,
    definition: Option<AdmittedDefinitionReference>,
    placement: SchedulePlacement,
    source_index: usize,
    after: Vec<String>,
    reads: Vec<String>,
    writes: Vec<String>,
    cadence: ScheduleCadence,
    payload: Value,
}

impl AdmittedScheduleSystem {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn capability(&self) -> &AdmittedCapabilityReference {
        &self.capability
    }

    pub fn definition(&self) -> Option<&AdmittedDefinitionReference> {
        self.definition.as_ref()
    }

    pub const fn placement(&self) -> SchedulePlacement {
        self.placement
    }

    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    pub fn after(&self) -> &[String] {
        &self.after
    }

    pub fn reads(&self) -> &[String] {
        &self.reads
    }

    pub fn writes(&self) -> &[String] {
        &self.writes
    }

    pub const fn cadence(&self) -> ScheduleCadence {
        self.cadence
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One admitted Runtime Composition phase with the explicit composition operation and
/// ordered product systems retained for inspection and runtime resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedSchedulePhase {
    phase: SchedulePhase,
    mode: ScheduleCompositionMode,
    systems: Vec<AdmittedScheduleSystem>,
}

impl AdmittedSchedulePhase {
    pub const fn phase(&self) -> SchedulePhase {
        self.phase
    }

    pub const fn mode(&self) -> ScheduleCompositionMode {
        self.mode
    }

    pub fn systems(&self) -> &[AdmittedScheduleSystem] {
        &self.systems
    }
}

/// Compatibility alias for callers that used the pre-7256 flat readout name.
/// The admitted value is now a system rather than a free-form schedule entry.
pub type AdmittedScheduleFragment = AdmittedScheduleSystem;

struct ScheduleAdmissionContext<'a> {
    capability_indices: &'a BTreeMap<String, usize>,
    capability_bindings: &'a [AdmittedCapabilityBinding],
    definition_indices: &'a BTreeMap<String, usize>,
    gameplay_definitions: &'a [AdmittedGameplayDefinition],
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
    intent_descriptors: Vec<AdmittedIntentDescriptor>,
    input_map: Vec<AdmittedInputMapEntry>,
    schedule: Vec<AdmittedSchedulePhase>,
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

    pub fn intent_descriptors(&self) -> &[AdmittedIntentDescriptor] {
        &self.intent_descriptors
    }

    pub fn schedule(&self) -> &[AdmittedSchedulePhase] {
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

    let intent_descriptors = candidate
        .intent_descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            if descriptor.capability.is_none() && manifest.runtime_entry().is_none() {
                return Err(failure(
                    "PRODUCT_COMPOSITION_INTENT_CAPABILITY_REQUIRED",
                    SOURCE,
                    format!("intentDescriptors[{index}].capability"),
                    "intent descriptors require a capability binding unless the Product Layout selects runtime.entry",
                ));
            }
            Ok(AdmittedIntentDescriptor {
                index,
                id: descriptor.id.clone(),
                value_kind: descriptor.value_kind,
                payload_contract: descriptor.payload_contract.clone(),
                capability: descriptor
                    .capability
                    .as_deref()
                    .map(|capability| {
                        resolve_capability(
                            capability,
                            &capability_indices,
                            &capability_bindings,
                            &format!("intentDescriptors[{index}].capability"),
                        )
                    })
                    .transpose()?,
                payload: descriptor.payload.clone(),
            })
        })
        .collect::<Result<Vec<_>, ProductModelError>>()?;
    let intent_indices = intent_descriptors
        .iter()
        .map(|intent| (intent.id.clone(), intent.index))
        .collect::<BTreeMap<_, _>>();

    let input_map = candidate
        .input_map
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            Ok(AdmittedInputMapEntry {
                id: entry.id.clone(),
                intent: resolve_intent(
                    &entry.intent,
                    &intent_indices,
                    &intent_descriptors,
                    &format!("inputMap[{index}].intent"),
                )?,
                trigger: entry.trigger.clone(),
            })
        })
        .collect::<Result<Vec<_>, ProductModelError>>()?;

    let schedule_context = ScheduleAdmissionContext {
        capability_indices: &capability_indices,
        capability_bindings: &capability_bindings,
        definition_indices: &definition_indices,
        gameplay_definitions: &gameplay_definitions,
    };
    let schedule = candidate
        .schedule
        .iter()
        .enumerate()
        .map(|(phase_index, phase)| {
            let systems = admitted_schedule_systems(&phase.composition)
                .into_iter()
                .map(|(system_index, system, placement)| {
                    admit_schedule_system(
                        system,
                        placement,
                        system_index,
                        &schedule_context,
                        &format!("schedule[{phase_index}].system[{system_index}]"),
                    )
                })
                .collect::<Result<Vec<_>, ProductModelError>>()?;
            Ok(AdmittedSchedulePhase {
                phase: phase.phase,
                mode: phase.composition.mode(),
                systems,
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
        intent_descriptors,
        input_map,
        schedule,
        gameplay_definitions,
        timelines,
        capability_bindings,
    })
}

fn admitted_schedule_systems(
    composition: &ScheduleComposition,
) -> Vec<(usize, &crate::ScheduleSystem, SchedulePlacement)> {
    match composition {
        ScheduleComposition::Append { systems } => systems
            .iter()
            .enumerate()
            .map(|(index, system)| (index, system, SchedulePlacement::Append))
            .collect(),
        ScheduleComposition::Prepend { systems } => systems
            .iter()
            .enumerate()
            .map(|(index, system)| (index, system, SchedulePlacement::Prepend))
            .collect(),
        ScheduleComposition::Extend { before, after } => before
            .iter()
            .enumerate()
            .map(|(index, system)| (index, system, SchedulePlacement::ExtendBefore))
            .chain(
                after
                    .iter()
                    .enumerate()
                    .map(|(index, system)| (index, system, SchedulePlacement::ExtendAfter)),
            )
            .collect(),
        ScheduleComposition::Replace { systems } => systems
            .iter()
            .enumerate()
            .map(|(index, system)| (index, system, SchedulePlacement::Replace))
            .collect(),
    }
}

fn admit_schedule_system(
    system: &crate::ScheduleSystem,
    placement: SchedulePlacement,
    source_index: usize,
    context: &ScheduleAdmissionContext<'_>,
    path: &str,
) -> Result<AdmittedScheduleSystem, ProductModelError> {
    Ok(AdmittedScheduleSystem {
        id: system.id.clone(),
        capability: resolve_capability(
            &system.capability,
            context.capability_indices,
            context.capability_bindings,
            &format!("{path}.capability"),
        )?,
        definition: system
            .definition
            .as_deref()
            .map(|id| {
                resolve_definition(
                    id,
                    context.definition_indices,
                    context.gameplay_definitions,
                    &format!("{path}.definition"),
                )
            })
            .transpose()?,
        placement,
        source_index,
        after: system.after.clone(),
        reads: system.reads.clone(),
        writes: system.writes.clone(),
        cadence: system.cadence,
        payload: system.payload.clone(),
    })
}

fn resolve_intent(
    id: &str,
    indices: &BTreeMap<String, usize>,
    descriptors: &[AdmittedIntentDescriptor],
    path: &str,
) -> Result<AdmittedIntentReference, ProductModelError> {
    let index = indices.get(id).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_UNRESOLVED_INTENT",
            SOURCE,
            path,
            format!("checked composition intent `{id}` was not available for admission"),
        )
    })?;
    let descriptor = descriptors.get(*index).ok_or_else(|| {
        failure(
            "PRODUCT_COMPOSITION_INTENT_INDEX",
            SOURCE,
            path,
            format!("checked composition intent `{id}` had an invalid admitted index {index}"),
        )
    })?;
    Ok(AdmittedIntentReference {
        descriptor_index: descriptor.index,
        id: descriptor.id.clone(),
        value_kind: descriptor.value_kind,
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
