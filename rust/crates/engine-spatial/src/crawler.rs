//! Surface crawling: a ball-bodied mover whose up direction follows the
//! surface under it, across floors, walls and ceilings.
//!
//! The ball is the whole collision body, so it never turns; the direction it
//! treats as up is a fact of where it touches. Each sub-step moves it along
//! the surface's tangent plane at most half its radius:
//!
//! - **Concave edges.** The move is a swept ball. Meeting a surface it may
//!   hold, it takes that surface's normal as up and turns the rest of its
//!   move, its velocity and its heading by the same rotation, so a move along
//!   a floor into a wall carries on up the wall.
//! - **Sticking.** After the move a swept ball along −up finds the surface it
//!   stands on and snaps to it, its skin off it, taking its normal as up.
//! - **Convex edges.** Past a lip the −up sweep finds nothing; the nearest
//!   surface within reach is then the edge itself, so up turns about the edge
//!   and the ball orbits it onto the face beyond in a few sub-steps.
//! - **Letting go.** Without `cling` it holds only surfaces gravity holds it
//!   on (a floor within the maximum slope); elsewhere it falls under gravity
//!   and holds the first such surface it lands on. With nothing within reach
//!   it is lost and falls.
//!
//! Nothing is retained: the product holds [`CrawlerMotion`] between steps.

use core_math::Vec3;
use core_space::{WorldPos, WorldVec};
use entity_state::Quat;
use svc_collision::{CharacterCapsule, CharacterCollisionQueryError, CharacterCollisionSource};

use crate::VoxelCollisionScene;

/// The longest interval one sub-step integrates.
const SUB_STEP_SECONDS: f64 = 1.0 / 60.0;
/// The most sub-steps one command is solved as.
const MAXIMUM_SUB_STEPS: u32 = 256;
/// A sub-step moves at most this fraction of the radius, so an edge is
/// rounded in several turns rather than skipped.
const SUB_STEP_RADIUS_FRACTION: f64 = 0.5;
/// How many surfaces one sub-step's move may meet in turn.
const MOVE_ITERATIONS: usize = 4;
/// How many push-outs a command's start makes before it gives up.
const RECOVERY_PASSES: usize = 4;
/// A turn of up smaller than this is following the surface, not an edge.
const EDGE_RADIANS: f64 = 0.02;
const EPSILON: f64 = 1.0e-9;
/// The least a grazing move steps off the surface it grazes, so a ball with
/// no skin (resting touching it) still clears it.
const MINIMUM_GRAZE_STEP: f64 = 1.0e-4;

/// Crawler body and movement. Construct it with [`Default`] and change
/// fields.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct CrawlerConfig {
    /// The ball that is the crawler's whole collision body, in metres.
    pub radius: f32,
    /// How far off a surface the ball rests.
    pub contact_skin: f32,
    /// Top speed along a surface, metres per second.
    pub speed: f32,
    /// Speed gained each second toward a commanded direction.
    pub acceleration: f32,
    /// Speed lost each second with no direction commanded.
    pub braking: f32,
    /// Downward acceleration while falling.
    pub gravity: f32,
    pub terminal_fall_speed: f32,
    /// How far beyond its skin a surface may fall away (a convex edge, a dip)
    /// and still be held. More than the ball's sub-step move is turned
    /// through at a lip, which is about an eighth of the radius.
    pub stick_distance: f32,
    /// A surface tilted at most this far from level is a floor: a crawler
    /// without `cling` holds it, and the receipt names it. As far from upside
    /// down, it is a ceiling; between, a wall.
    pub floor_maximum_slope_radians: f32,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            radius: 0.25,
            contact_skin: 0.02,
            speed: 4.0,
            acceleration: 30.0,
            braking: 40.0,
            gravity: 20.0,
            terminal_fall_speed: 55.0,
            stick_distance: 0.1,
            floor_maximum_slope_radians: 50.0_f32.to_radians(),
        }
    }
}

/// One command to a crawler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrawlerCommand {
    /// Where to go, in the world; projected onto the surface it is on, so a
    /// direction toward a point up a wall drives a crawler on the floor into
    /// the wall and up it. Longer than one is one; zero brakes.
    pub direction: Vec3,
    /// Hold walls and ceilings. Without it a crawler on one falls, and holds
    /// only floors.
    pub cling: bool,
    /// The interval this command covers, from 1 ms to 1 s.
    pub step_seconds: f32,
    pub sequence: u64,
}

/// What a crawler carries from one step to the next. The product holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrawlerMotion {
    /// On a surface rather than falling.
    pub attached: bool,
    /// The normal of the surface it is on: the body's up. A falling crawler
    /// keeps the last.
    pub up: Vec3,
    /// The body's heading, along the surface.
    pub forward: Vec3,
    pub velocity: Vec3,
    pub last_sequence: u64,
}

impl Default for CrawlerMotion {
    /// A crawler not yet on a surface, upright, facing +Z. Its first step
    /// holds a surface within reach or falls onto one.
    fn default() -> Self {
        Self {
            attached: false,
            up: Vec3::new(0.0, 1.0, 0.0),
            forward: Vec3::new(0.0, 0.0, 1.0),
            velocity: Vec3::ZERO,
            last_sequence: 0,
        }
    }
}

/// Which kind of surface a crawler is on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrawlerSurface {
    /// Falling.
    #[default]
    None,
    Floor,
    Wall,
    Ceiling,
}

/// What became of the crawler's hold during a step: the last thing that
/// happened, if anything did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrawlerTransition {
    #[default]
    None,
    /// Moved onto a surface it met, as from a floor up a wall.
    Concave,
    /// Rounded an edge onto the surface beyond, as over a lip.
    Convex,
    /// Let go of a surface it holds only with `cling`.
    Detached,
    /// Took hold of a surface after falling.
    Landed,
    /// Nothing within reach to hold; falling.
    Lost,
}

/// The surface a crawler is on after its step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrawlerContact {
    pub source: CharacterCollisionSource,
    pub point: Vec3,
    pub normal: Vec3,
    /// From the ball's centre to the surface.
    pub distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrawlerStepReceipt {
    /// The ball's centre.
    pub position: Vec3,
    /// Turns +Y to `motion.up` and +Z to `motion.forward`, to draw the body.
    pub rotation: Quat,
    pub surface: CrawlerSurface,
    pub transition: CrawlerTransition,
    /// Present while attached.
    pub contact: Option<CrawlerContact>,
    pub motion: CrawlerMotion,
    pub displacement: Vec3,
    pub sub_steps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrawlerError {
    InvalidConfig {
        field: &'static str,
    },
    InvalidCommand,
    InvalidMotion,
    /// The ball overlaps collision too deeply to push out of.
    UnresolvedPenetration {
        depth: f32,
    },
    Collision(CharacterCollisionQueryError),
}

impl CrawlerError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig { .. } => "invalid-crawler-config",
            Self::InvalidCommand => "invalid-crawler-command",
            Self::InvalidMotion => "invalid-crawler-motion",
            Self::UnresolvedPenetration { .. } => "unresolved-crawler-penetration",
            Self::Collision(_) => "crawler-collision-query-failed",
        }
    }
}

impl std::fmt::Display for CrawlerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for CrawlerError {}

impl From<CharacterCollisionQueryError> for CrawlerError {
    fn from(value: CharacterCollisionQueryError) -> Self {
        Self::Collision(value)
    }
}

impl CrawlerConfig {
    pub fn validate(&self) -> Result<(), CrawlerError> {
        let invalid = |field| Err(CrawlerError::InvalidConfig { field });
        if !(self.radius.is_finite() && self.radius > 0.0) {
            return invalid("radius");
        }
        if !(self.contact_skin.is_finite()
            && self.contact_skin >= 0.0
            && self.contact_skin < self.radius)
        {
            return invalid("contactSkin");
        }
        if !(self.stick_distance.is_finite() && self.stick_distance > 0.0) {
            return invalid("stickDistance");
        }
        for (field, value) in [
            ("speed", self.speed),
            ("acceleration", self.acceleration),
            ("braking", self.braking),
            ("gravity", self.gravity),
            ("terminalFallSpeed", self.terminal_fall_speed),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return invalid(field);
            }
        }
        if !(0.0..=std::f32::consts::FRAC_PI_2).contains(&self.floor_maximum_slope_radians) {
            return invalid("floorMaximumSlopeRadians");
        }
        Ok(())
    }
}

/// A small f64 vector for the solve; the scene is f64 and the boundary f32.
#[derive(Debug, Clone, Copy, PartialEq)]
struct V([f64; 3]);

impl V {
    const ZERO: V = V([0.0; 3]);
    const Y: V = V([0.0, 1.0, 0.0]);

    fn of(value: Vec3) -> Self {
        V([f64::from(value.x), f64::from(value.y), f64::from(value.z)])
    }
    fn world(value: WorldVec) -> Self {
        V([value.x, value.y, value.z])
    }
    fn at(value: WorldPos) -> Self {
        V([value.x, value.y, value.z])
    }
    fn vec3(self) -> Vec3 {
        Vec3::new(self.0[0] as f32, self.0[1] as f32, self.0[2] as f32)
    }
    fn pos(self) -> WorldPos {
        WorldPos::new(self.0[0], self.0[1], self.0[2])
    }
    fn wvec(self) -> WorldVec {
        WorldVec::new(self.0[0], self.0[1], self.0[2])
    }
    fn add(self, o: V) -> V {
        V([self.0[0] + o.0[0], self.0[1] + o.0[1], self.0[2] + o.0[2]])
    }
    fn sub(self, o: V) -> V {
        V([self.0[0] - o.0[0], self.0[1] - o.0[1], self.0[2] - o.0[2]])
    }
    fn mul(self, s: f64) -> V {
        V([self.0[0] * s, self.0[1] * s, self.0[2] * s])
    }
    fn dot(self, o: V) -> f64 {
        self.0[0] * o.0[0] + self.0[1] * o.0[1] + self.0[2] * o.0[2]
    }
    fn cross(self, o: V) -> V {
        V([
            self.0[1] * o.0[2] - self.0[2] * o.0[1],
            self.0[2] * o.0[0] - self.0[0] * o.0[2],
            self.0[0] * o.0[1] - self.0[1] * o.0[0],
        ])
    }
    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    fn normalized(self) -> Option<V> {
        let length = self.length();
        (length > EPSILON).then(|| self.mul(1.0 / length))
    }
    /// The part of `self` along the plane whose normal is `normal`.
    fn along_plane(self, normal: V) -> V {
        self.sub(normal.mul(self.dot(normal)))
    }
    fn limited(self, maximum: f64) -> V {
        let length = self.length();
        if length > maximum && length > 0.0 {
            self.mul(maximum / length)
        } else {
            self
        }
    }
    fn finite(self) -> bool {
        self.0.iter().all(|value| value.is_finite())
    }
}

/// The rotation taking unit `from` to unit `to`, applied to `value`
/// (Rodrigues). Opposite vectors turn half about `fallback_axis`.
fn rotate(value: V, from: V, to: V, fallback_axis: V) -> V {
    let cos = from.dot(to).clamp(-1.0, 1.0);
    let axis = from.cross(to);
    let sin = axis.length();
    let axis = if sin > 1.0e-6 {
        axis.mul(1.0 / sin)
    } else if cos > 0.0 {
        return value;
    } else {
        fallback_axis
            .along_plane(from)
            .normalized()
            .or_else(|| any_perpendicular(from).normalized())
            .unwrap_or(V::Y)
    };
    let sin = if sin > 1.0e-6 { sin } else { 0.0 };
    value
        .mul(cos)
        .add(axis.cross(value).mul(sin))
        .add(axis.mul(axis.dot(value) * (1.0 - cos)))
}

fn any_perpendicular(value: V) -> V {
    if value.0[0].abs() < 0.9 {
        value.cross(V([1.0, 0.0, 0.0]))
    } else {
        value.cross(V([0.0, 0.0, 1.0]))
    }
}

/// The solve's state through the sub-steps.
struct Crawl<'a> {
    scene: &'a VoxelCollisionScene,
    config: &'a CrawlerConfig,
    cling: bool,
    radius: f64,
    skin: f64,
    floor_cos: f64,
    position: V,
    up: V,
    forward: V,
    velocity: V,
    attached: bool,
    transition: CrawlerTransition,
    contact: Option<(CharacterCollisionSource, V, V)>,
}

/// One swept-ball hit: how far along the move, and the surface's normal.
struct Hit {
    fraction: f64,
    normal: V,
    source: CharacterCollisionSource,
    point: V,
}

impl Crawl<'_> {
    fn ball(&self, at: V) -> CharacterCapsule {
        CharacterCapsule {
            center: at.pos(),
            half_height: 0.0,
            radius: self.radius,
        }
    }

    /// A move sweeps to half the skin, so a ball resting its skin off a
    /// surface slides along it rather than meeting it at once; sticking
    /// sweeps to the whole skin.
    fn sweep(&self, translation: V, target: f64) -> Result<Option<Hit>, CrawlerError> {
        if translation.length() <= EPSILON {
            return Ok(None);
        }
        Ok(self
            .scene
            .cast_character_capsule(self.ball(self.position), translation.wvec(), target)?
            .and_then(|hit| {
                Some(Hit {
                    fraction: hit.time_of_impact.clamp(0.0, 1.0),
                    normal: V::world(hit.normal).normalized()?,
                    source: hit.source,
                    point: V::at(hit.point),
                })
            }))
    }

    /// A hit on a surface the move runs along or leaves, not into: the ball
    /// starts within the sweep's distance of it. It steps off to its full
    /// skin, or a tenth of a millimetre with less, and the move is swept
    /// again; sticking brings it back.
    fn grazes(&mut self, hit: &Hit, translation: V) -> bool {
        if hit.normal.dot(translation) < -1.0e-6 * translation.length() {
            return false;
        }
        let step = (self.skin * 0.5).max(MINIMUM_GRAZE_STEP);
        self.position = self.position.add(hit.normal.mul(step));
        true
    }

    fn holds(&self, normal: V) -> bool {
        self.cling || normal.0[1] >= self.floor_cos - 1.0e-6
    }

    /// Takes `normal` as up, turning the heading, the velocity and
    /// `carried` with it; true when that turn is an edge.
    fn turn_up(&mut self, normal: V, carried: &mut V) -> bool {
        let from = self.up;
        let fallback = self.forward.cross(from);
        self.velocity = rotate(self.velocity, from, normal, fallback);
        self.forward = rotate(self.forward, from, normal, fallback);
        *carried = rotate(*carried, from, normal, fallback);
        self.up = normal;
        from.dot(normal) < EDGE_RADIANS.cos()
    }

    /// Pushes the ball out of collision it overlaps, along the nearest
    /// surface's normal.
    fn recover(&mut self) -> Result<(), CrawlerError> {
        for _ in 0..RECOVERY_PASSES {
            let Some(nearest) =
                self.scene
                    .nearest_character_surface(self.position.pos(), self.radius, 0.0)?
            else {
                return Ok(());
            };
            if nearest.distance >= self.radius - 1.0e-6 {
                return Ok(());
            }
            let Some(normal) = V::world(nearest.normal).normalized() else {
                return Ok(());
            };
            self.position = V::at(nearest.point).add(normal.mul(self.radius + self.skin));
        }
        match self
            .scene
            .nearest_character_surface(self.position.pos(), self.radius, 0.0)?
        {
            Some(nearest) if nearest.distance < self.radius - 1.0e-6 => {
                Err(CrawlerError::UnresolvedPenetration {
                    depth: (self.radius - nearest.distance) as f32,
                })
            }
            _ => Ok(()),
        }
    }

    /// A falling crawler within reach of a surface it holds, not moving away
    /// from it, takes hold of it.
    fn try_land(&mut self) -> Result<(), CrawlerError> {
        let reach = self.skin + 1.0e-3;
        let Some(nearest) =
            self.scene
                .nearest_character_surface(self.position.pos(), self.radius, reach)?
        else {
            return Ok(());
        };
        let Some(normal) = V::world(nearest.normal).normalized() else {
            return Ok(());
        };
        if self.holds(normal) && self.velocity.dot(normal) <= 1.0e-6 {
            self.attach(normal, V::at(nearest.point), nearest.source);
        }
        Ok(())
    }

    fn attach(&mut self, normal: V, point: V, source: CharacterCollisionSource) {
        let mut ignored = V::ZERO;
        self.turn_up(normal, &mut ignored);
        self.velocity = self.velocity.along_plane(normal);
        self.position = point.add(normal.mul(self.radius + self.skin));
        self.attached = true;
        self.transition = CrawlerTransition::Landed;
        self.contact = Some((source, point, normal));
    }

    fn detach(&mut self, transition: CrawlerTransition) {
        self.attached = false;
        self.transition = transition;
        self.contact = None;
    }

    fn crawl(&mut self, direction: V, dt: f64) -> Result<(), CrawlerError> {
        if !self.holds(self.up) {
            self.detach(CrawlerTransition::Detached);
            return self.fall(dt);
        }
        // Accelerate along the surface toward the commanded direction.
        let wish = direction.along_plane(self.up).limited(1.0);
        let tangent = self.velocity.along_plane(self.up);
        let target = wish.mul(f64::from(self.config.speed));
        let rate = if wish.length() > EPSILON {
            self.config.acceleration
        } else {
            self.config.braking
        };
        self.velocity = tangent.add(target.sub(tangent).limited(f64::from(rate) * dt));
        // Move, turning onto each surface met.
        let mut remaining = self.velocity.mul(dt);
        for _ in 0..MOVE_ITERATIONS {
            let Some(hit) = self.sweep(remaining, self.skin * 0.5)? else {
                self.position = self.position.add(remaining);
                break;
            };
            if self.grazes(&hit, remaining) {
                continue;
            }
            self.position = self.position.add(remaining.mul(hit.fraction));
            let mut rest = remaining.mul(1.0 - hit.fraction);
            if self.holds(hit.normal) {
                if self.turn_up(hit.normal, &mut rest) {
                    self.transition = CrawlerTransition::Concave;
                }
                self.contact = Some((hit.source, hit.point, hit.normal));
            } else {
                rest = rest.along_plane(hit.normal);
                self.velocity = self.velocity.along_plane(hit.normal);
            }
            remaining = rest;
        }
        self.stick()
    }

    /// Holds the surface under the ball: the one along −up, or past a lip the
    /// nearest, which is the edge.
    fn stick(&mut self) -> Result<(), CrawlerError> {
        let reach = self.skin + f64::from(self.config.stick_distance);
        let down = self.up.mul(-reach);
        let mut under = None;
        for _ in 0..2 {
            match self.sweep(down, self.skin)? {
                // A surface roughly under the ball, whether the sweep reaches
                // it or the ball already rests its skin off it.
                Some(hit) if hit.normal.dot(self.up) >= 0.5 => {
                    under = Some((hit.source, hit.point, hit.normal));
                    break;
                }
                // A wall beside it, met at once: step off it and look again.
                Some(hit) if hit.fraction <= 1.0e-6 => {
                    let step = self.skin.max(MINIMUM_GRAZE_STEP);
                    self.position = self.position.add(hit.normal.mul(step));
                }
                _ => break,
            }
        }
        // Past a lip nothing is under it; the nearest surface is the edge.
        let held = match under {
            Some(under) => Some(under),
            None => self
                .scene
                .nearest_character_surface(self.position.pos(), self.radius, reach)?
                .and_then(|nearest| {
                    Some((
                        nearest.source,
                        V::at(nearest.point),
                        V::world(nearest.normal).normalized()?,
                    ))
                }),
        };
        let Some((source, point, normal)) = held else {
            self.detach(CrawlerTransition::Lost);
            return Ok(());
        };
        if !self.holds(normal) {
            self.detach(CrawlerTransition::Detached);
            return Ok(());
        }
        // The surface falls away ahead over a convex edge and rises ahead
        // into a concave one.
        let ahead = normal.dot(self.velocity);
        let mut ignored = V::ZERO;
        if self.turn_up(normal, &mut ignored) {
            self.transition = if ahead >= 0.0 {
                CrawlerTransition::Convex
            } else {
                CrawlerTransition::Concave
            };
        }
        self.velocity = self.velocity.along_plane(normal);
        self.position = point.add(normal.mul(self.radius + self.skin));
        self.contact = Some((source, point, normal));
        Ok(())
    }

    fn fall(&mut self, dt: f64) -> Result<(), CrawlerError> {
        self.velocity.0[1] -= f64::from(self.config.gravity) * dt;
        self.velocity.0[1] = self.velocity.0[1].max(-f64::from(self.config.terminal_fall_speed));
        let mut remaining = self.velocity.mul(dt);
        for _ in 0..MOVE_ITERATIONS {
            let Some(hit) = self.sweep(remaining, self.skin * 0.5)? else {
                self.position = self.position.add(remaining);
                return Ok(());
            };
            if self.grazes(&hit, remaining) {
                continue;
            }
            self.position = self.position.add(remaining.mul(hit.fraction));
            if self.holds(hit.normal) {
                self.attach(hit.normal, hit.point, hit.source);
                return Ok(());
            }
            remaining = remaining.mul(1.0 - hit.fraction).along_plane(hit.normal);
            self.velocity = self.velocity.along_plane(hit.normal);
        }
        Ok(())
    }

    fn classify(&self) -> CrawlerSurface {
        if !self.attached {
            CrawlerSurface::None
        } else if self.up.0[1] >= self.floor_cos - 1.0e-6 {
            CrawlerSurface::Floor
        } else if self.up.0[1] <= -self.floor_cos + 1.0e-6 {
            CrawlerSurface::Ceiling
        } else {
            CrawlerSurface::Wall
        }
    }

    /// The body's heading along the surface: where it moves, or the last
    /// heading laid onto the surface.
    fn settle_forward(&mut self) {
        let moving = self.velocity.along_plane(self.up);
        self.forward = moving
            .normalized()
            .filter(|_| moving.length() > 1.0e-3)
            .or_else(|| self.forward.along_plane(self.up).normalized())
            .or_else(|| any_perpendicular(self.up).normalized())
            .unwrap_or(V([0.0, 0.0, 1.0]));
    }
}

/// The rotation whose +Y is `up` and +Z `forward` (orthonormal).
fn basis_rotation(up: V, forward: V) -> Quat {
    let x = up.cross(forward);
    let (m00, m01, m02) = (x.0[0], up.0[0], forward.0[0]);
    let (m10, m11, m12) = (x.0[1], up.0[1], forward.0[1]);
    let (m20, m21, m22) = (x.0[2], up.0[2], forward.0[2]);
    let trace = m00 + m11 + m22;
    let (x, y, z, w) = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        ((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s)
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        (0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        ((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        ((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
    };
    let length = (x * x + y * y + z * z + w * w).sqrt();
    Quat {
        x: (x / length) as f32,
        y: (y / length) as f32,
        z: (z / length) as f32,
        w: (w / length) as f32,
    }
}

/// Solves one crawler command against the scene's voxel and static-mesh
/// collision. The command is solved as equal sub-steps no longer than 1/60 s
/// and moving no more than half the radius each.
pub fn step_crawler(
    scene: &VoxelCollisionScene,
    position: Vec3,
    motion: CrawlerMotion,
    config: &CrawlerConfig,
    command: CrawlerCommand,
) -> Result<CrawlerStepReceipt, CrawlerError> {
    config.validate()?;
    let direction = V::of(command.direction);
    if !direction.finite()
        || !command.step_seconds.is_finite()
        || !(0.001..=1.0).contains(&command.step_seconds)
    {
        return Err(CrawlerError::InvalidCommand);
    }
    let start = V::of(position);
    let velocity = V::of(motion.velocity);
    let (Some(up), true, true) = (
        V::of(motion.up).normalized(),
        start.finite(),
        velocity.finite(),
    ) else {
        return Err(CrawlerError::InvalidMotion);
    };
    let forward = V::of(motion.forward);
    if !forward.finite() {
        return Err(CrawlerError::InvalidMotion);
    }
    let radius = f64::from(config.radius);
    let mut crawl = Crawl {
        scene,
        config,
        cling: command.cling,
        radius,
        skin: f64::from(config.contact_skin),
        floor_cos: f64::from(config.floor_maximum_slope_radians).cos(),
        position: start,
        up,
        forward,
        velocity,
        attached: motion.attached,
        transition: CrawlerTransition::None,
        contact: None,
    };
    crawl.settle_forward();
    let dt_total = f64::from(command.step_seconds);
    let fastest =
        velocity.length() + f64::from(config.speed) + f64::from(config.gravity) * dt_total;
    let by_time = (dt_total / SUB_STEP_SECONDS).ceil();
    let by_distance = (fastest * dt_total / (radius * SUB_STEP_RADIUS_FRACTION)).ceil();
    let sub_steps = (by_time.max(by_distance).max(1.0) as u32).min(MAXIMUM_SUB_STEPS);
    let dt = dt_total / f64::from(sub_steps);
    crawl.recover()?;
    for _ in 0..sub_steps {
        if !crawl.attached {
            crawl.try_land()?;
        }
        let transition = crawl.transition;
        crawl.transition = CrawlerTransition::None;
        if crawl.attached {
            crawl.crawl(direction, dt)?;
        } else {
            crawl.fall(dt)?;
        }
        if crawl.transition == CrawlerTransition::None {
            crawl.transition = transition;
        }
    }
    crawl.settle_forward();
    let contact = match (crawl.attached, crawl.contact) {
        (true, Some((source, point, normal))) => Some(CrawlerContact {
            source,
            point: point.vec3(),
            normal: normal.vec3(),
            distance: crawl.position.sub(point).length() as f32,
        }),
        _ => None,
    };
    let motion = CrawlerMotion {
        attached: crawl.attached,
        up: crawl.up.vec3(),
        forward: crawl.forward.vec3(),
        velocity: crawl.velocity.vec3(),
        last_sequence: command.sequence,
    };
    Ok(CrawlerStepReceipt {
        position: crawl.position.vec3(),
        rotation: basis_rotation(crawl.up, crawl.forward),
        surface: crawl.classify(),
        transition: crawl.transition,
        contact,
        motion,
        displacement: crawl.position.sub(start).vec3(),
        sub_steps,
    })
}
