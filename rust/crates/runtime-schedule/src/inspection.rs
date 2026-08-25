use std::fmt;

use product_model::{
    CapabilityUse, LinkedCapabilityBinding, LinkedCapabilityTarget, SchedulePhase,
    SchedulePlacement,
};
use serde::Serialize;
use serde_json::Value;

use crate::compile::{CompiledPhase, CompiledSystem, StandardAnchorStatus};

/// Closed capability metadata carried into schedule inspection. It is a
/// readout, not a dispatch key or a service lookup handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityInspection {
    pub(crate) binding_index: usize,
    pub(crate) id: String,
    pub(crate) target: String,
    pub(crate) resolved_target: String,
    pub(crate) kind: String,
    pub(crate) uses: Vec<String>,
    pub(crate) resolved_owner: String,
    pub(crate) provenance_source: String,
    pub(crate) provenance_path: String,
    pub(crate) reads: Vec<String>,
    pub(crate) writes: Vec<String>,
    pub(crate) maximum_compact_json_payload_bytes: usize,
}

impl CapabilityInspection {
    pub(crate) fn from_binding(binding: &LinkedCapabilityBinding) -> Self {
        let metadata = binding.metadata();
        let provenance = metadata.provenance();
        let access = metadata.access();
        let resolved_target = match binding.resolved_target() {
            LinkedCapabilityTarget::Engine(capability) => capability.target().to_owned(),
            LinkedCapabilityTarget::ProductKernel(index) => {
                format!("product-kernel[{}]", index.index())
            }
        };
        let uses = [
            CapabilityUse::InputMap,
            CapabilityUse::Schedule,
            CapabilityUse::Timeline,
        ]
        .into_iter()
        .filter(|usage| metadata.uses().contains(*usage))
        .map(CapabilityUse::as_str)
        .map(str::to_owned)
        .collect();
        Self {
            binding_index: binding.binding_index(),
            id: binding.id().to_owned(),
            target: binding.target().to_owned(),
            resolved_target,
            kind: metadata.kind().as_str().to_owned(),
            uses,
            resolved_owner: provenance.owner().to_owned(),
            provenance_source: provenance.source().to_owned(),
            provenance_path: provenance.logical_path().to_owned(),
            reads: access
                .reads()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            writes: access
                .writes()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            maximum_compact_json_payload_bytes: metadata
                .budget()
                .maximum_compact_json_payload_bytes(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
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

    pub fn uses(&self) -> &[String] {
        &self.uses
    }

    pub fn resolved_owner(&self) -> &str {
        &self.resolved_owner
    }

    pub fn provenance_source(&self) -> &str {
        &self.provenance_source
    }

    pub fn provenance_path(&self) -> &str {
        &self.provenance_path
    }

    pub fn reads(&self) -> &[String] {
        &self.reads
    }

    pub fn writes(&self) -> &[String] {
        &self.writes
    }

    pub const fn maximum_compact_json_payload_bytes(&self) -> usize {
        self.maximum_compact_json_payload_bytes
    }
}

/// Exact step cadence retained in the resolved schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadenceInspection {
    pub(crate) every_steps: u32,
    pub(crate) offset_steps: u32,
}

impl CadenceInspection {
    pub(crate) const fn new(every_steps: u32, offset_steps: u32) -> Self {
        Self {
            every_steps,
            offset_steps,
        }
    }

    pub const fn every_steps(self) -> u32 {
        self.every_steps
    }

    pub const fn offset_steps(self) -> u32 {
        self.offset_steps
    }

    pub const fn is_due(self, step: u64) -> bool {
        if step < self.offset_steps as u64 {
            return false;
        }
        (step - self.offset_steps as u64).is_multiple_of(self.every_steps as u64)
    }
}

/// One system's complete printable schedule readout.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInspection {
    pub(crate) final_index: usize,
    pub(crate) id: String,
    pub(crate) capability: CapabilityInspection,
    pub(crate) definition: Option<String>,
    pub(crate) reads: Vec<String>,
    pub(crate) writes: Vec<String>,
    pub(crate) cadence: CadenceInspection,
    pub(crate) dependencies: Vec<String>,
    pub(crate) placement: String,
    pub(crate) payload: Value,
}

impl SystemInspection {
    pub const fn final_index(&self) -> usize {
        self.final_index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn capability(&self) -> &CapabilityInspection {
        &self.capability
    }

    pub fn definition(&self) -> Option<&str> {
        self.definition.as_deref()
    }

    pub fn reads(&self) -> &[String] {
        &self.reads
    }

    pub fn writes(&self) -> &[String] {
        &self.writes
    }

    pub const fn cadence(&self) -> CadenceInspection {
        self.cadence
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub fn placement(&self) -> &str {
        &self.placement
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One phase's retained anchor, composition operation, final order, and
/// system descriptors.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseInspection {
    pub(crate) phase: String,
    pub(crate) composition: String,
    pub(crate) standard_anchor: String,
    pub(crate) final_order: Vec<String>,
    pub(crate) ordered_items: Vec<ScheduleOrderItem>,
    pub(crate) systems: Vec<SystemInspection>,
}

/// One item in the final printable order. A retained standard phase anchor is
/// explicit even when the runtime catalog currently contributes no systems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleOrderItem {
    pub(crate) final_index: usize,
    pub(crate) kind: String,
    pub(crate) id: String,
}

impl ScheduleOrderItem {
    pub const fn final_index(&self) -> usize {
        self.final_index
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl PhaseInspection {
    pub fn phase(&self) -> &str {
        &self.phase
    }

    pub fn composition(&self) -> &str {
        &self.composition
    }

    pub fn standard_anchor(&self) -> &str {
        &self.standard_anchor
    }

    pub fn final_order(&self) -> &[String] {
        &self.final_order
    }

    pub fn ordered_items(&self) -> &[ScheduleOrderItem] {
        &self.ordered_items
    }

    pub fn systems(&self) -> &[SystemInspection] {
        &self.systems
    }
}

/// Stable typed inspection of one compiled schedule. Its JSON form is a
/// bounded newline-delimited document with no independently versioned schema;
/// actual Product Model changes are the compatibility boundary.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInspection {
    pub(crate) phases: Vec<PhaseInspection>,
}

impl ScheduleInspection {
    pub(crate) fn from_phases(phases: &[CompiledPhase; 5]) -> Self {
        Self {
            phases: phases.iter().map(phase_inspection).collect(),
        }
    }

    pub fn phases(&self) -> &[PhaseInspection] {
        &self.phases
    }

    /// Encodes a compact, deterministic JSON readout and appends one newline.
    pub fn to_json_newline(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

impl fmt::Display for ScheduleInspection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(value) => formatter.write_str(&value),
            Err(_) => formatter.write_str("<invalid schedule inspection>"),
        }
    }
}

fn phase_inspection(phase: &CompiledPhase) -> PhaseInspection {
    let mut ordered_items = Vec::new();
    let mut systems = Vec::new();
    let add_anchor = |ordered_items: &mut Vec<ScheduleOrderItem>, phase: SchedulePhase| {
        let final_index = ordered_items.len();
        ordered_items.push(ScheduleOrderItem {
            final_index,
            kind: "standard".to_owned(),
            id: format!("Standard.{}", phase.as_str()),
        });
    };
    match phase.standard_anchor {
        StandardAnchorStatus::Replaced => {
            for system_id in &phase.final_order {
                push_system(phase, system_id, &mut ordered_items, &mut systems);
            }
        }
        StandardAnchorStatus::Retained => match phase.composition.as_str() {
            "prepend" => {
                for system_id in &phase.final_order {
                    push_system(phase, system_id, &mut ordered_items, &mut systems);
                }
                add_anchor(&mut ordered_items, phase.phase);
            }
            "extend" => {
                let mut anchor_added = false;
                for system_id in &phase.final_order {
                    let system = phase
                        .systems
                        .iter()
                        .find(|system| system.id() == system_id)
                        .expect("compiled final order contains only compiled system ids");
                    if !anchor_added && system.placement().as_str() == "extend-after" {
                        add_anchor(&mut ordered_items, phase.phase);
                        anchor_added = true;
                    }
                    push_system(phase, system_id, &mut ordered_items, &mut systems);
                }
                if !anchor_added {
                    add_anchor(&mut ordered_items, phase.phase);
                }
            }
            _ => {
                add_anchor(&mut ordered_items, phase.phase);
                for system_id in &phase.final_order {
                    push_system(phase, system_id, &mut ordered_items, &mut systems);
                }
            }
        },
    }
    PhaseInspection {
        phase: phase.phase.as_str().to_owned(),
        composition: phase.composition.as_str().to_owned(),
        standard_anchor: phase.standard_anchor.as_str().to_owned(),
        final_order: ordered_items.iter().map(|item| item.id.clone()).collect(),
        ordered_items,
        systems,
    }
}

fn push_system(
    phase: &CompiledPhase,
    system_id: &str,
    ordered_items: &mut Vec<ScheduleOrderItem>,
    systems: &mut Vec<SystemInspection>,
) {
    let system = phase
        .systems
        .iter()
        .find(|system| system.id() == system_id)
        .expect("compiled final order contains only compiled system ids");
    let final_index = ordered_items.len();
    ordered_items.push(ScheduleOrderItem {
        final_index,
        kind: "system".to_owned(),
        id: system.id().to_owned(),
    });
    systems.push(system_inspection(system, final_index));
}

fn system_inspection(system: &CompiledSystem, final_index: usize) -> SystemInspection {
    SystemInspection {
        final_index,
        id: system.id.clone(),
        capability: system.capability.clone(),
        definition: system.definition.clone(),
        reads: system.reads.clone(),
        writes: system.writes.clone(),
        cadence: system.cadence,
        dependencies: system.after.clone(),
        placement: placement_name(system.placement),
        payload: system.payload.clone(),
    }
}

fn placement_name(placement: SchedulePlacement) -> String {
    placement.as_str().to_owned()
}
