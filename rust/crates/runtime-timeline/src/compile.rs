use serde_json::Value;

use crate::{model::validate_runtime_identity, RuntimeOpaqueData, RuntimeTimelineDataError};
use crate::{RuntimeTimelineError, RuntimeTimelineInspection};

/// Maximum number of timeline descriptors retained by one catalog.
pub const MAX_TIMELINES: usize = 256;
/// Maximum number of steps retained by one timeline descriptor.
pub const MAX_TIMELINE_STEPS: usize = 256;
/// Maximum total number of steps retained by one catalog.
pub const MAX_TIMELINE_DESCRIPTOR_STEPS: usize = MAX_TIMELINES * MAX_TIMELINE_STEPS;
/// Maximum compact inspection bytes for one catalog.
pub const MAX_RUNTIME_TIMELINE_INSPECTION_BYTES: usize = 1_048_576;

/// One caller-owned operation descriptor used by a retained timeline lane.
///
/// The descriptor identifies data that the product will interpret when the
/// step is released. It is not a service handle, dispatch registry entry, or
/// callback.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineStepDescriptor {
    id: String,
    operation: String,
    payload: RuntimeOpaqueData,
}

impl TimelineStepDescriptor {
    /// Creates one immutable step descriptor. The operation string is a
    /// product-owned identity; the timeline lane only carries it back in the
    /// release record.
    pub fn new(
        id: impl Into<String>,
        operation: impl Into<String>,
        payload: Value,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let id = id.into();
        let operation = operation.into();
        validate_runtime_identity(&id).map_err(|_| RuntimeTimelineDataError::InvalidIdentity)?;
        validate_runtime_identity(&operation)
            .map_err(|_| RuntimeTimelineDataError::InvalidIdentity)?;
        Ok(Self {
            id,
            operation,
            payload: RuntimeOpaqueData::new(payload)?,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn payload(&self) -> &Value {
        self.payload.value()
    }

    pub fn payload_data(&self) -> &RuntimeOpaqueData {
        &self.payload
    }
}

/// One immutable timeline descriptor retained by a catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineDescriptor {
    id: String,
    steps: Vec<TimelineStep>,
}

impl TimelineDescriptor {
    pub fn new(
        id: impl Into<String>,
        steps: impl IntoIterator<Item = TimelineStepDescriptor>,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let id = id.into();
        validate_runtime_identity(&id).map_err(|_| RuntimeTimelineDataError::InvalidIdentity)?;
        let steps = steps.into_iter().collect::<Vec<_>>();
        if steps.len() > MAX_TIMELINE_STEPS {
            return Err(RuntimeTimelineDataError::DescriptorTooLarge {
                kind: "timeline steps",
                maximum: MAX_TIMELINE_STEPS,
            });
        }
        let mut step_ids = std::collections::BTreeSet::new();
        let steps = steps
            .into_iter()
            .enumerate()
            .map(|(index, step)| {
                if !step_ids.insert(step.id.clone()) {
                    return Err(RuntimeTimelineDataError::DuplicateDescriptor(
                        step.id.clone(),
                    ));
                }
                Ok(TimelineStep {
                    timeline_id: id.clone(),
                    index,
                    id: step.id,
                    operation: step.operation,
                    payload: step.payload,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { id, steps })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn steps(&self) -> &[TimelineStep] {
        &self.steps
    }

    pub fn step(&self, id: &str) -> Option<&TimelineStep> {
        self.steps.iter().find(|step| step.id() == id)
    }
}

/// One immutable step record returned with a release. It is a normalized
/// descriptor owned by the timeline catalog and carries no external handle.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineStep {
    timeline_id: String,
    index: usize,
    id: String,
    operation: String,
    payload: RuntimeOpaqueData,
}

impl TimelineStep {
    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn payload(&self) -> &Value {
        self.payload.value()
    }

    pub fn payload_data(&self) -> &RuntimeOpaqueData {
        &self.payload
    }
}

/// Immutable catalog of caller-owned timeline descriptors.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineCatalog {
    timelines: Vec<TimelineDescriptor>,
    inspection: RuntimeTimelineInspection,
}

impl TimelineCatalog {
    pub fn new(
        timelines: impl IntoIterator<Item = TimelineDescriptor>,
    ) -> Result<Self, RuntimeTimelineError> {
        let timelines = timelines.into_iter().collect::<Vec<_>>();
        if timelines.len() > MAX_TIMELINES {
            return Err(RuntimeTimelineError::BoundsExceeded("timelines"));
        }
        let mut timeline_ids = std::collections::BTreeSet::new();
        let mut total_steps = 0usize;
        for timeline in &timelines {
            if !timeline_ids.insert(timeline.id()) {
                return Err(RuntimeTimelineError::DuplicateTimeline(
                    timeline.id().to_owned(),
                ));
            }
            total_steps = total_steps
                .checked_add(timeline.steps().len())
                .ok_or(RuntimeTimelineError::BoundsExceeded("timeline steps"))?;
            if total_steps > MAX_TIMELINE_DESCRIPTOR_STEPS {
                return Err(RuntimeTimelineError::BoundsExceeded("timeline steps"));
            }
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

    pub fn empty() -> Self {
        Self::new(std::iter::empty()).expect("empty timeline catalog is valid")
    }

    pub fn timelines(&self) -> &[TimelineDescriptor] {
        &self.timelines
    }

    pub fn timeline(&self, id: &str) -> Option<&TimelineDescriptor> {
        self.timelines.iter().find(|timeline| timeline.id() == id)
    }

    pub fn step(&self, timeline_id: &str, step_id: &str) -> Option<&TimelineStep> {
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

    /// Binds this static descriptor catalog to one fresh running lifecycle
    /// instance.
    pub fn bind(
        &self,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
    ) -> Result<crate::RuntimeTimeline, RuntimeTimelineError> {
        crate::RuntimeTimeline::bind(self.clone(), lifecycle)
    }
}
