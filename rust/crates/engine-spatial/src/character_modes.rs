//! Environment-selected movement using the ordinary character collision sweep.
use crate::CharacterControllerError;
use core_math::Vec3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CharacterMovementMode {
    #[default]
    Walking,
    Swimming,
    Climbing,
    Flying,
}

/// Copied, call-local environment facts. Products select the water body or
/// climb rail; the Engine derives overlap, motion and collision constraints.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CharacterMovementRequest {
    pub mode: CharacterMovementMode,
    pub vertical_intent: f32,
    pub speed: f32,
    pub acceleration: f32,
    pub drag: f32,
    /// Water AABB for swimming; bottom/top endpoints of a vertical climb rail.
    pub minimum: Vec3,
    pub maximum: Vec3,
    /// Gravity fraction in water; buoyancy acceleration is gravity * buoyancy * immersion.
    pub gravity_scale: f32,
    pub buoyancy: f32,
    pub climb_reach: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CharacterMovementFact {
    pub mode: CharacterMovementMode,
    pub immersion: f32,
    pub head_submerged: bool,
    pub climb_attached: bool,
    pub climb_at_bottom: bool,
    pub climb_at_top: bool,
}

impl CharacterMovementRequest {
    pub fn validate(self) -> Result<(), CharacterControllerError> {
        if self.mode == CharacterMovementMode::Walking {
            return Ok(());
        }
        let values = [
            self.vertical_intent,
            self.speed,
            self.acceleration,
            self.drag,
            self.minimum.x,
            self.minimum.y,
            self.minimum.z,
            self.maximum.x,
            self.maximum.y,
            self.maximum.z,
            self.gravity_scale,
            self.buoyancy,
            self.climb_reach,
        ];
        if !values.into_iter().all(f32::is_finite)
            || self.vertical_intent.abs() > 1.0
            || self.speed < 0.0
            || self.acceleration < 0.0
            || self.drag < 0.0
            || self.gravity_scale < 0.0
            || self.buoyancy < 0.0
        {
            return Err(CharacterControllerError::InvalidCommand);
        }
        match self.mode {
            CharacterMovementMode::Swimming
                if self.minimum.x >= self.maximum.x
                    || self.minimum.y >= self.maximum.y
                    || self.minimum.z >= self.maximum.z =>
            {
                return Err(CharacterControllerError::InvalidCommand)
            }
            CharacterMovementMode::Climbing
                if self.minimum.y >= self.maximum.y
                    || self.minimum.x != self.maximum.x
                    || self.minimum.z != self.maximum.z
                    || self.climb_reach <= 0.0 =>
            {
                return Err(CharacterControllerError::InvalidCommand)
            }
            _ => {}
        }
        Ok(())
    }

    pub fn observe(self, center: Vec3, height: f32) -> CharacterMovementFact {
        let mut fact = CharacterMovementFact::default();
        match self.mode {
            CharacterMovementMode::Swimming => {
                if center.x >= self.minimum.x
                    && center.x <= self.maximum.x
                    && center.z >= self.minimum.z
                    && center.z <= self.maximum.z
                {
                    let bottom = center.y - height * 0.5;
                    let top = center.y + height * 0.5;
                    fact.immersion = ((top.min(self.maximum.y) - bottom.max(self.minimum.y))
                        / height)
                        .clamp(0.0, 1.0);
                    fact.head_submerged = top < self.maximum.y && top > self.minimum.y;
                    if fact.immersion > 0.0 {
                        fact.mode = self.mode;
                    }
                }
            }
            CharacterMovementMode::Climbing => {
                let offset = Vec3::new(center.x - self.minimum.x, 0.0, center.z - self.minimum.z);
                fact.climb_attached = offset.length() <= self.climb_reach
                    && center.y >= self.minimum.y
                    && center.y <= self.maximum.y;
                if fact.climb_attached {
                    fact.mode = self.mode;
                    fact.climb_at_bottom = center.y <= self.minimum.y + 1.0e-4;
                    fact.climb_at_top = center.y >= self.maximum.y - 1.0e-4;
                }
            }
            CharacterMovementMode::Flying => fact.mode = self.mode,
            CharacterMovementMode::Walking => {}
        }
        fact
    }

    pub(crate) fn velocity(
        self,
        fact: CharacterMovementFact,
        center: Vec3,
        current: Vec3,
        planar_direction: Vec3,
        gravity: f32,
        dt: f32,
    ) -> Vec3 {
        if fact.mode == CharacterMovementMode::Climbing {
            // Approach the rail through the collision sweep, never teleport to it.
            let offset = Vec3::new(self.minimum.x - center.x, 0.0, self.minimum.z - center.z);
            let planar = limit(offset * (1.0 / dt), self.speed);
            let target_y = (center.y + self.vertical_intent * self.speed * dt)
                .clamp(self.minimum.y, self.maximum.y);
            return Vec3::new(planar.x, (target_y - center.y) / dt, planar.z);
        }
        let direction = limit(
            Vec3::new(planar_direction.x, self.vertical_intent, planar_direction.z),
            1.0,
        );
        let target = direction * self.speed;
        let mut velocity = current + limit(target - current, self.acceleration * dt);
        if fact.mode == CharacterMovementMode::Swimming {
            velocity.y += gravity * (self.buoyancy * fact.immersion - self.gravity_scale) * dt;
            velocity = velocity * (-self.drag * fact.immersion * dt).exp();
        } else {
            velocity = velocity * (-self.drag * dt).exp();
        }
        velocity
    }
}
fn limit(value: Vec3, maximum: f32) -> Vec3 {
    let length = value.length();
    if length > maximum && length > 0.0 {
        value * (maximum / length)
    } else {
        value
    }
}
