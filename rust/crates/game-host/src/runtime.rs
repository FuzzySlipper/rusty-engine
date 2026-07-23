use std::collections::VecDeque;

use core_ids::EntityId;
use core_time::{Tick, TickDelta};

use crate::content::{decode_project_content, ProjectContentError};
use crate::model::{
    readout, security_door_definitions, GameEvent, GameSession, JournalEntry, RuntimeReadout,
    RuntimeReceipt, SecurityDoorIds,
};
use crate::scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};
use crate::services::{
    CombatService, DoorService, DoorTransition, EncounterService, InteractionService,
};

pub const MAX_EVENT_WAVE: usize = 256;
pub const MAX_TICK_ADVANCE: u64 = 100_000;

#[derive(Debug)]
pub enum RuntimeError {
    Content(ProjectContentError),
    Definition(crate::model::GameEntityDefinitionError),
    UnknownActor { actor: EntityId },
    NotInteractable { entity: EntityId },
    UnknownDoor { door: EntityId },
    UnknownEnemy { enemy: EntityId },
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

    pub fn from_project_content(input: &str) -> Result<Self, RuntimeError> {
        let session = decode_project_content(input).map_err(RuntimeError::Content)?;
        Ok(Self::new(session))
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

    pub fn defeat_enemy(
        &mut self,
        actor: EntityId,
        enemy: EntityId,
    ) -> Result<RuntimeReceipt, RuntimeError> {
        if let Some(event) = CombatService::defeat_enemy(&mut self.session, actor, enemy)? {
            self.events.push_back(event);
        }
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
            match &event {
                GameEvent::SwitchActivated { switch, .. } => {
                    let targets = self
                        .session
                        .controls
                        .get(switch)
                        .cloned()
                        .unwrap_or_default();
                    for door in targets {
                        if let Some(transition) = DoorService::open(&mut self.session, door)? {
                            self.queue_door_transition(door, transition);
                        }
                    }
                }
                GameEvent::EnemyDefeated { enemy, .. } => {
                    self.events.extend(EncounterService::observe_enemy_defeat(
                        &mut self.session,
                        *enemy,
                    ));
                }
                GameEvent::EncounterCleared { exit, .. } => {
                    if let Some(transition) = DoorService::open(&mut self.session, *exit)? {
                        self.queue_door_transition(*exit, transition);
                    }
                }
                GameEvent::DoorOpened { .. } | GameEvent::DoorClosed { .. } => {}
            }
            processed.push(event);
        }
        Ok(processed)
    }

    fn queue_door_transition(&mut self, door: EntityId, transition: DoorTransition) {
        if let Some(delay) = transition.auto_close_after {
            self.scheduler.schedule(ScheduledIntent {
                due: self.tick.advance(delay),
                kind: ScheduledIntentKind::CloseDoor { door },
            });
        }
        self.events.push_back(transition.event);
    }

    fn receipt(&self, events: Vec<GameEvent>) -> RuntimeReceipt {
        RuntimeReceipt {
            tick: self.tick,
            events,
            projection: self.session.world.projection(),
        }
    }
}
