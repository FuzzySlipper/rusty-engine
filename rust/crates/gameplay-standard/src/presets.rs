//! Small ordinary-owner recommendations for common adoption starting points.
//!
//! A preset only creates catalog fragments, inert components, and explicit existing service
//! requests. It never creates entities, a component registry, a runtime, or an aggregate session.

use core_ids::EntityId;
use gameplay_mechanics::{
    CatalogError, CatalogVersion, DamageKindId, DamagePart, DamageRequest,
    MechanicsCatalogDefinition, MechanicsScalar, OperationId, SourceInstanceIdentity,
    StatDefinition, StatId, StatValue, StatsComponent, TrackDefinition, TrackId, TrackMaximum,
    TrackMutationRequest, TrackValue, TracksComponent,
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

/// Why a preset capacity or initial value was not admitted.
///
/// Presets deliberately accept the existing exact `MechanicsScalar` value rather than adding a
/// parallel numeric representation. The caller remains responsible for constructing a bounded
/// scalar; this error describes the relationship between ordinary mechanic values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetValueError {
    NegativeMaximum {
        field: &'static str,
        value: i64,
    },
    NegativeInitial {
        field: &'static str,
        value: i64,
    },
    InitialExceedsMaximum {
        initial_field: &'static str,
        initial: i64,
        maximum_field: &'static str,
        maximum: i64,
    },
}

impl std::fmt::Display for PresetValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "preset value rejected: {self:?}")
    }
}

impl std::error::Error for PresetValueError {}

fn validate_initial(
    maximum_field: &'static str,
    maximum: MechanicsScalar,
    initial_field: &'static str,
    initial: MechanicsScalar,
) -> Result<(), PresetValueError> {
    validate_nonnegative_maximum(maximum_field, maximum)?;
    if initial.get() < 0 {
        return Err(PresetValueError::NegativeInitial {
            field: initial_field,
            value: initial.get(),
        });
    }
    if initial > maximum {
        return Err(PresetValueError::InitialExceedsMaximum {
            initial_field,
            initial: initial.get(),
            maximum_field,
            maximum: maximum.get(),
        });
    }
    Ok(())
}

fn validate_nonnegative_maximum(
    field: &'static str,
    value: MechanicsScalar,
) -> Result<(), PresetValueError> {
    if value.get() < 0 {
        return Err(PresetValueError::NegativeMaximum {
            field,
            value: value.get(),
        });
    }
    Ok(())
}

/// Admitted ordinary-mechanics values emitted by [`ActionActorPreset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionActorPresetConfig {
    vitality_definition_maximum: MechanicsScalar,
    vitality_maximum: MechanicsScalar,
    vitality_initial: MechanicsScalar,
    resource_maximum: MechanicsScalar,
    resource_initial: MechanicsScalar,
}

impl ActionActorPresetConfig {
    pub fn try_new(
        vitality_definition_maximum: MechanicsScalar,
        vitality_maximum: MechanicsScalar,
        vitality_initial: MechanicsScalar,
        resource_maximum: MechanicsScalar,
        resource_initial: MechanicsScalar,
    ) -> Result<Self, PresetValueError> {
        validate_nonnegative_maximum("vitalityDefinitionMaximum", vitality_definition_maximum)?;
        validate_nonnegative_maximum("vitalityMaximum", vitality_maximum)?;
        if vitality_maximum > vitality_definition_maximum {
            return Err(PresetValueError::InitialExceedsMaximum {
                initial_field: "vitalityMaximum",
                initial: vitality_maximum.get(),
                maximum_field: "vitalityDefinitionMaximum",
                maximum: vitality_definition_maximum.get(),
            });
        }
        validate_initial(
            "vitalityMaximum",
            vitality_maximum,
            "vitalityInitial",
            vitality_initial,
        )?;
        validate_initial(
            "resourceMaximum",
            resource_maximum,
            "resourceInitial",
            resource_initial,
        )?;
        Ok(Self {
            vitality_definition_maximum,
            vitality_maximum,
            vitality_initial,
            resource_maximum,
            resource_initial,
        })
    }

    pub fn vitality_definition_maximum(&self) -> MechanicsScalar {
        self.vitality_definition_maximum
    }

    pub fn vitality_maximum(&self) -> MechanicsScalar {
        self.vitality_maximum
    }

    pub fn vitality_initial(&self) -> MechanicsScalar {
        self.vitality_initial
    }

    pub fn resource_maximum(&self) -> MechanicsScalar {
        self.resource_maximum
    }

    pub fn resource_initial(&self) -> MechanicsScalar {
        self.resource_initial
    }
}

impl Default for ActionActorPresetConfig {
    fn default() -> Self {
        Self::try_new(
            scalar(100_000),
            scalar(100),
            scalar(100),
            scalar(100),
            scalar(100),
        )
        .expect("fixed action-actor preset configuration is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionActorPresetComponents {
    pub stats: StatsComponent,
    pub tracks: TracksComponent,
}

/// Minimal vitality/resource facts for an action actor. Vitality is structurally stat-bounded;
/// resource has a configured fixed maximum. Neither maximum is evaluated by this preset.
pub struct ActionActorPreset;
impl ActionActorPreset {
    pub const VITALITY_MAX_STAT: &'static str = "vitality_max";
    pub const VITALITY_TRACK: &'static str = "vitality";
    pub const RESOURCE_TRACK: &'static str = "actor_resource";

    /// The compatibility configuration emitted by the pre-configured helpers.
    pub fn default_config() -> ActionActorPresetConfig {
        ActionActorPresetConfig::default()
    }

    /// Emits the ordinary catalog definitions for one admitted configuration.
    pub fn catalog_fragment_with_config(
        version: CatalogVersion,
        config: &ActionActorPresetConfig,
    ) -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version,
            stats: vec![StatDefinition {
                id: stat_id(Self::VITALITY_MAX_STAT),
                minimum: scalar(0),
                maximum: config.vitality_definition_maximum(),
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
                    maximum: TrackMaximum::Fixed {
                        value: config.resource_maximum(),
                    },
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

    /// Emits inert ordinary component facts for one admitted configuration.
    pub fn components_with_config(
        version: CatalogVersion,
        config: &ActionActorPresetConfig,
    ) -> ActionActorPresetComponents {
        ActionActorPresetComponents {
            stats: StatsComponent::new(
                version.clone(),
                vec![StatValue::new(
                    stat_id(Self::VITALITY_MAX_STAT),
                    config.vitality_maximum(),
                )],
            )
            .expect("fixed action-actor component identities are valid"),
            tracks: TracksComponent::new(
                version,
                vec![
                    TrackValue::new(track_id(Self::VITALITY_TRACK), config.vitality_initial()),
                    TrackValue::new(track_id(Self::RESOURCE_TRACK), config.resource_initial()),
                ],
            )
            .expect("fixed action-actor component identities are valid"),
        }
    }

    /// Emits the compatibility catalog fragment (vitality bound 100_000; initial vitality and
    /// resource both 100).
    pub fn catalog_fragment(version: CatalogVersion) -> MechanicsCatalogDefinition {
        Self::catalog_fragment_with_config(version, &Self::default_config())
    }

    /// Emits the compatibility component facts (vitality and resource both 100).
    pub fn components(version: CatalogVersion) -> ActionActorPresetComponents {
        Self::components_with_config(version, &Self::default_config())
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

/// Admitted ordinary-mechanics values emitted by [`DestructibleResourcePreset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructibleResourcePresetConfig {
    integrity_maximum: MechanicsScalar,
    integrity_initial: MechanicsScalar,
    resource_maximum: MechanicsScalar,
    resource_initial: MechanicsScalar,
}

impl DestructibleResourcePresetConfig {
    pub fn try_new(
        integrity_maximum: MechanicsScalar,
        integrity_initial: MechanicsScalar,
        resource_maximum: MechanicsScalar,
        resource_initial: MechanicsScalar,
    ) -> Result<Self, PresetValueError> {
        validate_initial(
            "integrityMaximum",
            integrity_maximum,
            "integrityInitial",
            integrity_initial,
        )?;
        validate_initial(
            "resourceMaximum",
            resource_maximum,
            "resourceInitial",
            resource_initial,
        )?;
        Ok(Self {
            integrity_maximum,
            integrity_initial,
            resource_maximum,
            resource_initial,
        })
    }

    pub fn integrity_maximum(&self) -> MechanicsScalar {
        self.integrity_maximum
    }

    pub fn integrity_initial(&self) -> MechanicsScalar {
        self.integrity_initial
    }

    pub fn resource_maximum(&self) -> MechanicsScalar {
        self.resource_maximum
    }

    pub fn resource_initial(&self) -> MechanicsScalar {
        self.resource_initial
    }
}

impl Default for DestructibleResourcePresetConfig {
    fn default() -> Self {
        Self::try_new(scalar(50), scalar(50), scalar(25), scalar(25))
            .expect("fixed destructible-resource preset configuration is valid")
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

    /// The compatibility configuration emitted by the pre-configured helpers.
    pub fn default_config() -> DestructibleResourcePresetConfig {
        DestructibleResourcePresetConfig::default()
    }

    /// Emits ordinary catalog definitions for one admitted configuration.
    pub fn catalog_fragment_with_config(
        version: CatalogVersion,
        config: &DestructibleResourcePresetConfig,
    ) -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version,
            stats: vec![],
            tracks: vec![
                TrackDefinition {
                    id: track_id(Self::INTEGRITY_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed {
                        value: config.integrity_maximum(),
                    },
                },
                TrackDefinition {
                    id: track_id(Self::RESOURCE_TRACK),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed {
                        value: config.resource_maximum(),
                    },
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

    /// Emits inert ordinary component facts for one admitted configuration.
    pub fn components_with_config(
        version: CatalogVersion,
        config: &DestructibleResourcePresetConfig,
    ) -> DestructibleResourcePresetComponents {
        DestructibleResourcePresetComponents {
            tracks: TracksComponent::new(
                version,
                vec![
                    TrackValue::new(track_id(Self::INTEGRITY_TRACK), config.integrity_initial()),
                    TrackValue::new(track_id(Self::RESOURCE_TRACK), config.resource_initial()),
                ],
            )
            .expect("fixed destructible-resource component identities are valid"),
        }
    }

    /// Emits the compatibility catalog fragment (integrity 50, resource 25).
    pub fn catalog_fragment(version: CatalogVersion) -> MechanicsCatalogDefinition {
        Self::catalog_fragment_with_config(version, &Self::default_config())
    }

    /// Emits the compatibility component facts (integrity 50, resource 25).
    pub fn components(version: CatalogVersion) -> DestructibleResourcePresetComponents {
        Self::components_with_config(version, &Self::default_config())
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

/// A mechanics-definition namespace that a deliberate preset composition can conflict with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetDefinitionNamespace {
    Stat,
    Track,
    Source,
    DamageKind,
    Effect,
    CapacityMetric,
    Item,
    EquipmentSlot,
}

/// Why the explicit actor/resource preset composition could not be admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetCompositionError {
    CatalogVersionMismatch {
        expected: CatalogVersion,
        actual: CatalogVersion,
    },
    DuplicateDefinition {
        namespace: PresetDefinitionNamespace,
        identity: String,
    },
    Catalog(CatalogError),
}

impl std::fmt::Display for PresetCompositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "preset composition rejected: {self:?}")
    }
}

impl std::error::Error for PresetCompositionError {}

/// Combines exactly these two optional preset fragments with a product-owned catalog definition.
///
/// This is intentionally not a general catalog merge API. It preserves the supplied version and
/// authored identities verbatim, rejects a shared-version mismatch and every duplicate namespace
/// with typed errors, and then asks the ordinary mechanics catalog owner to validate the result.
pub fn compose_action_actor_and_destructible_resource_catalog(
    version: CatalogVersion,
    mut base: MechanicsCatalogDefinition,
    action_actor: Option<&ActionActorPresetConfig>,
    destructible_resource: Option<&DestructibleResourcePresetConfig>,
) -> Result<MechanicsCatalogDefinition, PresetCompositionError> {
    if base.version != version {
        return Err(PresetCompositionError::CatalogVersionMismatch {
            expected: version,
            actual: base.version,
        });
    }

    if let Some(config) = action_actor {
        extend_catalog(
            &mut base,
            ActionActorPreset::catalog_fragment_with_config(version.clone(), config),
        );
    }
    if let Some(config) = destructible_resource {
        extend_catalog(
            &mut base,
            DestructibleResourcePreset::catalog_fragment_with_config(version, config),
        );
    }

    canonicalize_preset_composition(&mut base);
    reject_preset_duplicates(&base)?;
    gameplay_mechanics::MechanicsCatalog::admit(base.clone())
        .map_err(PresetCompositionError::Catalog)?;
    Ok(base)
}

fn extend_catalog(base: &mut MechanicsCatalogDefinition, fragment: MechanicsCatalogDefinition) {
    base.stats.extend(fragment.stats);
    base.tracks.extend(fragment.tracks);
    base.sources.extend(fragment.sources);
    base.damage_kinds.extend(fragment.damage_kinds);
    base.effects.extend(fragment.effects);
    base.capacity_metrics.extend(fragment.capacity_metrics);
    base.items.extend(fragment.items);
    base.equipment_slots.extend(fragment.equipment_slots);
}

fn canonicalize_preset_composition(definition: &mut MechanicsCatalogDefinition) {
    definition
        .stats
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .tracks
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .sources
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .damage_kinds
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .effects
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .capacity_metrics
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .items
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .equipment_slots
        .sort_by(|left, right| left.id.cmp(&right.id));
}

fn reject_preset_duplicates(
    definition: &MechanicsCatalogDefinition,
) -> Result<(), PresetCompositionError> {
    reject_duplicate_identities(
        &definition.stats,
        PresetDefinitionNamespace::Stat,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.tracks,
        PresetDefinitionNamespace::Track,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.sources,
        PresetDefinitionNamespace::Source,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.damage_kinds,
        PresetDefinitionNamespace::DamageKind,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.effects,
        PresetDefinitionNamespace::Effect,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.capacity_metrics,
        PresetDefinitionNamespace::CapacityMetric,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.items,
        PresetDefinitionNamespace::Item,
        |value| value.id.as_str(),
    )?;
    reject_duplicate_identities(
        &definition.equipment_slots,
        PresetDefinitionNamespace::EquipmentSlot,
        |value| value.id.as_str(),
    )
}

fn reject_duplicate_identities<T>(
    values: &[T],
    namespace: PresetDefinitionNamespace,
    identity: impl Fn(&T) -> &str,
) -> Result<(), PresetCompositionError> {
    for pair in values.windows(2) {
        if identity(&pair[0]) == identity(&pair[1]) {
            return Err(PresetCompositionError::DuplicateDefinition {
                namespace,
                identity: identity(&pair[0]).to_owned(),
            });
        }
    }
    Ok(())
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

    #[test]
    fn configured_capacities_are_visible_in_ordinary_definitions_components_and_fingerprints() {
        let actor = ActionActorPresetConfig::try_new(
            scalar(240),
            scalar(240),
            scalar(180),
            scalar(75),
            scalar(60),
        )
        .unwrap();
        let object = DestructibleResourcePresetConfig::try_new(
            scalar(9_000),
            scalar(8_750),
            scalar(700),
            scalar(500),
        )
        .unwrap();

        let actor_catalog = gameplay_mechanics::MechanicsCatalog::admit(
            ActionActorPreset::catalog_fragment_with_config(version(), &actor),
        )
        .unwrap();
        let object_catalog = gameplay_mechanics::MechanicsCatalog::admit(
            DestructibleResourcePreset::catalog_fragment_with_config(version(), &object),
        )
        .unwrap();
        let default_object_catalog = gameplay_mechanics::MechanicsCatalog::admit(
            DestructibleResourcePreset::catalog_fragment(version()),
        )
        .unwrap();

        assert_eq!(
            actor_catalog
                .stat(&stat_id(ActionActorPreset::VITALITY_MAX_STAT))
                .unwrap()
                .maximum,
            scalar(240)
        );
        assert_eq!(
            ActionActorPreset::components_with_config(version(), &actor)
                .tracks
                .current(&track_id(ActionActorPreset::RESOURCE_TRACK)),
            Some(scalar(60))
        );
        assert!(matches!(
            object_catalog
                .track(&track_id(DestructibleResourcePreset::INTEGRITY_TRACK))
                .unwrap()
                .maximum,
            TrackMaximum::Fixed { value } if value == scalar(9_000)
        ));
        assert_eq!(
            DestructibleResourcePreset::components_with_config(version(), &object)
                .tracks
                .current(&track_id(DestructibleResourcePreset::RESOURCE_TRACK)),
            Some(scalar(500))
        );
        assert_ne!(
            object_catalog.fingerprint(),
            default_object_catalog.fingerprint()
        );
    }

    #[test]
    fn configuration_rejects_negative_and_out_of_bounds_values_without_panicking() {
        assert!(matches!(
            ActionActorPresetConfig::try_new(
                scalar(0),
                scalar(-1),
                scalar(0),
                scalar(0),
                scalar(0),
            ),
            Err(PresetValueError::NegativeMaximum {
                field: "vitalityMaximum",
                ..
            })
        ));
        assert!(matches!(
            ActionActorPresetConfig::try_new(scalar(1), scalar(2), scalar(0), scalar(0), scalar(0),),
            Err(PresetValueError::InitialExceedsMaximum {
                initial_field: "vitalityMaximum",
                maximum_field: "vitalityDefinitionMaximum",
                ..
            })
        ));
        assert!(matches!(
            DestructibleResourcePresetConfig::try_new(scalar(1), scalar(-1), scalar(0), scalar(0)),
            Err(PresetValueError::NegativeInitial {
                field: "integrityInitial",
                ..
            })
        ));
        assert!(matches!(
            DestructibleResourcePresetConfig::try_new(scalar(1), scalar(2), scalar(0), scalar(0)),
            Err(PresetValueError::InitialExceedsMaximum {
                initial_field: "integrityInitial",
                maximum_field: "integrityMaximum",
                ..
            })
        ));
        assert!(ActionActorPresetConfig::try_new(
            gameplay_mechanics::MechanicsScalar::new(gameplay_mechanics::MAX_ABS_MECHANICS_SCALAR)
                .unwrap(),
            scalar(0),
            scalar(0),
            scalar(0),
            scalar(0),
        )
        .is_ok());
        assert!(gameplay_mechanics::MechanicsScalar::new(
            gameplay_mechanics::MAX_ABS_MECHANICS_SCALAR + 1
        )
        .is_err());
    }

    #[test]
    fn compatibility_helpers_emit_the_default_configuration_unchanged() {
        let actor = ActionActorPreset::default_config();
        assert_eq!(actor.vitality_definition_maximum(), scalar(100_000));
        assert_eq!(actor.vitality_maximum(), scalar(100));
        assert_eq!(actor.vitality_initial(), scalar(100));
        assert_eq!(
            ActionActorPreset::catalog_fragment(version()),
            ActionActorPreset::catalog_fragment_with_config(version(), &actor)
        );
        assert_eq!(
            ActionActorPreset::components(version()),
            ActionActorPreset::components_with_config(version(), &actor)
        );

        let object = DestructibleResourcePreset::default_config();
        assert_eq!(
            DestructibleResourcePreset::catalog_fragment(version()),
            DestructibleResourcePreset::catalog_fragment_with_config(version(), &object)
        );
        assert_eq!(
            DestructibleResourcePreset::components(version()),
            DestructibleResourcePreset::components_with_config(version(), &object)
        );
    }

    #[test]
    fn explicit_composition_preserves_version_is_deterministic_and_admits_both_presets() {
        let actor = ActionActorPresetConfig::try_new(
            scalar(250),
            scalar(250),
            scalar(250),
            scalar(80),
            scalar(80),
        )
        .unwrap();
        let object = DestructibleResourcePresetConfig::try_new(
            scalar(1_250),
            scalar(1_000),
            scalar(320),
            scalar(200),
        )
        .unwrap();
        let base = MechanicsCatalogDefinition {
            version: version(),
            stats: vec![],
            tracks: vec![
                TrackDefinition {
                    id: track_id("zeta"),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed { value: scalar(3) },
                },
                TrackDefinition {
                    id: track_id("alpha"),
                    minimum: scalar(0),
                    maximum: TrackMaximum::Fixed { value: scalar(4) },
                },
            ],
            sources: vec![],
            damage_kinds: vec![],
            effects: vec![],
            capacity_metrics: vec![],
            items: vec![],
            equipment_slots: vec![],
        };
        let mut permuted = base.clone();
        permuted.tracks.reverse();

        let first = compose_action_actor_and_destructible_resource_catalog(
            version(),
            base,
            Some(&actor),
            Some(&object),
        )
        .unwrap();
        let second = compose_action_actor_and_destructible_resource_catalog(
            version(),
            permuted,
            Some(&actor),
            Some(&object),
        )
        .unwrap();
        assert_eq!(first, second);

        let catalog = gameplay_mechanics::MechanicsCatalog::admit(first).unwrap();
        assert_eq!(catalog.version(), &version());
        assert!(catalog
            .track(&track_id(DestructibleResourcePreset::INTEGRITY_TRACK))
            .is_some());
        assert!(catalog
            .track(&track_id(ActionActorPreset::VITALITY_TRACK))
            .is_some());
        assert!(catalog
            .damage_kind(&damage_kind_id(DestructibleResourcePreset::DAMAGE_KIND))
            .is_some());
    }

    #[test]
    fn composition_reports_shared_version_and_identity_conflicts_without_renaming() {
        let actor = ActionActorPreset::default_config();
        let object = DestructibleResourcePreset::default_config();
        let empty = || MechanicsCatalogDefinition {
            version: version(),
            stats: vec![],
            tracks: vec![],
            sources: vec![],
            damage_kinds: vec![],
            effects: vec![],
            capacity_metrics: vec![],
            items: vec![],
            equipment_slots: vec![],
        };

        let mut wrong_version = empty();
        wrong_version.version = CatalogVersion::parse("another-catalog").unwrap();
        assert!(matches!(
            compose_action_actor_and_destructible_resource_catalog(
                version(),
                wrong_version,
                Some(&actor),
                None,
            ),
            Err(PresetCompositionError::CatalogVersionMismatch { .. })
        ));

        let mut stat_collision = empty();
        stat_collision.stats.push(StatDefinition {
            id: stat_id(ActionActorPreset::VITALITY_MAX_STAT),
            minimum: scalar(0),
            maximum: scalar(1),
        });
        assert!(matches!(
            compose_action_actor_and_destructible_resource_catalog(
                version(),
                stat_collision,
                Some(&actor),
                None,
            ),
            Err(PresetCompositionError::DuplicateDefinition {
                namespace: PresetDefinitionNamespace::Stat,
                identity,
            }) if identity == ActionActorPreset::VITALITY_MAX_STAT
        ));

        let mut track_collision = empty();
        track_collision.tracks.push(TrackDefinition {
            id: track_id(DestructibleResourcePreset::INTEGRITY_TRACK),
            minimum: scalar(0),
            maximum: TrackMaximum::Fixed { value: scalar(1) },
        });
        assert!(matches!(
            compose_action_actor_and_destructible_resource_catalog(
                version(),
                track_collision,
                None,
                Some(&object),
            ),
            Err(PresetCompositionError::DuplicateDefinition {
                namespace: PresetDefinitionNamespace::Track,
                identity,
            }) if identity == DestructibleResourcePreset::INTEGRITY_TRACK
        ));

        let mut damage_collision = empty();
        damage_collision
            .damage_kinds
            .push(gameplay_mechanics::DamageKindDefinition {
                id: damage_kind_id(DestructibleResourcePreset::DAMAGE_KIND),
            });
        assert!(matches!(
            compose_action_actor_and_destructible_resource_catalog(
                version(),
                damage_collision,
                None,
                Some(&object),
            ),
            Err(PresetCompositionError::DuplicateDefinition {
                namespace: PresetDefinitionNamespace::DamageKind,
                identity,
            }) if identity == DestructibleResourcePreset::DAMAGE_KIND
        ));
    }

    #[test]
    fn configured_component_snapshot_reopens_only_against_matching_ordinary_bounds() {
        use entity_state::{
            encode_snapshot, EntityAuthoringService, EntityDefinition, EntityState,
        };

        let configured = DestructibleResourcePresetConfig::try_new(
            scalar(400),
            scalar(350),
            scalar(90),
            scalar(80),
        )
        .unwrap();
        let catalog = gameplay_mechanics::MechanicsCatalog::admit(
            DestructibleResourcePreset::catalog_fragment_with_config(version(), &configured),
        )
        .unwrap();
        let components = DestructibleResourcePreset::components_with_config(version(), &configured);
        let entity = EntityId::new(7203);
        let mut state = EntityState::from_definitions_with_registry(
            gameplay_mechanics::gameplay_component_registry().unwrap(),
            [EntityDefinition::new(entity, "ordinary-resource")],
        )
        .unwrap();
        let tracks_revision = state.component_revision::<TracksComponent>(entity).unwrap();
        EntityAuthoringService
            .attach_component(&mut state, tracks_revision, entity, components.tracks)
            .unwrap();

        let snapshot = encode_snapshot(&state).unwrap();
        let reopened =
            gameplay_mechanics::decode_snapshot_with_catalog(&snapshot, &catalog).unwrap();
        assert_eq!(encode_snapshot(&reopened).unwrap(), snapshot);

        let drifted = DestructibleResourcePresetConfig::try_new(
            scalar(300),
            scalar(300),
            scalar(90),
            scalar(80),
        )
        .unwrap();
        let drifted_catalog = gameplay_mechanics::MechanicsCatalog::admit(
            DestructibleResourcePreset::catalog_fragment_with_config(version(), &drifted),
        )
        .unwrap();
        assert!(matches!(
            gameplay_mechanics::decode_snapshot_with_catalog(&snapshot, &drifted_catalog),
            Err(gameplay_mechanics::MechanicsSnapshotError::Mechanics(
                gameplay_mechanics::MechanicsError::TrackOutOfBounds {
                    track,
                    attempted: 350,
                    maximum: 300,
                    ..
                }
            )) if track.as_str() == DestructibleResourcePreset::INTEGRITY_TRACK
        ));
    }
}
