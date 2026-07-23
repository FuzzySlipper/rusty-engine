use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use engine_spatial::{
    KinematicMotionSystem, MotionFact, NavigationStepError, VoxelCollisionScene,
    MAX_MOTION_DELTA_SECONDS,
};
use entity_state::{EntityCommand, EntityCommandBatch};

use crate::model::{
    CombatFact, CombatMissReason, CombatRejectionReason, DoorState, EncounterState, EnemyState,
    GameEvent, GameSession, NavigationFact, NavigationFailure, NavigationPhaseReceipt,
    NavigationState, PlayerControlFact, PlayerControlReceipt, ResolvedAttackAction,
    ResolvedPlayerAction,
};
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

#[derive(Debug)]
pub(crate) struct CombatResolution {
    pub(crate) action: ResolvedAttackAction,
    pub(crate) facts: Vec<CombatFact>,
    pub(crate) event: Option<GameEvent>,
}

#[derive(Debug, Clone, Copy)]
struct CombatTargetHit {
    entity: EntityId,
    distance: f32,
}

impl CombatService {
    pub(crate) fn attack(
        session: &mut GameSession,
        scene: &VoxelCollisionScene,
        tick: Tick,
        attacker: EntityId,
        action: ResolvedAttackAction,
    ) -> Result<CombatResolution, RuntimeError> {
        if !session.entities.contains(attacker) {
            return Err(RuntimeError::UnknownActor { actor: attacker });
        }
        let Some(weapon) = session.weapons.get(&attacker).copied() else {
            return Err(RuntimeError::UnknownWeapon { entity: attacker });
        };
        if tick.raw() < weapon.state.ready_at_tick.raw() {
            return Err(RuntimeError::CombatRejected {
                entity: attacker,
                reason: CombatRejectionReason::Cooldown,
            });
        }
        if weapon.state.ammo_remaining == 0 {
            return Err(RuntimeError::CombatRejected {
                entity: attacker,
                reason: CombatRejectionReason::NoAmmo,
            });
        }
        let controller = session
            .player_controllers
            .get(&attacker)
            .expect("weapon admission requires a player controller");
        let transform = session
            .entities
            .view(attacker)
            .expect("weapon admission requires an entity")
            .transform
            .expect("player controller admission requires a transform")
            .translation;
        let direction = aim_direction(controller.state.yaw_degrees, controller.state.pitch_degrees);
        let origin =
            transform + local_aim_offset(weapon.config.muzzle_offset, controller.state.yaw_degrees);
        let target = nearest_combat_target(
            session,
            attacker,
            origin,
            direction,
            weapon.config.max_distance,
        );
        let world_blocker = scene
            .raycast(
                [origin.x as f64, origin.y as f64, origin.z as f64],
                [direction.x as f64, direction.y as f64, direction.z as f64],
                weapon.config.max_distance as f64,
            )
            .map(|hit| hit.distance as f32);
        let ammo_after = weapon.state.ammo_remaining - 1;
        let ready_at_tick = tick.advance(TickDelta::new(weapon.config.cooldown_ticks));
        let mut facts = vec![CombatFact::AttackFired {
            attacker,
            origin,
            direction,
            ammo_before: weapon.state.ammo_remaining,
            ammo_after,
            ready_at_tick,
        }];
        let mut event = None;

        match target {
            Some(hit) if world_blocker.is_none_or(|distance| hit.distance + 0.000_1 < distance) => {
                let health = session
                    .health
                    .get(&hit.entity)
                    .copied()
                    .expect("target selection requires health");
                let amount = weapon.config.damage.min(health.current);
                let after = health.current - amount;
                facts.push(CombatFact::AttackHit {
                    attacker,
                    target: hit.entity,
                    distance: hit.distance,
                });
                facts.push(CombatFact::DamageApplied {
                    attacker,
                    target: hit.entity,
                    amount,
                    before: health.current,
                    after,
                });
                if after == 0 {
                    event = Self::defeat_enemy(session, attacker, hit.entity)?;
                    facts.push(CombatFact::EnemyDefeated {
                        attacker,
                        enemy: hit.entity,
                    });
                } else {
                    session
                        .health
                        .get_mut(&hit.entity)
                        .expect("target health remains attached")
                        .current = after;
                }
            }
            Some(_) => facts.push(CombatFact::AttackMissed {
                attacker,
                reason: CombatMissReason::WorldBlocked,
            }),
            None => facts.push(CombatFact::AttackMissed {
                attacker,
                reason: CombatMissReason::NoTarget,
            }),
        }

        session
            .weapons
            .get_mut(&attacker)
            .expect("weapon validated above")
            .state = crate::model::WeaponState {
            ammo_remaining: ammo_after,
            ready_at_tick,
        };
        Ok(CombatResolution {
            action,
            facts,
            event,
        })
    }

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

        let mut commands = vec![
            EntityCommand::SetCollisionEnabled {
                entity: enemy,
                enabled: false,
            },
            EntityCommand::SetVisible {
                entity: enemy,
                visible: false,
            },
        ];
        if session
            .entities
            .view(enemy)
            .expect("enemy entity validated during admission")
            .kinematic
            .is_some()
        {
            commands.push(EntityCommand::SetKinematicVelocity {
                entity: enemy,
                velocity: Vec3::ZERO,
            });
        }
        let receipt = session
            .entities
            .apply_batch(EntityCommandBatch::new(commands))
            .map_err(RuntimeError::EntityBatch)?;
        session
            .enemies
            .get_mut(&enemy)
            .expect("enemy validated above")
            .state = EnemyState::Defeated;
        if let Some(health) = session.health.get_mut(&enemy) {
            health.current = 0;
        }
        Ok(Some(GameEvent::EnemyDefeated {
            enemy,
            actor,
            entity_facts: receipt.facts,
        }))
    }
}

fn aim_direction(yaw_degrees: f32, pitch_degrees: f32) -> Vec3 {
    let yaw = yaw_degrees.to_radians();
    let pitch = pitch_degrees.to_radians();
    let horizontal = pitch.cos();
    Vec3::new(
        -yaw.sin() * horizontal,
        pitch.sin(),
        -yaw.cos() * horizontal,
    )
}

fn local_aim_offset(offset: Vec3, yaw_degrees: f32) -> Vec3 {
    let yaw = yaw_degrees.to_radians();
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    right * offset.x + Vec3::new(0.0, offset.y, 0.0) + forward * offset.z
}

fn nearest_combat_target(
    session: &GameSession,
    attacker: EntityId,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<CombatTargetHit> {
    let mut best = None;
    for (entity, enemy) in &session.enemies {
        if *entity == attacker || enemy.state != EnemyState::Alive {
            continue;
        }
        let Some(health) = session.health.get(entity) else {
            continue;
        };
        if health.current == 0 {
            continue;
        }
        let Ok(view) = session.entities.view(*entity) else {
            continue;
        };
        if !view.collision.is_some_and(|collision| collision.enabled) {
            continue;
        }
        let Some(transform) = view.transform else {
            continue;
        };
        let min = transform.translation - health.config.hitbox_half_extents;
        let max = transform.translation + health.config.hitbox_half_extents;
        let Some(distance) = ray_aabb_distance(origin, direction, min, max) else {
            continue;
        };
        if distance > max_distance {
            continue;
        }
        if best.is_none_or(|hit: CombatTargetHit| distance < hit.distance) {
            best = Some(CombatTargetHit {
                entity: *entity,
                distance,
            });
        }
    }
    best
}

// Adapted from Asha svc-combat at the pinned donor revision. The service-owned
// health table and replay/hash machinery deliberately remain behind.
fn ray_aabb_distance(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = f32::INFINITY;
    for (axis_origin, axis_direction, lo, hi) in [
        (origin.x, direction.x, min.x.min(max.x), min.x.max(max.x)),
        (origin.y, direction.y, min.y.min(max.y), min.y.max(max.y)),
        (origin.z, direction.z, min.z.min(max.z), min.z.max(max.z)),
    ] {
        if axis_direction.abs() < f32::EPSILON {
            if axis_origin < lo || axis_origin > hi {
                return None;
            }
            continue;
        }
        let inverse = axis_direction.recip();
        let mut near = (lo - axis_origin) * inverse;
        let mut far = (hi - axis_origin) * inverse;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t_min = t_min.max(near);
        t_max = t_max.min(far);
        if t_min > t_max {
            return None;
        }
    }
    (t_max >= 0.0).then_some(t_min.max(0.0))
}

pub(crate) struct PlayerControllerService;

impl PlayerControllerService {
    pub(crate) fn apply(
        session: &mut GameSession,
        scene: &VoxelCollisionScene,
        player: EntityId,
        action: ResolvedPlayerAction,
    ) -> Result<PlayerControlReceipt, RuntimeError> {
        if !player_action_is_valid(action) {
            return Err(RuntimeError::InvalidPlayerAction { action });
        }
        let Some(component) = session.player_controllers.get(&player).cloned() else {
            return Err(RuntimeError::UnknownPlayerController { player });
        };
        match action {
            ResolvedPlayerAction::Look {
                yaw_delta,
                pitch_delta,
            } => {
                let before = component.state;
                let controller = session
                    .player_controllers
                    .get_mut(&player)
                    .expect("player controller validated above");
                controller.state.yaw_degrees = normalize_yaw(
                    before.yaw_degrees + yaw_delta * component.config.look_degrees_per_unit,
                );
                controller.state.pitch_degrees = (before.pitch_degrees
                    + pitch_delta * component.config.look_degrees_per_unit)
                    .clamp(-89.0, 89.0);
                Ok(PlayerControlReceipt {
                    action,
                    facts: vec![PlayerControlFact::LookChanged {
                        entity: player,
                        before_yaw_degrees: before.yaw_degrees,
                        after_yaw_degrees: controller.state.yaw_degrees,
                        before_pitch_degrees: before.pitch_degrees,
                        after_pitch_degrees: controller.state.pitch_degrees,
                    }],
                    motion: None,
                })
            }
            ResolvedPlayerAction::Move { forward, right } => {
                let input_length = (forward * forward + right * right).sqrt();
                if input_length == 0.0 {
                    return Ok(PlayerControlReceipt {
                        action,
                        facts: Vec::new(),
                        motion: None,
                    });
                }
                let scale = 1.0 / input_length.max(1.0);
                let yaw = component.state.yaw_degrees.to_radians();
                let forward_basis = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
                let right_basis = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
                let velocity = (forward_basis * (forward * scale) + right_basis * (right * scale))
                    * component.config.move_speed_units_per_second;
                session
                    .entities
                    .apply_batch(EntityCommandBatch::new([
                        EntityCommand::SetKinematicVelocity {
                            entity: player,
                            velocity,
                        },
                    ]))
                    .map_err(RuntimeError::EntityBatch)?;
                let selected = BTreeSet::from([player]);
                let motion_result = KinematicMotionSystem::run_selected(
                    &mut session.entities,
                    scene,
                    component.config.move_step_seconds,
                    &selected,
                );
                session
                    .entities
                    .apply_batch(EntityCommandBatch::new([
                        EntityCommand::SetKinematicVelocity {
                            entity: player,
                            velocity: Vec3::ZERO,
                        },
                    ]))
                    .map_err(RuntimeError::EntityBatch)?;
                let motion = motion_result.map_err(RuntimeError::Motion)?;
                let facts = motion
                    .facts
                    .iter()
                    .filter_map(|fact| match fact {
                        MotionFact::Moved {
                            entity,
                            before,
                            after,
                        } if *entity == player => Some(PlayerControlFact::Moved {
                            entity: *entity,
                            before: *before,
                            after: *after,
                        }),
                        MotionFact::Blocked { entity, .. } if *entity == player => {
                            Some(PlayerControlFact::Blocked {
                                entity: *entity,
                                attempted_velocity: velocity,
                            })
                        }
                        MotionFact::Moved { .. } | MotionFact::Blocked { .. } => None,
                    })
                    .collect();
                Ok(PlayerControlReceipt {
                    action,
                    facts,
                    motion: Some(motion),
                })
            }
        }
    }
}

fn player_action_is_valid(action: ResolvedPlayerAction) -> bool {
    match action {
        ResolvedPlayerAction::Move { forward, right } => {
            forward.is_finite()
                && right.is_finite()
                && (-1.0..=1.0).contains(&forward)
                && (-1.0..=1.0).contains(&right)
        }
        ResolvedPlayerAction::Look {
            yaw_delta,
            pitch_delta,
        } => {
            yaw_delta.is_finite()
                && pitch_delta.is_finite()
                && (-1.0..=1.0).contains(&yaw_delta)
                && (-1.0..=1.0).contains(&pitch_delta)
        }
    }
}

fn normalize_yaw(yaw_degrees: f32) -> f32 {
    (yaw_degrees + 180.0).rem_euclid(360.0) - 180.0
}

pub(crate) struct EnemyNavigationSystem;

#[derive(Debug, Clone, Copy)]
struct NavigationPlan {
    goal: Vec3,
    before: Vec3,
    path_hash: u64,
    reaches_goal: bool,
}

impl EnemyNavigationSystem {
    pub(crate) fn run(
        session: &mut GameSession,
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
    ) -> Result<NavigationPhaseReceipt, RuntimeError> {
        if !delta_seconds.is_finite()
            || !(0.0..=MAX_MOTION_DELTA_SECONDS).contains(&delta_seconds)
            || delta_seconds == 0.0
        {
            return Err(RuntimeError::InvalidNavigationDelta {
                actual: delta_seconds,
            });
        }

        let active: Vec<_> = session
            .navigators
            .iter()
            .filter(|(entity, navigation)| {
                matches!(
                    navigation.state,
                    NavigationState::Following | NavigationState::Blocked
                ) && session
                    .enemies
                    .get(entity)
                    .is_some_and(|enemy| enemy.state == EnemyState::Alive)
            })
            .map(|(entity, navigation)| (*entity, navigation.config))
            .collect();
        let agents_considered = active.len();
        let selected: BTreeSet<_> = active.iter().map(|(entity, _)| *entity).collect();
        let mut plans = BTreeMap::new();
        let mut velocity_commands = Vec::new();
        let mut facts = Vec::new();
        let mut unreachable_agents = 0usize;

        for (entity, config) in active {
            let view = session
                .entities
                .view(entity)
                .expect("navigation entity validated during admission");
            let before = view
                .transform
                .expect("navigation transform validated during admission")
                .translation;
            let current_velocity = view
                .kinematic
                .expect("navigation kinematic validated during admission")
                .velocity;
            match scene.navigation_step(
                before,
                config.goal,
                current_velocity,
                config.speed_units_per_second * delta_seconds,
                config.max_visited,
            ) {
                Ok(step) => {
                    let velocity = (step.next_waypoint - before) * (1.0 / delta_seconds);
                    velocity_commands
                        .push(EntityCommand::SetKinematicVelocity { entity, velocity });
                    plans.insert(
                        entity,
                        NavigationPlan {
                            goal: config.goal,
                            before,
                            path_hash: step.path_hash,
                            reaches_goal: step.reached,
                        },
                    );
                }
                Err(error) => {
                    let reason = match error {
                        NavigationStepError::StartNotWalkable { .. } => {
                            NavigationFailure::StartNotWalkable
                        }
                        NavigationStepError::GoalNotWalkable { .. } => {
                            NavigationFailure::GoalNotWalkable
                        }
                        NavigationStepError::NoPath { .. } => NavigationFailure::NoPath,
                        NavigationStepError::InvalidRequest { .. } => {
                            return Err(RuntimeError::NavigationStep {
                                entity,
                                source: error,
                            });
                        }
                    };
                    session
                        .navigators
                        .get_mut(&entity)
                        .expect("active navigation component")
                        .state = NavigationState::Unreachable;
                    velocity_commands.push(EntityCommand::SetKinematicVelocity {
                        entity,
                        velocity: Vec3::ZERO,
                    });
                    facts.push(NavigationFact::Unreachable {
                        entity,
                        goal: config.goal,
                        reason,
                    });
                    unreachable_agents += 1;
                }
            }
        }

        if !velocity_commands.is_empty() {
            session
                .entities
                .apply_batch(EntityCommandBatch::new(velocity_commands))
                .map_err(RuntimeError::EntityBatch)?;
        }
        let motion = KinematicMotionSystem::run_selected(
            &mut session.entities,
            scene,
            delta_seconds,
            &selected,
        )
        .map_err(RuntimeError::Motion)?;

        let mut advanced = BTreeSet::new();
        let mut blocked = BTreeSet::new();
        for fact in &motion.facts {
            match fact {
                MotionFact::Moved { entity, after, .. } if plans.contains_key(entity) => {
                    advanced.insert(*entity);
                    let plan = plans[entity];
                    facts.push(NavigationFact::Advanced {
                        entity: *entity,
                        before: plan.before,
                        after: *after,
                        path_hash: plan.path_hash,
                    });
                }
                MotionFact::Blocked { entity, .. } if plans.contains_key(entity) => {
                    blocked.insert(*entity);
                }
                MotionFact::Moved { .. } | MotionFact::Blocked { .. } => {}
            }
        }

        let mut arrived_agents = 0usize;
        let mut stop_commands = Vec::new();
        for (entity, plan) in plans {
            let navigation = session
                .navigators
                .get_mut(&entity)
                .expect("planned navigation component");
            if blocked.contains(&entity) {
                navigation.state = NavigationState::Blocked;
                stop_commands.push(EntityCommand::SetKinematicVelocity {
                    entity,
                    velocity: Vec3::ZERO,
                });
                facts.push(NavigationFact::Blocked {
                    entity,
                    goal: plan.goal,
                });
            } else if plan.reaches_goal {
                navigation.state = NavigationState::Arrived;
                stop_commands.push(EntityCommand::SetKinematicVelocity {
                    entity,
                    velocity: Vec3::ZERO,
                });
                facts.push(NavigationFact::Arrived {
                    entity,
                    goal: plan.goal,
                });
                arrived_agents += 1;
            } else {
                navigation.state = NavigationState::Following;
            }
        }
        if !stop_commands.is_empty() {
            session
                .entities
                .apply_batch(EntityCommandBatch::new(stop_commands))
                .map_err(RuntimeError::EntityBatch)?;
        }

        Ok(NavigationPhaseReceipt {
            agents_considered,
            advanced_agents: advanced.len(),
            arrived_agents,
            blocked_agents: blocked.len(),
            unreachable_agents,
            navigation_hash: scene.navigation_hash(),
            facts,
            motion,
        })
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
