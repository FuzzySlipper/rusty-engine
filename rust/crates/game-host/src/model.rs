use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use engine_spatial::{MotionPhaseReceipt, MAX_MOTION_DELTA_SECONDS};
use entity_state::{EntityDefinition, EntityFact, EntityState, EntityView, ProjectionNode};
use serde::{Deserialize, Serialize};

use crate::scheduler::Scheduler;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoorConfig {
    pub closed_translation: Vec3,
    pub open_translation: Vec3,
    pub auto_close_after: Option<TickDelta>,
}

impl DoorConfig {
    pub fn new(
        closed_translation: Vec3,
        open_translation: Vec3,
        auto_close_after: Option<TickDelta>,
    ) -> Self {
        Self {
            closed_translation,
            open_translation,
            auto_close_after,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorState {
    Closed,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoorComponent {
    pub config: DoorConfig,
    pub state: DoorState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SwitchComponent {
    pub activation_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyState {
    Alive,
    Defeated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyComponent {
    pub state: EnemyState,
}

pub const MAX_PLAYER_SPEED_UNITS_PER_SECOND: f32 = 1_000.0;
pub const MAX_PLAYER_LOOK_DEGREES_PER_UNIT: f32 = 180.0;
pub const MAX_INPUT_CONTROL_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInputBindings {
    pub move_forward: String,
    pub move_backward: String,
    pub move_left: String,
    pub move_right: String,
    pub mouse_look: String,
}

impl PlayerInputBindings {
    pub fn new(
        move_forward: impl Into<String>,
        move_backward: impl Into<String>,
        move_left: impl Into<String>,
        move_right: impl Into<String>,
        mouse_look: impl Into<String>,
    ) -> Self {
        Self {
            move_forward: move_forward.into(),
            move_backward: move_backward.into(),
            move_left: move_left.into(),
            move_right: move_right.into(),
            mouse_look: mouse_look.into(),
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        let controls = [
            self.move_forward.as_str(),
            self.move_backward.as_str(),
            self.move_left.as_str(),
            self.move_right.as_str(),
            self.mouse_look.as_str(),
        ];
        if controls
            .iter()
            .any(|control| control.is_empty() || control.len() > MAX_INPUT_CONTROL_LENGTH)
        {
            return false;
        }
        let keyboard_controls = &controls[..4];
        keyboard_controls
            .iter()
            .enumerate()
            .all(|(index, control)| !keyboard_controls[..index].contains(control))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerControllerConfig {
    pub move_speed_units_per_second: f32,
    pub move_step_seconds: f32,
    pub look_degrees_per_unit: f32,
    pub initial_yaw_degrees: f32,
    pub initial_pitch_degrees: f32,
    pub bindings: PlayerInputBindings,
}

impl PlayerControllerConfig {
    pub(crate) fn is_valid(&self) -> bool {
        self.move_speed_units_per_second.is_finite()
            && self.move_speed_units_per_second > 0.0
            && self.move_speed_units_per_second <= MAX_PLAYER_SPEED_UNITS_PER_SECOND
            && self.move_step_seconds.is_finite()
            && self.move_step_seconds > 0.0
            && self.move_step_seconds <= MAX_MOTION_DELTA_SECONDS
            && self.look_degrees_per_unit.is_finite()
            && self.look_degrees_per_unit > 0.0
            && self.look_degrees_per_unit <= MAX_PLAYER_LOOK_DEGREES_PER_UNIT
            && self.initial_yaw_degrees.is_finite()
            && self.initial_pitch_degrees.is_finite()
            && (-89.0..=89.0).contains(&self.initial_pitch_degrees)
            && self.bindings.is_valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerControllerState {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerControllerComponent {
    pub config: PlayerControllerConfig,
    pub state: PlayerControllerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResolvedPlayerAction {
    Move { forward: f32, right: f32 },
    Look { yaw_delta: f32, pitch_delta: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerControlFact {
    Moved {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
    },
    Blocked {
        entity: EntityId,
        attempted_velocity: Vec3,
    },
    LookChanged {
        entity: EntityId,
        before_yaw_degrees: f32,
        after_yaw_degrees: f32,
        before_pitch_degrees: f32,
        after_pitch_degrees: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerControlReceipt {
    pub action: ResolvedPlayerAction,
    pub facts: Vec<PlayerControlFact>,
    pub motion: Option<MotionPhaseReceipt>,
}

pub const MAX_NAVIGATION_SPEED_UNITS_PER_SECOND: f32 = 1_000.0;
pub const MAX_NAVIGATION_QUERY_BUDGET: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationConfig {
    pub goal: Vec3,
    pub speed_units_per_second: f32,
    pub max_visited: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationState {
    Following,
    Arrived,
    Blocked,
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationComponent {
    pub config: NavigationConfig,
    pub state: NavigationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationFailure {
    StartNotWalkable,
    GoalNotWalkable,
    NoPath,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationFact {
    Advanced {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
        path_hash: u64,
    },
    Arrived {
        entity: EntityId,
        goal: Vec3,
    },
    Blocked {
        entity: EntityId,
        goal: Vec3,
    },
    Unreachable {
        entity: EntityId,
        goal: Vec3,
        reason: NavigationFailure,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationPhaseReceipt {
    pub agents_considered: usize,
    pub advanced_agents: usize,
    pub arrived_agents: usize,
    pub blocked_agents: usize,
    pub unreachable_agents: usize,
    pub navigation_hash: u64,
    pub facts: Vec<NavigationFact>,
    pub motion: MotionPhaseReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterConfig {
    pub members: Vec<EntityId>,
    pub exit: EntityId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncounterState {
    Active,
    Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterComponent {
    pub config: EncounterConfig,
    pub state: EncounterState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameEntityDefinition {
    pub entity: EntityDefinition,
    pub door: Option<DoorConfig>,
    pub switch: bool,
    pub controls_targets: Vec<EntityId>,
    pub enemy: bool,
    pub encounter: Option<EncounterConfig>,
    pub navigation: Option<NavigationConfig>,
    pub player_controller: Option<PlayerControllerConfig>,
}

impl GameEntityDefinition {
    pub fn new(entity: EntityDefinition) -> Self {
        Self {
            entity,
            door: None,
            switch: false,
            controls_targets: Vec::new(),
            enemy: false,
            encounter: None,
            navigation: None,
            player_controller: None,
        }
    }

    pub fn as_door(mut self, config: DoorConfig) -> Self {
        self.door = Some(config);
        self
    }

    pub fn as_switch(mut self) -> Self {
        self.switch = true;
        self
    }

    pub fn controls(mut self, targets: impl IntoIterator<Item = EntityId>) -> Self {
        self.controls_targets = targets.into_iter().collect();
        self
    }

    pub fn as_enemy(mut self) -> Self {
        self.enemy = true;
        self
    }

    pub fn as_encounter(
        mut self,
        members: impl IntoIterator<Item = EntityId>,
        exit: EntityId,
    ) -> Self {
        self.encounter = Some(EncounterConfig {
            members: members.into_iter().collect(),
            exit,
        });
        self
    }

    pub fn with_navigation(mut self, config: NavigationConfig) -> Self {
        self.navigation = Some(config);
        self
    }

    pub fn with_player_controller(mut self, config: PlayerControllerConfig) -> Self {
        self.player_controller = Some(config);
        self
    }
}

#[derive(Debug)]
pub enum GameEntityDefinitionError {
    EntityState(entity_state::EntityDefinitionError),
    DuplicateControlTarget {
        switch: EntityId,
        target: EntityId,
    },
    ControlsWithoutSwitch {
        entity: EntityId,
    },
    UnknownControlTarget {
        switch: EntityId,
        target: EntityId,
    },
    ControlTargetIsNotDoor {
        switch: EntityId,
        target: EntityId,
    },
    DoorMissingTransform {
        entity: EntityId,
    },
    DoorMissingCollision {
        entity: EntityId,
    },
    DoorMissingRenderable {
        entity: EntityId,
    },
    EnemyMissingCollision {
        entity: EntityId,
    },
    EnemyMissingRenderable {
        entity: EntityId,
    },
    NavigationWithoutEnemy {
        entity: EntityId,
    },
    NavigationMissingTransform {
        entity: EntityId,
    },
    NavigationMissingCollision {
        entity: EntityId,
    },
    NavigationMissingKinematic {
        entity: EntityId,
    },
    InvalidNavigationGoal {
        entity: EntityId,
    },
    InvalidNavigationSpeed {
        entity: EntityId,
    },
    InvalidNavigationQueryBudget {
        entity: EntityId,
    },
    PlayerControllerMissingTransform {
        entity: EntityId,
    },
    PlayerControllerMissingCollision {
        entity: EntityId,
    },
    PlayerControllerMissingKinematic {
        entity: EntityId,
    },
    PlayerControllerMissingRenderable {
        entity: EntityId,
    },
    InvalidPlayerControllerConfig {
        entity: EntityId,
    },
    EmptyEncounter {
        encounter: EntityId,
    },
    DuplicateEncounterMember {
        encounter: EntityId,
        member: EntityId,
    },
    UnknownEncounterMember {
        encounter: EntityId,
        member: EntityId,
    },
    EncounterMemberIsNotEnemy {
        encounter: EntityId,
        member: EntityId,
    },
    UnknownEncounterExit {
        encounter: EntityId,
        exit: EntityId,
    },
    EncounterExitIsNotDoor {
        encounter: EntityId,
        exit: EntityId,
    },
    EnemyInMultipleEncounters {
        enemy: EntityId,
        first: EntityId,
        second: EntityId,
    },
}

impl std::fmt::Display for GameEntityDefinitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GameEntityDefinitionError {}

#[derive(Debug)]
pub struct GameSession {
    pub(crate) entities: EntityState,
    pub(crate) doors: BTreeMap<EntityId, DoorComponent>,
    pub(crate) switches: BTreeMap<EntityId, SwitchComponent>,
    pub(crate) controls: BTreeMap<EntityId, Vec<EntityId>>,
    pub(crate) enemies: BTreeMap<EntityId, EnemyComponent>,
    pub(crate) encounters: BTreeMap<EntityId, EncounterComponent>,
    pub(crate) navigators: BTreeMap<EntityId, NavigationComponent>,
    pub(crate) player_controllers: BTreeMap<EntityId, PlayerControllerComponent>,
}

impl GameSession {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = GameEntityDefinition>,
    ) -> Result<Self, GameEntityDefinitionError> {
        let definitions: Vec<GameEntityDefinition> = definitions.into_iter().collect();
        let entities = EntityState::from_definitions(
            definitions
                .iter()
                .map(|definition| definition.entity.clone()),
        )
        .map_err(GameEntityDefinitionError::EntityState)?;

        let mut doors = BTreeMap::new();
        let mut switches = BTreeMap::new();
        let mut controls = BTreeMap::new();
        let mut enemies = BTreeMap::new();
        let mut encounters = BTreeMap::new();
        let mut navigators = BTreeMap::new();
        let mut player_controllers = BTreeMap::new();

        for definition in &definitions {
            let entity = definition.entity.id;
            if let Some(config) = definition.door {
                let view = entities.view(entity).expect("definition created entity");
                if view.transform.is_none() {
                    return Err(GameEntityDefinitionError::DoorMissingTransform { entity });
                }
                if view.collision.is_none() {
                    return Err(GameEntityDefinitionError::DoorMissingCollision { entity });
                }
                if view.renderable.is_none() {
                    return Err(GameEntityDefinitionError::DoorMissingRenderable { entity });
                }
                doors.insert(
                    entity,
                    DoorComponent {
                        config,
                        state: DoorState::Closed,
                    },
                );
            }
            if definition.switch {
                switches.insert(entity, SwitchComponent::default());
            }
            if !definition.controls_targets.is_empty() {
                if !definition.switch {
                    return Err(GameEntityDefinitionError::ControlsWithoutSwitch { entity });
                }
                let mut unique = BTreeSet::new();
                for target in &definition.controls_targets {
                    if !unique.insert(*target) {
                        return Err(GameEntityDefinitionError::DuplicateControlTarget {
                            switch: entity,
                            target: *target,
                        });
                    }
                }
                controls.insert(entity, definition.controls_targets.clone());
            }
            if definition.enemy {
                let view = entities.view(entity).expect("definition created entity");
                if view.collision.is_none() {
                    return Err(GameEntityDefinitionError::EnemyMissingCollision { entity });
                }
                if view.renderable.is_none() {
                    return Err(GameEntityDefinitionError::EnemyMissingRenderable { entity });
                }
                enemies.insert(
                    entity,
                    EnemyComponent {
                        state: EnemyState::Alive,
                    },
                );
            }
            if let Some(config) = definition.navigation {
                if !definition.enemy {
                    return Err(GameEntityDefinitionError::NavigationWithoutEnemy { entity });
                }
                let view = entities.view(entity).expect("definition created entity");
                if view.transform.is_none() {
                    return Err(GameEntityDefinitionError::NavigationMissingTransform { entity });
                }
                if view.collision.is_none() {
                    return Err(GameEntityDefinitionError::NavigationMissingCollision { entity });
                }
                if view.kinematic.is_none() {
                    return Err(GameEntityDefinitionError::NavigationMissingKinematic { entity });
                }
                if !vec3_is_finite(config.goal) {
                    return Err(GameEntityDefinitionError::InvalidNavigationGoal { entity });
                }
                if !config.speed_units_per_second.is_finite()
                    || !(0.0..=MAX_NAVIGATION_SPEED_UNITS_PER_SECOND)
                        .contains(&config.speed_units_per_second)
                    || config.speed_units_per_second == 0.0
                {
                    return Err(GameEntityDefinitionError::InvalidNavigationSpeed { entity });
                }
                if !(1..=MAX_NAVIGATION_QUERY_BUDGET).contains(&config.max_visited) {
                    return Err(GameEntityDefinitionError::InvalidNavigationQueryBudget { entity });
                }
                navigators.insert(
                    entity,
                    NavigationComponent {
                        config,
                        state: NavigationState::Following,
                    },
                );
            }
            if let Some(config) = &definition.player_controller {
                let view = entities.view(entity).expect("definition created entity");
                if view.transform.is_none() {
                    return Err(
                        GameEntityDefinitionError::PlayerControllerMissingTransform { entity },
                    );
                }
                if view.collision.is_none() {
                    return Err(
                        GameEntityDefinitionError::PlayerControllerMissingCollision { entity },
                    );
                }
                if view.kinematic.is_none() {
                    return Err(
                        GameEntityDefinitionError::PlayerControllerMissingKinematic { entity },
                    );
                }
                if view.renderable.is_none() {
                    return Err(
                        GameEntityDefinitionError::PlayerControllerMissingRenderable { entity },
                    );
                }
                if !config.is_valid() {
                    return Err(GameEntityDefinitionError::InvalidPlayerControllerConfig {
                        entity,
                    });
                }
                player_controllers.insert(
                    entity,
                    PlayerControllerComponent {
                        config: config.clone(),
                        state: PlayerControllerState {
                            yaw_degrees: config.initial_yaw_degrees,
                            pitch_degrees: config.initial_pitch_degrees,
                        },
                    },
                );
            }
            if let Some(config) = &definition.encounter {
                if config.members.is_empty() {
                    return Err(GameEntityDefinitionError::EmptyEncounter { encounter: entity });
                }
                let mut unique = BTreeSet::new();
                for member in &config.members {
                    if !unique.insert(*member) {
                        return Err(GameEntityDefinitionError::DuplicateEncounterMember {
                            encounter: entity,
                            member: *member,
                        });
                    }
                }
                encounters.insert(
                    entity,
                    EncounterComponent {
                        config: config.clone(),
                        state: EncounterState::Active,
                    },
                );
            }
        }

        for (switch, targets) in &controls {
            for target in targets {
                if !entities.contains(*target) {
                    return Err(GameEntityDefinitionError::UnknownControlTarget {
                        switch: *switch,
                        target: *target,
                    });
                }
                if !doors.contains_key(target) {
                    return Err(GameEntityDefinitionError::ControlTargetIsNotDoor {
                        switch: *switch,
                        target: *target,
                    });
                }
            }
        }

        let mut encounter_by_enemy = BTreeMap::new();
        for (encounter, component) in &encounters {
            if !entities.contains(component.config.exit) {
                return Err(GameEntityDefinitionError::UnknownEncounterExit {
                    encounter: *encounter,
                    exit: component.config.exit,
                });
            }
            if !doors.contains_key(&component.config.exit) {
                return Err(GameEntityDefinitionError::EncounterExitIsNotDoor {
                    encounter: *encounter,
                    exit: component.config.exit,
                });
            }
            for member in &component.config.members {
                if !entities.contains(*member) {
                    return Err(GameEntityDefinitionError::UnknownEncounterMember {
                        encounter: *encounter,
                        member: *member,
                    });
                }
                if !enemies.contains_key(member) {
                    return Err(GameEntityDefinitionError::EncounterMemberIsNotEnemy {
                        encounter: *encounter,
                        member: *member,
                    });
                }
                if let Some(first) = encounter_by_enemy.insert(*member, *encounter) {
                    return Err(GameEntityDefinitionError::EnemyInMultipleEncounters {
                        enemy: *member,
                        first,
                        second: *encounter,
                    });
                }
            }
        }

        Ok(Self {
            entities,
            doors,
            switches,
            controls,
            enemies,
            encounters,
            navigators,
            player_controllers,
        })
    }

    pub fn entities(&self) -> &EntityState {
        &self.entities
    }

    pub fn entity(&self, entity: EntityId) -> Result<EntityView, entity_state::ViewError> {
        self.entities.view(entity)
    }

    pub fn door(&self, entity: EntityId) -> Option<DoorView> {
        let component = self.doors.get(&entity)?;
        Some(DoorView {
            entity,
            config: component.config,
            state: component.state,
            entity_view: self.entities.view(entity).ok()?,
        })
    }

    pub fn switch(&self, entity: EntityId) -> Option<SwitchView> {
        let component = self.switches.get(&entity)?;
        Some(SwitchView {
            entity,
            activation_count: component.activation_count,
            controls_targets: self.controls.get(&entity).cloned().unwrap_or_default(),
        })
    }

    pub fn enemy(&self, entity: EntityId) -> Option<EnemyView> {
        let component = self.enemies.get(&entity)?;
        Some(EnemyView {
            entity,
            state: component.state,
            entity_view: self.entities.view(entity).ok()?,
        })
    }

    pub fn encounter(&self, entity: EntityId) -> Option<EncounterView> {
        let component = self.encounters.get(&entity)?;
        Some(EncounterView {
            entity,
            members: component.config.members.clone(),
            exit: component.config.exit,
            state: component.state,
        })
    }

    pub fn navigation(&self, entity: EntityId) -> Option<NavigationView> {
        let component = self.navigators.get(&entity)?;
        Some(NavigationView {
            entity,
            config: component.config,
            state: component.state,
            entity_view: self.entities.view(entity).ok()?,
        })
    }

    pub fn player_controller(&self, entity: EntityId) -> Option<PlayerControllerView> {
        let component = self.player_controllers.get(&entity)?;
        Some(PlayerControllerView {
            entity,
            config: component.config.clone(),
            state: component.state,
            entity_view: self.entities.view(entity).ok()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoorView {
    pub entity: EntityId,
    pub config: DoorConfig,
    pub state: DoorState,
    pub entity_view: EntityView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchView {
    pub entity: EntityId,
    pub activation_count: u64,
    pub controls_targets: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnemyView {
    pub entity: EntityId,
    pub state: EnemyState,
    pub entity_view: EntityView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterView {
    pub entity: EntityId,
    pub members: Vec<EntityId>,
    pub exit: EntityId,
    pub state: EncounterState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationView {
    pub entity: EntityId,
    pub config: NavigationConfig,
    pub state: NavigationState,
    pub entity_view: EntityView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerControllerView {
    pub entity: EntityId,
    pub config: PlayerControllerConfig,
    pub state: PlayerControllerState,
    pub entity_view: EntityView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    SwitchActivated {
        switch: EntityId,
        actor: EntityId,
    },
    DoorOpened {
        door: EntityId,
        entity_facts: Vec<EntityFact>,
    },
    DoorClosed {
        door: EntityId,
        entity_facts: Vec<EntityFact>,
    },
    EnemyDefeated {
        enemy: EntityId,
        actor: EntityId,
        entity_facts: Vec<EntityFact>,
    },
    EncounterCleared {
        encounter: EntityId,
        exit: EntityId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct JournalEntry {
    pub tick: Tick,
    pub event: GameEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeReceipt {
    pub tick: Tick,
    pub events: Vec<GameEvent>,
    pub projection: Vec<ProjectionNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeReadout {
    pub tick: Tick,
    pub entity_revision: u64,
    pub projection: Vec<ProjectionNode>,
    pub pending_schedules: usize,
    pub journal: Vec<JournalEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityDoorIds {
    pub actor: EntityId,
    pub switch: EntityId,
    pub door: EntityId,
}

impl SecurityDoorIds {
    pub const fn standard() -> Self {
        Self {
            actor: EntityId::new(1),
            switch: EntityId::new(2),
            door: EntityId::new(3),
        }
    }
}

pub fn security_door_definitions(
    auto_close_after: Option<TickDelta>,
) -> (SecurityDoorIds, Vec<GameEntityDefinition>) {
    let ids = SecurityDoorIds::standard();
    let door_config = DoorConfig::new(Vec3::ZERO, Vec3::new(0.0, 3.0, 0.0), auto_close_after);
    (
        ids,
        vec![
            GameEntityDefinition::new(EntityDefinition::new(ids.actor, "player")),
            GameEntityDefinition::new(EntityDefinition::new(ids.switch, "security-switch"))
                .as_switch()
                .controls([ids.door]),
            GameEntityDefinition::new(
                EntityDefinition::new(ids.door, "security-door")
                    .with_transform(door_config.closed_translation)
                    .with_collision(true, true)
                    .with_renderable("mesh/security-door", true),
            )
            .as_door(door_config),
        ],
    )
}

pub(crate) fn readout(
    tick: Tick,
    session: &GameSession,
    scheduler: &Scheduler,
    journal: &[JournalEntry],
) -> RuntimeReadout {
    RuntimeReadout {
        tick,
        entity_revision: session.entities.revision(),
        projection: session.entities.projection(),
        pending_schedules: scheduler.len(),
        journal: journal.to_vec(),
    }
}

fn vec3_is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
