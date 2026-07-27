use std::collections::BTreeMap;
use std::fmt::Write;

use core_ids::EntityId;
use entity_state::{
    decode_snapshot, EntitySnapshot, EntitySourceSnapshot, EntityState, EntityStateSnapshot,
    EntityStateSnapshotError, SnapshotLifecycle,
};
use serde::Serialize;

use crate::{
    catalog::NamedCount, Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet,
    DiagnosticSeverity, RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityStateInspection {
    pub schema_version: u32,
    pub revision: u64,
    pub entity_count: usize,
    pub lifecycle: Vec<NamedCount>,
    pub sources: Vec<NamedCount>,
    pub components: Vec<NamedCount>,
    pub relationships: Vec<NamedCount>,
    pub entity_ids: Vec<u64>,
    pub diagnostics: DiagnosticSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityInspection {
    pub id: u64,
    pub name: String,
    pub lifecycle: String,
    pub source: String,
    pub labels: Vec<u64>,
    pub components: Vec<String>,
    pub relationships: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCategory {
    All,
    Active,
    Disabled,
    Tombstoned,
    Spatial,
    NonSpatial,
    Rendered,
    Colliding,
    Contained,
    AssetBound,
}

impl EntityCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Tombstoned => "tombstoned",
            Self::Spatial => "spatial",
            Self::NonSpatial => "non-spatial",
            Self::Rendered => "rendered",
            Self::Colliding => "colliding",
            Self::Contained => "contained",
            Self::AssetBound => "asset-bound",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        Some(match value {
            "all" => Self::All,
            "active" => Self::Active,
            "disabled" => Self::Disabled,
            "tombstoned" => Self::Tombstoned,
            "spatial" => Self::Spatial,
            "non-spatial" => Self::NonSpatial,
            "rendered" => Self::Rendered,
            "colliding" => Self::Colliding,
            "contained" => Self::Contained,
            "asset-bound" => Self::AssetBound,
            _ => return None,
        })
    }
}

pub fn inspect_entity_state(state: &EntityState) -> EntityStateInspection {
    let mut inspection = inspect_snapshot(&state.snapshot());
    inspection.components = state
        .component_inspection()
        .kinds
        .into_iter()
        .map(|kind| NamedCount {
            name: kind.type_id.as_str().to_string(),
            count: kind.count,
        })
        .collect();
    inspection
}

pub fn inspect_entity_state_json(input: &str) -> Result<EntityStateInspection, DiagnosticSet> {
    let state = decode_snapshot(input).map_err(entity_decode_failure)?;
    Ok(inspect_entity_state(&state))
}

pub fn inspect_entity(state: &EntityState, id: u64) -> Option<EntityInspection> {
    let snapshot = state.snapshot();
    snapshot
        .entities
        .iter()
        .find(|entity| entity.id == id)
        .map(|entity| {
            inspect_entity_record(
                entity,
                state
                    .component_types_for_entity(EntityId::new(id))
                    .into_iter()
                    .map(|type_id| type_id.as_str().to_string())
                    .collect(),
            )
        })
}

pub fn entity_ids_in_category(state: &EntityState, category: EntityCategory) -> Vec<u64> {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| matches_category(entity, category))
        .map(|entity| entity.id)
        .collect()
}

impl EntityStateInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "entity-state schema={} revision={} entities={}\n",
            self.schema_version, self.revision, self.entity_count
        );
        push_counts(&mut output, "lifecycle", &self.lifecycle);
        push_counts(&mut output, "sources", &self.sources);
        push_counts(&mut output, "components", &self.components);
        push_counts(&mut output, "relationships", &self.relationships);
        let ids = self
            .entity_ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(output, "entity-ids [{ids}]");
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

impl EntityInspection {
    pub fn to_text(&self) -> String {
        format!(
            "entity id={} name={:?}\nlifecycle {}\nsource {}\nlabels [{}]\ncomponents [{}]\nrelationships [{}]\n",
            self.id,
            self.name,
            self.lifecycle,
            self.source,
            self.labels
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(","),
            self.components.join(","),
            self.relationships.join(",")
        )
    }
}

fn inspect_snapshot(snapshot: &EntityStateSnapshot) -> EntityStateInspection {
    let mut lifecycle = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut relationships = BTreeMap::new();
    for entity in &snapshot.entities {
        increment(&mut lifecycle, lifecycle_label(entity.lifecycle));
        increment(&mut sources, source_label(&entity.source));
        for relationship in relationship_kinds(entity) {
            increment(&mut relationships, relationship);
        }
    }
    EntityStateInspection {
        schema_version: snapshot.schema_version,
        revision: snapshot.revision,
        entity_count: snapshot.entities.len(),
        lifecycle: NamedCount::from_map(lifecycle),
        sources: NamedCount::from_map(sources),
        components: Vec::new(),
        relationships: NamedCount::from_map(relationships),
        entity_ids: snapshot.entities.iter().map(|entity| entity.id).collect(),
        diagnostics: DiagnosticSet::new(),
    }
}

fn inspect_entity_record(entity: &EntitySnapshot, components: Vec<String>) -> EntityInspection {
    let mut relationships = Vec::new();
    if let Some(target) = entity.transform_parent {
        relationships.push(format!("transformParent={target}"));
    }
    if let Some(target) = entity.contained_in {
        relationships.push(format!("containedIn={target}"));
    }
    if let Some(target) = entity.derived_from {
        relationships.push(format!("derivedFrom={target}"));
    }
    EntityInspection {
        id: entity.id,
        name: entity.name.clone(),
        lifecycle: lifecycle_label(entity.lifecycle).to_string(),
        source: source_label(&entity.source).to_string(),
        labels: entity.labels.clone(),
        components,
        relationships,
    }
}

fn relationship_kinds(entity: &EntitySnapshot) -> Vec<&'static str> {
    let mut names = Vec::new();
    if entity.transform_parent.is_some() {
        names.push("transformParent");
    }
    if entity.contained_in.is_some() {
        names.push("containedIn");
    }
    if entity.derived_from.is_some() {
        names.push("derivedFrom");
    }
    names
}

fn matches_category(entity: &EntitySnapshot, category: EntityCategory) -> bool {
    match category {
        EntityCategory::All => true,
        EntityCategory::Active => entity.lifecycle == SnapshotLifecycle::Active,
        EntityCategory::Disabled => entity.lifecycle == SnapshotLifecycle::Disabled,
        EntityCategory::Tombstoned => entity.lifecycle == SnapshotLifecycle::Tombstoned,
        EntityCategory::Spatial => entity.transform.is_some(),
        EntityCategory::NonSpatial => entity.transform.is_none(),
        EntityCategory::Rendered => entity.renderable.is_some(),
        EntityCategory::Colliding => entity.collision.is_some(),
        EntityCategory::Contained => entity.contained_in.is_some(),
        EntityCategory::AssetBound => entity.asset_binding.is_some(),
    }
}

fn lifecycle_label(lifecycle: SnapshotLifecycle) -> &'static str {
    match lifecycle {
        SnapshotLifecycle::Active => "active",
        SnapshotLifecycle::Disabled => "disabled",
        SnapshotLifecycle::Tombstoned => "tombstoned",
    }
}

fn source_label(source: &EntitySourceSnapshot) -> &'static str {
    match source {
        EntitySourceSnapshot::AuthoredScene { .. } => "authoredScene",
        EntitySourceSnapshot::RuntimeCreated { .. } => "runtimeCreated",
        EntitySourceSnapshot::Imported { .. } => "imported",
        EntitySourceSnapshot::PrefabInstance { .. } => "prefabInstance",
        EntitySourceSnapshot::DiagnosticTooling => "diagnosticTooling",
        EntitySourceSnapshot::PolicyProposed { .. } => "policyProposed",
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, name: &str) {
    *counts.entry(name.to_string()).or_insert(0) += 1;
}

fn push_counts(output: &mut String, label: &str, counts: &[NamedCount]) {
    let values = counts
        .iter()
        .map(|item| format!("{}={}", item.name, item.count))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(output, "{label} {values}");
}

fn entity_decode_failure(error: EntityStateSnapshotError) -> DiagnosticSet {
    let code = match &error {
        EntityStateSnapshotError::Encode(_) => "entityState.encode",
        EntityStateSnapshotError::Decode(_) => "entityState.decode",
        EntityStateSnapshotError::MissingSchema => "entityState.missingSchema",
        EntityStateSnapshotError::UnsupportedSchema { .. } => "entityState.unsupportedSchema",
        EntityStateSnapshotError::DuplicateEntity { .. } => "entityState.duplicateEntity",
        EntityStateSnapshotError::InvalidLifecycleState { .. } => {
            "entityState.invalidLifecycleState"
        }
        EntityStateSnapshotError::InvalidAssetReference { .. } => {
            "entityState.invalidAssetReference"
        }
        EntityStateSnapshotError::InvalidDefinition(_) => "entityState.invalidDefinition",
        EntityStateSnapshotError::RegisteredComponent(_) => "entityState.registeredComponent",
    };
    DiagnosticSet::one(
        Diagnostic::new(
            DiagnosticDomain::EntityState,
            DiagnosticSeverity::Fatal,
            code,
            DiagnosticLocation::path("$"),
            error.to_string(),
        )
        .with_remedy(
            RemedyAction::RestoreArtifact,
            "restore or correct the entity snapshot",
        ),
    )
}

#[cfg(test)]
mod tests {
    use core_ids::EntityId;
    use core_math::Vec3;
    use entity_state::{
        ComponentRegistration, ComponentTypeId, EntityAuthoringService, EntityComponent,
        EntityDefinition, EntitySource, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
    };

    use super::*;

    fn state() -> EntityState {
        EntityState::from_definitions([
            EntityDefinition::new(EntityId::new(1), "room")
                .with_transform(Vec3::ZERO)
                .with_collision(true, true),
            EntityDefinition::new(EntityId::new(2), "prop")
                .with_source(EntitySource::RuntimeCreated { by: None })
                .with_transform(Vec3::ONE)
                .with_transform_parent(EntityId::new(1)),
        ])
        .unwrap()
    }

    #[test]
    fn successor_state_summary_and_focused_queries_are_deterministic() {
        let state = state();
        let report = inspect_entity_state(&state);
        assert_eq!(report.schema_version, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(report.entity_ids, vec![1, 2]);
        assert_eq!(
            entity_ids_in_category(&state, EntityCategory::Spatial),
            vec![1, 2]
        );
        assert_eq!(
            entity_ids_in_category(&state, EntityCategory::Colliding),
            vec![1]
        );
        let entity = inspect_entity(&state, 2).unwrap();
        assert_eq!(
            entity.components,
            vec![entity_state::TRANSFORM_COMPONENT_TYPE_ID]
        );
        assert_eq!(entity.relationships, vec!["transformParent=1"]);
        assert!(report.to_text().contains("entities=2"));
    }

    #[test]
    fn malformed_snapshot_is_a_fatal_local_diagnostic() {
        let failure = inspect_entity_state_json("{ nope").unwrap_err();
        assert!(failure.blocks_load());
        assert_eq!(failure.diagnostics[0].domain, DiagnosticDomain::EntityState);
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct InspectionFixtureComponent;

    impl EntityComponent for InspectionFixtureComponent {}

    #[test]
    fn registered_component_kinds_counts_and_entity_presence_are_visible() {
        let mut state = state();
        state
            .register_component(
                ComponentRegistration::<InspectionFixtureComponent>::runtime_only(
                    ComponentTypeId::parse("fixture.inspection").unwrap(),
                ),
            )
            .unwrap();
        let revision = state
            .component_revision::<InspectionFixtureComponent>(EntityId::new(2))
            .unwrap();
        EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                EntityId::new(2),
                InspectionFixtureComponent,
            )
            .unwrap();

        let summary = inspect_entity_state(&state);
        assert!(summary
            .components
            .iter()
            .any(|item| item.name == "fixture.inspection" && item.count == 1));
        let entity = inspect_entity(&state, 2).unwrap();
        assert!(entity
            .components
            .iter()
            .any(|component| component == "fixture.inspection"));
    }
}
