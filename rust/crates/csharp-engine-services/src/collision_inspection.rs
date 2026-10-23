//! The retained collision scene near a point, as wire lines and the
//! colliders they draw, for inspection (`engine.inspect.overlay show
//! collision`, `engine.inspect.collision`). Reading it changes nothing.
//!
//! The Spatial sessions' character capsules, the product obstacle boxes
//! their last character step collided with, and their active triggers draw
//! first, then every live dynamics body, then voxel chunks and static
//! meshes nearest the point first, until the lines run out.

/// The most wire lines one inspection draws.
pub const COLLISION_INSPECTION_LINES: usize = 65_536;
/// How far a wire stands off the face or box it outlines, so the drawn
/// surface in the same place does not hide it.
const STANDOFF: f64 = 0.01;
const CIRCLE_SEGMENTS: usize = 16;

/// Where a collider comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum CollisionSource {
    /// A resident voxel chunk's cuboids and reconstructed surface.
    VoxelChunk([i64; 3]),
    /// A spatial content artifact's collision: the base artifact or a
    /// placement, whose id is its static-mesh instance.
    ContentArtifact {
        path: String,
        instance: u64,
        base: bool,
    },
    /// A static-mesh instance the product admitted.
    StaticMesh {
        instance: u64,
        asset: u64,
    },
    /// A product obstacle box of the last character step.
    Obstacle {
        entity: u64,
    },
    Trigger {
        entity: u64,
    },
    /// A dynamics body.
    Body {
        world: u64,
        body: u64,
    },
    /// The character capsule after the last character step.
    Character,
}

/// One collider the inspection found: its whole bounds, the lines it drew
/// and the parts left out when the lines ran out.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectedCollision {
    /// The Spatial session holding it; none for a dynamics body.
    pub session: Option<u64>,
    pub source: CollisionSource,
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub lines: usize,
    pub omitted: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionLineKind {
    Voxel,
    StaticMesh,
    Obstacle,
    Trigger,
    Body,
    Character,
}

/// One wire line. `steep` marks a face steeper than the character's slope
/// limit (walls and ceilings too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionLine {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub kind: CollisionLineKind,
    pub steep: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollisionInspection {
    pub colliders: Vec<InspectedCollision>,
    pub lines: Vec<CollisionLine>,
}

impl CollisionInspection {
    /// Parts left out over every collider.
    pub fn omitted(&self) -> usize {
        self.colliders.iter().map(|collider| collider.omitted).sum()
    }
}

/// The box an inspection reads: `radius` along each axis around `center`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Region {
    pub(crate) min: [f64; 3],
    pub(crate) max: [f64; 3],
}

impl Region {
    pub(crate) fn around(center: [f64; 3], radius: f64) -> Self {
        Self {
            min: center.map(|value| value - radius),
            max: center.map(|value| value + radius),
        }
    }

    pub(crate) fn meets(&self, min: [f64; 3], max: [f64; 3]) -> bool {
        (0..3).all(|axis| min[axis] <= self.max[axis] && max[axis] >= self.min[axis])
    }
}

/// Draws colliders into an inspection while its lines last.
pub(crate) struct Wires<'a> {
    pub(crate) inspection: &'a mut CollisionInspection,
    pub(crate) budget: usize,
}

impl Wires<'_> {
    /// Adds a collider drawn by `draw`, whole or, past the budget, not at
    /// all.
    pub(crate) fn primitive(
        &mut self,
        session: Option<u64>,
        source: CollisionSource,
        (min, max): ([f64; 3], [f64; 3]),
        kind: CollisionLineKind,
        draw: impl FnOnce(&mut Pen),
    ) {
        let mut pen = Pen {
            lines: Vec::new(),
            kind,
        };
        draw(&mut pen);
        let fits = pen.lines.len() <= self.budget;
        let lines = if fits { pen.lines.len() } else { 0 };
        if fits {
            self.budget -= lines;
            self.inspection.lines.extend(pen.lines);
        }
        self.inspection.colliders.push(InspectedCollision {
            session,
            source,
            min,
            max,
            lines,
            omitted: usize::from(!fits),
        });
    }

    /// Adds a voxel chunk or static mesh as the projection read it.
    pub(crate) fn projected(
        &mut self,
        session: u64,
        source: CollisionSource,
        collider: &engine_spatial::InspectedCollider,
        slope_cos: f64,
    ) {
        let kind = match source {
            CollisionSource::VoxelChunk(_) => CollisionLineKind::Voxel,
            _ => CollisionLineKind::StaticMesh,
        };
        let mut pen = Pen {
            lines: Vec::new(),
            kind,
        };
        for [min, max] in &collider.boxes {
            pen.wire_box(
                min.map(|value| value - STANDOFF),
                max.map(|value| value + STANDOFF),
            );
        }
        for triangle in &collider.triangles {
            pen.triangle(*triangle, slope_cos);
        }
        // The projection already kept these within the budget.
        self.budget = self.budget.saturating_sub(pen.lines.len());
        self.inspection.colliders.push(InspectedCollision {
            session: Some(session),
            source,
            min: collider.min,
            max: collider.max,
            lines: pen.lines.len(),
            omitted: collider.omitted,
        });
        self.inspection.lines.extend(pen.lines);
    }
}

pub(crate) struct Pen {
    lines: Vec<CollisionLine>,
    kind: CollisionLineKind,
}

impl Pen {
    fn line(&mut self, from: [f64; 3], to: [f64; 3], steep: bool) {
        self.lines.push(CollisionLine {
            from: from.map(|value| value as f32),
            to: to.map(|value| value as f32),
            kind: self.kind,
            steep,
        });
    }

    pub(crate) fn wire_box(&mut self, min: [f64; 3], max: [f64; 3]) {
        self.oriented_box(
            [0.0; 3],
            [0.0, 0.0, 0.0, 1.0],
            std::array::from_fn(|axis| (max[axis] - min[axis]) * 0.5),
            std::array::from_fn(|axis| (max[axis] + min[axis]) * 0.5),
        );
    }

    /// A box of `half` extents about `center` in its own frame, turned by
    /// `rotation` (`[x, y, z, w]`) and moved to `at`.
    pub(crate) fn oriented_box(
        &mut self,
        center: [f64; 3],
        rotation: [f64; 4],
        half: [f64; 3],
        at: [f64; 3],
    ) {
        let corner = |bits: usize| {
            let local = std::array::from_fn(|axis| {
                center[axis]
                    + if bits & (1 << axis) == 0 {
                        -half[axis]
                    } else {
                        half[axis]
                    }
            });
            add(rotate(rotation, local), at)
        };
        for bits in 0..8 {
            for axis in 0..3 {
                if bits & (1 << axis) == 0 {
                    self.line(corner(bits), corner(bits | (1 << axis)), false);
                }
            }
        }
    }

    /// A Y-axis capsule: `half_height` from its centre to each
    /// hemisphere's centre, turned by `rotation` and moved to `at`.
    pub(crate) fn capsule(
        &mut self,
        half_height: f64,
        radius: f64,
        rotation: [f64; 4],
        at: [f64; 3],
    ) {
        let place = |local: [f64; 3]| add(rotate(rotation, local), at);
        let point = |angle: f64, up: f64, plane: usize| match plane {
            // Around Y.
            0 => [radius * angle.cos(), up, radius * angle.sin()],
            // In the XY and ZY planes.
            1 => [radius * angle.cos(), up + radius * angle.sin(), 0.0],
            _ => [0.0, up + radius * angle.sin(), radius * angle.cos()],
        };
        let step = std::f64::consts::TAU / CIRCLE_SEGMENTS as f64;
        for up in [-half_height, half_height] {
            for segment in 0..CIRCLE_SEGMENTS {
                let (a, b) = (segment as f64 * step, (segment + 1) as f64 * step);
                self.line(place(point(a, up, 0)), place(point(b, up, 0)), false);
            }
        }
        for plane in [1, 2] {
            for segment in 0..CIRCLE_SEGMENTS {
                let (a, b) = (segment as f64 * step, (segment + 1) as f64 * step);
                // The upper half circle on the top hemisphere, the lower on
                // the bottom one.
                let up = if segment < CIRCLE_SEGMENTS / 2 {
                    half_height
                } else {
                    -half_height
                };
                self.line(
                    place(point(a, up, plane)),
                    place(point(b, up, plane)),
                    false,
                );
            }
            for side in [0.0, std::f64::consts::PI] {
                self.line(
                    place(point(side, -half_height, plane)),
                    place(point(side, half_height, plane)),
                    false,
                );
            }
        }
    }

    /// A triangle's edges, stood off along its face normal; `steep` when
    /// the normal is steeper than `slope_cos`.
    fn triangle(&mut self, [a, b, c]: [[f64; 3]; 3], slope_cos: f64) {
        let normal = cross(sub(b, a), sub(c, a));
        let length = dot(normal, normal).sqrt();
        let normal = if length > 0.0 {
            normal.map(|value| value / length)
        } else {
            [0.0; 3]
        };
        let steep = normal[1] < slope_cos;
        let lift = |point: [f64; 3]| add(point, normal.map(|value| value * STANDOFF));
        let (a, b, c) = (lift(a), lift(b), lift(c));
        self.line(a, b, steep);
        self.line(b, c, steep);
        self.line(c, a, steep);
    }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] + b[axis])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] - b[axis])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `v` turned by the unit quaternion `[x, y, z, w]`.
fn rotate([x, y, z, w]: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let q = [x, y, z];
    let t = cross(q, v).map(|value| value * 2.0);
    add(add(v, t.map(|value| value * w)), cross(q, t))
}

/// The bounds of a box of `half` extents turned by `rotation` and moved to
/// `at`.
pub(crate) fn turned_bounds(
    rotation: [f64; 4],
    half: [f64; 3],
    at: [f64; 3],
) -> ([f64; 3], [f64; 3]) {
    let reach: [f64; 3] = std::array::from_fn(|axis| {
        (0..3)
            .map(|other| {
                let mut unit = [0.0; 3];
                unit[other] = half[other];
                rotate(rotation, unit)[axis].abs()
            })
            .sum()
    });
    (sub(at, reach), add(at, reach))
}
