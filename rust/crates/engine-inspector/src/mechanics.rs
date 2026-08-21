use std::{collections::BTreeMap, fmt::Write};

use core_ids::EntityId;
use entity_state::EntityState;
use gameplay_mechanics::{
    decode_snapshot_with_catalog, DamageFact, DamageReceipt, DecisionOutcome,
    EffectSourceActivation, InventoryReadCost, InventoryView, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsComponentKind, MechanicsEntityView, MechanicsError,
    ObservedComponentRevision, ResponseDecisionKind, RoundingPolicy, SourceCollectionCost,
    SourceInstanceIdentity, StackingPolicy, StatEvaluation, TrackMaximum,
};
use serde::Serialize;
use serde_json::Value;

use crate::{
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity,
    RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsComponentInspection {
    pub kind: String,
    pub type_id: String,
    pub codec_id: String,
    pub codec_version: u32,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsSourceCostInspection {
    pub intrinsic_entries_visited: usize,
    pub effect_entries_visited: usize,
    pub effect_source_activations_visited: usize,
    pub equipment_entries_visited: usize,
    pub item_components_read: usize,
    pub request_entries_visited: usize,
}

impl From<SourceCollectionCost> for MechanicsSourceCostInspection {
    fn from(value: SourceCollectionCost) -> Self {
        Self {
            intrinsic_entries_visited: value.intrinsic_entries_visited,
            effect_entries_visited: value.effect_entries_visited,
            effect_source_activations_visited: value.effect_source_activations_visited,
            equipment_entries_visited: value.equipment_entries_visited,
            item_components_read: value.item_components_read,
            request_entries_visited: value.request_entries_visited,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsStatDecisionInspection {
    pub source: Value,
    pub source_definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution_index: Option<u16>,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacking_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsStoredStatInspection {
    pub id: String,
    pub base: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsStoredTrackInspection {
    pub id: String,
    pub current: i64,
    pub declared_minimum: i64,
    pub declared_maximum: MechanicsTrackMaximumInspection,
}

/// The catalog declaration, not a recomputed current bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MechanicsTrackMaximumInspection {
    Fixed { value: i64 },
    Stat { stat: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsSourceBindingInspection {
    pub instance: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEffectInspection {
    pub instance: String,
    pub definition: String,
    pub provenance: Value,
    pub stacks: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsSourceActivationInspection {
    pub source: Value,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsInventoryItemInspection {
    pub entity: u64,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsCapacityInspection {
    pub metric: String,
    pub used: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsInventoryCostInspection {
    pub stack_entries_visited: usize,
    pub containment_entries_visited: usize,
    pub item_components_read: usize,
    pub capacity_limits_visited: usize,
    pub capacity_costs_visited: usize,
}

impl From<InventoryReadCost> for MechanicsInventoryCostInspection {
    fn from(value: InventoryReadCost) -> Self {
        Self {
            stack_entries_visited: value.stack_entries_visited,
            containment_entries_visited: value.containment_entries_visited,
            item_components_read: value.item_components_read,
            capacity_limits_visited: value.capacity_limits_visited,
            capacity_costs_visited: value.capacity_costs_visited,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsInventoryInspection {
    pub stacks: Vec<gameplay_mechanics::ItemStack>,
    pub capacity_limits: Vec<gameplay_mechanics::InventoryCapacityLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsItemInspection {
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEquipmentAssignmentInspection {
    pub slot: String,
    pub item: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsStructuralEntityInspection {
    pub entity: u64,
    pub catalog_version: String,
    pub catalog_fingerprint: String,
    pub components: Vec<MechanicsComponentInspection>,
    #[serde(rename = "storedStats")]
    pub stats: Vec<MechanicsStoredStatInspection>,
    #[serde(rename = "storedTracks")]
    pub tracks: Vec<MechanicsStoredTrackInspection>,
    pub intrinsic_sources: Vec<MechanicsSourceBindingInspection>,
    pub effects: Vec<MechanicsEffectInspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<MechanicsInventoryInspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<MechanicsItemInspection>,
    pub equipment: Vec<MechanicsEquipmentAssignmentInspection>,
}

/// Compatibility inspection shape for callers that already gathered all derived owner facts.
/// Unlike the structural report, this retains the pre-existing enriched JSON field meanings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEntityInspection {
    pub entity: u64,
    pub catalog_version: String,
    pub catalog_fingerprint: String,
    pub components: Vec<MechanicsComponentInspection>,
    pub stats: Vec<MechanicsStatInspection>,
    pub tracks: Vec<MechanicsTrackInspection>,
    pub intrinsic_sources: Vec<MechanicsSourceBindingInspection>,
    pub effects: Vec<MechanicsEffectInspection>,
    pub effect_source_activations: Vec<MechanicsSourceActivationInspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<MechanicsEnrichedInventoryInspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<MechanicsItemInspection>,
    pub equipment: Vec<MechanicsEquipmentAssignmentInspection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsStatInspection {
    pub id: String,
    pub base: i64,
    pub after_additions: i64,
    pub combined_scale_numerator: u128,
    pub combined_scale_denominator: u128,
    pub after_scaling: i64,
    pub unconstrained: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub value: i64,
    pub decisions: Vec<MechanicsStatDecisionInspection>,
    pub source_cost: MechanicsSourceCostInspection,
    pub observed_revisions: Vec<MechanicsObservedComponentRevisionInspection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsTrackInspection {
    pub id: String,
    pub current: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub maximum_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEnrichedInventoryInspection {
    pub relationship_revision: u64,
    pub stacks: Vec<gameplay_mechanics::ItemStack>,
    pub unique_items: Vec<MechanicsInventoryItemInspection>,
    pub capacity: Vec<MechanicsCapacityInspection>,
    pub read_cost: MechanicsInventoryCostInspection,
}

/// Derived owner evidence explicitly gathered before inspection. The inspector only copies it.
#[derive(Debug, Clone, Copy)]
pub struct MechanicsInspectionEvidence<'a> {
    pub stat_evaluations: &'a [StatEvaluation],
    pub effect_source_activations: &'a [EffectSourceActivation],
    pub inventory: Option<&'a InventoryView>,
}

impl MechanicsEntityInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "gameplay-mechanics entity={} catalogVersion={} fingerprint={} enrichedFromOwnerEvidence=true\n",
            self.entity, self.catalog_version, self.catalog_fingerprint
        );
        for stat in &self.stats {
            let _ = writeln!(
                output,
                "evaluated stat {} base={} value={} bounds={}..{} decisions={}",
                stat.id,
                stat.base,
                stat.value,
                stat.minimum,
                stat.maximum,
                stat.decisions.len()
            );
        }
        let _ = writeln!(
            output,
            "sources intrinsic={} suppliedEffectActivations={}",
            self.intrinsic_sources.len(),
            self.effect_source_activations.len()
        );
        output
    }
}

impl<'a> MechanicsInspectionEvidence<'a> {
    pub const fn empty() -> Self {
        Self {
            stat_evaluations: &[],
            effect_source_activations: &[],
            inventory: None,
        }
    }
}

/// Assemble the compatibility report from a structural snapshot and caller-supplied owner facts.
/// This never evaluates stats, activates effects, joins inventory, or mutates state.
pub fn inspect_mechanics_entity_from_evidence(
    structural: &MechanicsStructuralEntityInspection,
    evidence: MechanicsInspectionEvidence<'_>,
) -> Result<MechanicsEntityInspection, MechanicsError> {
    let evaluations: BTreeMap<_, _> = evidence
        .stat_evaluations
        .iter()
        .map(|evaluation| (evaluation.stat.as_str(), evaluation))
        .collect();
    let stats = evidence
        .stat_evaluations
        .iter()
        .map(|evaluation| {
            let evaluation = inspect_stat_evaluation(evaluation);
            MechanicsStatInspection {
                id: evaluation.id,
                base: evaluation.base,
                after_additions: evaluation.after_additions,
                combined_scale_numerator: evaluation.combined_scale_numerator,
                combined_scale_denominator: evaluation.combined_scale_denominator,
                after_scaling: evaluation.after_scaling,
                unconstrained: evaluation.unconstrained,
                minimum: evaluation.minimum,
                maximum: evaluation.maximum,
                value: evaluation.value,
                decisions: evaluation.decisions,
                source_cost: evaluation.source_cost,
                observed_revisions: evaluation.observed_revisions,
            }
        })
        .collect();
    let tracks = structural
        .tracks
        .iter()
        .map(|track| {
            let (maximum, maximum_source) = match &track.declared_maximum {
                MechanicsTrackMaximumInspection::Fixed { value } => (*value, "fixed".to_string()),
                MechanicsTrackMaximumInspection::Stat { stat } => {
                    let evaluation = evaluations.get(stat.as_str()).ok_or_else(|| {
                        MechanicsError::MissingStat {
                            entity: EntityId::new(structural.entity),
                            stat: gameplay_mechanics::StatId::parse(stat)
                                .expect("stored catalog stat identity is valid"),
                        }
                    })?;
                    (evaluation.value.get(), format!("stat:{stat}"))
                }
            };
            Ok(MechanicsTrackInspection {
                id: track.id.clone(),
                current: track.current,
                minimum: track.declared_minimum,
                maximum,
                maximum_source,
            })
        })
        .collect::<Result<Vec<_>, MechanicsError>>()?;
    let inventory = evidence
        .inventory
        .map(|inventory| MechanicsEnrichedInventoryInspection {
            relationship_revision: inventory.relationship_revision(),
            stacks: inventory.stacks().to_vec(),
            unique_items: inventory
                .unique_items()
                .iter()
                .map(|item| MechanicsInventoryItemInspection {
                    entity: item.entity.raw(),
                    definition: item.definition.to_string(),
                })
                .collect(),
            capacity: inventory
                .capacity()
                .iter()
                .map(|capacity| MechanicsCapacityInspection {
                    metric: capacity.metric.to_string(),
                    used: capacity.used,
                    maximum: capacity.maximum,
                })
                .collect(),
            read_cost: inventory.read_cost().into(),
        });
    Ok(MechanicsEntityInspection {
        entity: structural.entity,
        catalog_version: structural.catalog_version.clone(),
        catalog_fingerprint: structural.catalog_fingerprint.clone(),
        components: structural.components.clone(),
        stats,
        tracks,
        intrinsic_sources: structural.intrinsic_sources.clone(),
        effects: structural.effects.clone(),
        effect_source_activations: evidence
            .effect_source_activations
            .iter()
            .map(|activation| MechanicsSourceActivationInspection {
                source: source_identity_value(&activation.identity),
                definition: activation.definition.to_string(),
            })
            .collect(),
        inventory,
        item: structural.item.clone(),
        equipment: structural.equipment.clone(),
    })
}

/// Owner-supplied evaluated facts. The inspector only copies these receipts/readouts; callers
/// decide when evaluation is valid and which request sources/timing they represent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEvaluationReadoutInspection {
    #[serde(rename = "evaluatedStats")]
    pub stats: Vec<MechanicsEvaluatedStatInspection>,
}

impl MechanicsEvaluationReadoutInspection {
    pub fn to_text(&self) -> String {
        self.stats
            .iter()
            .map(|stat| {
                format!(
                    "evaluated stat {} value={} bounds={}..{} decisions={}\n",
                    stat.id,
                    stat.value,
                    stat.minimum,
                    stat.maximum,
                    stat.decisions.len()
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsEvaluatedStatInspection {
    pub catalog_version: String,
    pub catalog_fingerprint: String,
    pub entity: u64,
    pub id: String,
    pub base: i64,
    pub after_additions: i64,
    pub combined_scale_numerator: u128,
    pub combined_scale_denominator: u128,
    pub after_scaling: i64,
    pub unconstrained: i64,
    pub value: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub decisions: Vec<MechanicsStatDecisionInspection>,
    pub source_cost: MechanicsSourceCostInspection,
    pub observed_revisions: Vec<MechanicsObservedComponentRevisionInspection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MechanicsObservedComponentRevisionInspection {
    pub entity: u64,
    pub component: String,
    pub revision: u64,
}

fn inspect_stat_evaluation(evaluation: &StatEvaluation) -> MechanicsEvaluatedStatInspection {
    MechanicsEvaluatedStatInspection {
        catalog_version: evaluation.catalog_version.to_string(),
        catalog_fingerprint: evaluation.catalog_fingerprint.clone(),
        entity: evaluation.entity.raw(),
        id: evaluation.stat.to_string(),
        base: evaluation.base.get(),
        after_additions: evaluation.after_additions.get(),
        combined_scale_numerator: evaluation.combined_scale_numerator,
        combined_scale_denominator: evaluation.combined_scale_denominator,
        after_scaling: evaluation.after_scaling.get(),
        unconstrained: evaluation.unconstrained.get(),
        value: evaluation.value.get(),
        minimum: evaluation.minimum.get(),
        maximum: evaluation.maximum.get(),
        decisions: evaluation
            .decisions
            .iter()
            .map(inspect_stat_decision)
            .collect(),
        source_cost: evaluation.source_cost.into(),
        observed_revisions: evaluation
            .observed_revisions
            .iter()
            .map(inspect_observed_revision)
            .collect(),
    }
}

fn inspect_observed_revision(
    revision: &ObservedComponentRevision,
) -> MechanicsObservedComponentRevisionInspection {
    MechanicsObservedComponentRevisionInspection {
        entity: revision.entity.raw(),
        component: revision.component.label().to_string(),
        revision: revision.revision,
    }
}

pub fn inspect_stat_evaluations(
    evaluations: &[StatEvaluation],
) -> MechanicsEvaluationReadoutInspection {
    MechanicsEvaluationReadoutInspection {
        stats: evaluations.iter().map(inspect_stat_evaluation).collect(),
    }
}

impl MechanicsStructuralEntityInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "gameplay-mechanics entity={} catalogVersion={} fingerprint={}\n",
            self.entity, self.catalog_version, self.catalog_fingerprint
        );
        for component in &self.components {
            let revision = component
                .revision
                .map_or_else(|| "-".to_string(), |value| value.to_string());
            let _ = writeln!(
                output,
                "component {} type={} codec={}@{} present={} revision={} entries={}",
                component.kind,
                component.type_id,
                component.codec_id,
                component.codec_version,
                component.present,
                revision,
                component.entry_count
            );
        }
        for stat in &self.stats {
            let _ = writeln!(output, "stored stat {} base={}", stat.id, stat.base);
        }
        for track in &self.tracks {
            let _ = writeln!(
                output,
                "stored track {} current={} declaredMinimum={} declaredMaximum={:?}",
                track.id, track.current, track.declared_minimum, track.declared_maximum
            );
        }
        let _ = writeln!(
            output,
            "stored sources intrinsic={}",
            self.intrinsic_sources.len()
        );
        let _ = writeln!(output, "effects {}", self.effects.len());
        if let Some(inventory) = &self.inventory {
            let _ = writeln!(
                output,
                "stored inventory stacks={} capacityLimits={}",
                inventory.stacks.len(),
                inventory.capacity_limits.len(),
            );
        }
        if let Some(item) = &self.item {
            let _ = writeln!(output, "item definition={}", item.definition);
        }
        let _ = writeln!(output, "equipment assignments={}", self.equipment.len());
        output
    }
}

pub fn inspect_mechanics_entity_structural(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
) -> Result<MechanicsStructuralEntityInspection, MechanicsError> {
    let view = MechanicsEntityView::read(state, entity)?;
    let components = component_inspections(&view);
    let mut stats = Vec::new();
    if let Some(stat_view) = view.stats() {
        for stored in stat_view.values() {
            stats.push(MechanicsStoredStatInspection {
                id: stored.stat().to_string(),
                base: stored.base().get(),
            });
        }
    }

    let mut tracks = Vec::new();
    if let Some(track_view) = view.tracks() {
        for stored in track_view.values() {
            let definition =
                catalog
                    .track(stored.track())
                    .ok_or_else(|| MechanicsError::UnknownTrack {
                        track: stored.track().clone(),
                    })?;
            let declared_maximum = match &definition.maximum {
                TrackMaximum::Fixed { value } => {
                    MechanicsTrackMaximumInspection::Fixed { value: value.get() }
                }
                TrackMaximum::Stat { stat } => MechanicsTrackMaximumInspection::Stat {
                    stat: stat.to_string(),
                },
            };
            tracks.push(MechanicsStoredTrackInspection {
                id: stored.track().to_string(),
                current: stored.current().get(),
                declared_minimum: definition.minimum.get(),
                declared_maximum,
            });
        }
    }

    let intrinsic_sources = view
        .intrinsic_sources()
        .map_or(&[][..], |sources| sources.bindings())
        .iter()
        .map(|binding| MechanicsSourceBindingInspection {
            instance: binding.instance().to_string(),
            definition: binding.definition().to_string(),
        })
        .collect();

    let effects = view
        .active_effects()
        .map_or(&[][..], |active| active.effects())
        .iter()
        .map(|effect| MechanicsEffectInspection {
            instance: effect.instance().to_string(),
            definition: effect.definition().to_string(),
            provenance: source_identity_value(effect.provenance()),
            stacks: effect.stacks(),
        })
        .collect();
    let inventory = view
        .inventory()
        .map(|inventory| MechanicsInventoryInspection {
            stacks: inventory.stacks().to_vec(),
            capacity_limits: inventory.capacity_limits().to_vec(),
        });
    let item = view.item().map(|item| MechanicsItemInspection {
        definition: item.definition().to_string(),
    });
    let equipment = view
        .equipment()
        .map_or(&[][..], |equipment| equipment.assignments())
        .iter()
        .map(|assignment| MechanicsEquipmentAssignmentInspection {
            slot: assignment.slot.to_string(),
            item: assignment.item.raw(),
        })
        .collect();

    Ok(MechanicsStructuralEntityInspection {
        entity: entity.raw(),
        catalog_version: catalog.version().to_string(),
        catalog_fingerprint: catalog.fingerprint().to_string(),
        components,
        stats,
        tracks,
        intrinsic_sources,
        effects,
        inventory,
        item,
        equipment,
    })
}

/// Reopen a snapshot into the structural JSON-v2 report.
///
/// The `storedStats` and `storedTracks` fields intentionally distinguish component facts from
/// evaluated owner readouts. No mechanics service is called while reopening the snapshot.
pub fn inspect_mechanics_snapshot_structural_json_v2(
    snapshot: &str,
    catalog_definition: &str,
    entity: u64,
) -> Result<MechanicsStructuralEntityInspection, DiagnosticSet> {
    let (state, catalog) = decode_mechanics_snapshot_for_inspection(snapshot, catalog_definition)?;
    inspect_mechanics_entity_structural(&state, &catalog, EntityId::new(entity)).map_err(|error| {
        mechanics_failure(
            "entity.inspection",
            DiagnosticLocation::path("$").with_entity(entity),
            error.to_string(),
            "inspect the reported component or catalog reference",
        )
    })
}

/// Reopen a snapshot into the legacy enriched JSON shape using owner-supplied evidence.
///
/// This is the explicit compatibility route for consumers of the historical `stats` and
/// `tracks` fields. The caller gathers evaluations, activated sources, and inventory before
/// calling this function; the inspector only copies that evidence.
pub fn inspect_mechanics_snapshot_json_v1_from_evidence(
    snapshot: &str,
    catalog_definition: &str,
    entity: u64,
    evidence: MechanicsInspectionEvidence<'_>,
) -> Result<MechanicsEntityInspection, DiagnosticSet> {
    let (state, catalog) = decode_mechanics_snapshot_for_inspection(snapshot, catalog_definition)?;
    let structural = inspect_mechanics_entity_structural(&state, &catalog, EntityId::new(entity))
        .map_err(|error| {
        mechanics_failure(
            "entity.inspection",
            DiagnosticLocation::path("$").with_entity(entity),
            error.to_string(),
            "inspect the reported component or catalog reference",
        )
    })?;
    inspect_mechanics_entity_from_evidence(&structural, evidence).map_err(|error| {
        mechanics_failure(
            "entity.inspection",
            DiagnosticLocation::path("$").with_entity(entity),
            error.to_string(),
            "inspect the supplied owner evidence",
        )
    })
}

/// Reopen a snapshot into the legacy enriched shape from explicit owner evidence.
///
/// Alias retained for callers that prefer the unversioned evidence-oriented spelling.
pub fn inspect_mechanics_snapshot_json_with_evidence(
    snapshot: &str,
    catalog_definition: &str,
    entity: u64,
    evidence: MechanicsInspectionEvidence<'_>,
) -> Result<MechanicsEntityInspection, DiagnosticSet> {
    inspect_mechanics_snapshot_json_v1_from_evidence(snapshot, catalog_definition, entity, evidence)
}

fn decode_mechanics_snapshot_for_inspection(
    snapshot: &str,
    catalog_definition: &str,
) -> Result<(EntityState, MechanicsCatalog), DiagnosticSet> {
    let definition: MechanicsCatalogDefinition =
        serde_json::from_str(catalog_definition).map_err(|error| {
            mechanics_failure(
                "catalog.decode",
                DiagnosticLocation::path("$"),
                error.to_string(),
                "correct the strict mechanics catalog definition",
            )
        })?;
    let catalog = MechanicsCatalog::admit(definition).map_err(|error| {
        mechanics_failure(
            "catalog.admission",
            DiagnosticLocation::path("$"),
            error.to_string(),
            "correct the catalog references, bounds, or quotas",
        )
    })?;
    let state = decode_snapshot_with_catalog(snapshot, &catalog).map_err(|error| {
        mechanics_failure(
            "snapshot.reconstruction",
            DiagnosticLocation::path("$"),
            error.to_string(),
            "restore or migrate the snapshot with its matching catalog",
        )
    })?;
    Ok((state, catalog))
}

fn component_inspections(view: &MechanicsEntityView<'_>) -> Vec<MechanicsComponentInspection> {
    MechanicsComponentKind::ALL
        .into_iter()
        .map(|kind| {
            let (revision, entry_count) = match kind {
                MechanicsComponentKind::Stats => view.stats().map_or((None, 0), |value| {
                    (Some(value.revision().revision()), value.values().len())
                }),
                MechanicsComponentKind::Tracks => view.tracks().map_or((None, 0), |value| {
                    (Some(value.revision().revision()), value.values().len())
                }),
                MechanicsComponentKind::IntrinsicSources => {
                    view.intrinsic_sources().map_or((None, 0), |value| {
                        (Some(value.revision().revision()), value.bindings().len())
                    })
                }
                MechanicsComponentKind::ActiveEffects => {
                    view.active_effects().map_or((None, 0), |value| {
                        (Some(value.revision().revision()), value.effects().len())
                    })
                }
                MechanicsComponentKind::Inventory => view.inventory().map_or((None, 0), |value| {
                    (
                        Some(value.revision().revision()),
                        value.stacks().len() + value.capacity_limits().len(),
                    )
                }),
                MechanicsComponentKind::Item => view
                    .item()
                    .map_or((None, 0), |value| (Some(value.revision().revision()), 1)),
                MechanicsComponentKind::Equipment => view.equipment().map_or((None, 0), |value| {
                    (Some(value.revision().revision()), value.assignments().len())
                }),
            };
            MechanicsComponentInspection {
                kind: kind.label().to_string(),
                type_id: kind.type_id().to_string(),
                codec_id: kind.codec_id().to_string(),
                codec_version: kind.codec_version(),
                present: revision.is_some(),
                revision,
                entry_count,
            }
        })
        .collect()
}

fn inspect_stat_decision(
    decision: &gameplay_mechanics::StatDecision,
) -> MechanicsStatDecisionInspection {
    MechanicsStatDecisionInspection {
        source: source_identity_value(&decision.source),
        source_definition: decision.source_definition.to_string(),
        contribution_index: decision.contribution_index,
        outcome: decision_outcome_label(decision.outcome).to_string(),
        stacking_group: decision.stacking_group.as_ref().map(ToString::to_string),
        stacking: decision.stacking.map(stacking_label).map(str::to_string),
        contribution: decision
            .contribution
            .as_ref()
            .map(|value| serde_json::to_value(value).expect("stat contributions serialize")),
    }
}

fn source_identity_value(source: &SourceInstanceIdentity) -> Value {
    serde_json::to_value(source).expect("source identities serialize")
}

const fn decision_outcome_label(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Applied => "applied",
        DecisionOutcome::Suppressed => "suppressed",
        DecisionOutcome::Inapplicable => "inapplicable",
    }
}

const fn stacking_label(stacking: StackingPolicy) -> &'static str {
    match stacking {
        StackingPolicy::Sum => "sum",
        StackingPolicy::Highest => "highest",
        StackingPolicy::Lowest => "lowest",
        StackingPolicy::UniqueBySource => "uniqueBySource",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageReceiptPartInspection {
    pub index: u16,
    pub kind: String,
    pub original: i64,
    pub prevented: bool,
    pub after_flat: i64,
    pub combined_scale_numerator: u128,
    pub combined_scale_denominator: u128,
    pub rounding: String,
    pub after_scale: i64,
    pub absorbed: i64,
    pub applied: i64,
    pub unapplied: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageReceiptDecisionInspection {
    pub part_index: u16,
    pub source: Value,
    pub source_definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_index: Option<u16>,
    pub kind: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageReceiptTrackInspection {
    pub track: String,
    pub before: i64,
    pub after: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageReceiptFactInspection {
    pub kind: String,
    pub track: String,
    pub part_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageReceiptInspection {
    pub catalog_version: String,
    pub catalog_fingerprint: String,
    pub operation: String,
    pub source: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<u64>,
    pub target: u64,
    pub target_track: String,
    pub observed_tracks_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed_tracks_revision: Option<u64>,
    pub parts: Vec<DamageReceiptPartInspection>,
    pub decisions: Vec<DamageReceiptDecisionInspection>,
    pub track_changes: Vec<DamageReceiptTrackInspection>,
    pub facts: Vec<DamageReceiptFactInspection>,
    pub source_cost: MechanicsSourceCostInspection,
}

impl DamageReceiptInspection {
    pub fn to_text(&self) -> String {
        format!(
            "damage operation={} target={} track={} parts={} decisions={} changes={} facts={}\n",
            self.operation,
            self.target,
            self.target_track,
            self.parts.len(),
            self.decisions.len(),
            self.track_changes.len(),
            self.facts.len()
        )
    }
}

pub fn inspect_damage_receipt(receipt: &DamageReceipt) -> DamageReceiptInspection {
    DamageReceiptInspection {
        catalog_version: receipt.catalog_version.to_string(),
        catalog_fingerprint: receipt.catalog_fingerprint.clone(),
        operation: receipt.operation.to_string(),
        source: source_identity_value(&receipt.source),
        actor: receipt.actor.map(EntityId::raw),
        target: receipt.target.raw(),
        target_track: receipt.target_track.to_string(),
        observed_tracks_revision: receipt.observed_tracks_revision,
        committed_tracks_revision: receipt.committed_tracks_revision,
        parts: receipt
            .parts
            .iter()
            .map(|part| DamageReceiptPartInspection {
                index: part.index,
                kind: part.kind.to_string(),
                original: part.original.get(),
                prevented: part.prevented,
                after_flat: part.after_flat.get(),
                combined_scale_numerator: part.combined_scale_numerator,
                combined_scale_denominator: part.combined_scale_denominator,
                rounding: rounding_label(part.rounding).to_string(),
                after_scale: part.after_scale.get(),
                absorbed: part.absorbed.get(),
                applied: part.applied.get(),
                unapplied: part.unapplied.get(),
            })
            .collect(),
        decisions: receipt
            .decisions
            .iter()
            .map(|decision| DamageReceiptDecisionInspection {
                part_index: decision.part_index,
                source: source_identity_value(&decision.source),
                source_definition: decision.source_definition.to_string(),
                response_index: decision.response_index,
                kind: response_kind_label(&decision.kind),
                outcome: decision_outcome_label(decision.outcome).to_string(),
            })
            .collect(),
        track_changes: receipt
            .track_changes
            .iter()
            .map(|change| DamageReceiptTrackInspection {
                track: change.track.to_string(),
                before: change.before.get(),
                after: change.after.get(),
            })
            .collect(),
        facts: receipt
            .facts
            .iter()
            .map(|fact| match fact {
                DamageFact::ProtectionTrackDepleted { track, part_index } => {
                    DamageReceiptFactInspection {
                        kind: "protectionTrackDepleted".to_string(),
                        track: track.to_string(),
                        part_index: *part_index,
                    }
                }
                DamageFact::TargetTrackDepleted { track, part_index } => {
                    DamageReceiptFactInspection {
                        kind: "targetTrackDepleted".to_string(),
                        track: track.to_string(),
                        part_index: *part_index,
                    }
                }
            })
            .collect(),
        source_cost: receipt.source_cost.into(),
    }
}

fn response_kind_label(kind: &ResponseDecisionKind) -> String {
    match kind {
        ResponseDecisionKind::NoDamageResponse => "noDamageResponse".to_string(),
        ResponseDecisionKind::Prevent => "prevent".to_string(),
        ResponseDecisionKind::FlatReduction { amount } => {
            format!("flatReduction:{}", amount.get())
        }
        ResponseDecisionKind::Scale { ratio } => {
            format!("scale:{}/{}", ratio.numerator(), ratio.denominator())
        }
        ResponseDecisionKind::Absorb { track } => format!("absorb:{}", track.as_str()),
    }
}

const fn rounding_label(rounding: RoundingPolicy) -> &'static str {
    match rounding {
        RoundingPolicy::TowardZero => "towardZero",
    }
}

fn mechanics_failure(
    code: &'static str,
    location: DiagnosticLocation,
    message: String,
    remedy: &'static str,
) -> DiagnosticSet {
    DiagnosticSet::one(
        Diagnostic::new(
            DiagnosticDomain::GameplayMechanics,
            DiagnosticSeverity::Fatal,
            code,
            location,
            message,
        )
        .with_remedy(RemedyAction::FixReference, remedy),
    )
}

#[cfg(test)]
mod tests {
    use entity_state::{
        encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition,
    };
    use gameplay_mechanics::{
        gameplay_component_registry, ActiveEffectInstance, ActiveEffectsComponent,
        CapacityMetricDefinition, CapacityMetricId, CatalogVersion, DamageKindDefinition,
        DamageKindId, DamageKindSelector, DamagePart, DamageRequest, DamageResponseDefinition,
        DamageService, EffectDefinition, EffectDefinitionId, EffectInstanceId,
        EffectStackingPolicy, EquipmentAssignment, EquipmentComponent, EquipmentSlotDefinition,
        EquipmentSlotId, IntrinsicSourceBinding, IntrinsicSourcesComponent, InventoryCapacityLimit,
        InventoryComponent, InventoryService, ItemCapacityCost, ItemClassificationId,
        ItemComponent, ItemDefinition, ItemDefinitionId, ItemEquipmentPolicy, ItemKind,
        MechanicsCatalogDefinition, MechanicsScalar, OperationId, SourceDefinition,
        SourceDefinitionId, SourceInstanceId, StackingGroupId, StackingPolicy, StatContribution,
        StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
        TrackDefinition, TrackId, TrackMaximum, TrackValue, TracksComponent,
    };

    use super::*;

    const OWNER: EntityId = EntityId::new(1);
    const ITEM: EntityId = EntityId::new(2);

    fn scalar(value: i64) -> MechanicsScalar {
        MechanicsScalar::new(value).unwrap()
    }

    fn version() -> CatalogVersion {
        CatalogVersion::parse("inspector.v1").unwrap()
    }

    fn source_identity(value: &str) -> SourceInstanceIdentity {
        SourceInstanceIdentity::Intrinsic {
            entity: OWNER,
            instance: SourceInstanceId::parse(value).unwrap(),
        }
    }

    fn definition() -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version: version(),
            stats: vec![StatDefinition {
                id: StatId::parse("power").unwrap(),
                minimum: scalar(0),
                maximum: scalar(100),
            }],
            tracks: vec![TrackDefinition {
                id: TrackId::parse("health").unwrap(),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: StatId::parse("power").unwrap(),
                },
            }],
            sources: vec![SourceDefinition {
                id: SourceDefinitionId::parse("guard").unwrap(),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: StatId::parse("power").unwrap(),
                    contribution: StatContribution::Add { amount: scalar(5) },
                    stacking_group: StackingGroupId::parse("power_bonus").unwrap(),
                    stacking: StackingPolicy::Highest,
                }],
                damage_responses: vec![DamageResponseDefinition::FlatReduction {
                    selector: DamageKindSelector::Any,
                    amount: scalar(2),
                    stacking_group: StackingGroupId::parse("guard_reduction").unwrap(),
                    stacking: StackingPolicy::Highest,
                }],
            }],
            damage_kinds: vec![DamageKindDefinition {
                id: DamageKindId::parse("kinetic").unwrap(),
            }],
            effects: vec![EffectDefinition {
                id: EffectDefinitionId::parse("guarded").unwrap(),
                stacking_group: StackingGroupId::parse("guarded_lifecycle").unwrap(),
                stacking: EffectStackingPolicy::Refresh,
                maximum_stacks: 1,
                sources: vec![SourceDefinitionId::parse("guard").unwrap()],
            }],
            capacity_metrics: vec![CapacityMetricDefinition {
                id: CapacityMetricId::parse("mass").unwrap(),
            }],
            items: vec![ItemDefinition {
                id: ItemDefinitionId::parse("armor").unwrap(),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![ItemClassificationId::parse("armor").unwrap()],
                capacity_costs: vec![ItemCapacityCost {
                    metric: CapacityMetricId::parse("mass").unwrap(),
                    units: 10,
                }],
                equipment: Some(ItemEquipmentPolicy {
                    required_slots: 1,
                    exclusive_group: None,
                }),
                sources: vec![SourceDefinitionId::parse("guard").unwrap()],
            }],
            equipment_slots: vec![EquipmentSlotDefinition {
                id: EquipmentSlotId::parse("body").unwrap(),
                allowed_classifications: vec![ItemClassificationId::parse("armor").unwrap()],
            }],
        }
    }

    fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
        let revision = state.component_revision::<T>(entity).unwrap();
        EntityAuthoringService
            .attach_component(state, revision, entity, value)
            .unwrap();
    }

    fn fixture() -> (MechanicsCatalog, EntityState) {
        let catalog = MechanicsCatalog::admit(definition()).unwrap();
        let mut state = EntityState::from_definitions_with_registry(
            gameplay_component_registry().unwrap(),
            [
                EntityDefinition::new(OWNER, "owner"),
                EntityDefinition::new(ITEM, "armor").with_containment(OWNER),
            ],
        )
        .unwrap();
        attach(
            &mut state,
            OWNER,
            StatsComponent::new(
                version(),
                vec![StatValue::new(StatId::parse("power").unwrap(), scalar(80))],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            OWNER,
            TracksComponent::new(
                version(),
                vec![TrackValue::new(
                    TrackId::parse("health").unwrap(),
                    scalar(70),
                )],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            OWNER,
            IntrinsicSourcesComponent::new(
                version(),
                vec![IntrinsicSourceBinding::new(
                    SourceInstanceId::parse("intrinsic_guard").unwrap(),
                    SourceDefinitionId::parse("guard").unwrap(),
                )],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            OWNER,
            ActiveEffectsComponent::new(
                version(),
                vec![ActiveEffectInstance::new(
                    EffectInstanceId::parse("guard_one").unwrap(),
                    EffectDefinitionId::parse("guarded").unwrap(),
                    source_identity("effect_origin"),
                    1,
                )
                .unwrap()],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            OWNER,
            InventoryComponent::with_capacity_limits(
                version(),
                vec![],
                vec![InventoryCapacityLimit::new(
                    CapacityMetricId::parse("mass").unwrap(),
                    20,
                )],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            ITEM,
            ItemComponent::new(version(), ItemDefinitionId::parse("armor").unwrap()),
        );
        attach(
            &mut state,
            OWNER,
            EquipmentComponent::new(
                version(),
                vec![EquipmentAssignment {
                    slot: EquipmentSlotId::parse("body").unwrap(),
                    item: ITEM,
                }],
            )
            .unwrap(),
        );
        (catalog, state)
    }

    #[test]
    fn entity_and_receipt_projection_are_structural_and_read_only() {
        let (catalog, mut state) = fixture();
        let before = encode_snapshot(&state).unwrap();
        let report = inspect_mechanics_entity_structural(&state, &catalog, OWNER).unwrap();
        assert_eq!(report.components.len(), MechanicsComponentKind::ALL.len());
        assert_eq!(report.stats.len(), 1);
        assert_eq!(report.stats[0].base, 80);
        assert!(matches!(
            report.tracks[0].declared_maximum,
            MechanicsTrackMaximumInspection::Stat { .. }
        ));
        assert_eq!(report.effects.len(), 1);
        assert_eq!(report.inventory.as_ref().unwrap().capacity_limits.len(), 1);
        assert_eq!(report.equipment.len(), 1);
        let text = report.to_text();
        assert!(text.contains("stored stat power base=80"));
        assert!(text.contains("declaredMaximum=Stat"));
        assert!(!text.contains("effectActivations="));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("storedStats"));
        assert!(json.contains("storedTracks"));
        assert_eq!(encode_snapshot(&state).unwrap(), before);

        let from_json = inspect_mechanics_snapshot_structural_json_v2(
            &before,
            &serde_json::to_string(&definition()).unwrap(),
            OWNER.raw(),
        )
        .unwrap();
        assert_eq!(from_json.stats, report.stats);
        assert_eq!(from_json.tracks, report.tracks);
        assert_eq!(from_json.intrinsic_sources, report.intrinsic_sources);
        assert_eq!(from_json.effects, report.effects);
        assert_eq!(from_json.inventory, report.inventory);
        assert_eq!(from_json.equipment, report.equipment);
        for (restored, live) in from_json.components.iter().zip(&report.components) {
            assert_eq!(restored.kind, live.kind);
            assert_eq!(restored.present, live.present);
            assert_eq!(restored.entry_count, live.entry_count);
        }

        let operation = OperationId::parse("inspector_damage").unwrap();
        let receipt = DamageService::apply(
            &mut state,
            &catalog,
            DamageRequest {
                source: SourceInstanceIdentity::Request {
                    operation: operation.clone(),
                    instance: SourceInstanceId::parse("origin").unwrap(),
                },
                operation,
                actor: None,
                target: OWNER,
                target_track: TrackId::parse("health").unwrap(),
                parts: vec![DamagePart {
                    amount: scalar(10),
                    kind: DamageKindId::parse("kinetic").unwrap(),
                }],
                request_sources: vec![],
                expected_tracks_revision: None,
            },
        )
        .unwrap();
        let receipt = inspect_damage_receipt(&receipt);
        assert_eq!(receipt.parts[0].applied, 8);
        assert_eq!(receipt.decisions.len(), 3);
        assert!(receipt
            .decisions
            .iter()
            .all(|decision| !decision.source.is_null()));
    }

    #[test]
    fn supplied_evidence_preserves_complete_stat_evaluation_and_compatibility_shape() {
        let (catalog, state) = fixture();
        let structural = inspect_mechanics_entity_structural(&state, &catalog, OWNER).unwrap();
        let operation = OperationId::parse("owner_readout").unwrap();
        let evaluation = StatService::evaluate(
            &state,
            &catalog,
            OWNER,
            &StatId::parse("power").unwrap(),
            &operation,
            &[],
        )
        .unwrap();
        let view = MechanicsEntityView::read(&state, OWNER).unwrap();
        let activations = view
            .active_effects()
            .unwrap()
            .activated_sources(&catalog)
            .unwrap();
        let inventory = InventoryService::view(&state, &catalog, OWNER).unwrap();

        let copied = &inspect_stat_evaluations(std::slice::from_ref(&evaluation)).stats[0];
        assert_eq!(
            copied.catalog_version,
            evaluation.catalog_version.to_string()
        );
        assert_eq!(copied.catalog_fingerprint, evaluation.catalog_fingerprint);
        assert_eq!(copied.entity, evaluation.entity.raw());
        assert_eq!(copied.base, evaluation.base.get());
        assert_eq!(copied.after_additions, evaluation.after_additions.get());
        assert_eq!(copied.after_scaling, evaluation.after_scaling.get());
        assert_eq!(copied.unconstrained, evaluation.unconstrained.get());
        assert_eq!(
            copied.combined_scale_numerator,
            evaluation.combined_scale_numerator
        );
        assert_eq!(
            copied.observed_revisions.len(),
            evaluation.observed_revisions.len()
        );

        let compatibility = inspect_mechanics_entity_from_evidence(
            &structural,
            MechanicsInspectionEvidence {
                stat_evaluations: std::slice::from_ref(&evaluation),
                effect_source_activations: &activations,
                inventory: Some(&inventory),
            },
        )
        .unwrap();
        assert_eq!(compatibility.stats[0].value, evaluation.value.get());
        assert_eq!(compatibility.tracks[0].maximum, evaluation.value.get());
        assert_eq!(
            compatibility.effect_source_activations.len(),
            activations.len()
        );
        assert_eq!(compatibility.inventory.unwrap().unique_items.len(), 1);

        let snapshot_compatibility = inspect_mechanics_snapshot_json_v1_from_evidence(
            &encode_snapshot(&state).unwrap(),
            &serde_json::to_string(&definition()).unwrap(),
            OWNER.raw(),
            MechanicsInspectionEvidence {
                stat_evaluations: std::slice::from_ref(&evaluation),
                effect_source_activations: &activations,
                inventory: Some(&inventory),
            },
        )
        .unwrap();
        assert_eq!(snapshot_compatibility.stats, compatibility.stats);
        assert_eq!(snapshot_compatibility.tracks, compatibility.tracks);
    }
}
