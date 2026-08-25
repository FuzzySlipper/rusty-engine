//! Bounded, read-only Product Model inspection documents.
//!
//! This module deliberately translates admitted and linked declarations into
//! small printable facts instead of exposing their opaque JSON payloads or a
//! second runtime topology.  It is a CLI presentation owner: it does not
//! admit files, resolve capabilities, start a host, or query a global runtime.

use std::fmt;

use product_assembly::{AssemblyEntryKind, AssemblyReceipt};
use product_model::{
    AdmittedProductComposition, CapabilityAvailability, CapabilityUse, InputTrigger, LifecycleMode,
    LinkedCapabilityBinding, LinkedCapabilityTarget, LinkedProductComposition, ProductManifest,
};
use serde::Serialize;

/// The maximum number of printable facts retained in one inspection result.
/// Product Model permits a large cartesian timeline declaration space, so this
/// independent CLI ceiling keeps a diagnostic command from becoming an
/// unbounded export mechanism.
pub(crate) const MAX_INSPECTION_FACTS: usize = crate::report::MAX_FACTS;
pub(crate) const MAX_INSPECTION_DIAGNOSTICS: usize = 32;
pub(crate) const MAX_INSPECTION_FIELD_BYTES: usize = 256;

/// A closed inspection section accepted by `rusty inspect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InspectSubject {
    All,
    Composition,
    Input,
    Schedule,
    CapabilityBindings,
    Timelines,
    Lifecycle,
    Mutation,
}

/// One bounded, stable printable fact. `owner` identifies the mechanism that
/// owns the fact; `source` identifies its declaration or readout surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectionFact {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source: String,
    pub(crate) value: String,
}

/// A bounded note or error explaining an unavailable inspection readout.
/// Every diagnostic includes an owner, source, and concrete remedy so a CLI
/// caller does not have to infer where a missing fact should come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectionDiagnostic {
    pub(crate) level: &'static str,
    pub(crate) code: &'static str,
    pub(crate) owner: String,
    pub(crate) source: String,
    pub(crate) remedy: String,
}

/// One serializable inspection response. It contains only bounded strings and
/// facts derived from typed admitted/linker/receipt readouts; it intentionally
/// has no `serde_json::Value` escape hatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectionDocument {
    pub(crate) status: &'static str,
    pub(crate) subject: InspectSubject,
    pub(crate) product: String,
    pub(crate) facts: Vec<InspectionFact>,
    pub(crate) diagnostics: Vec<InspectionDiagnostic>,
}

/// Typed input to the inspection presentation layer. The optional linked
/// composition and receipt are supplied only after their respective workflow
/// stages have completed; their absence is reported as a bounded diagnostic,
/// never hidden by a synthetic runtime status.
pub(crate) struct InspectionRequest<'a> {
    pub(crate) manifest: &'a ProductManifest,
    pub(crate) admitted: &'a AdmittedProductComposition,
    pub(crate) linked: Option<&'a LinkedProductComposition>,
    pub(crate) assembly_receipt: Option<&'a AssemblyReceipt>,
}

/// A request failure caused by inconsistent typed workflow inputs. This is
/// intentionally distinct from an unavailable optional readout, which is a
/// successful inspection document with a diagnostic and remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionError {
    diagnostic: InspectionDiagnostic,
}

impl InspectionError {
    pub(crate) fn diagnostic(&self) -> &InspectionDiagnostic {
        &self.diagnostic
    }
}

impl fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}, {}): {}",
            self.diagnostic.code,
            self.diagnostic.owner,
            self.diagnostic.source,
            self.diagnostic.remedy
        )
    }
}

impl std::error::Error for InspectionError {}

/// Produces one deterministic, bounded read-only inspection document.
pub(crate) fn inspect(
    subject: InspectSubject,
    request: InspectionRequest<'_>,
) -> Result<InspectionDocument, InspectionError> {
    validate_request(&request)?;
    let mut builder = InspectionBuilder::new(subject, request.manifest.product_id());
    match subject {
        InspectSubject::All => {
            inspect_composition(&mut builder, &request);
            inspect_input(&mut builder, &request);
            inspect_schedule(&mut builder, &request);
            inspect_capability_bindings(&mut builder, &request);
            inspect_timelines(&mut builder, &request);
            inspect_lifecycle(&mut builder, &request);
            inspect_mutation(&mut builder, &request);
        }
        InspectSubject::Composition => inspect_composition(&mut builder, &request),
        InspectSubject::Input => inspect_input(&mut builder, &request),
        InspectSubject::Schedule => inspect_schedule(&mut builder, &request),
        InspectSubject::CapabilityBindings => inspect_capability_bindings(&mut builder, &request),
        InspectSubject::Timelines => inspect_timelines(&mut builder, &request),
        InspectSubject::Lifecycle => inspect_lifecycle(&mut builder, &request),
        InspectSubject::Mutation => inspect_mutation(&mut builder, &request),
    }
    Ok(builder.finish())
}

fn validate_request(request: &InspectionRequest<'_>) -> Result<(), InspectionError> {
    if request.manifest.product_id() != request.admitted.product_id() {
        return Err(InspectionError {
            diagnostic: diagnostic(
                "error",
                "RUSTY_INSPECT_PRODUCT_MISMATCH",
                "product-model",
                "rusty.toml / product-composition-admission",
                "regenerate the admitted composition from this product manifest before inspecting it",
            ),
        });
    }
    if let Some(linked) = request.linked {
        if linked.admitted() != request.admitted {
            return Err(InspectionError {
                diagnostic: diagnostic(
                    "error",
                    "RUSTY_INSPECT_LINKED_COMPOSITION_MISMATCH",
                    "product-model",
                    "linked-product-composition",
                    "link this exact admitted composition before requesting capability or mutation inspection",
                ),
            });
        }
    }
    if let Some(receipt) = request.assembly_receipt {
        if receipt.product() != request.manifest.product_id() {
            return Err(InspectionError {
                diagnostic: diagnostic(
                    "error",
                    "RUSTY_INSPECT_ASSEMBLY_PRODUCT_MISMATCH",
                    "product-assembly",
                    "generated/assembly.json",
                    "regenerate or select the Product Assembly receipt for this product before inspecting it",
                ),
            });
        }
    }
    Ok(())
}

fn inspect_composition(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    let admitted = request.admitted;
    builder.fact(
        "composition.product",
        "product-model",
        "rusty.toml",
        format!(
            "entrypoints={}; runtime={}; kernel={}; ui={}; content={}",
            request.manifest.composition_entrypoints().len(),
            optional_path(request.manifest.runtime_entry()),
            optional_path(request.manifest.kernel_entry()),
            request.manifest.ui_entry().as_str(),
            request.manifest.content_root().as_str(),
        ),
    );
    builder.fact(
        "composition.admitted",
        "product-model",
        "compiled-composition.json",
        format!(
            "canonicalBytes={}; definitions={}; intents={}; inputMappings={}; schedulePhases={}; timelines={}; capabilityBindings={}",
            admitted.canonical_bytes().len(),
            admitted.gameplay_definitions().len(),
            admitted.intent_descriptors().len(),
            admitted.input_map().len(),
            admitted.schedule().len(),
            admitted.timelines().len(),
            admitted.capability_bindings().len(),
        ),
    );
    builder.fact(
        "composition.generatedOutputs",
        "product-model",
        "rusty.toml",
        format!(
            "compiled={}; admittedContent={}; assembly={}; bundle={}",
            request.manifest.compiled_composition_output().as_str(),
            request.manifest.admitted_runtime_content_output().as_str(),
            request.manifest.product_assembly_output().as_str(),
            request.manifest.product_bundle_output().as_str(),
        ),
    );
    match request.assembly_receipt {
        Some(receipt) => {
            builder.fact(
                "composition.assemblyReceipt",
                "product-assembly",
                "generated/assembly.json",
                format!(
                    "artifact={}; entries={}; authoredSource={}; compiledComposition={}; runtimeContent={}; executableWorkspace={}; browserBundle={}",
                    receipt.artifact(),
                    receipt.entries().len(),
                    receipt_entries(receipt, AssemblyEntryKind::AuthoredSource),
                    receipt_entries(receipt, AssemblyEntryKind::CompiledComposition),
                    receipt_entries(receipt, AssemblyEntryKind::RuntimeContent),
                    receipt_entries(receipt, AssemblyEntryKind::ExecutableWorkspace),
                    receipt_entries(receipt, AssemblyEntryKind::BrowserBundle),
                ),
            );
        }
        None => builder.diagnostic(
            "note",
            "RUSTY_INSPECT_ASSEMBLY_RECEIPT_UNAVAILABLE",
            "product-assembly",
            "generated/assembly.json",
            "run Product Assembly or provide its exact receipt to inspect the generated closure",
        ),
    }
}

fn inspect_input(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    for descriptor in request.admitted.intent_descriptors() {
        let capability = descriptor
            .capability()
            .map(|capability| capability_label(request.linked, capability.binding_index()))
            .unwrap_or_else(|| "none".to_owned());
        builder.fact(
            format!("input.intents[{}]", descriptor.index()),
            "runtime-input",
            "compiled-composition.json",
            format!(
                "id={}; valueKind={}; payloadContract={}; capability={}; payload=omitted",
                descriptor.id(),
                intent_value_kind(descriptor.value_kind()),
                descriptor.payload_contract().unwrap_or("none"),
                capability,
            ),
        );
    }
    for (index, mapping) in request.admitted.input_map().iter().enumerate() {
        builder.fact(
            format!("input.mappings[{index}]"),
            "runtime-input",
            "compiled-composition.json",
            format!(
                "id={}; intent={}; descriptorIndex={}; trigger={}",
                mapping.id(),
                mapping.intent(),
                mapping.intent_descriptor().descriptor_index(),
                input_trigger_label(mapping.trigger()),
            ),
        );
    }
}

fn inspect_schedule(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    for (phase_index, phase) in request.admitted.schedule().iter().enumerate() {
        builder.fact(
            format!("schedule.phases[{phase_index}]"),
            "runtime-schedule",
            "compiled-composition.json",
            format!(
                "phase={}; composition={}; systems={}",
                schedule_phase(phase.phase()),
                schedule_mode(phase.mode()),
                phase.systems().len(),
            ),
        );
        for system in phase.systems() {
            builder.fact(
                format!("schedule.phases[{phase_index}].systems[{}]", system.source_index()),
                "runtime-schedule",
                "compiled-composition.json",
                format!(
                    "id={}; capability={}; definition={}; placement={}; cadence={}/{}; after={}; reads={}; writes={}; payload=omitted",
                    system.id(),
                    capability_label(request.linked, system.capability().binding_index()),
                    system.definition().map_or("none", |definition| definition.id()),
                    schedule_placement(system.placement()),
                    system.cadence().every_steps,
                    system.cadence().offset_steps,
                    joined(system.after()),
                    joined(system.reads()),
                    joined(system.writes()),
                ),
            );
        }
    }
}

fn inspect_capability_bindings(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    match request.linked {
        Some(linked) => {
            for binding in linked.capability_bindings() {
                let metadata = binding.metadata();
                let provenance = metadata.provenance();
                let access = metadata.access();
                builder.fact(
                    format!("capabilityBindings[{}]", binding.binding_index()),
                    provenance.owner(),
                    provenance.source(),
                    format!(
                        "id={}; target={}; resolvedTarget={}; kind={}; uses={}; availability={}; reads={}; writes={}; payloadBudget={}; provenancePath={}",
                        binding.id(),
                        binding.target(),
                        linked_target_label(binding),
                        metadata.kind().as_str(),
                        capability_uses_label(metadata.uses()),
                        availability_label(metadata.availability()),
                        joined_static(access.reads()),
                        joined_static(access.writes()),
                        metadata.budget().maximum_compact_json_payload_bytes(),
                        provenance.logical_path(),
                    ),
                );
            }
        }
        None => {
            for binding in request.admitted.capability_bindings() {
                builder.fact(
                    format!("capabilityBindings[{}]", binding.index()),
                    "product-model",
                    "compiled-composition.json",
                    format!(
                        "id={}; target={}; linkage=unavailable",
                        binding.id(),
                        binding.target(),
                    ),
                );
            }
            builder.diagnostic(
                "note",
                "RUSTY_INSPECT_LINKED_CAPABILITIES_UNAVAILABLE",
                "product-model",
                "linked-product-composition",
                "link the admitted composition with the selected Engine and Product Kernel descriptors to inspect resolved capability ownership",
            );
        }
    }
}

fn inspect_timelines(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    for (timeline_index, timeline) in request.admitted.timelines().iter().enumerate() {
        builder.fact(
            format!("timelines[{timeline_index}]"),
            "runtime-timeline",
            "compiled-composition.json",
            format!("id={}; steps={}", timeline.id(), timeline.steps().len()),
        );
        for (step_index, step) in timeline.steps().iter().enumerate() {
            builder.fact(
                format!("timelines[{timeline_index}].steps[{step_index}]"),
                "runtime-timeline",
                "compiled-composition.json",
                format!(
                    "id={}; capability={}; payload=omitted",
                    step.id(),
                    capability_label(request.linked, step.capability().binding_index()),
                ),
            );
        }
    }
}

fn inspect_lifecycle(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    let lifecycle = match request.manifest.lifecycle() {
        LifecycleMode::Realtime => {
            let clock = request.manifest.realtime();
            format!(
                "mode=realtime; fixedStepHz={}; maxCatchUpSteps={}",
                clock.map_or(0, |value| value.fixed_step_hz()),
                clock.map_or(0, |value| value.max_catch_up_steps()),
            )
        }
        LifecycleMode::Demand => "mode=demand".to_owned(),
        LifecycleMode::External => "mode=external".to_owned(),
    };
    builder.fact(
        "lifecycle.declaration",
        "runtime-lifecycle",
        "rusty.toml",
        lifecycle,
    );
    builder.diagnostic(
        "note",
        "RUSTY_INSPECT_LIVE_LIFECYCLE_UNAVAILABLE",
        "runtime-lifecycle",
        "runtime instance readout",
        "connect an explicit selected runtime instance readout to inspect current state, generation, revision, and admitted steps",
    );
}

fn inspect_mutation(builder: &mut InspectionBuilder, request: &InspectionRequest<'_>) {
    let Some(linked) = request.linked else {
        builder.diagnostic(
            "note",
            "RUSTY_INSPECT_MUTATION_CATALOG_UNAVAILABLE",
            "runtime-mutation",
            "linked-product-composition",
            "link the admitted composition before inspecting selected operation capability declarations",
        );
        return;
    };
    let mut operations = 0usize;
    for binding in linked.capability_bindings() {
        let metadata = binding.metadata();
        if metadata.kind().as_str() != "operation" {
            continue;
        }
        operations += 1;
        let provenance = metadata.provenance();
        builder.fact(
            format!("mutation.operations[{}]", binding.binding_index()),
            provenance.owner(),
            provenance.source(),
            format!(
                "bindingId={}; target={}; resolvedTarget={}; uses={}; payloadBudget={}; execution=runtime-readout-unavailable",
                binding.id(),
                binding.target(),
                linked_target_label(binding),
                capability_uses_label(metadata.uses()),
                metadata.budget().maximum_compact_json_payload_bytes(),
            ),
        );
    }
    if operations == 0 {
        builder.diagnostic(
            "note",
            "RUSTY_INSPECT_MUTATION_OPERATION_ABSENT",
            "runtime-mutation",
            "linked-product-composition",
            "declare and link an operation capability when this product requires runtime-mutation execution",
        );
    }
    builder.diagnostic(
        "note",
        "RUSTY_INSPECT_LIVE_MUTATION_UNAVAILABLE",
        "runtime-mutation",
        "runtime mutation readout",
        "connect an explicit selected runtime mutation readout to inspect applied or rejected operation receipts",
    );
}

fn receipt_entries(receipt: &AssemblyReceipt, kind: AssemblyEntryKind) -> usize {
    receipt
        .entries()
        .iter()
        .filter(|entry| entry.kind() == kind)
        .count()
}

fn optional_path(path: Option<&product_model::ProductPath>) -> &str {
    path.map_or("none", product_model::ProductPath::as_str)
}

fn capability_label(linked: Option<&LinkedProductComposition>, binding_index: usize) -> String {
    linked
        .and_then(|composition| composition.capability_binding(binding_index))
        .map(|binding| format!("{} ({})", binding.id(), linked_target_label(binding)))
        .unwrap_or_else(|| format!("binding[{binding_index}] (declared)"))
}

fn linked_target_label(binding: &LinkedCapabilityBinding) -> String {
    match binding.resolved_target() {
        LinkedCapabilityTarget::Engine(capability) => capability.target().to_owned(),
        LinkedCapabilityTarget::ProductKernel(index) => {
            format!("product-kernel[{}]", index.index())
        }
    }
}

fn capability_uses_label(uses: product_model::CapabilityUses) -> String {
    [
        CapabilityUse::InputMap,
        CapabilityUse::Schedule,
        CapabilityUse::Timeline,
    ]
    .into_iter()
    .filter(|usage| uses.contains(*usage))
    .map(CapabilityUse::as_str)
    .collect::<Vec<_>>()
    .join(",")
}

fn availability_label(availability: CapabilityAvailability) -> String {
    match availability {
        CapabilityAvailability::Linkable => "linkable".to_owned(),
        CapabilityAvailability::Unavailable { reason } => format!("unavailable:{reason}"),
    }
}

fn input_trigger_label(trigger: &InputTrigger) -> String {
    serde_json::to_string(trigger).unwrap_or_else(|_| "<unserializable-trigger>".to_owned())
}

fn intent_value_kind(value: product_model::IntentValueKind) -> &'static str {
    match value {
        product_model::IntentValueKind::Digital => "digital",
        product_model::IntentValueKind::Axis => "axis",
        product_model::IntentValueKind::ProductPayload => "product-payload",
    }
}

fn schedule_phase(value: product_model::SchedulePhase) -> &'static str {
    match value {
        product_model::SchedulePhase::Input => "input",
        product_model::SchedulePhase::Simulation => "simulation",
        product_model::SchedulePhase::Consequences => "consequences",
        product_model::SchedulePhase::Commit => "commit",
        product_model::SchedulePhase::Projection => "projection",
    }
}

fn schedule_mode(value: product_model::ScheduleCompositionMode) -> &'static str {
    match value {
        product_model::ScheduleCompositionMode::Append => "append",
        product_model::ScheduleCompositionMode::Prepend => "prepend",
        product_model::ScheduleCompositionMode::Extend => "extend",
        product_model::ScheduleCompositionMode::Replace => "replace",
    }
}

fn schedule_placement(value: product_model::SchedulePlacement) -> &'static str {
    match value {
        product_model::SchedulePlacement::Append => "append",
        product_model::SchedulePlacement::Prepend => "prepend",
        product_model::SchedulePlacement::ExtendBefore => "extend-before",
        product_model::SchedulePlacement::ExtendAfter => "extend-after",
        product_model::SchedulePlacement::Replace => "replace",
    }
}

fn joined(values: &[String]) -> String {
    values.join(",")
}

fn joined_static(values: &[&str]) -> String {
    values.join(",")
}

fn diagnostic(
    level: &'static str,
    code: &'static str,
    owner: impl Into<String>,
    source: impl Into<String>,
    remedy: impl Into<String>,
) -> InspectionDiagnostic {
    InspectionDiagnostic {
        level,
        code,
        owner: bounded(owner.into()),
        source: bounded(source.into()),
        remedy: bounded(remedy.into()),
    }
}

struct InspectionBuilder {
    subject: InspectSubject,
    product: String,
    facts: Vec<InspectionFact>,
    diagnostics: Vec<InspectionDiagnostic>,
    omitted_facts: usize,
}

impl InspectionBuilder {
    fn new(subject: InspectSubject, product: &str) -> Self {
        Self {
            subject,
            product: bounded(product.to_owned()),
            facts: Vec::new(),
            diagnostics: Vec::new(),
            omitted_facts: 0,
        }
    }

    fn fact(
        &mut self,
        path: impl Into<String>,
        owner: impl Into<String>,
        source: impl Into<String>,
        value: impl Into<String>,
    ) {
        if self.facts.len() >= MAX_INSPECTION_FACTS {
            self.omitted_facts += 1;
            return;
        }
        self.facts.push(InspectionFact {
            path: bounded(path.into()),
            owner: bounded(owner.into()),
            source: bounded(source.into()),
            value: bounded(value.into()),
        });
    }

    fn diagnostic(
        &mut self,
        level: &'static str,
        code: &'static str,
        owner: impl Into<String>,
        source: impl Into<String>,
        remedy: impl Into<String>,
    ) {
        if self.diagnostics.len() < MAX_INSPECTION_DIAGNOSTICS {
            self.diagnostics
                .push(diagnostic(level, code, owner, source, remedy));
        }
    }

    fn finish(mut self) -> InspectionDocument {
        if self.omitted_facts > 0 {
            self.diagnostic(
                "note",
                "RUSTY_INSPECT_FACT_LIMIT",
                "rusty-cli",
                "inspect",
                format!(
                    "narrow the inspection subject; {} facts were omitted after the {MAX_INSPECTION_FACTS}-fact limit",
                    self.omitted_facts
                ),
            );
        }
        InspectionDocument {
            status: if self.diagnostics.iter().any(|item| item.level == "error") {
                "error"
            } else if self.diagnostics.is_empty() {
                "ok"
            } else {
                "incomplete"
            },
            subject: self.subject,
            product: self.product,
            facts: self.facts,
            diagnostics: self.diagnostics,
        }
    }
}

fn bounded(value: String) -> String {
    if value.len() <= MAX_INSPECTION_FIELD_BYTES {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = MAX_INSPECTION_FIELD_BYTES - ELLIPSIS.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::{bounded, InspectSubject, InspectionBuilder, MAX_INSPECTION_FACTS};

    #[test]
    fn facts_are_capped_with_a_deterministic_remedy() {
        let mut builder = InspectionBuilder::new(InspectSubject::Input, "counter");
        for index in 0..=MAX_INSPECTION_FACTS {
            builder.fact(
                format!("input[{index}]"),
                "runtime-input",
                "compiled-composition.json",
                "bounded",
            );
        }

        let document = builder.finish();

        assert_eq!(document.facts.len(), MAX_INSPECTION_FACTS);
        assert!(document
            .diagnostics
            .iter()
            .any(|item| item.code == "RUSTY_INSPECT_FACT_LIMIT"));
    }

    #[test]
    fn field_truncation_preserves_utf8() {
        let value = "🦀".repeat(128);
        let bounded = bounded(value);

        assert!(bounded.len() <= super::MAX_INSPECTION_FIELD_BYTES);
        assert!(bounded.ends_with('…'));
    }
}
