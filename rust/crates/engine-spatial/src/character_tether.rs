use core_math::Vec3;
use entity_state::CharacterMotionComponent;

use crate::CharacterControllerError;

const LENGTH_TOLERANCE: f32 = 0.001;

/// One caller-selected attachment. A nonzero anchor identity denotes an
/// Engine-observed dynamic anchor; zero denotes an immobile world point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterTetherRequest {
    pub id: u64,
    pub local_anchor: Vec3,
    pub anchor_id: u64,
    pub anchor_valid: bool,
    pub anchor_point: Vec3,
    pub anchor_velocity: Vec3,
    pub anchor_response: [Vec3; 3],
    pub maximum_length: f32,
    pub target_length: f32,
    pub reel_speed: f32,
}

impl CharacterTetherRequest {
    pub const fn fixed(id: u64, anchor_point: Vec3, maximum_length: f32) -> Self {
        Self {
            id,
            local_anchor: Vec3::ZERO,
            anchor_id: 0,
            anchor_valid: true,
            anchor_point,
            anchor_velocity: Vec3::ZERO,
            anchor_response: [Vec3::ZERO; 3],
            maximum_length,
            target_length: maximum_length,
            reel_speed: 0.0,
        }
    }

    pub(crate) fn validate(self) -> Result<(), CharacterControllerError> {
        if ![self.maximum_length, self.target_length, self.reel_speed]
            .into_iter()
            .all(f32::is_finite)
            || self.maximum_length <= 0.0
            || self.target_length <= 0.0
            || !(0.0..=crate::MAX_TETHER_REEL_SPEED as f32).contains(&self.reel_speed)
            || ![self.local_anchor, self.anchor_point, self.anchor_velocity]
                .into_iter()
                .all(|point| point.to_array().into_iter().all(f32::is_finite))
            || !self
                .anchor_response
                .into_iter()
                .all(|column| column.to_array().into_iter().all(f32::is_finite))
            || (self.anchor_id == 0 && self.anchor_velocity != Vec3::ZERO)
        {
            return Err(CharacterControllerError::InvalidTether);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CharacterTetherFact {
    pub id: u64,
    pub attached: bool,
    pub released: bool,
    pub invalidated: bool,
    pub taut: bool,
    pub caught: bool,
    pub saturated: bool,
    pub unresolved: bool,
    pub character_point: Vec3,
    pub anchor_point: Vec3,
    pub maximum_length: f32,
    pub distance: f32,
    pub radial_velocity: f32,
    pub tangential_velocity: Vec3,
    pub correction: Vec3,
    /// Equal-and-opposite to the accepted tether velocity changes, in N*s.
    pub reaction_impulse: Vec3,
}

pub(crate) struct CharacterTetherSolve {
    pub fact: CharacterTetherFact,
    offset: Vec3,
    anchor_velocity: Vec3,
    anchor_response: [Vec3; 3],
    previous_taut: bool,
    dynamic: bool,
    remaining_velocity_change: f32,
    mass: f32,
    dt: f32,
}

impl CharacterTetherSolve {
    pub fn prepare(
        motion: &mut CharacterMotionComponent,
        request: Option<CharacterTetherRequest>,
        position: Vec3,
        offset: Vec3,
        dt: f32,
        mass: f32,
        maximum_impulse: f32,
    ) -> Result<Self, CharacterControllerError> {
        let mut solve = Self {
            fact: CharacterTetherFact {
                anchor_point: motion.tether_anchor_point,
                maximum_length: motion.tether_length,
                ..Default::default()
            },
            offset,
            anchor_velocity: Vec3::ZERO,
            anchor_response: [Vec3::ZERO; 3],
            previous_taut: false,
            dynamic: false,
            remaining_velocity_change: f32::MAX,
            mass,
            dt,
        };
        let Some(request) = request else {
            solve.fact.id = motion.tether_id;
            solve.fact.released = motion.tether_attached;
            motion.tether_attached = false;
            motion.tether_taut = false;
            return Ok(solve);
        };
        solve.fact.id = request.id;
        if !request.anchor_valid {
            solve.fact.invalidated = true;
            motion.tether_attached = false;
            motion.tether_taut = false;
            return Ok(solve);
        }
        let continuing = motion.tether_attached
            && motion.tether_id == request.id
            && motion.tether_local_anchor == request.local_anchor
            && motion.tether_anchor_id == request.anchor_id
            && (request.anchor_id != 0 || motion.tether_anchor_point == request.anchor_point);
        let distance = (position + offset - request.anchor_point).length();
        if !distance.is_finite() {
            return Err(CharacterControllerError::InvalidTether);
        }
        if !continuing && distance > request.maximum_length + LENGTH_TOLERANCE {
            return Err(CharacterControllerError::TetherOutOfReach);
        }
        let previous_length = if continuing {
            motion.tether_length
        } else {
            request.maximum_length
        };
        let length = previous_length
            + (request.target_length - previous_length)
                .clamp(-request.reel_speed * dt, request.reel_speed * dt);
        solve.previous_taut = continuing && motion.tether_taut;
        solve.dynamic = request.anchor_id != 0;
        solve.remaining_velocity_change = if solve.dynamic {
            maximum_impulse / mass
        } else {
            f32::MAX
        };
        solve.anchor_velocity = request.anchor_velocity;
        solve.anchor_response = request.anchor_response;
        solve.fact.attached = true;
        solve.fact.maximum_length = length;
        solve.fact.anchor_point = request.anchor_point + request.anchor_velocity * dt;
        motion.tether_attached = true;
        motion.tether_id = request.id;
        motion.tether_length = length;
        motion.tether_anchor_id = request.anchor_id;
        motion.tether_anchor_point = solve.fact.anchor_point;
        motion.tether_local_anchor = request.local_anchor;
        Ok(solve)
    }

    fn anchor_response(&self, impulse: Vec3) -> Vec3 {
        self.anchor_response[0] * impulse.x
            + self.anchor_response[1] * impulse.y
            + self.anchor_response[2] * impulse.z
    }

    fn coupled_change(&self, relative_change: Vec3) -> Vec3 {
        let length = relative_change.length();
        if !self.dynamic || length <= f32::EPSILON {
            return relative_change;
        }
        let direction = relative_change * (1.0 / length);
        let inverse_mass = direction.dot(self.anchor_response(direction)).max(0.0);
        relative_change * (1.0 / (1.0 + self.mass * inverse_mass))
    }

    fn apply_velocity_change(&mut self, requested: Vec3) -> Vec3 {
        let size = requested.length();
        let scale = if size > self.remaining_velocity_change && size > 0.0 {
            self.fact.saturated = true;
            self.remaining_velocity_change / size
        } else {
            1.0
        };
        let accepted = requested * scale;
        if self.dynamic {
            self.remaining_velocity_change =
                (self.remaining_velocity_change - accepted.length()).max(0.0);
            let reaction = accepted * -self.mass;
            self.fact.reaction_impulse = self.fact.reaction_impulse + reaction;
            let anchor_change = self.anchor_response(reaction);
            self.anchor_velocity = self.anchor_velocity + anchor_change;
            self.fact.anchor_point = self.fact.anchor_point + anchor_change * self.dt;
        }
        accepted
    }

    /// Project only the predicted outward displacement; tangent components are
    /// preserved about that predicted radial direction. Collision still owns
    /// how much of the resulting displacement is accepted.
    pub fn constrain_velocity(&mut self, position: Vec3, velocity: Vec3) -> Vec3 {
        if !self.fact.attached {
            return velocity;
        }
        let predicted = position + self.offset + velocity * self.dt - self.fact.anchor_point;
        let distance = predicted.length();
        if distance <= self.fact.maximum_length {
            return velocity;
        }
        let correction = predicted * (self.fact.maximum_length / distance - 1.0);
        velocity + self.apply_velocity_change(self.coupled_change(correction * (1.0 / self.dt)))
    }

    /// Floor snapping is optional adhesion, not a collision response. Do not
    /// snap outside the rope and then manufacture a compensating launch.
    pub fn admits_floor_snap(&self, position: Vec3) -> bool {
        !self.fact.attached
            || (position + self.offset - self.fact.anchor_point).length()
                <= self.fact.maximum_length + LENGTH_TOLERANCE
    }

    pub fn requested_correction(&mut self, position: Vec3, maximum_distance: f32) -> Vec3 {
        if !self.fact.attached {
            return Vec3::ZERO;
        }
        let relative = position + self.offset - self.fact.anchor_point;
        let distance = relative.length();
        let extension = (distance - self.fact.maximum_length).max(0.0);
        if extension <= LENGTH_TOLERANCE {
            return Vec3::ZERO;
        }
        let correction = self.coupled_change(relative * (-extension / distance));
        let length = correction.length();
        let impulse_limit = self.remaining_velocity_change * self.dt;
        if length > impulse_limit && self.dynamic {
            self.fact.saturated = true;
        }
        let allowed = maximum_distance.min(impulse_limit);
        if length > allowed && length > 0.0 {
            correction * (allowed / length)
        } else {
            correction
        }
    }

    pub fn accept_correction(&mut self, correction: Vec3, velocity: Vec3) -> Vec3 {
        self.fact.correction = correction;
        velocity + self.apply_velocity_change(correction * (1.0 / self.dt))
    }

    pub fn finish(
        &mut self,
        position: Vec3,
        velocity: Vec3,
        motion: &mut CharacterMotionComponent,
    ) -> Vec3 {
        self.fact.character_point = position + self.offset;
        if !self.fact.attached {
            if self.fact.released || self.fact.invalidated {
                self.fact.distance = (self.fact.character_point - self.fact.anchor_point).length();
            }
            return velocity;
        }
        let relative = self.fact.character_point - self.fact.anchor_point;
        let distance = relative.length();
        let radial = if distance > f32::EPSILON {
            relative * (1.0 / distance)
        } else {
            Vec3::ZERO
        };
        let mut velocity = velocity;
        self.fact.taut = distance >= self.fact.maximum_length - LENGTH_TOLERANCE;
        if self.fact.taut {
            let outward = (velocity - self.anchor_velocity).dot(radial).max(0.0);
            velocity =
                velocity + self.apply_velocity_change(self.coupled_change(radial * -outward));
        }
        let relative = self.fact.character_point - self.fact.anchor_point;
        let distance = relative.length();
        let radial = if distance > f32::EPSILON {
            relative * (1.0 / distance)
        } else {
            Vec3::ZERO
        };
        self.fact.taut = distance >= self.fact.maximum_length - LENGTH_TOLERANCE;
        self.fact.caught = self.fact.taut && !self.previous_taut;
        self.fact.distance = distance;
        self.fact.unresolved = distance > self.fact.maximum_length + LENGTH_TOLERANCE;
        self.fact.radial_velocity = (velocity - self.anchor_velocity).dot(radial);
        self.fact.tangential_velocity =
            velocity - self.anchor_velocity - radial * self.fact.radial_velocity;
        motion.tether_taut = self.fact.taut;
        motion.tether_anchor_point = self.fact.anchor_point;
        velocity
    }
}
