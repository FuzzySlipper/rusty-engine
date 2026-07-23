use core_ids::EntityId;
use core_time::Tick;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use world_kernel::{EntityLifecycle, EntityView, ProjectionNode, WorldFact};

pub const PROJECT_HOST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorBinding {
    pub instance_id: String,
    pub behavior_type: String,
    pub version: u32,
    pub owner_entity: EntityId,
    pub related_entities: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectStateRecord {
    pub instance_id: String,
    pub behavior_type: String,
    pub version: u32,
    pub revision: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectInvocationWave {
    pub schema_version: u32,
    pub tick: u64,
    pub expected_world_revision: u64,
    pub invocations: Vec<ProjectInvocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectInvocation {
    pub invocation_id: u64,
    pub behavior: InvocationBehavior,
    pub events: Vec<HostInputEvent>,
    pub owner: HostEntityView,
    pub related: Vec<HostEntityView>,
    pub state: ProjectStateRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvocationBehavior {
    pub instance_id: String,
    pub behavior_type: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HostInputEvent {
    Interaction {
        actor: u64,
        target: u64,
    },
    Message {
        message_id: String,
        message_kind: String,
        payload: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostEntityView {
    pub entity: u64,
    pub name: String,
    pub lifecycle: HostLifecycle,
    pub translation: Option<[f32; 3]>,
    pub collision: Option<HostCollisionView>,
    pub renderable: Option<HostRenderableView>,
}

impl From<EntityView> for HostEntityView {
    fn from(value: EntityView) -> Self {
        Self {
            entity: value.id.raw(),
            name: value.name,
            lifecycle: match value.lifecycle {
                EntityLifecycle::Active => HostLifecycle::Active,
                EntityLifecycle::Disabled => HostLifecycle::Disabled,
            },
            translation: value
                .transform
                .map(|transform| transform.translation.to_array()),
            collision: value.collision.map(|collision| HostCollisionView {
                enabled: collision.enabled,
                static_collider: collision.static_collider,
            }),
            renderable: value.renderable.map(|renderable| HostRenderableView {
                visible: renderable.visible,
                asset: renderable.asset,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostLifecycle {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostCollisionView {
    pub enabled: bool,
    pub static_collider: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostRenderableView {
    pub visible: bool,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectDecisionBatch {
    pub schema_version: u32,
    pub expected_world_revision: u64,
    pub decisions: Vec<ProjectDecision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectDecision {
    pub invocation_id: u64,
    pub commands: Vec<ProjectWorldCommand>,
    pub state_update: Option<ProjectStateUpdate>,
    pub schedules: Vec<ProjectScheduleRequest>,
    pub facts: Vec<ProjectFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectStateUpdate {
    pub expected_revision: u64,
    pub version: u32,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectWorldCommand {
    SetTranslation { entity: u64, translation: [f32; 3] },
    SetCollisionEnabled { entity: u64, enabled: bool },
    SetVisible { entity: u64, visible: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectScheduleRequest {
    Upsert {
        message_id: String,
        due_after_ticks: u64,
        message_kind: String,
        payload: Value,
    },
    Cancel {
        message_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectFact {
    pub kind: String,
    pub version: u32,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectApplyReceipt {
    pub tick: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub engine_facts: Vec<HostEngineFact>,
    pub project_facts: Vec<ProjectFact>,
    pub state_records: Vec<ProjectStateRecord>,
    pub pending_message_count: usize,
    pub projection: Vec<HostProjectionNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HostEngineFact {
    TranslationChanged {
        entity: u64,
        before: [f32; 3],
        after: [f32; 3],
        revision: u64,
    },
    CollisionChanged {
        entity: u64,
        before: bool,
        after: bool,
        revision: u64,
    },
    VisibilityChanged {
        entity: u64,
        before: bool,
        after: bool,
        revision: u64,
    },
}

impl From<WorldFact> for HostEngineFact {
    fn from(value: WorldFact) -> Self {
        match value {
            WorldFact::TranslationChanged {
                entity,
                before,
                after,
                revision,
            } => Self::TranslationChanged {
                entity: entity.raw(),
                before: before.to_array(),
                after: after.to_array(),
                revision,
            },
            WorldFact::CollisionChanged {
                entity,
                before,
                after,
                revision,
            } => Self::CollisionChanged {
                entity: entity.raw(),
                before,
                after,
                revision,
            },
            WorldFact::VisibilityChanged {
                entity,
                before,
                after,
                revision,
            } => Self::VisibilityChanged {
                entity: entity.raw(),
                before,
                after,
                revision,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostProjectionNode {
    pub entity: u64,
    pub name: String,
    pub asset: String,
    pub translation: Option<[f32; 3]>,
    pub visible: bool,
}

impl From<ProjectionNode> for HostProjectionNode {
    fn from(value: ProjectionNode) -> Self {
        Self {
            entity: value.entity.raw(),
            name: value.name,
            asset: value.asset,
            translation: value.translation.map(|translation| translation.to_array()),
            visible: value.visible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectRuntimeReadout {
    pub tick: u64,
    pub world_revision: u64,
    pub state_records: Vec<ProjectStateRecord>,
    pub pending_message_count: usize,
    pub pending_invocation: bool,
    pub project_facts: Vec<ProjectFactJournalEntry>,
    pub projection: Vec<HostProjectionNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectFactJournalEntry {
    pub tick: u64,
    pub instance_id: String,
    pub fact: ProjectFact,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledProjectMessage {
    pub instance_id: String,
    pub message_id: String,
    pub due: Tick,
    pub message_kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectDoorIds {
    pub actor: EntityId,
    pub switch: EntityId,
    pub door: EntityId,
}

impl ProjectDoorIds {
    pub const fn standard() -> Self {
        Self {
            actor: EntityId::new(1),
            switch: EntityId::new(2),
            door: EntityId::new(3),
        }
    }
}
