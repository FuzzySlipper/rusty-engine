use product_model::{
    CapabilityKind, CapabilityUse, LinkedCapabilityBinding, LinkedCapabilityTarget,
    LinkedProductComposition,
};
use serde_json::Value;

use crate::{RuntimeTimelineError, RuntimeTimelineInspection};

/// Maximum number of linked timeline declarations retained by a compiled
/// catalog. This mirrors the current Product Model bound.
pub const MAX_COMPILED_TIMELINES: usize = product_model::MAX_TIMELINES;
/// Maximum total linked timeline steps retained by one compiled catalog.
pub const MAX_COMPILED_TIMELINE_STEPS: usize =
    product_model::MAX_TIMELINES.saturating_mul(product_model::MAX_TIMELINE_STEPS);
/// Maximum compact inspection bytes for one compiled catalog.
pub const MAX_RUNTIME_TIMELINE_INSPECTION_BYTES: usize = 1_048_576;

/// Resolved descriptive metadata for one timeline capability. It is not a
/// dispatch key, service handle, callback, or owner registry entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledTimelineCapability {
    binding_index: usize,
    id: String,
    target: String,
    resolved_target: String,
    kind: String,
    owner: String,
    provenance_source: String,
    provenance_path: String,
}

impl CompiledTimelineCapability {
    fn from_binding(binding: &LinkedCapabilityBinding) -> Self {
        let metadata = binding.metadata();
        let provenance = metadata.provenance();
        let resolved_target = match binding.resolved_target() {
            LinkedCapabilityTarget::Engine(capability) => capability.target().to_owned(),
            LinkedCapabilityTarget::ProductKernel(index) => {
                format!("product-kernel[{}]", index.index())
            }
        };
        Self {
            binding_index: binding.binding_index(),
            id: binding.id().to_owned(),
            target: binding.target().to_owned(),
            resolved_target,
            kind: metadata.kind().as_str().to_owned(),
            owner: provenance.owner().to_owned(),
            provenance_source: provenance.source().to_owned(),
            provenance_path: provenance.logical_path().to_owned(),
        }
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn resolved_target(&self) -> &str {
        &self.resolved_target
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn provenance_source(&self) -> &str {
        &self.provenance_source
    }

    pub fn provenance_path(&self) -> &str {
        &self.provenance_path
    }
}

/// A statically linked timeline step retained after Product Model admission.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTimelineStep {
    timeline_id: String,
    index: usize,
    id: String,
    capability: CompiledTimelineCapability,
    payload: Value,
}

impl CompiledTimelineStep {
    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn capability(&self) -> &CompiledTimelineCapability {
        &self.capability
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One statically linked timeline declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTimeline {
    index: usize,
    id: String,
    steps: Vec<CompiledTimelineStep>,
}

impl CompiledTimeline {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn steps(&self) -> &[CompiledTimelineStep] {
        &self.steps
    }

    pub fn step(&self, id: &str) -> Option<&CompiledTimelineStep> {
        self.steps.iter().find(|step| step.id() == id)
    }
}

/// Immutable, pre-runtime catalog compiled from a linked Product Model
/// composition. It owns no runtime queue, external work, or operation state.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTimelineCatalog {
    timelines: Vec<CompiledTimeline>,
    inspection: RuntimeTimelineInspection,
}

impl CompiledTimelineCatalog {
    pub fn compile(linked: &LinkedProductComposition) -> Result<Self, RuntimeTimelineError> {
        let admitted = linked.admitted();
        if admitted.timelines().len() > MAX_COMPILED_TIMELINES {
            return Err(RuntimeTimelineError::BoundsExceeded("timelines"));
        }
        let mut total_steps = 0usize;
        let mut timelines = Vec::with_capacity(admitted.timelines().len());
        for (timeline_index, timeline) in admitted.timelines().iter().enumerate() {
            if timeline.steps().len() > product_model::MAX_TIMELINE_STEPS {
                return Err(RuntimeTimelineError::BoundsExceeded("timeline steps"));
            }
            let mut steps = Vec::with_capacity(timeline.steps().len());
            for (step_index, step) in timeline.steps().iter().enumerate() {
                total_steps = total_steps
                    .checked_add(1)
                    .ok_or(RuntimeTimelineError::BoundsExceeded("timeline steps"))?;
                if total_steps > MAX_COMPILED_TIMELINE_STEPS {
                    return Err(RuntimeTimelineError::BoundsExceeded("timeline steps"));
                }
                let reference = step.capability();
                let binding = linked
                    .capability_binding(reference.binding_index())
                    .filter(|binding| {
                        binding.id() == reference.id() && binding.target() == reference.target()
                    })
                    .ok_or_else(|| {
                        RuntimeTimelineError::UnknownCapability(reference.id().to_owned())
                    })?;
                let metadata = binding.metadata();
                if !metadata.availability().is_linkable()
                    || !metadata.uses().contains(CapabilityUse::Timeline)
                {
                    return Err(RuntimeTimelineError::CapabilityUnavailable {
                        capability: binding.target().to_owned(),
                    });
                }
                if metadata.kind() != CapabilityKind::Operation {
                    return Err(RuntimeTimelineError::CapabilityKindMismatch {
                        capability: binding.target().to_owned(),
                        expected: CapabilityKind::Operation.as_str(),
                        received: metadata.kind().as_str().to_owned(),
                    });
                }
                let payload = serde_json::to_vec(step.payload()).map_err(|error| {
                    RuntimeTimelineError::InvalidTemplate(format!(
                        "timeline `{}` step `{}` payload cannot be encoded: {error}",
                        timeline.id(),
                        step.id()
                    ))
                })?;
                if payload.len() > metadata.budget().maximum_compact_json_payload_bytes() {
                    return Err(RuntimeTimelineError::PayloadTooLarge {
                        timeline: timeline.id().to_owned(),
                        step: step.id().to_owned(),
                        actual: payload.len(),
                        maximum: metadata.budget().maximum_compact_json_payload_bytes(),
                    });
                }
                steps.push(CompiledTimelineStep {
                    timeline_id: timeline.id().to_owned(),
                    index: step_index,
                    id: step.id().to_owned(),
                    capability: CompiledTimelineCapability::from_binding(binding),
                    payload: step.payload().clone(),
                });
            }
            timelines.push(CompiledTimeline {
                index: timeline_index,
                id: timeline.id().to_owned(),
                steps,
            });
        }
        let inspection = RuntimeTimelineInspection::from_catalog(&timelines);
        let encoded = inspection
            .to_json_newline()
            .map_err(RuntimeTimelineError::InspectionEncode)?;
        if encoded.len() > MAX_RUNTIME_TIMELINE_INSPECTION_BYTES {
            return Err(RuntimeTimelineError::BoundsExceeded("inspection bytes"));
        }
        Ok(Self {
            timelines,
            inspection,
        })
    }

    pub fn timelines(&self) -> &[CompiledTimeline] {
        &self.timelines
    }

    pub fn timeline(&self, id: &str) -> Option<&CompiledTimeline> {
        self.timelines.iter().find(|timeline| timeline.id() == id)
    }

    pub fn step(&self, timeline_id: &str, step_id: &str) -> Option<&CompiledTimelineStep> {
        self.timeline(timeline_id)?.step(step_id)
    }

    pub fn inspection(&self) -> &RuntimeTimelineInspection {
        &self.inspection
    }

    pub fn inspection_json_newline(&self) -> Result<Vec<u8>, RuntimeTimelineError> {
        self.inspection
            .to_json_newline()
            .map_err(RuntimeTimelineError::InspectionEncode)
    }

    /// Binds this static catalog to one fresh running lifecycle instance.
    pub fn bind(
        &self,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
    ) -> Result<crate::RuntimeTimeline, RuntimeTimelineError> {
        crate::RuntimeTimeline::bind(self.clone(), lifecycle)
    }
}
