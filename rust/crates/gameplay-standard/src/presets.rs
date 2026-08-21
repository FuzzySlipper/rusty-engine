//! Small ordinary-owner recommendations for common adoption starting points.
//!
//! A preset only creates catalog fragments, inert components, and explicit existing service
//! requests. It never creates entities, a component registry, a runtime, or an aggregate session.

use core_ids::EntityId;
use gameplay_mechanics::{
    CatalogVersion, DamageKindId, DamagePart, DamageRequest, MechanicsCatalogDefinition,
    MechanicsScalar, OperationId, SourceInstanceIdentity, StatDefinition, StatId, StatValue,
    StatsComponent, TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest, TrackValue,
    TracksComponent,
};

fn stat_id(value: &str) -> StatId {
    StatId::parse(value).expect("fixed standard preset stat identity is valid")
}
fn track_id(value: &str) -> TrackId {
    TrackId::parse(value).expect("fixed standard preset track identity is valid")
}
fn damage_kind_id(value: &str) -> DamageKindId {
    DamageKindId::parse(value).expect("fixed standard preset damage identity is valid")
}
fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).expect("fixed preset scalar is valid")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionActorPresetComponents {
    pub stats: StatsComponent,
    pub tracks: TracksComponent,
}

/// Minimal vitality/resource facts for an action actor. Vitality is structurally stat-bounded;
/// resource has a fixed maximum. Neither maximum is evaluated by this preset.
pub struct ActionActorPreset;
impl ActionActorPreset {
    pub const VITALITY_MAX_STAT: &'static str = "vitality_max";
    pub const VITALITY_TRACK: &'static str = "vitality";
    pub const RESOURCE_TRACK: &'static str = "actor_resource";

    pub fn catalog_fragment(version: CatalogVersion) -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version,
            stats: vec![StatDefinition {
                id: stat_id(Self::VITALITY_MAX_STAT),
                minimum: scalar(0),
                maximum: scalar(100_000),
            }],
            tracks: vec![
                TrackDefinition {
                    id: track_id(Self::VITALITY_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Stat {
                        stat: stat_id(Self::VITALITY_MAX_STAT),
                    },
                },
                TrackDefinition {
                    id: track_id(Self::RESOURCE_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed { value: scalar(100) },
                },
            ],
            sources: vec![],
            damage_kinds: vec![],
            effects: vec![],
            capacity_metrics: vec![],
            items: vec![],
            equipment_slots: vec![],
        }
    }
    pub fn components(version: CatalogVersion) -> ActionActorPresetComponents {
        ActionActorPresetComponents {
            stats: StatsComponent::new(
                version.clone(),
                vec![StatValue::new(
                    stat_id(Self::VITALITY_MAX_STAT),
                    scalar(100),
                )],
            )
            .expect("fixed preset components are valid"),
            tracks: TracksComponent::new(
                version,
                vec![
                    TrackValue::new(track_id(Self::VITALITY_TRACK), scalar(100)),
                    TrackValue::new(track_id(Self::RESOURCE_TRACK), scalar(100)),
                ],
            )
            .expect("fixed preset components are valid"),
        }
    }
    pub fn spend_resource_request(
        entity: EntityId,
        operation: OperationId,
        source: SourceInstanceIdentity,
        amount: MechanicsScalar,
    ) -> TrackMutationRequest {
        TrackMutationRequest {
            operation,
            source,
            entity,
            track: track_id(Self::RESOURCE_TRACK),
            amount,
            kind: gameplay_mechanics::TrackAdjustmentKind::Spend,
            expected_revision: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructibleResourcePresetComponents {
    pub tracks: TracksComponent,
}

/// A non-character object with a fixed destructible integrity track and a separately fixed
/// resource-bearing track. Product code may take either fragment without adopting the other.
pub struct DestructibleResourcePreset;
impl DestructibleResourcePreset {
    pub const INTEGRITY_TRACK: &'static str = "integrity";
    pub const RESOURCE_TRACK: &'static str = "object_resource";
    pub const DAMAGE_KIND: &'static str = "impact";
    pub fn catalog_fragment(version: CatalogVersion) -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version,
            stats: vec![],
            tracks: vec![
                TrackDefinition {
                    id: track_id(Self::INTEGRITY_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed { value: scalar(50) },
                },
                TrackDefinition {
                    id: track_id(Self::RESOURCE_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed { value: scalar(25) },
                },
            ],
            sources: vec![],
            damage_kinds: vec![gameplay_mechanics::DamageKindDefinition {
                id: damage_kind_id(Self::DAMAGE_KIND),
            }],
            effects: vec![],
            capacity_metrics: vec![],
            items: vec![],
            equipment_slots: vec![],
        }
    }
    pub fn components(version: CatalogVersion) -> DestructibleResourcePresetComponents {
        DestructibleResourcePresetComponents {
            tracks: TracksComponent::new(
                version,
                vec![
                    TrackValue::new(track_id(Self::INTEGRITY_TRACK), scalar(50)),
                    TrackValue::new(track_id(Self::RESOURCE_TRACK), scalar(25)),
                ],
            )
            .expect("fixed preset components are valid"),
        }
    }
    pub fn damage_request(
        target: EntityId,
        operation: OperationId,
        source: SourceInstanceIdentity,
        amount: MechanicsScalar,
    ) -> DamageRequest {
        DamageRequest {
            operation,
            source,
            actor: None,
            target,
            target_track: track_id(Self::INTEGRITY_TRACK),
            parts: vec![DamagePart {
                amount,
                kind: damage_kind_id(Self::DAMAGE_KIND),
            }],
            request_sources: vec![],
            expected_tracks_revision: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version() -> CatalogVersion {
        CatalogVersion::parse("preset-test").unwrap()
    }
    fn source(operation: &OperationId) -> SourceInstanceIdentity {
        SourceInstanceIdentity::Request {
            operation: operation.clone(),
            instance: gameplay_mechanics::SourceInstanceId::parse("preset").unwrap(),
        }
    }

    #[test]
    fn action_actor_fragments_are_independently_admissible_and_structural() {
        let catalog = gameplay_mechanics::MechanicsCatalog::admit(
            ActionActorPreset::catalog_fragment(version()),
        )
        .unwrap();
        let components = ActionActorPreset::components(version());
        assert_eq!(components.stats.values().len(), 1);
        assert_eq!(components.tracks.values().len(), 2);
        assert!(matches!(
            catalog
                .track(&TrackId::parse(ActionActorPreset::VITALITY_TRACK).unwrap())
                .unwrap()
                .maximum,
            TrackMaximum::Stat { .. }
        ));
        assert!(matches!(
            catalog
                .track(&TrackId::parse(ActionActorPreset::RESOURCE_TRACK).unwrap())
                .unwrap()
                .maximum,
            TrackMaximum::Fixed { .. }
        ));

        let operation = OperationId::parse("spend").unwrap();
        let request = ActionActorPreset::spend_resource_request(
            EntityId::new(7),
            operation.clone(),
            source(&operation),
            scalar(3),
        );
        assert_eq!(request.entity, EntityId::new(7));
        assert_eq!(request.track.as_str(), ActionActorPreset::RESOURCE_TRACK);
        assert!(request.expected_revision.is_none());
    }

    #[test]
    fn destructible_fragment_does_not_require_an_actor_or_runtime() {
        let catalog = gameplay_mechanics::MechanicsCatalog::admit(
            DestructibleResourcePreset::catalog_fragment(version()),
        )
        .unwrap();
        let components = DestructibleResourcePreset::components(version());
        assert_eq!(components.tracks.values().len(), 2);
        let operation = OperationId::parse("damage").unwrap();
        let request = DestructibleResourcePreset::damage_request(
            EntityId::new(8),
            operation.clone(),
            source(&operation),
            scalar(2),
        );
        assert!(request.actor.is_none());
        assert_eq!(
            request.target_track.as_str(),
            DestructibleResourcePreset::INTEGRITY_TRACK
        );
        assert_eq!(catalog.view().tracks().len(), 2);
    }

    #[test]
    fn preset_catalogs_merge_and_each_fragment_is_independently_adoptable() {
        let mut actor = ActionActorPreset::catalog_fragment(version());
        let object = DestructibleResourcePreset::catalog_fragment(version());
        actor.tracks.extend(object.tracks);
        actor.damage_kinds.extend(object.damage_kinds);
        gameplay_mechanics::MechanicsCatalog::admit(actor).unwrap();

        let actor_components = ActionActorPreset::components(version());
        assert_eq!(actor_components.stats.values().len(), 1);
        let object_components = DestructibleResourcePreset::components(version());
        assert_eq!(object_components.tracks.values().len(), 2);
        assert_ne!(
            ActionActorPreset::RESOURCE_TRACK,
            DestructibleResourcePreset::RESOURCE_TRACK
        );
    }
}
