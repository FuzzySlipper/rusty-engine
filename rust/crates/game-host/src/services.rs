use core_ids::EntityId;
use core_time::TickDelta;
use entity_state::{EntityCommand, EntityCommandBatch};

use crate::model::{DoorState, EncounterState, EnemyState, GameEvent, GameSession};
use crate::runtime::RuntimeError;

pub(crate) struct InteractionService;

impl InteractionService {
    pub(crate) fn interact(
        session: &mut GameSession,
        actor: EntityId,
        target: EntityId,
    ) -> Result<GameEvent, RuntimeError> {
        if !session.entities.contains(actor) {
            return Err(RuntimeError::UnknownActor { actor });
        }
        let Some(switch) = session.switches.get_mut(&target) else {
            return Err(RuntimeError::NotInteractable { entity: target });
        };
        switch.activation_count = switch.activation_count.saturating_add(1);
        Ok(GameEvent::SwitchActivated {
            switch: target,
            actor,
        })
    }
}

pub(crate) struct DoorTransition {
    pub(crate) event: GameEvent,
    pub(crate) auto_close_after: Option<TickDelta>,
}

pub(crate) struct DoorService;

impl DoorService {
    pub(crate) fn open(
        session: &mut GameSession,
        door: EntityId,
    ) -> Result<Option<DoorTransition>, RuntimeError> {
        let Some(component) = session.doors.get(&door).copied() else {
            return Err(RuntimeError::UnknownDoor { door });
        };
        if component.state == DoorState::Open {
            return Ok(None);
        }
        let receipt = session
            .entities
            .apply_batch(EntityCommandBatch::new([
                EntityCommand::SetTranslation {
                    entity: door,
                    translation: component.config.open_translation,
                },
                EntityCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: false,
                },
            ]))
            .map_err(RuntimeError::EntityBatch)?;
        session
            .doors
            .get_mut(&door)
            .expect("door validated above")
            .state = DoorState::Open;
        Ok(Some(DoorTransition {
            event: GameEvent::DoorOpened {
                door,
                entity_facts: receipt.facts,
            },
            auto_close_after: component.config.auto_close_after,
        }))
    }

    pub(crate) fn close(
        session: &mut GameSession,
        door: EntityId,
    ) -> Result<Option<GameEvent>, RuntimeError> {
        let Some(component) = session.doors.get(&door).copied() else {
            return Err(RuntimeError::UnknownDoor { door });
        };
        if component.state == DoorState::Closed {
            return Ok(None);
        }
        let receipt = session
            .entities
            .apply_batch(EntityCommandBatch::new([
                EntityCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: true,
                },
                EntityCommand::SetTranslation {
                    entity: door,
                    translation: component.config.closed_translation,
                },
            ]))
            .map_err(RuntimeError::EntityBatch)?;
        session
            .doors
            .get_mut(&door)
            .expect("door validated above")
            .state = DoorState::Closed;
        Ok(Some(GameEvent::DoorClosed {
            door,
            entity_facts: receipt.facts,
        }))
    }
}

pub(crate) struct CombatService;

impl CombatService {
    pub(crate) fn defeat_enemy(
        session: &mut GameSession,
        actor: EntityId,
        enemy: EntityId,
    ) -> Result<Option<GameEvent>, RuntimeError> {
        if !session.entities.contains(actor) {
            return Err(RuntimeError::UnknownActor { actor });
        }
        let Some(component) = session.enemies.get(&enemy).copied() else {
            return Err(RuntimeError::UnknownEnemy { enemy });
        };
        if component.state == EnemyState::Defeated {
            return Ok(None);
        }

        let receipt = session
            .entities
            .apply_batch(EntityCommandBatch::new([
                EntityCommand::SetCollisionEnabled {
                    entity: enemy,
                    enabled: false,
                },
                EntityCommand::SetVisible {
                    entity: enemy,
                    visible: false,
                },
            ]))
            .map_err(RuntimeError::EntityBatch)?;
        session
            .enemies
            .get_mut(&enemy)
            .expect("enemy validated above")
            .state = EnemyState::Defeated;
        Ok(Some(GameEvent::EnemyDefeated {
            enemy,
            actor,
            entity_facts: receipt.facts,
        }))
    }
}

pub(crate) struct EncounterService;

impl EncounterService {
    pub(crate) fn observe_enemy_defeat(
        session: &mut GameSession,
        enemy: EntityId,
    ) -> Vec<GameEvent> {
        let candidates: Vec<EntityId> = session
            .encounters
            .iter()
            .filter(|(_, encounter)| {
                encounter.state == EncounterState::Active
                    && encounter.config.members.contains(&enemy)
            })
            .map(|(entity, _)| *entity)
            .collect();
        let mut events = Vec::new();

        for encounter in candidates {
            let cleared = session.encounters[&encounter]
                .config
                .members
                .iter()
                .all(|member| {
                    session
                        .enemies
                        .get(member)
                        .is_some_and(|enemy| enemy.state == EnemyState::Defeated)
                });
            if !cleared {
                continue;
            }
            let component = session
                .encounters
                .get_mut(&encounter)
                .expect("candidate encounter exists");
            component.state = EncounterState::Cleared;
            events.push(GameEvent::EncounterCleared {
                encounter,
                exit: component.config.exit,
            });
        }

        events
    }
}
