use std::collections::{BTreeMap, BTreeSet};

use product_model::{
    CapabilityKind, CapabilityUse, LinkedProductComposition, ScheduleCompositionMode,
    SchedulePhase, SchedulePlacement,
};
use serde_json::Value;

use crate::{
    error::RuntimeScheduleError,
    inspection::{CadenceInspection, CapabilityInspection, ScheduleInspection},
};

/// Maximum number of authored systems admitted by one runtime schedule.
/// Product Model has a matching artifact bound; keeping a runtime bound makes
/// inspection and invocation allocation independently safe if a future linker
/// supplies a different source.
pub const MAX_RUNTIME_SCHEDULE_SYSTEMS: usize = 512;
pub const MAX_RUNTIME_SCHEDULE_INSPECTION_BYTES: usize = 1_048_576;

/// Whether the implicit standard phase anchor remains in a composed phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardAnchorStatus {
    Retained,
    Replaced,
}

impl StandardAnchorStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retained => "retained",
            Self::Replaced => "replaced",
        }
    }
}

/// One resolved, immutable runtime system. It contains data-only capability
/// metadata and never stores an executable owner.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSystem {
    pub(crate) id: String,
    pub(crate) phase: SchedulePhase,
    pub(crate) capability: CapabilityInspection,
    pub(crate) definition: Option<String>,
    pub(crate) reads: Vec<String>,
    pub(crate) writes: Vec<String>,
    pub(crate) cadence: CadenceInspection,
    pub(crate) after: Vec<String>,
    pub(crate) payload: Value,
    pub(crate) placement: SchedulePlacement,
    pub(crate) source_order: usize,
}

impl CompiledSystem {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn phase(&self) -> SchedulePhase {
        self.phase
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

    pub const fn cadence(&self) -> &CadenceInspection {
        &self.cadence
    }

    pub fn after(&self) -> &[String] {
        &self.after
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub const fn placement(&self) -> SchedulePlacement {
        self.placement
    }
}

/// One phase with the stable topological execution order already resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPhase {
    pub(crate) phase: SchedulePhase,
    pub(crate) composition: ScheduleCompositionMode,
    pub(crate) standard_anchor: StandardAnchorStatus,
    pub(crate) systems: Vec<CompiledSystem>,
    pub(crate) final_order: Vec<String>,
}

impl CompiledPhase {
    pub const fn phase(&self) -> SchedulePhase {
        self.phase
    }

    pub const fn composition(&self) -> ScheduleCompositionMode {
        self.composition
    }

    pub const fn standard_anchor(&self) -> StandardAnchorStatus {
        self.standard_anchor
    }

    pub fn systems(&self) -> &[CompiledSystem] {
        &self.systems
    }

    pub fn final_order(&self) -> &[String] {
        &self.final_order
    }
}

/// An immutable schedule compilation. It can be inspected or bound to more
/// than one lifecycle, but every bound [`RuntimeSchedule`](crate::RuntimeSchedule)
/// owns its own progression and is intentionally not cloneable.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRuntimeSchedule {
    pub(crate) phases: [CompiledPhase; 5],
    pub(crate) inspection: ScheduleInspection,
}

impl CompiledRuntimeSchedule {
    /// Resolves a linked composition without accepting a callback, service,
    /// registry, clock, or host value.
    pub fn compile(linked: &LinkedProductComposition) -> Result<Self, RuntimeScheduleError> {
        let admitted_schedule = linked.admitted().schedule();
        if admitted_schedule.len() != SchedulePhase::ALL.len() {
            return Err(RuntimeScheduleError::InvalidComposition(format!(
                "schedule must declare exactly {} phases",
                SchedulePhase::ALL.len()
            )));
        }

        let mut phases = Vec::with_capacity(5);
        let mut system_phases = BTreeMap::new();
        for declaration in admitted_schedule {
            for system in declaration.systems() {
                if system_phases
                    .insert(system.id().to_owned(), declaration.phase())
                    .is_some()
                {
                    return Err(RuntimeScheduleError::DuplicateSystemId(
                        system.id().to_owned(),
                    ));
                }
            }
        }
        let mut total_systems = 0usize;
        for phase in SchedulePhase::ALL {
            let declaration = admitted_schedule
                .iter()
                .find(|declaration| declaration.phase() == phase)
                .ok_or_else(|| {
                    RuntimeScheduleError::InvalidComposition(format!(
                        "schedule phase `{}` is missing",
                        phase.as_str()
                    ))
                })?;
            let composition = declaration.mode();
            let standard_anchor = match composition {
                ScheduleCompositionMode::Replace => StandardAnchorStatus::Replaced,
                _ => StandardAnchorStatus::Retained,
            };
            let mut systems = Vec::with_capacity(declaration.systems().len());
            for system in declaration.systems() {
                let source_order = system.source_index();
                total_systems = total_systems
                    .checked_add(1)
                    .ok_or(RuntimeScheduleError::BoundsExceeded("system count"))?;
                if total_systems > MAX_RUNTIME_SCHEDULE_SYSTEMS {
                    return Err(RuntimeScheduleError::BoundsExceeded("runtime systems"));
                }
                let capability_reference = system.capability();
                let binding = linked
                    .capability_binding(capability_reference.binding_index())
                    .filter(|binding| {
                        binding.id() == capability_reference.id()
                            && binding.target() == capability_reference.target()
                    })
                    .ok_or_else(|| {
                        RuntimeScheduleError::UnknownCapability(
                            capability_reference.id().to_owned(),
                        )
                    })?;
                let metadata = binding.metadata();
                if !metadata.availability().is_linkable() {
                    return Err(RuntimeScheduleError::CapabilityUnavailable(
                        binding.target().to_owned(),
                    ));
                }
                let required_kind = if phase == SchedulePhase::Projection {
                    CapabilityKind::Projection
                } else {
                    CapabilityKind::System
                };
                if metadata.kind() != required_kind {
                    return Err(RuntimeScheduleError::CapabilityUseMismatch {
                        system: system.id().to_owned(),
                        phase,
                        kind: metadata.kind().as_str().to_owned(),
                    });
                }
                if !metadata.uses().contains(CapabilityUse::Schedule) {
                    return Err(RuntimeScheduleError::CapabilityUseMismatch {
                        system: system.id().to_owned(),
                        phase,
                        kind: "not-schedule-capable".to_owned(),
                    });
                }
                let cadence = system.cadence();
                if cadence.every_steps == 0 || cadence.offset_steps >= cadence.every_steps {
                    return Err(RuntimeScheduleError::InvalidCadence {
                        system: system.id().to_owned(),
                        every_steps: cadence.every_steps,
                        offset_steps: cadence.offset_steps,
                    });
                }
                let payload = serde_json::to_vec(system.payload()).map_err(|error| {
                    RuntimeScheduleError::InvalidComposition(format!(
                        "system `{}` payload cannot be encoded: {error}",
                        system.id()
                    ))
                })?;
                let maximum = metadata.budget().maximum_compact_json_payload_bytes();
                if payload.len() > maximum {
                    return Err(RuntimeScheduleError::PayloadTooLarge {
                        system: system.id().to_owned(),
                        actual: payload.len(),
                        maximum,
                    });
                }
                let capability = CapabilityInspection::from_binding(binding);
                let after = validate_dependencies(system.id(), system.after())?;
                systems.push(CompiledSystem {
                    id: system.id().to_owned(),
                    phase,
                    capability,
                    definition: system
                        .definition()
                        .map(|definition| definition.id().to_owned()),
                    reads: system.reads().to_vec(),
                    writes: system.writes().to_vec(),
                    cadence: CadenceInspection::new(cadence.every_steps, cadence.offset_steps),
                    after,
                    payload: system.payload().clone(),
                    placement: system.placement(),
                    source_order,
                });
            }
            let final_order = resolve_phase_order(
                phase,
                composition,
                standard_anchor,
                &systems,
                &system_phases,
            )?;
            phases.push(CompiledPhase {
                phase,
                composition,
                standard_anchor,
                systems,
                final_order,
            });
        }
        let phases: [CompiledPhase; 5] = phases.try_into().map_err(|_| {
            RuntimeScheduleError::InvalidComposition(
                "schedule phase count changed during compilation".to_owned(),
            )
        })?;
        let inspection = ScheduleInspection::from_phases(&phases);
        let encoded = inspection
            .to_json_newline()
            .map_err(RuntimeScheduleError::InspectionEncode)?;
        if encoded.len() > MAX_RUNTIME_SCHEDULE_INSPECTION_BYTES {
            return Err(RuntimeScheduleError::BoundsExceeded("inspection bytes"));
        }
        Ok(Self { phases, inspection })
    }

    pub fn phases(&self) -> &[CompiledPhase; 5] {
        &self.phases
    }

    pub fn phase(&self, phase: SchedulePhase) -> &CompiledPhase {
        &self.phases[phase.index()]
    }

    pub fn inspection(&self) -> &ScheduleInspection {
        &self.inspection
    }

    pub fn inspection_json_newline(&self) -> Result<Vec<u8>, RuntimeScheduleError> {
        self.inspection
            .to_json_newline()
            .map_err(RuntimeScheduleError::InspectionEncode)
    }

    pub fn bind(
        &self,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
    ) -> Result<crate::RuntimeSchedule, RuntimeScheduleError> {
        crate::RuntimeSchedule::bind(self.clone(), lifecycle)
    }
}

fn validate_dependencies(
    system: &str,
    dependencies: &[String],
) -> Result<Vec<String>, RuntimeScheduleError> {
    if dependencies.len() > product_model::MAX_SCHEDULE_DEPENDENCIES {
        return Err(RuntimeScheduleError::BoundsExceeded("system dependencies"));
    }
    let mut seen = BTreeSet::new();
    for dependency in dependencies {
        if dependency == system {
            return Err(RuntimeScheduleError::SelfDependency(system.to_owned()));
        }
        if !seen.insert(dependency) {
            return Err(RuntimeScheduleError::DuplicateDependency {
                system: system.to_owned(),
                dependency: dependency.clone(),
            });
        }
    }
    Ok(dependencies.to_vec())
}

fn resolve_phase_order(
    phase: SchedulePhase,
    composition: ScheduleCompositionMode,
    standard_anchor: StandardAnchorStatus,
    systems: &[CompiledSystem],
    system_phases: &BTreeMap<String, SchedulePhase>,
) -> Result<Vec<String>, RuntimeScheduleError> {
    let by_id = systems
        .iter()
        .enumerate()
        .map(|(index, system)| (system.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut edges = vec![Vec::<usize>::new(); systems.len()];
    let mut indegree = vec![0usize; systems.len()];
    for (index, system) in systems.iter().enumerate() {
        for dependency in &system.after {
            let dependency_index = by_id.get(dependency.as_str()).copied().ok_or_else(|| {
                if system_phases.contains_key(dependency) {
                    RuntimeScheduleError::CrossPhaseDependency {
                        system: system.id.clone(),
                        dependency: dependency.clone(),
                    }
                } else {
                    RuntimeScheduleError::UnknownDependency {
                        system: system.id.clone(),
                        dependency: dependency.clone(),
                    }
                }
            })?;
            let dependency_rank = placement_rank(
                composition,
                standard_anchor,
                systems[dependency_index].placement,
            );
            let system_rank = placement_rank(composition, standard_anchor, system.placement);
            if dependency_rank > system_rank {
                return Err(RuntimeScheduleError::PlacementConflict {
                    phase,
                    system: system.id.clone(),
                    dependency: dependency.clone(),
                });
            }
            edges[dependency_index].push(index);
            indegree[index] += 1;
        }
    }
    reject_access_ambiguity(phase, composition, standard_anchor, systems, &edges)?;

    let mut order = Vec::with_capacity(systems.len());
    let mut ready = Vec::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.push(index);
        }
    }
    while !ready.is_empty() {
        ready.sort_by_key(|index| {
            (
                placement_rank(composition, standard_anchor, systems[*index].placement),
                systems[*index].source_order,
            )
        });
        let index = ready.remove(0);
        order.push(index);
        for successor in &edges[index] {
            indegree[*successor] -= 1;
            if indegree[*successor] == 0 {
                ready.push(*successor);
            }
        }
    }
    if order.len() != systems.len() {
        return Err(RuntimeScheduleError::DependencyCycle { phase });
    }
    Ok(order
        .into_iter()
        .map(|index| systems[index].id.clone())
        .collect())
}

fn placement_rank(
    composition: ScheduleCompositionMode,
    standard_anchor: StandardAnchorStatus,
    placement: SchedulePlacement,
) -> u8 {
    if standard_anchor == StandardAnchorStatus::Replaced {
        return 1;
    }
    match composition {
        ScheduleCompositionMode::Append => 2,
        ScheduleCompositionMode::Prepend => 0,
        ScheduleCompositionMode::Extend => match placement {
            SchedulePlacement::ExtendBefore => 0,
            SchedulePlacement::ExtendAfter => 2,
            _ => 1,
        },
        ScheduleCompositionMode::Replace => 1,
    }
}

fn reject_access_ambiguity(
    phase: SchedulePhase,
    composition: ScheduleCompositionMode,
    standard_anchor: StandardAnchorStatus,
    systems: &[CompiledSystem],
    edges: &[Vec<usize>],
) -> Result<(), RuntimeScheduleError> {
    for first in 0..systems.len() {
        for second in (first + 1)..systems.len() {
            let resource = systems[first]
                .writes
                .iter()
                .find(|resource| {
                    systems[second].writes.contains(resource)
                        || systems[second].reads.contains(resource)
                })
                .or_else(|| {
                    systems[second]
                        .writes
                        .iter()
                        .find(|resource| systems[first].reads.contains(resource))
                });
            let Some(resource) = resource else { continue };
            let placement_orders =
                placement_rank(composition, standard_anchor, systems[first].placement)
                    != placement_rank(composition, standard_anchor, systems[second].placement);
            if !ordered_before(first, second, edges)
                && !ordered_before(second, first, edges)
                && !placement_orders
            {
                return Err(RuntimeScheduleError::AccessConflict {
                    phase,
                    first: systems[first].id.clone(),
                    second: systems[second].id.clone(),
                    resource: resource.clone(),
                });
            }
        }
    }
    Ok(())
}

fn ordered_before(first: usize, second: usize, edges: &[Vec<usize>]) -> bool {
    let mut stack = vec![first];
    let mut visited = vec![false; edges.len()];
    visited[first] = true;
    while let Some(current) = stack.pop() {
        for successor in &edges[current] {
            if *successor == second {
                return true;
            }
            if !visited[*successor] {
                visited[*successor] = true;
                stack.push(*successor);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system(
        id: &str,
        placement: SchedulePlacement,
        reads: &[&str],
        writes: &[&str],
    ) -> CompiledSystem {
        CompiledSystem {
            id: id.to_owned(),
            phase: SchedulePhase::Simulation,
            capability: CapabilityInspection {
                binding_index: 0,
                id: format!("capability.{id}"),
                target: format!("kernel.{id}"),
                resolved_target: format!("kernel.{id}"),
                kind: "system".to_owned(),
                uses: vec!["schedule".to_owned()],
                resolved_owner: "test.owner".to_owned(),
                provenance_source: "test.rs".to_owned(),
                provenance_path: id.to_owned(),
                reads: reads.iter().map(|value| (*value).to_owned()).collect(),
                writes: writes.iter().map(|value| (*value).to_owned()).collect(),
                maximum_compact_json_payload_bytes: 1_024,
            },
            definition: None,
            reads: reads.iter().map(|value| (*value).to_owned()).collect(),
            writes: writes.iter().map(|value| (*value).to_owned()).collect(),
            cadence: CadenceInspection::new(1, 0),
            after: Vec::new(),
            payload: Value::Null,
            placement,
            source_order: 0,
        }
    }

    #[test]
    fn extend_before_and_after_partition_is_explicit_ordering() {
        let systems = vec![
            system(
                "before",
                SchedulePlacement::ExtendBefore,
                &[],
                &["shared.fact"],
            ),
            system(
                "after",
                SchedulePlacement::ExtendAfter,
                &["shared.fact"],
                &[],
            ),
        ];
        let phases = BTreeMap::from([
            ("before".to_owned(), SchedulePhase::Simulation),
            ("after".to_owned(), SchedulePhase::Simulation),
        ]);
        let order = resolve_phase_order(
            SchedulePhase::Simulation,
            ScheduleCompositionMode::Extend,
            StandardAnchorStatus::Retained,
            &systems,
            &phases,
        )
        .expect("anchor partitions order the accesses");
        assert_eq!(order, ["before", "after"]);
    }

    #[test]
    fn same_partition_conflict_requires_dependency() {
        let systems = vec![
            system("first", SchedulePlacement::Append, &[], &["shared.fact"]),
            system("second", SchedulePlacement::Append, &["shared.fact"], &[]),
        ];
        let phases = BTreeMap::from([
            ("first".to_owned(), SchedulePhase::Simulation),
            ("second".to_owned(), SchedulePhase::Simulation),
        ]);
        assert!(matches!(
            resolve_phase_order(
                SchedulePhase::Simulation,
                ScheduleCompositionMode::Append,
                StandardAnchorStatus::Retained,
                &systems,
                &phases,
            ),
            Err(RuntimeScheduleError::AccessConflict { .. })
        ));
    }
}
