//! Additive host DTOs for exact standard admin commands.
//!
//! These are not owner request types. They accept JSON-safe decimal identifiers,
//! reacquire the live opaque component revision, then map into the exact owner
//! request immediately before the named service dispatches it.

use core_ids::EntityId;
use entity_state::EntityState;
use gameplay_mechanics::{
    ActiveEffectsComponent, EffectApplyRequest, EffectDefinitionId, EffectInstanceId,
    EffectRemovalRequest, MechanicsScalar, OperationId, SourceDefinitionId, SourceInstanceId,
    SourceInstanceIdentity, StatBaseMutationRequest, StatId, StatsComponent, TrackId,
    TrackSetPolicy, TrackSetRequest, TracksComponent,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostStatBaseRequest {
    pub operation: OperationId,
    pub source: HostSourceIdentity,
    pub entity: String,
    pub stat: StatId,
    pub base: MechanicsScalar,
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostTrackSetRequest {
    pub operation: OperationId,
    pub source: HostSourceIdentity,
    pub entity: String,
    pub track: TrackId,
    pub value: MechanicsScalar,
    pub policy: HostTrackSetPolicy,
    pub expected_revision: Option<String>,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostTrackSetPolicy {
    RejectOutOfBounds,
    ClampToBounds,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostEffectApplyRequest {
    pub operation: OperationId,
    pub entity: String,
    pub instance: EffectInstanceId,
    pub definition: EffectDefinitionId,
    pub provenance: HostSourceIdentity,
    pub stacks: u16,
    pub expected_revision: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostEffectRemovalRequest {
    pub operation: OperationId,
    pub entity: String,
    pub instance: EffectInstanceId,
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum HostSourceIdentity {
    Intrinsic {
        entity: String,
        instance: SourceInstanceId,
    },
    Effect {
        entity: String,
        effect: gameplay_mechanics::EffectInstanceId,
        stack: u16,
        source: SourceDefinitionId,
    },
    EquippedItem {
        owner: String,
        item: String,
        source: SourceDefinitionId,
    },
    Request {
        operation: OperationId,
        instance: SourceInstanceId,
    },
}

impl HostStatBaseRequest {
    /// Converts at the product safe point; the opaque component guard is never
    /// serialized and is freshly acquired from the live state.
    pub fn map_live(self, state: &EntityState) -> Result<StatBaseMutationRequest, HostWireError> {
        let entity = decimal_entity(&self.entity, "entity")?;
        require_live_component::<StatsComponent>(state, entity)?;
        let actual = state
            .component_revision::<StatsComponent>(entity)
            .map_err(|error| HostWireError::Owner(error.to_string()))?;
        if let Some(expected) = self.expected_revision.as_deref() {
            let expected = decimal_u64(expected, "expectedRevision")?;
            if expected != actual.revision() {
                return Err(HostWireError::StaleRevision {
                    expected,
                    actual: actual.revision(),
                });
            }
        }
        Ok(StatBaseMutationRequest {
            operation: self.operation,
            source: self.source.map()?,
            entity,
            stat: self.stat,
            base: self.base,
            expected_revision: Some(actual),
        })
    }
}
impl HostTrackSetRequest {
    pub fn map_live(self, state: &EntityState) -> Result<TrackSetRequest, HostWireError> {
        let entity = decimal_entity(&self.entity, "entity")?;
        require_live_component::<TracksComponent>(state, entity)?;
        let actual = state
            .component_revision::<TracksComponent>(entity)
            .map_err(|e| HostWireError::Owner(e.to_string()))?;
        check_revision(self.expected_revision.as_deref(), actual.revision())?;
        Ok(TrackSetRequest {
            operation: self.operation,
            source: self.source.map()?,
            entity,
            track: self.track,
            value: self.value,
            policy: match self.policy {
                HostTrackSetPolicy::RejectOutOfBounds => TrackSetPolicy::RejectOutOfBounds,
                HostTrackSetPolicy::ClampToBounds => TrackSetPolicy::ClampToBounds,
            },
            expected_revision: Some(actual),
        })
    }
}
impl HostEffectApplyRequest {
    pub fn map_live(self, state: &EntityState) -> Result<EffectApplyRequest, HostWireError> {
        let entity = decimal_entity(&self.entity, "entity")?;
        require_live_component::<ActiveEffectsComponent>(state, entity)?;
        let actual = state
            .component_revision::<ActiveEffectsComponent>(entity)
            .map_err(|e| HostWireError::Owner(e.to_string()))?;
        check_revision(self.expected_revision.as_deref(), actual.revision())?;
        Ok(EffectApplyRequest {
            operation: self.operation,
            entity,
            instance: self.instance,
            definition: self.definition,
            provenance: self.provenance.map()?,
            stacks: self.stacks,
            expected_revision: Some(actual),
        })
    }
}
impl HostEffectRemovalRequest {
    pub fn map_live(self, state: &EntityState) -> Result<EffectRemovalRequest, HostWireError> {
        let entity = decimal_entity(&self.entity, "entity")?;
        require_live_component::<ActiveEffectsComponent>(state, entity)?;
        let actual = state
            .component_revision::<ActiveEffectsComponent>(entity)
            .map_err(|e| HostWireError::Owner(e.to_string()))?;
        check_revision(self.expected_revision.as_deref(), actual.revision())?;
        Ok(EffectRemovalRequest {
            operation: self.operation,
            entity,
            instance: self.instance,
            expected_revision: Some(actual),
        })
    }
}

impl HostSourceIdentity {
    fn map(self) -> Result<SourceInstanceIdentity, HostWireError> {
        Ok(match self {
            Self::Intrinsic { entity, instance } => SourceInstanceIdentity::Intrinsic {
                entity: decimal_entity(&entity, "source.entity")?,
                instance,
            },
            Self::Effect {
                entity,
                effect,
                stack,
                source,
            } => SourceInstanceIdentity::Effect {
                entity: decimal_entity(&entity, "source.entity")?,
                effect,
                stack,
                source,
            },
            Self::EquippedItem {
                owner,
                item,
                source,
            } => SourceInstanceIdentity::EquippedItem {
                owner: decimal_entity(&owner, "source.owner")?,
                item: decimal_entity(&item, "source.item")?,
                source,
            },
            Self::Request {
                operation,
                instance,
            } => SourceInstanceIdentity::Request {
                operation,
                instance,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostWireError {
    InvalidDecimal { field: &'static str },
    StaleRevision { expected: u64, actual: u64 },
    Owner(String),
}

impl std::fmt::Display for HostWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDecimal { field } => write!(formatter, "invalid decimal field {field}"),
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "stale component revision: expected {expected}, actual {actual}"
            ),
            Self::Owner(message) => {
                write!(formatter, "owner rejected host wire request: {message}")
            }
        }
    }
}

impl std::error::Error for HostWireError {}

fn decimal_entity(value: &str, field: &'static str) -> Result<EntityId, HostWireError> {
    Ok(EntityId::new(decimal_u64(value, field)?))
}
fn check_revision(value: Option<&str>, actual: u64) -> Result<(), HostWireError> {
    if let Some(value) = value {
        let expected = decimal_u64(value, "expectedRevision")?;
        if expected != actual {
            return Err(HostWireError::StaleRevision { expected, actual });
        }
    }
    Ok(())
}

fn require_live_component<T: entity_state::EntityComponent>(
    state: &EntityState,
    entity: EntityId,
) -> Result<(), HostWireError> {
    let present = state
        .has_component::<T>(entity)
        .map_err(|error| HostWireError::Owner(error.to_string()))?;
    if present {
        Ok(())
    } else {
        Err(HostWireError::Owner(format!(
            "required live component {} is absent for entity {}",
            std::any::type_name::<T>(),
            entity.raw()
        )))
    }
}
fn decimal_u64(value: &str, field: &'static str) -> Result<u64, HostWireError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HostWireError::InvalidDecimal { field });
    }
    value
        .parse()
        .map_err(|_| HostWireError::InvalidDecimal { field })
}

/// Strict schema metadata for the additive host DTO seam, exported for generated TS clients.
pub fn standard_host_wire_schemas_json() -> String {
    let mut contract = serde_json::json!({
      "kind":"rusty-developer-command-standard-host-wire.v1",
      "commands": {
        "standard.admin.stat.set-base": {"request":{"kind":"object","fields":{"operation":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"source":{"required":true,"value":{"kind":"opaqueJson","maximumBytes":1024,"maximumNodes":32}},"entity":{"required":true,"value":{"kind":"decimalU64"}},"stat":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"base":{"required":true,"value":{"kind":"integer","minimum":-1000000000000i64,"maximum":1000000000000i64}},"expectedRevision":{"required":false,"value":{"kind":"decimalU64"}}}},"result":{"kind":"opaqueJson","maximumBytes":16384,"maximumNodes":256},"error":{"kind":"opaqueJson","maximumBytes":8192,"maximumNodes":128}},
        "standard.admin.track.set": {"request":{"kind":"object","fields":{"operation":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"source":{"required":true,"value":{"kind":"opaqueJson","maximumBytes":1024,"maximumNodes":32}},"entity":{"required":true,"value":{"kind":"decimalU64"}},"track":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"value":{"required":true,"value":{"kind":"integer","minimum":-1000000000000i64,"maximum":1000000000000i64}},"policy":{"required":true,"value":{"kind":"string","maximumBytes":32,"pattern":"identifier"}},"expectedRevision":{"required":false,"value":{"kind":"decimalU64"}}}},"result":{"kind":"opaqueJson","maximumBytes":16384,"maximumNodes":256},"error":{"kind":"opaqueJson","maximumBytes":8192,"maximumNodes":128}},
        "standard.admin.effect.apply": {"request":{"kind":"object","fields":{"operation":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"entity":{"required":true,"value":{"kind":"decimalU64"}},"instance":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"definition":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"provenance":{"required":true,"value":{"kind":"opaqueJson","maximumBytes":1024,"maximumNodes":32}},"stacks":{"required":true,"value":{"kind":"integer","minimum":1,"maximum":65535}},"expectedRevision":{"required":false,"value":{"kind":"decimalU64"}}}},"result":{"kind":"opaqueJson","maximumBytes":16384,"maximumNodes":256},"error":{"kind":"opaqueJson","maximumBytes":8192,"maximumNodes":128}},
        "standard.admin.effect.remove": {"request":{"kind":"object","fields":{"operation":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"entity":{"required":true,"value":{"kind":"decimalU64"}},"instance":{"required":true,"value":{"kind":"string","maximumBytes":96,"pattern":"identifier"}},"expectedRevision":{"required":false,"value":{"kind":"decimalU64"}}}},"result":{"kind":"opaqueJson","maximumBytes":16384,"maximumNodes":256},"error":{"kind":"opaqueJson","maximumBytes":8192,"maximumNodes":128}}
      }
    });
    let commands = contract["commands"]
        .as_object_mut()
        .expect("fixed command map");
    for (command, field) in [
        ("standard.admin.stat.set-base", "source"),
        ("standard.admin.track.set", "source"),
        ("standard.admin.effect.apply", "provenance"),
    ] {
        commands[command]["request"]["fields"][field]["value"] = host_source_schema();
    }
    commands["standard.admin.track.set"]["request"]["fields"]["policy"]["value"] =
        serde_json::json!({"kind":"enum","values":["rejectOutOfBounds","clampToBounds"]});
    serde_json::to_string_pretty(&contract).expect("standard host wire schemas serialize") + "\n"
}

fn host_source_schema() -> serde_json::Value {
    let identity = |maximum_bytes| serde_json::json!({"kind":"string","maximumBytes":maximum_bytes,"pattern":"identifier"});
    let decimal = serde_json::json!({"kind":"decimalU64"});
    let object = |fields| serde_json::json!({"kind":"object","fields":fields});
    serde_json::json!({"kind":"taggedUnion","tag":"kind","variants":{
      "intrinsic": object(serde_json::json!({"kind":{"required":true,"value":{"kind":"enum","values":["intrinsic"]}},"entity":{"required":true,"value":decimal},"instance":{"required":true,"value":identity(96)}})),
      "effect": object(serde_json::json!({"kind":{"required":true,"value":{"kind":"enum","values":["effect"]}},"entity":{"required":true,"value":decimal},"effect":{"required":true,"value":identity(96)},"stack":{"required":true,"value":{"kind":"integer","minimum":0,"maximum":65535}},"source":{"required":true,"value":identity(96)}})),
      "equippedItem": object(serde_json::json!({"kind":{"required":true,"value":{"kind":"enum","values":["equippedItem"]}},"owner":{"required":true,"value":decimal},"item":{"required":true,"value":decimal},"source":{"required":true,"value":identity(96)}})),
      "request": object(serde_json::json!({"kind":{"required":true,"value":{"kind":"enum","values":["request"]}},"operation":{"required":true,"value":identity(96)},"instance":{"required":true,"value":identity(96)}}))
    }})
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTITY: EntityId = EntityId::new(70);

    fn state_with_components() -> EntityState {
        let mut state = EntityState::from_definitions_with_registry(
            gameplay_mechanics::gameplay_component_registry().unwrap(),
            [entity_state::EntityDefinition::new(
                ENTITY,
                "host-wire-fixture",
            )],
        )
        .unwrap();
        let catalog = gameplay_mechanics::CatalogVersion::parse("host-wire.v1").unwrap();
        let revision = state.component_revision::<StatsComponent>(ENTITY).unwrap();
        entity_state::EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                ENTITY,
                StatsComponent::new(
                    catalog.clone(),
                    vec![gameplay_mechanics::StatValue::new(
                        StatId::parse("vitality").unwrap(),
                        MechanicsScalar::new(10).unwrap(),
                    )],
                )
                .unwrap(),
            )
            .unwrap();
        let revision = state.component_revision::<TracksComponent>(ENTITY).unwrap();
        entity_state::EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                ENTITY,
                TracksComponent::new(
                    catalog.clone(),
                    vec![gameplay_mechanics::TrackValue::new(
                        TrackId::parse("health").unwrap(),
                        MechanicsScalar::new(10).unwrap(),
                    )],
                )
                .unwrap(),
            )
            .unwrap();
        let revision = state
            .component_revision::<ActiveEffectsComponent>(ENTITY)
            .unwrap();
        entity_state::EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                ENTITY,
                ActiveEffectsComponent::new(catalog, vec![]).unwrap(),
            )
            .unwrap();
        state
    }

    fn empty_state() -> EntityState {
        EntityState::from_definitions_with_registry(
            gameplay_mechanics::gameplay_component_registry().unwrap(),
            [entity_state::EntityDefinition::new(
                ENTITY,
                "host-wire-fixture",
            )],
        )
        .unwrap()
    }

    fn revision_snapshot(state: &EntityState) -> (u64, u64, u64) {
        (
            state
                .component_revision::<StatsComponent>(ENTITY)
                .unwrap()
                .revision(),
            state
                .component_revision::<TracksComponent>(ENTITY)
                .unwrap()
                .revision(),
            state
                .component_revision::<ActiveEffectsComponent>(ENTITY)
                .unwrap()
                .revision(),
        )
    }

    fn stat_request(expected_revision: Option<u64>) -> HostStatBaseRequest {
        HostStatBaseRequest {
            operation: OperationId::parse("host-stat").unwrap(),
            source: HostSourceIdentity::Request {
                operation: OperationId::parse("host-stat").unwrap(),
                instance: SourceInstanceId::parse("admin").unwrap(),
            },
            entity: ENTITY.raw().to_string(),
            stat: StatId::parse("vitality").unwrap(),
            base: MechanicsScalar::new(20).unwrap(),
            expected_revision: expected_revision.map(|value| value.to_string()),
        }
    }

    fn track_request(expected_revision: Option<u64>) -> HostTrackSetRequest {
        HostTrackSetRequest {
            operation: OperationId::parse("host-track").unwrap(),
            source: HostSourceIdentity::Request {
                operation: OperationId::parse("host-track").unwrap(),
                instance: SourceInstanceId::parse("admin").unwrap(),
            },
            entity: ENTITY.raw().to_string(),
            track: TrackId::parse("health").unwrap(),
            value: MechanicsScalar::new(20).unwrap(),
            policy: HostTrackSetPolicy::RejectOutOfBounds,
            expected_revision: expected_revision.map(|value| value.to_string()),
        }
    }

    fn apply_request(expected_revision: Option<u64>) -> HostEffectApplyRequest {
        HostEffectApplyRequest {
            operation: OperationId::parse("host-effect-apply").unwrap(),
            entity: ENTITY.raw().to_string(),
            instance: EffectInstanceId::parse("effect-instance").unwrap(),
            definition: EffectDefinitionId::parse("effect-definition").unwrap(),
            provenance: HostSourceIdentity::Request {
                operation: OperationId::parse("host-effect-apply").unwrap(),
                instance: SourceInstanceId::parse("admin").unwrap(),
            },
            stacks: 1,
            expected_revision: expected_revision.map(|value| value.to_string()),
        }
    }

    fn removal_request(expected_revision: Option<u64>) -> HostEffectRemovalRequest {
        HostEffectRemovalRequest {
            operation: OperationId::parse("host-effect-remove").unwrap(),
            entity: ENTITY.raw().to_string(),
            instance: EffectInstanceId::parse("effect-instance").unwrap(),
            expected_revision: expected_revision.map(|value| value.to_string()),
        }
    }
    #[test]
    fn source_dto_rejects_unknown_fields_and_maps_every_variant() {
        for source in [
            r#"{"kind":"intrinsic","entity":"1","instance":"source"}"#,
            r#"{"kind":"effect","entity":"1","effect":"effect","stack":1,"source":"source"}"#,
            r#"{"kind":"equippedItem","owner":"1","item":"2","source":"source"}"#,
            r#"{"kind":"request","operation":"operation","instance":"source"}"#,
        ] {
            assert!(serde_json::from_str::<HostSourceIdentity>(source).is_ok());
        }
        assert!(serde_json::from_str::<HostSourceIdentity>(
            r#"{"kind":"intrinsic","entity":"1","instance":"source","extra":true}"#
        )
        .is_err());
    }

    #[test]
    fn source_mapping_keeps_each_discriminant_and_decimal_field_identity() {
        let state = state_with_components();
        let sources = [
            HostSourceIdentity::Intrinsic {
                entity: "71".to_owned(),
                instance: SourceInstanceId::parse("intrinsic").unwrap(),
            },
            HostSourceIdentity::Effect {
                entity: "72".to_owned(),
                effect: EffectInstanceId::parse("effect").unwrap(),
                stack: 2,
                source: SourceDefinitionId::parse("source").unwrap(),
            },
            HostSourceIdentity::EquippedItem {
                owner: "73".to_owned(),
                item: "74".to_owned(),
                source: SourceDefinitionId::parse("source").unwrap(),
            },
            HostSourceIdentity::Request {
                operation: OperationId::parse("host-source").unwrap(),
                instance: SourceInstanceId::parse("request").unwrap(),
            },
        ];
        for (index, source) in sources.into_iter().enumerate() {
            let mapped = HostStatBaseRequest {
                source,
                ..stat_request(None)
            }
            .map_live(&state)
            .unwrap();
            match (index, mapped.source) {
                (0, SourceInstanceIdentity::Intrinsic { entity, .. }) => {
                    assert_eq!(entity, EntityId::new(71));
                }
                (1, SourceInstanceIdentity::Effect { entity, stack, .. }) => {
                    assert_eq!(entity, EntityId::new(72));
                    assert_eq!(stack, 2);
                }
                (2, SourceInstanceIdentity::EquippedItem { owner, item, .. }) => {
                    assert_eq!(owner, EntityId::new(73));
                    assert_eq!(item, EntityId::new(74));
                }
                (3, SourceInstanceIdentity::Request { operation, .. }) => {
                    assert_eq!(operation.as_str(), "host-source");
                }
                (_, other) => panic!("source discriminant changed during mapping: {other:?}"),
            }
        }

        let mut malformed = stat_request(None);
        malformed.source = HostSourceIdentity::EquippedItem {
            owner: "01".to_owned(),
            item: "2".to_owned(),
            source: SourceDefinitionId::parse("source").unwrap(),
        };
        assert!(matches!(
            malformed.map_live(&state),
            Err(HostWireError::InvalidDecimal {
                field: "source.owner"
            })
        ));
    }
    #[test]
    fn committed_standard_schema_matches_rust_export() {
        assert_eq!(
            super::standard_host_wire_schemas_json(),
            include_str!("../../../../render/contracts/developer-command-standard-host-wire.json")
        );
    }

    #[test]
    fn exported_schema_marks_every_json_decimal_identifier_explicitly() {
        let contract: serde_json::Value =
            serde_json::from_str(&super::standard_host_wire_schemas_json()).unwrap();
        let decimal = |pointer: &str| {
            assert_eq!(
                contract
                    .pointer(pointer)
                    .and_then(serde_json::Value::as_str),
                Some("decimalU64"),
                "expected decimalU64 at {pointer}"
            );
        };
        for command in [
            "standard.admin.stat.set-base",
            "standard.admin.track.set",
            "standard.admin.effect.apply",
            "standard.admin.effect.remove",
        ] {
            decimal(&format!(
                "/commands/{command}/request/fields/entity/value/kind"
            ));
            decimal(&format!(
                "/commands/{command}/request/fields/expectedRevision/value/kind"
            ));
        }
        for command in [
            "standard.admin.stat.set-base",
            "standard.admin.track.set",
            "standard.admin.effect.apply",
        ] {
            let field = if command == "standard.admin.effect.apply" {
                "provenance"
            } else {
                "source"
            };
            for variant_field in ["entity", "owner", "item"] {
                for variant in ["intrinsic", "effect", "equippedItem"] {
                    let pointer = format!(
                        "/commands/{command}/request/fields/{field}/value/variants/{variant}/fields/{variant_field}/value/kind"
                    );
                    // Only the identities actually present in a tagged variant
                    // are expected to have the explicit decimal contract.
                    if contract.pointer(&pointer).is_some() {
                        decimal(&pointer);
                    }
                }
            }
        }
    }

    #[test]
    fn every_host_dto_strictly_decodes_and_rejects_unknown_fields() {
        let stat = r#"{"operation":"host-stat","source":{"kind":"request","operation":"host-stat","instance":"admin"},"entity":"70","stat":"vitality","base":20,"unexpected":true}"#;
        let track = r#"{"operation":"host-track","source":{"kind":"request","operation":"host-track","instance":"admin"},"entity":"70","track":"health","value":20,"policy":"rejectOutOfBounds","unexpected":true}"#;
        let apply = r#"{"operation":"host-effect-apply","entity":"70","instance":"effect-instance","definition":"effect-definition","provenance":{"kind":"request","operation":"host-effect-apply","instance":"admin"},"stacks":1,"unexpected":true}"#;
        let remove = r#"{"operation":"host-effect-remove","entity":"70","instance":"effect-instance","unexpected":true}"#;
        for error in [
            serde_json::from_str::<HostStatBaseRequest>(stat)
                .expect_err("unknown stat field must be rejected"),
            serde_json::from_str::<HostTrackSetRequest>(track)
                .expect_err("unknown track field must be rejected"),
            serde_json::from_str::<HostEffectApplyRequest>(apply)
                .expect_err("unknown effect field must be rejected"),
            serde_json::from_str::<HostEffectRemovalRequest>(remove)
                .expect_err("unknown removal field must be rejected"),
        ] {
            assert_eq!(error.classify(), serde_json::error::Category::Data);
            assert!(
                error.to_string().contains("unknown field"),
                "expected deny_unknown_fields error, got {error}"
            );
        }
    }

    #[test]
    fn every_host_dto_reacquires_live_guards_and_mapping_never_mutates_state() {
        let state = state_with_components();
        let before = revision_snapshot(&state);
        let stat = stat_request(None).map_live(&state).unwrap();
        let track = track_request(None).map_live(&state).unwrap();
        let apply = apply_request(None).map_live(&state).unwrap();
        let remove = removal_request(None).map_live(&state).unwrap();
        assert_eq!(stat.entity, ENTITY);
        assert_eq!(track.entity, ENTITY);
        assert_eq!(apply.entity, ENTITY);
        assert_eq!(remove.entity, ENTITY);
        assert_eq!(stat.expected_revision.unwrap().revision(), before.0);
        assert_eq!(track.expected_revision.unwrap().revision(), before.1);
        assert_eq!(apply.expected_revision.unwrap().revision(), before.2);
        assert_eq!(remove.expected_revision.unwrap().revision(), before.2);
        assert_eq!(
            revision_snapshot(&state),
            before,
            "wire mapping only reads owner facts"
        );
    }

    #[test]
    fn every_host_dto_rejects_stale_or_absent_owner_components_without_mutation() {
        let state = state_with_components();
        let before = revision_snapshot(&state);
        assert!(matches!(
            stat_request(Some(before.0 + 1)).map_live(&state),
            Err(HostWireError::StaleRevision { .. })
        ));
        assert!(matches!(
            track_request(Some(before.1 + 1)).map_live(&state),
            Err(HostWireError::StaleRevision { .. })
        ));
        assert!(matches!(
            apply_request(Some(before.2 + 1)).map_live(&state),
            Err(HostWireError::StaleRevision { .. })
        ));
        assert!(matches!(
            removal_request(Some(before.2 + 1)).map_live(&state),
            Err(HostWireError::StaleRevision { .. })
        ));
        assert_eq!(revision_snapshot(&state), before);

        let absent = empty_state();
        let absent_before = revision_snapshot(&absent);
        assert!(matches!(
            stat_request(None).map_live(&absent),
            Err(HostWireError::Owner(_))
        ));
        assert!(matches!(
            track_request(None).map_live(&absent),
            Err(HostWireError::Owner(_))
        ));
        assert!(matches!(
            apply_request(None).map_live(&absent),
            Err(HostWireError::Owner(_))
        ));
        assert!(matches!(
            removal_request(None).map_live(&absent),
            Err(HostWireError::Owner(_))
        ));
        assert_eq!(revision_snapshot(&absent), absent_before);
    }
}
