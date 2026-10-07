//! Cosmetic particle simulation for `PresentationOp::Particle`.
//!
//! Emitters are a handle-keyed table (retained emitters) plus one entry per
//! direct burst; live particles are one dense `Vec`. Nothing here reads a
//! clock: particles age only when the runtime passes Engine update time to
//! [`Particles::advance`], so a held simulation freezes every burst. The
//! descriptors carry no policy caps; each emitter's own
//! `max_particles` is the only bound.
//!
//! A billboard visual's texture is a slot in the effects cache. Each emitter
//! and each live particle holds the slot its visual resolved to (a particle
//! keeps its emitter's slot from spawn, so an updated visual does not change
//! particles already alive). `texture_users` counts those holders per slot;
//! a slot whose count reaches zero is queued in `released` for the cache to
//! drop, so ownership ends with the last user and nothing is swept per frame.

use std::collections::HashMap;
use std::sync::Arc;

use glam::Vec3;
use render_presentation::{
    ParticleAnchor, ParticleCollisionDescriptor, ParticleCollisionLimitBehavior,
    ParticleCollisionVolume, ParticleColorKey, ParticleEmitterDescriptor, ParticleEmitterHandle,
    ParticleEmitterPatch, ParticleProjectionOp, ParticleScalarKey, ParticleVisual,
};

/// Where a particle or emitter came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EmitterKey {
    Retained(ParticleEmitterHandle),
    Burst(u64),
}

struct Emitter {
    descriptor: Arc<ParticleEmitterDescriptor>,
    /// The texture slot of the current visual (`None` for cubes).
    texture: Option<u32>,
    random: u32,
    carry: f32,
    live: u32,
}

pub(crate) struct Particle {
    pub emitter: EmitterKey,
    pub descriptor: Arc<ParticleEmitterDescriptor>,
    /// The texture slot of the visual it spawned with.
    pub texture: Option<u32>,
    pub age: f32,
    pub lifetime: f32,
    pub position: Vec3,
    velocity: Vec3,
    origin: Vec3,
    impacts: u16,
    sleeping: bool,
}

/// What one particle draws this frame.
pub(crate) struct ParticleView {
    pub position: Vec3,
    pub size: f32,
    pub color: [f32; 4],
    pub frame: u32,
}

impl Particle {
    pub fn view(&self) -> ParticleView {
        let descriptor = &self.descriptor;
        let age = (self.age / self.lifetime).min(1.0);
        let frame_count = match &descriptor.visual {
            ParticleVisual::Billboard { sprite } => u32::from(sprite.frame_count.max(1)),
            ParticleVisual::Cube => 1,
        };
        let frame = if frame_count == 1 {
            0
        } else {
            (self.age * descriptor.flipbook_frames_per_second).floor() as u32 % frame_count
        };
        ParticleView {
            position: self.position,
            size: scalar_at(&descriptor.size_curve, age),
            color: color_at(&descriptor.color_curve, age),
            frame,
        }
    }
}

/// Why an op or a spawn did not fully happen. Rendering continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParticleIssue {
    DuplicateHandle,
    UnknownHandle,
    AnchorMissing,
    /// The emitter's own `max_particles` dropped this many.
    Dropped(u32),
}

#[derive(Default)]
pub(crate) struct Particles {
    emitters: HashMap<EmitterKey, Emitter>,
    pub particles: Vec<Particle>,
    next_burst: u64,
    /// Emitters and live particles holding each texture slot.
    texture_users: Vec<u32>,
    /// Slots whose last holder left, for the cache to drop.
    released: Vec<u32>,
}

fn hold(users: &mut Vec<u32>, slot: Option<u32>) {
    if let Some(slot) = slot {
        let index = slot as usize;
        if users.len() <= index {
            users.resize(index + 1, 0);
        }
        users[index] += 1;
    }
}

fn let_go(users: &mut [u32], released: &mut Vec<u32>, slot: Option<u32>) {
    if let Some(slot) = slot {
        let count = &mut users[slot as usize];
        *count -= 1;
        if *count == 0 {
            released.push(slot);
        }
    }
}

/// Resolves an entity-attached anchor: the entity's retained world position.
pub type EntityPositions<'a> = &'a dyn Fn(u64) -> Option<[f32; 3]>;

impl Particles {
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
            && !self.emitters.values().any(|emitter| {
                emitter.descriptor.visible && emitter.descriptor.rate_per_second > 0.0
            })
    }

    pub fn emitter_count(&self) -> usize {
        self.emitters
            .keys()
            .filter(|key| matches!(key, EmitterKey::Retained(_)))
            .count()
    }

    /// Whether an emitter or a live particle holds this texture slot.
    pub fn holds_texture(&self, slot: u32) -> bool {
        self.texture_users
            .get(slot as usize)
            .is_some_and(|count| *count > 0)
    }

    /// Texture slots whose last holder has left since the previous call.
    pub fn take_released_textures(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.released)
    }

    /// Apply one op. `texture` is the cache slot of the op's billboard visual
    /// (`Emit`, `Create`, or an `Update` whose patch changes the visual).
    pub fn apply(
        &mut self,
        op: &ParticleProjectionOp,
        texture: Option<u32>,
        entities: EntityPositions<'_>,
    ) -> Result<(), ParticleIssue> {
        match op {
            ParticleProjectionOp::Emit { descriptor, .. } => {
                // Each emit is its own burst; the signal id is a product label.
                self.next_burst += 1;
                let key = EmitterKey::Burst(self.next_burst);
                hold(&mut self.texture_users, texture);
                self.emitters
                    .insert(key, emitter(descriptor.clone(), texture));
                let result = self.spawn(key, descriptor.burst_count, entities);
                self.forget_finished_bursts();
                result
            }
            ParticleProjectionOp::Create { handle, descriptor } => {
                let key = EmitterKey::Retained(*handle);
                if self.emitters.contains_key(&key) {
                    return Err(ParticleIssue::DuplicateHandle);
                }
                hold(&mut self.texture_users, texture);
                self.emitters
                    .insert(key, emitter(descriptor.clone(), texture));
                self.spawn(key, descriptor.burst_count, entities)
            }
            ParticleProjectionOp::Update { handle, patch } => {
                let emitter = self
                    .emitters
                    .get_mut(&EmitterKey::Retained(*handle))
                    .ok_or(ParticleIssue::UnknownHandle)?;
                emitter.descriptor = Arc::new(patched(&emitter.descriptor, patch));
                if patch.visual.is_some() {
                    hold(&mut self.texture_users, texture);
                    let prior = std::mem::replace(&mut emitter.texture, texture);
                    let_go(&mut self.texture_users, &mut self.released, prior);
                }
                Ok(())
            }
            ParticleProjectionOp::Destroy { handle } => {
                let key = EmitterKey::Retained(*handle);
                let emitter = self
                    .emitters
                    .remove(&key)
                    .ok_or(ParticleIssue::UnknownHandle)?;
                let (users, released) = (&mut self.texture_users, &mut self.released);
                let_go(users, released, emitter.texture);
                self.particles.retain(|particle| {
                    let keep = particle.emitter != key;
                    if !keep {
                        let_go(users, released, particle.texture);
                    }
                    keep
                });
                Ok(())
            }
        }
    }

    /// Advance every emitter and particle by `seconds` of Engine update time.
    pub fn advance(&mut self, seconds: f32, entities: EntityPositions<'_>) -> Vec<ParticleIssue> {
        let mut issues = Vec::new();
        if seconds <= 0.0 || !seconds.is_finite() {
            return issues;
        }
        let mut spawns = Vec::new();
        for (key, emitter) in &mut self.emitters {
            if !emitter.descriptor.visible || matches!(key, EmitterKey::Burst(_)) {
                continue;
            }
            emitter.carry += emitter.descriptor.rate_per_second * seconds;
            let count = emitter.carry.floor();
            emitter.carry -= count;
            if count >= 1.0 {
                spawns.push((*key, count as u32));
            }
        }
        spawns.sort_by_key(|(key, _)| match key {
            EmitterKey::Retained(handle) => handle.raw(),
            EmitterKey::Burst(id) => *id,
        });
        for (key, count) in spawns {
            if let Err(issue) = self.spawn(key, count, entities) {
                issues.push(issue);
            }
        }
        let emitters = &mut self.emitters;
        let (users, released) = (&mut self.texture_users, &mut self.released);
        self.particles.retain_mut(|particle| {
            particle.age += seconds;
            let alive =
                particle.age < particle.lifetime && (particle.sleeping || !particle.step(seconds));
            if !alive {
                if let Some(emitter) = emitters.get_mut(&particle.emitter) {
                    emitter.live = emitter.live.saturating_sub(1);
                }
                let_go(users, released, particle.texture);
            }
            alive
        });
        self.forget_finished_bursts();
        issues
    }

    fn forget_finished_bursts(&mut self) {
        let (users, released) = (&mut self.texture_users, &mut self.released);
        self.emitters.retain(|key, emitter| {
            let keep = matches!(key, EmitterKey::Retained(_)) || emitter.live > 0;
            if !keep {
                let_go(users, released, emitter.texture);
            }
            keep
        });
    }

    fn spawn(
        &mut self,
        key: EmitterKey,
        requested: u32,
        entities: EntityPositions<'_>,
    ) -> Result<(), ParticleIssue> {
        let Some(emitter) = self.emitters.get_mut(&key) else {
            return Ok(());
        };
        if requested == 0 || !emitter.descriptor.visible {
            return Ok(());
        }
        let anchor = match &emitter.descriptor.anchor {
            ParticleAnchor::World { position } => crate::convert::vec3(*position),
            ParticleAnchor::EntityAttached { entity, offset } => {
                let base = entities(*entity).ok_or(ParticleIssue::AnchorMissing)?;
                crate::convert::vec3(base) + crate::convert::vec3(*offset)
            }
        };
        let room = emitter
            .descriptor
            .max_particles
            .saturating_sub(emitter.live);
        let count = requested.min(room);
        let texture = emitter.texture;
        if let Some(slot) = texture {
            let index = slot as usize;
            if self.texture_users.len() <= index {
                self.texture_users.resize(index + 1, 0);
            }
            self.texture_users[index] += count;
        }
        self.particles.reserve(count as usize);
        for _ in 0..count {
            let descriptor = emitter.descriptor.clone();
            let [low, high] = descriptor.lifetime_seconds;
            let lifetime = random_range(&mut emitter.random, low, high);
            let velocity = crate::convert::vec3(std::array::from_fn(|axis| {
                random_range(
                    &mut emitter.random,
                    descriptor.velocity_min[axis],
                    descriptor.velocity_max[axis],
                )
            }));
            self.particles.push(Particle {
                emitter: key,
                descriptor,
                texture,
                age: 0.0,
                lifetime,
                position: anchor,
                velocity,
                origin: anchor,
                impacts: 0,
                sleeping: false,
            });
        }
        emitter.live += count;
        if count < requested {
            return Err(ParticleIssue::Dropped(requested - count));
        }
        Ok(())
    }
}

impl Particle {
    /// Integrate one step. Returns true when a collision limit kills it.
    fn step(&mut self, seconds: f32) -> bool {
        self.velocity += crate::convert::vec3(self.descriptor.acceleration) * seconds;
        let descriptor = self.descriptor.clone();
        match &descriptor.collision {
            None => {
                self.position += self.velocity * seconds;
                false
            }
            Some(collision) => self.step_with_collision(collision, seconds),
        }
    }

    fn step_with_collision(
        &mut self,
        collision: &ParticleCollisionDescriptor,
        seconds: f32,
    ) -> bool {
        let mut remaining = seconds;
        let mut iterations = 0;
        while remaining > 1e-6 && iterations < 4 {
            iterations += 1;
            let start = self.position - self.origin;
            let end = start + self.velocity * remaining;
            let earliest = collision
                .volumes
                .iter()
                .filter_map(|volume| sweep(start, end, collision.radius, volume))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((time, normal)) = earliest else {
                self.position += self.velocity * remaining;
                return false;
            };
            self.position += self.velocity * (time * remaining).max(0.0) + normal * 1e-4;
            remaining *= (1.0 - time).max(0.0);
            let normal_speed = self.velocity.dot(normal);
            if normal_speed < 0.0 {
                let normal_part = normal * normal_speed;
                self.velocity = (self.velocity - normal_part) * (1.0 - collision.friction)
                    - normal_part * collision.restitution;
            }
            self.impacts = self.impacts.saturating_add(1);
            if self.impacts >= collision.maximum_impacts {
                if collision.limit_behavior == ParticleCollisionLimitBehavior::Kill {
                    return true;
                }
                self.velocity = Vec3::ZERO;
                self.sleeping = true;
                return false;
            }
            if self.velocity.length() <= collision.sleep_speed {
                self.velocity = Vec3::ZERO;
                self.sleeping = true;
                return false;
            }
        }
        self.position += self.velocity * remaining;
        false
    }
}

/// Earliest hit on a swept sphere: (fraction of the step, surface normal).
fn sweep(
    start: Vec3,
    end: Vec3,
    radius: f32,
    volume: &ParticleCollisionVolume,
) -> Option<(f32, Vec3)> {
    match volume {
        ParticleCollisionVolume::Plane { normal, offset } => {
            let normal = crate::convert::vec3(*normal);
            let start_distance = normal.dot(start) - offset - radius;
            let end_distance = normal.dot(end) - offset - radius;
            if start_distance < 0.0 {
                return Some((0.0, normal));
            }
            if end_distance >= 0.0 || start_distance == end_distance {
                return None;
            }
            Some((start_distance / (start_distance - end_distance), normal))
        }
        ParticleCollisionVolume::Aabb { minimum, maximum } => {
            let minimum = crate::convert::vec3(*minimum) - Vec3::splat(radius);
            let maximum = crate::convert::vec3(*maximum) + Vec3::splat(radius);
            sweep_aabb(start, end, minimum, maximum)
        }
    }
}

fn sweep_aabb(start: Vec3, end: Vec3, minimum: Vec3, maximum: Vec3) -> Option<(f32, Vec3)> {
    let inside = (0..3).all(|axis| start[axis] >= minimum[axis] && start[axis] <= maximum[axis]);
    if inside {
        // Leave through the nearest face.
        let mut best = (f32::INFINITY, Vec3::Y);
        for axis in 0..3 {
            let low = start[axis] - minimum[axis];
            if low < best.0 {
                best = (low, -Vec3::AXES[axis]);
            }
            let high = maximum[axis] - start[axis];
            if high < best.0 {
                best = (high, Vec3::AXES[axis]);
            }
        }
        return Some((0.0, best.1));
    }
    let (mut enter, mut exit, mut normal) = (0.0_f32, 1.0_f32, Vec3::ZERO);
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        if delta.abs() < 1e-9 {
            if start[axis] < minimum[axis] || start[axis] > maximum[axis] {
                return None;
            }
            continue;
        }
        let inverse = 1.0 / delta;
        let mut first = (minimum[axis] - start[axis]) * inverse;
        let mut second = (maximum[axis] - start[axis]) * inverse;
        if first > second {
            std::mem::swap(&mut first, &mut second);
        }
        if first > enter {
            enter = first;
            normal = Vec3::AXES[axis] * -delta.signum();
        }
        exit = exit.min(second);
        if enter > exit {
            return None;
        }
    }
    (0.0..=1.0).contains(&enter).then_some((enter, normal))
}

fn emitter(descriptor: ParticleEmitterDescriptor, texture: Option<u32>) -> Emitter {
    let seed = descriptor.seed as u32;
    Emitter {
        texture,
        random: if seed == 0 { 0x9e37_79b9 } else { seed },
        descriptor: Arc::new(descriptor),
        carry: 0.0,
        live: 0,
    }
}

/// xorshift32: the same seed gives the same burst.
fn random_range(state: &mut u32, low: f32, high: f32) -> f32 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    *state = value;
    low + (high - low) * (f64::from(value) / 4_294_967_296.0) as f32
}

fn curve_pair<T>(keys: &[T], age: f32, key_age: impl Fn(&T) -> f32) -> Option<(&T, &T)> {
    for pair in keys.windows(2) {
        if age <= key_age(&pair[1]) {
            return Some((&pair[0], &pair[1]));
        }
    }
    keys.last().map(|last| (last, last))
}

fn blend(start: f32, end: f32, age: f32) -> f32 {
    if end == start {
        0.0
    } else {
        (age - start) / (end - start)
    }
}

fn scalar_at(keys: &[ParticleScalarKey], age: f32) -> f32 {
    curve_pair(keys, age, |key| key.age).map_or(1.0, |(left, right)| {
        left.value + (right.value - left.value) * blend(left.age, right.age, age)
    })
}

fn color_at(keys: &[ParticleColorKey], age: f32) -> [f32; 4] {
    curve_pair(keys, age, |key| key.age).map_or([1.0; 4], |(left, right)| {
        let amount = blend(left.age, right.age, age);
        std::array::from_fn(|channel| {
            left.color[channel] + (right.color[channel] - left.color[channel]) * amount
        })
    })
}

fn patched(
    descriptor: &ParticleEmitterDescriptor,
    patch: &ParticleEmitterPatch,
) -> ParticleEmitterDescriptor {
    let visual = match (&patch.visual, &patch.sprite) {
        (Some(visual), _) => visual.clone(),
        (None, Some(sprite)) => ParticleVisual::Billboard {
            sprite: sprite.clone(),
        },
        (None, None) => descriptor.visual.clone(),
    };
    ParticleEmitterDescriptor {
        anchor: patch
            .anchor
            .clone()
            .unwrap_or_else(|| descriptor.anchor.clone()),
        visual,
        size_mode: patch.size_mode.unwrap_or(descriptor.size_mode),
        blend: patch.blend.unwrap_or(descriptor.blend),
        softness_metres: patch.softness_metres.unwrap_or(descriptor.softness_metres),
        rate_per_second: patch.rate_per_second.unwrap_or(descriptor.rate_per_second),
        burst_count: patch.burst_count.unwrap_or(descriptor.burst_count),
        lifetime_seconds: patch
            .lifetime_seconds
            .unwrap_or(descriptor.lifetime_seconds),
        velocity_min: patch.velocity_min.unwrap_or(descriptor.velocity_min),
        velocity_max: patch.velocity_max.unwrap_or(descriptor.velocity_max),
        acceleration: patch.acceleration.unwrap_or(descriptor.acceleration),
        size_curve: patch
            .size_curve
            .clone()
            .unwrap_or_else(|| descriptor.size_curve.clone()),
        color_curve: patch
            .color_curve
            .clone()
            .unwrap_or_else(|| descriptor.color_curve.clone()),
        flipbook_frames_per_second: patch
            .flipbook_frames_per_second
            .unwrap_or(descriptor.flipbook_frames_per_second),
        seed: descriptor.seed,
        max_particles: patch.max_particles.unwrap_or(descriptor.max_particles),
        visible: patch.visible.unwrap_or(descriptor.visible),
        collision: match &patch.collision {
            Some(collision) => collision.clone(),
            None => descriptor.collision.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burst(count: u32) -> ParticleEmitterDescriptor {
        ParticleEmitterDescriptor {
            anchor: ParticleAnchor::World {
                position: [0.0, 1.0, 0.0],
            },
            visual: ParticleVisual::Cube,
            size_mode: Default::default(),
            blend: Default::default(),
            softness_metres: 0.0,
            rate_per_second: 0.0,
            burst_count: count,
            lifetime_seconds: [1.0, 2.0],
            velocity_min: [-1.0, 2.0, -1.0],
            velocity_max: [1.0, 3.0, 1.0],
            acceleration: [0.0, -9.8, 0.0],
            size_curve: vec![
                ParticleScalarKey {
                    age: 0.0,
                    value: 0.2,
                },
                ParticleScalarKey {
                    age: 1.0,
                    value: 0.0,
                },
            ],
            color_curve: vec![ParticleColorKey {
                age: 0.0,
                color: [1.0, 0.5, 0.0, 1.0],
            }],
            flipbook_frames_per_second: 0.0,
            seed: 7,
            max_particles: 64,
            visible: true,
            collision: None,
        }
    }

    const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;

    fn emit(
        particles: &mut Particles,
        descriptor: ParticleEmitterDescriptor,
    ) -> Result<(), ParticleIssue> {
        particles.apply(
            &ParticleProjectionOp::Emit {
                signal_id: "hit".to_owned(),
                descriptor,
            },
            None,
            NO_ENTITIES,
        )
    }

    #[test]
    fn a_burst_ages_only_on_engine_time_and_is_seeded() {
        let mut particles = Particles::default();
        emit(&mut particles, burst(16)).unwrap();
        assert_eq!(particles.particles.len(), 16);
        let first: Vec<Vec3> = particles.particles.iter().map(|p| p.velocity).collect();
        // Held: no Engine time passes, nothing moves.
        particles.advance(0.0, NO_ENTITIES);
        assert!(particles
            .particles
            .iter()
            .all(|p| p.age == 0.0 && p.position == Vec3::Y));
        particles.advance(0.1, NO_ENTITIES);
        assert!(particles.particles.iter().all(|p| p.position.y > 1.0));
        // The same seed spawns the same burst.
        let mut again = Particles::default();
        emit(&mut again, burst(16)).unwrap();
        let second: Vec<Vec3> = again.particles.iter().map(|p| p.velocity).collect();
        assert_eq!(first, second);
        // Past the longest lifetime the burst and its emitter are gone.
        particles.advance(2.5, NO_ENTITIES);
        assert!(particles.particles.is_empty());
        assert!(particles.emitters.is_empty());
    }

    #[test]
    fn a_two_second_advance_ages_effects_fully() {
        // A two-second advance arrives as 120 admitted 60 Hz steps (an
        // inspection advance, or a bounded gameplay advance) or as one long
        // update; either way every particle ages the full duration and a
        // continuous emitter spawns for all of it.
        let run = |deltas: &[f32]| {
            let mut particles = Particles::default();
            emit(&mut particles, burst(16)).unwrap();
            let mut descriptor = burst(0);
            descriptor.rate_per_second = 4.0;
            descriptor.lifetime_seconds = [10.0, 10.0];
            particles
                .apply(
                    &ParticleProjectionOp::Create {
                        handle: ParticleEmitterHandle::new(5),
                        descriptor,
                    },
                    None,
                    NO_ENTITIES,
                )
                .unwrap();
            for delta in deltas {
                particles.advance(*delta, NO_ENTITIES);
            }
            let mut ages = particles
                .particles
                .iter()
                .map(|p| p.age)
                .collect::<Vec<_>>();
            ages.sort_by(f32::total_cmp);
            ages
        };
        let stepped = run(&[1.0 / 60.0; 120]);
        let whole = run(&[2.0]);
        // Every burst particle (lifetime at most 2 s) is gone in both, and
        // the emitter spawned its eight. Stepped, their ages spread over the
        // advance as they were spawned through it, the oldest nearly 2 s.
        assert_eq!(stepped.len(), 8);
        assert_eq!(whole.len(), 8);
        let oldest = stepped.last().copied().unwrap();
        assert!((1.7..=2.0 + 1e-3).contains(&oldest), "oldest {oldest}");
        assert!(stepped.first().copied().unwrap() < 0.3);
    }

    #[test]
    fn continuous_emitters_spawn_on_their_rate_and_respect_their_bound() {
        let mut particles = Particles::default();
        let mut descriptor = burst(0);
        descriptor.rate_per_second = 10.0;
        descriptor.max_particles = 5;
        let handle = ParticleEmitterHandle::new(3);
        particles
            .apply(
                &ParticleProjectionOp::Create { handle, descriptor },
                None,
                NO_ENTITIES,
            )
            .unwrap();
        particles.advance(0.25, NO_ENTITIES);
        assert_eq!(particles.particles.len(), 2);
        let issues = particles.advance(0.5, NO_ENTITIES);
        assert_eq!(particles.particles.len(), 5);
        assert_eq!(issues, vec![ParticleIssue::Dropped(2)]);
        particles
            .apply(&ParticleProjectionOp::Destroy { handle }, None, NO_ENTITIES)
            .unwrap();
        assert!(particles.particles.is_empty());
    }

    #[test]
    fn entity_anchors_resolve_through_the_caller_and_planes_stop_particles() {
        let mut particles = Particles::default();
        let mut descriptor = burst(4);
        descriptor.anchor = ParticleAnchor::EntityAttached {
            entity: 9,
            offset: [0.0, 0.5, 0.0],
        };
        descriptor.collision = Some(ParticleCollisionDescriptor {
            radius: 0.0,
            restitution: 0.0,
            friction: 1.0,
            maximum_impacts: 1,
            sleep_speed: 0.1,
            limit_behavior: ParticleCollisionLimitBehavior::Sleep,
            volumes: vec![ParticleCollisionVolume::Plane {
                normal: [0.0, 1.0, 0.0],
                offset: -0.5,
            }],
        });
        assert_eq!(
            emit(&mut particles, descriptor.clone()),
            Err(ParticleIssue::AnchorMissing)
        );
        particles
            .apply(
                &ParticleProjectionOp::Emit {
                    signal_id: "hit".to_owned(),
                    descriptor,
                },
                None,
                &|entity| (entity == 9).then_some([2.0, 0.0, 0.0]),
            )
            .unwrap();
        for _ in 0..20 {
            particles.advance(0.05, NO_ENTITIES);
        }
        // Relative to the spawn anchor (y 0.5), the plane sits at y -0.5.
        assert!(particles
            .particles
            .iter()
            .all(|p| p.position.y >= -0.001 && p.sleeping));
    }
}
