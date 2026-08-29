use serde::Serialize;
use serde_json::Value;

use crate::compile::{TimelineDescriptor, TimelineStep};

/// Printable, deterministic operation metadata for one timeline step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineOperationInspection {
    operation: String,
}

impl TimelineOperationInspection {
    fn from_step(step: &TimelineStep) -> Self {
        Self {
            operation: step.operation().to_owned(),
        }
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }
}

/// One immutable timeline step inspection item.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineStepInspection {
    timeline_id: String,
    index: usize,
    id: String,
    operation: TimelineOperationInspection,
    payload: Value,
}

impl TimelineStepInspection {
    fn from_step(step: &TimelineStep) -> Self {
        Self {
            timeline_id: step.timeline_id().to_owned(),
            index: step.index(),
            id: step.id().to_owned(),
            operation: TimelineOperationInspection::from_step(step),
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

    pub fn operation(&self) -> &TimelineOperationInspection {
        &self.operation
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One complete immutable timeline inspection item.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineInspection {
    index: usize,
    id: String,
    steps: Vec<TimelineStepInspection>,
}

impl TimelineInspection {
    fn from_timeline(index: usize, timeline: &TimelineDescriptor) -> Self {
        Self {
            index,
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

/// Deterministic static timeline catalog readout. It contains only the
/// caller-owned descriptors retained by the Engine mechanism.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimelineInspection {
    timelines: Vec<TimelineInspection>,
}

impl RuntimeTimelineInspection {
    pub(crate) fn from_catalog(timelines: &[TimelineDescriptor]) -> Self {
        Self {
            timelines: timelines
                .iter()
                .enumerate()
                .map(|(index, timeline)| TimelineInspection::from_timeline(index, timeline))
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
