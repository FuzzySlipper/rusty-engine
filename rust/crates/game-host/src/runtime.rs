use std::collections::VecDeque;

use core_ids::EntityId;
use core_time::{Tick, TickDelta};
use world_kernel::{WorldCommand, WorldCommandBatch};

use crate::model::{
    readout, security_door_definitions, DoorState, GameEvent, GameSession, JournalEntry,
    RuntimeReadout, RuntimeReceipt, SecurityDoorIds,
};
use crate::scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};

pub const MAX_EVENT_WAVE: usize = 256;
pub const MAX_TICK_ADVANCE: u64 = 100_000;

#[derive(Debug)]
pub enum RuntimeError {
    Definition(crate::model::GameEntityDefinitionError),
    UnknownActor { actor: EntityId },
    NotInteractable { entity: EntityId },
    UnknownDoor { door: EntityId },
    WorldBatch(world_kernel::BatchRejection),
    EventWaveLimit { limit: usize },
    TickAdvanceLimit { requested: u64, limit: u64 },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug)]
pub struct GameRuntime {
    pub(crate) session: GameSession,
    pub(crate) tick: Tick,
    pub(crate) scheduler: Scheduler,
    pub(crate) events: VecDeque<GameEvent>,
    pub(crate) journal: Vec<JournalEntry>,
}

impl GameRuntime {
    pub fn new(session: GameSession) -> Self {
        Self {
            session,
            tick: Tick::ZERO,
            scheduler: Scheduler::default(),
            events: VecDeque::new(),
            journal: Vec::new(),
        }
    }

    pub fn security_door(
        auto_close_after: Option<TickDelta>,
    ) -> Result<(SecurityDoorIds, Self), RuntimeError> {
        let (ids, definitions) = security_door_definitions(auto_close_after);
        let session =
            GameSession::from_definitions(definitions).map_err(RuntimeError::Definition)?;
        Ok((ids, Self::new(session)))
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn session(&self) -> &GameSession {
        &self.session
    }

    pub fn readout(&self) -> RuntimeReadout {
        readout(self.tick, &self.session, &self.scheduler, &self.journal)
    }

    pub fn interact(
        &mut self,
        actor: EntityId,
        target: EntityId,
    ) -> Result<RuntimeReceipt, RuntimeError> {
        let event = InteractionService::interact(&mut self.session, actor, target)?;
        self.events.push_back(event);
        let events = self.drain_events()?;
        Ok(self.receipt(events))
    }

    pub fn advance_by(&mut self, ticks: u64) -> Result<RuntimeReceipt, RuntimeError> {
        if ticks > MAX_TICK_ADVANCE {
            return Err(RuntimeError::TickAdvanceLimit {
                requested: ticks,
                limit: MAX_TICK_ADVANCE,
            });
        }
        let mut processed = Vec::new();
        for _ in 0..ticks {
            self.tick = self.tick.next();
            for intent in self.scheduler.drain_due(self.tick) {
                self.handle_scheduled_intent(intent)?;
            }
            processed.extend(self.drain_events()?);
        }
        Ok(self.receipt(processed))
    }

    fn handle_scheduled_intent(&mut self, intent: ScheduledIntent) -> Result<(), RuntimeError> {
        match intent.kind {
            ScheduledIntentKind::CloseDoor { door } => {
                if let Some(event) = DoorService::close(&mut self.session, door)? {
                    self.events.push_back(event);
                }
            }
        }
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<GameEvent>, RuntimeError> {
        let mut processed = Vec::new();
        while let Some(event) = self.events.pop_front() {
            if processed.len() >= MAX_EVENT_WAVE {
                self.events.clear();
                return Err(RuntimeError::EventWaveLimit {
                    limit: MAX_EVENT_WAVE,
                });
            }
            self.journal.push(JournalEntry {
                tick: self.tick,
                event: event.clone(),
            });
            if let GameEvent::SwitchActivated { switch, .. } = &event {
                let targets = self
                    .session
                    .controls
                    .get(switch)
                    .cloned()
                    .unwrap_or_default();
                for door in targets {
                    if let Some(transition) = DoorService::open(&mut self.session, door)? {
                        if let Some(delay) = transition.auto_close_after {
                            self.scheduler.schedule(ScheduledIntent {
                                due: self.tick.advance(delay),
                                kind: ScheduledIntentKind::CloseDoor { door },
                            });
                        }
                        self.events.push_back(transition.event);
                    }
                }
            }
            processed.push(event);
        }
        Ok(processed)
    }

    fn receipt(&self, events: Vec<GameEvent>) -> RuntimeReceipt {
        RuntimeReceipt {
            tick: self.tick,
            events,
            projection: self.session.world.projection(),
        }
    }
}

struct InteractionService;

impl InteractionService {
    fn interact(
        session: &mut GameSession,
        actor: EntityId,
        target: EntityId,
    ) -> Result<GameEvent, RuntimeError> {
        if !session.world.contains(actor) {
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

struct DoorTransition {
    event: GameEvent,
    auto_close_after: Option<TickDelta>,
}

struct DoorService;

impl DoorService {
    fn open(
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
            .world
            .apply_batch(WorldCommandBatch::new([
                WorldCommand::SetTranslation {
                    entity: door,
                    translation: component.config.open_translation,
                },
                WorldCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: false,
                },
            ]))
            .map_err(RuntimeError::WorldBatch)?;
        session
            .doors
            .get_mut(&door)
            .expect("door validated above")
            .state = DoorState::Open;
        Ok(Some(DoorTransition {
            event: GameEvent::DoorOpened {
                door,
                world_facts: receipt.facts,
            },
            auto_close_after: component.config.auto_close_after,
        }))
    }

    fn close(session: &mut GameSession, door: EntityId) -> Result<Option<GameEvent>, RuntimeError> {
        let Some(component) = session.doors.get(&door).copied() else {
            return Err(RuntimeError::UnknownDoor { door });
        };
        if component.state == DoorState::Closed {
            return Ok(None);
        }
        let receipt = session
            .world
            .apply_batch(WorldCommandBatch::new([
                WorldCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: true,
                },
                WorldCommand::SetTranslation {
                    entity: door,
                    translation: component.config.closed_translation,
                },
            ]))
            .map_err(RuntimeError::WorldBatch)?;
        session
            .doors
            .get_mut(&door)
            .expect("door validated above")
            .state = DoorState::Closed;
        Ok(Some(GameEvent::DoorClosed {
            door,
            world_facts: receipt.facts,
        }))
    }
}
