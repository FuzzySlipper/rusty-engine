use serde::Serialize;
use serde_json::Value;

use crate::compile::{CompiledTimeline, CompiledTimelineStep};

/// Printable, deterministic capability provenance for one timeline step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineCapabilityInspection {
    binding_index: usize,
    id: String,
    target: String,
    resolved_target: String,
    kind: String,
    owner: String,
    provenance_source: String,
    provenance_path: String,
}

impl TimelineCapabilityInspection {
    fn from_step(step: &CompiledTimelineStep) -> Self {
        let capability = step.capability();
        Self {
            binding_index: capability.binding_index(),
            id: capability.id().to_owned(),
            target: capability.target().to_owned(),
            resolved_target: capability.resolved_target().to_owned(),
            kind: capability.kind().to_owned(),
            owner: capability.owner().to_owned(),
            provenance_source: capability.provenance_source().to_owned(),
            provenance_path: capability.provenance_path().to_owned(),
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

/// One statically compiled timeline step inspection item.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineStepInspection {
    timeline_id: String,
    index: usize,
    id: String,
    capability: TimelineCapabilityInspection,
    payload: Value,
}

impl TimelineStepInspection {
    fn from_step(step: &CompiledTimelineStep) -> Self {
        Self {
            timeline_id: step.timeline_id().to_owned(),
            index: step.index(),
            id: step.id().to_owned(),
            capability: TimelineCapabilityInspection::from_step(step),
            payload: step.payload().clone(),
        }
    }

    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn capability(&self) -> &TimelineCapabilityInspection {
        &self.capability
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One complete statically compiled timeline inspection item.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineInspection {
    index: usize,
    id: String,
    steps: Vec<TimelineStepInspection>,
}

impl TimelineInspection {
    fn from_timeline(timeline: &CompiledTimeline) -> Self {
        Self {
            index: timeline.index(),
            id: timeline.id().to_owned(),
            steps: timeline
                .steps()
                .iter()
                .map(TimelineStepInspection::from_step)
                .collect(),
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn steps(&self) -> &[TimelineStepInspection] {
        &self.steps
    }
}

/// Deterministic static timeline catalog readout. It has no independently
/// versioned schema; Product Model changes are the compatibility boundary.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimelineInspection {
    timelines: Vec<TimelineInspection>,
}

impl RuntimeTimelineInspection {
    pub(crate) fn from_catalog(timelines: &[CompiledTimeline]) -> Self {
        Self {
            timelines: timelines
                .iter()
                .map(TimelineInspection::from_timeline)
                .collect(),
        }
    }

    pub fn timelines(&self) -> &[TimelineInspection] {
        &self.timelines
    }

    pub fn to_json_newline(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
