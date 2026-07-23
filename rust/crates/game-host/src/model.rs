use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use world_kernel::{EntityDefinition, EntityView, ProjectionNode, WorldFact, WorldKernel};

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

#[derive(Debug, Clone, PartialEq)]
pub struct GameEntityDefinition {
    pub world: EntityDefinition,
    pub door: Option<DoorConfig>,
    pub switch: bool,
    pub controls_targets: Vec<EntityId>,
}

impl GameEntityDefinition {
    pub fn new(world: EntityDefinition) -> Self {
        Self {
            world,
            door: None,
            switch: false,
            controls_targets: Vec::new(),
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
}

#[derive(Debug)]
pub enum GameEntityDefinitionError {
    World(world_kernel::EntityDefinitionError),
    DuplicateControlTarget { switch: EntityId, target: EntityId },
    ControlsWithoutSwitch { entity: EntityId },
    UnknownControlTarget { switch: EntityId, target: EntityId },
    ControlTargetIsNotDoor { switch: EntityId, target: EntityId },
    DoorMissingTransform { entity: EntityId },
    DoorMissingCollision { entity: EntityId },
    DoorMissingRenderable { entity: EntityId },
}

impl std::fmt::Display for GameEntityDefinitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GameEntityDefinitionError {}

#[derive(Debug)]
pub struct GameSession {
    pub(crate) world: WorldKernel,
    pub(crate) doors: BTreeMap<EntityId, DoorComponent>,
    pub(crate) switches: BTreeMap<EntityId, SwitchComponent>,
    pub(crate) controls: BTreeMap<EntityId, Vec<EntityId>>,
}

impl GameSession {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = GameEntityDefinition>,
    ) -> Result<Self, GameEntityDefinitionError> {
        let definitions: Vec<GameEntityDefinition> = definitions.into_iter().collect();
        let world = WorldKernel::from_definitions(
            definitions
                .iter()
                .map(|definition| definition.world.clone()),
        )
        .map_err(GameEntityDefinitionError::World)?;

        let mut doors = BTreeMap::new();
        let mut switches = BTreeMap::new();
        let mut controls = BTreeMap::new();

        for definition in &definitions {
            let entity = definition.world.id;
            if let Some(config) = definition.door {
                let view = world.view(entity).expect("definition created entity");
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
        }

        for (switch, targets) in &controls {
            for target in targets {
                if !world.contains(*target) {
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

        Ok(Self {
            world,
            doors,
            switches,
            controls,
        })
    }

    pub fn world(&self) -> &WorldKernel {
        &self.world
    }

    pub fn entity(&self, entity: EntityId) -> Result<EntityView, world_kernel::ViewError> {
        self.world.view(entity)
    }

    pub fn door(&self, entity: EntityId) -> Option<DoorView> {
        let component = self.doors.get(&entity)?;
        Some(DoorView {
            entity,
            config: component.config,
            state: component.state,
            world: self.world.view(entity).ok()?,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoorView {
    pub entity: EntityId,
    pub config: DoorConfig,
    pub state: DoorState,
    pub world: EntityView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchView {
    pub entity: EntityId,
    pub activation_count: u64,
    pub controls_targets: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    SwitchActivated {
        switch: EntityId,
        actor: EntityId,
    },
    DoorOpened {
        door: EntityId,
        world_facts: Vec<WorldFact>,
    },
    DoorClosed {
        door: EntityId,
        world_facts: Vec<WorldFact>,
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
    pub world_revision: u64,
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
        world_revision: session.world.revision(),
        projection: session.world.projection(),
        pending_schedules: scheduler.len(),
        journal: journal.to_vec(),
    }
}
