//! Soft bodies: Rapier's deformable particle bodies in a Dynamics world.
//! Cloth and shells are particles joined by edges and bending constraints;
//! volumes are tetrahedral cells. They collide with the world's static
//! environment, its bodies and, when asked, themselves.
//!
//! A particle is free, pinned (held where it was pinned), driven to a
//! target the product or a bone gives each step, or attached to a body by a
//! two-way constraint. Air moving past a body's surface pushes each triangle
//! along its normal, which makes a flag flap; a uniform acceleration pushes
//! every particle alike.
//!
//! The drawn surface is the body's boundary over its particles (or, for a
//! filled mesh, that mesh riding the cells), with smooth normals and the
//! UVs it was created with. A tear splits particles and may split pieces off
//! as bodies of their own; they are drawn and read as part of the body.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use rapier3d_f64::dynamics::{
    RigidBodyHandle, RigidBodySet, SoftBody, SoftBodyBuilder, SoftBodyCellModel, SoftBodyHandle,
    SoftBodyMaterial, SoftBodySet, SoftBodySolver, SoftBodyTearEvent, SpringCoefficients,
};
use rapier3d_f64::geometry::{ColliderSet, CollisionEvent, ContactPair};
use rapier3d_f64::pipeline::EventHandler;
use rapier3d_f64::prelude::Vector;
use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};

use crate::dynamics::{DynamicsBodyId, DynamicsSolver};
use crate::DynamicsError;

/// Soft-body colliders carry this bit in their user data, so contact
/// readouts and rolling resistance tell them from rigid bodies (whose user
/// data is their identity plus one) and from the environment (zero).
pub(crate) const SOFT_COLLIDER_TAG: u128 = 1 << 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicsSoftBodyId(pub u64);

/// What a soft body is made of. Positions are in the world.
#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsSoftBodyShape {
    /// A rectangular sheet of `columns` × `rows` particles at
    /// `origin + column * across + row * down`, with structural, shear and
    /// bending edges. UVs run 0 to 1 across the columns and down the rows.
    Cloth {
        origin: [f64; 3],
        across: [f64; 3],
        down: [f64; 3],
        columns: u32,
        rows: u32,
    },
    /// A solid box of tetrahedra on a grid of `particles` per axis. UVs
    /// project each particle onto the box's front (x, y).
    Box {
        center: [f64; 3],
        half_extents: [f64; 3],
        particles: [u32; 3],
    },
    /// A hollow icosphere of `subdivisions` refinements, with bending across
    /// its edges. UVs are the particle's longitude and latitude.
    Shell {
        center: [f64; 3],
        radius: f64,
        subdivisions: u32,
    },
    /// A triangle mesh whose vertices are the particles: a banner, a cloak,
    /// a shell of any shape. Edges and bending follow the triangles.
    Surface {
        positions: Vec<[f64; 3]>,
        uvs: Vec<[f32; 2]>,
        triangles: Vec<[u32; 3]>,
    },
    /// Tetrahedral cells over the particles; the drawn surface is their
    /// boundary.
    Solid {
        positions: Vec<[f64; 3]>,
        uvs: Vec<[f32; 2]>,
        tetrahedra: Vec<[u32; 4]>,
    },
    /// A closed, outward-wound triangle mesh filled with tetrahedra of about
    /// `cell_size`. The mesh rides the cells and is what is drawn, so detail
    /// finer than the cells survives; contacts are as coarse as the cells.
    Filled {
        positions: Vec<[f64; 3]>,
        uvs: Vec<[f32; 2]>,
        triangles: Vec<[u32; 3]>,
        cell_size: f64,
        /// Meet the world through the mesh rather than the cells' boundary,
        /// which stands off the mesh by up to a cell. The mesh's triangles
        /// must be at least as fine as the cells.
        mesh_collision: bool,
    },
}

/// A constraint family's stiffness: how fast it springs back (Hz) and its
/// damping ratio (1 is critical). Independent of mass and step length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsSoftness {
    pub frequency: f64,
    pub damping_ratio: f64,
}

impl DynamicsSoftness {
    fn rapier(self) -> SpringCoefficients<f64> {
        SpringCoefficients::new(self.frequency, self.damping_ratio)
    }

    fn of(value: SpringCoefficients<f64>) -> Self {
        Self {
            frequency: value.natural_frequency,
            damping_ratio: value.damping_ratio,
        }
    }

    fn valid(self) -> bool {
        non_negative(self.frequency) && non_negative(self.damping_ratio)
    }
}

/// How a soft body's material resists, yields and tears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsSoftBodyMaterial {
    pub edge: DynamicsSoftness,
    pub bend: DynamicsSoftness,
    pub volume: DynamicsSoftness,
    pub shape_matching: DynamicsSoftness,
    /// Young's modulus (Pa) of elastic cells.
    pub young_modulus: f64,
    /// Poisson's ratio of elastic cells, in `[0, 0.5)`.
    pub poisson_ratio: f64,
    pub elastic_damping_ratio: f64,
    /// Per second: pulls the particles toward the body's rigid motion,
    /// damping wobble but not the motion as a whole.
    pub deformation_damping: f64,
    pub plastic_yield: f64,
    pub plastic_creep: f64,
    pub plastic_max: f64,
    pub edge_plastic_yield: f64,
    pub edge_plastic_creep: f64,
    pub edge_plastic_max: f64,
    /// Stretch (a fraction of rest length) past which an element tears.
    pub tear_strain: Option<f64>,
    /// Force (N) past which an edge tears.
    pub tear_force: Option<f64>,
    pub tear_smoothing: f64,
    pub interior_strength: f64,
    pub max_tears_per_step: u32,
    /// Smallest piece (in elements) a tear may split off.
    pub min_piece: Option<u32>,
}

impl Default for DynamicsSoftBodyMaterial {
    fn default() -> Self {
        let material = SoftBodyMaterial::default();
        Self {
            edge: DynamicsSoftness::of(material.edge_softness),
            bend: DynamicsSoftness::of(material.bend_softness),
            volume: DynamicsSoftness::of(material.volume_softness),
            shape_matching: DynamicsSoftness::of(material.shape_matching_softness),
            young_modulus: material.young_modulus,
            poisson_ratio: material.poisson_ratio,
            elastic_damping_ratio: material.elastic_damping_ratio,
            deformation_damping: material.deformation_damping,
            plastic_yield: material.plastic_yield,
            plastic_creep: material.plastic_creep,
            plastic_max: material.plastic_max,
            edge_plastic_yield: material.edge_plastic_yield,
            edge_plastic_creep: material.edge_plastic_creep,
            edge_plastic_max: material.edge_plastic_max,
            tear_strain: material.tear_strain,
            tear_force: material.tear_force,
            tear_smoothing: material.tear_smoothing,
            interior_strength: material.interior_strength,
            max_tears_per_step: material.max_tears_per_step,
            min_piece: material.min_piece,
        }
    }
}

impl DynamicsSoftBodyMaterial {
    fn rapier(&self) -> SoftBodyMaterial {
        SoftBodyMaterial {
            edge_softness: self.edge.rapier(),
            bend_softness: self.bend.rapier(),
            volume_softness: self.volume.rapier(),
            shape_matching_softness: self.shape_matching.rapier(),
            young_modulus: self.young_modulus,
            poisson_ratio: self.poisson_ratio,
            elastic_damping_ratio: self.elastic_damping_ratio,
            deformation_damping: self.deformation_damping,
            plastic_yield: self.plastic_yield,
            plastic_creep: self.plastic_creep,
            plastic_max: self.plastic_max,
            edge_plastic_yield: self.edge_plastic_yield,
            edge_plastic_creep: self.edge_plastic_creep,
            edge_plastic_max: self.edge_plastic_max,
            tear_strain: self.tear_strain,
            tear_force: self.tear_force,
            tear_smoothing: self.tear_smoothing,
            interior_strength: self.interior_strength,
            max_tears_per_step: self.max_tears_per_step,
            min_piece: self.min_piece,
            ..SoftBodyMaterial::default()
        }
    }

    fn valid(&self) -> bool {
        [self.edge, self.bend, self.volume, self.shape_matching]
            .into_iter()
            .all(DynamicsSoftness::valid)
            && [
                self.young_modulus,
                self.elastic_damping_ratio,
                self.deformation_damping,
                self.plastic_yield,
                self.plastic_creep,
                self.plastic_max,
                self.edge_plastic_yield,
                self.edge_plastic_creep,
                self.edge_plastic_max,
                self.tear_smoothing,
                self.interior_strength,
            ]
            .into_iter()
            .all(non_negative)
            && (0.0..0.5).contains(&self.poisson_ratio)
            && self.tear_strain.is_none_or(positive)
            && self.tear_force.is_none_or(positive)
    }
}

/// How cells resist deformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicsSoftBodyCellModel {
    /// Each cell keeps its volume; with the edges, a jelly.
    #[default]
    Volume,
    /// Linear elasticity in each cell's own frame (Young's modulus and
    /// Poisson's ratio), recovering from inverted cells.
    Corotational,
    /// Neo-Hookean: stiffer under compression, softer under stretch; twice
    /// the cost.
    NeoHookean,
}

/// How a body's elasticity is solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicsSoftBodySolver {
    /// Constraints swept with contacts and joints each substep: cheap, a
    /// stiff body keeps some give.
    #[default]
    Constraints,
    /// Implicit finite elements over the whole body each substep: stiff
    /// bodies hold their shape, at a factorization per step.
    Fem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicsSoftBodyInput {
    pub id: DynamicsSoftBodyId,
    pub shape: DynamicsSoftBodyShape,
    /// Total mass, spread evenly over the particles.
    pub mass: f64,
    /// Contact thickness of the surface; `None` takes half the mean edge.
    pub particle_radius: Option<f64>,
    pub linear_damping: f64,
    pub gravity_scale: f64,
    pub cell_model: DynamicsSoftBodyCellModel,
    pub solver: DynamicsSoftBodySolver,
    pub material: DynamicsSoftBodyMaterial,
    /// One constraint per closed piece keeping its enclosed volume, times
    /// `volume_factor` (above 1 inflates).
    pub volume_preservation: bool,
    pub volume_factor: f64,
    /// Pulls the particles toward the rest shape's best rigid fit.
    pub shape_matching: bool,
    /// The surface collides with itself.
    pub self_contacts: bool,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub friction: f64,
    pub restitution: f64,
    /// Extra solver substeps for the body and what it touches.
    pub extra_substeps: u32,
    /// Extra solver iterations per substep (Rapier's default is 3).
    pub extra_iterations: u32,
    pub can_sleep: bool,
    /// Particles held where they are.
    pub pinned: Vec<u32>,
}

/// Forces a soft body feels every substep until they are set again.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicsSoftBodyForces {
    /// Added to every free particle's acceleration (m/s²).
    pub acceleration: [f64; 3],
    /// The air's velocity (m/s).
    pub wind: [f64; 3],
    /// Pressure per squared normal air speed (kg/m³): each surface triangle
    /// is pushed along its normal by `drag × area × v|v|`, where `v` is the
    /// air's speed through it. Half the air density times a drag
    /// coefficient; about 0.6 for a flag.
    pub drag: f64,
}

impl DynamicsSoftBodyForces {
    fn valid(&self) -> bool {
        self.acceleration
            .into_iter()
            .chain(self.wind)
            .all(f64::is_finite)
            && non_negative(self.drag)
    }

    fn is_none(&self) -> bool {
        self.acceleration == [0.0; 3] && (self.drag == 0.0 || self.wind == [0.0; 3])
    }
}

/// Soft-body settings shared by every soft body of a world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsSoftBodySettings {
    /// Each step is split into this many substeps while the world holds a
    /// soft body (at least ropes' substeps when it also holds ropes).
    pub substeps: u32,
    /// Most extra substeps a body asks for while it is hit fast.
    pub max_extra_substeps: u32,
    /// Stiffening of soft contacts over rigid ones.
    pub contact_stiffening: f64,
    /// Strain past which a constraint is solved again after the contacts.
    pub resweep_strain: f64,
}

impl Default for DynamicsSoftBodySettings {
    fn default() -> Self {
        let rapier = rapier3d_f64::dynamics::SoftBodiesSettings::default();
        Self {
            substeps: 1,
            max_extra_substeps: rapier.max_extra_substeps as u32,
            contact_stiffening: rapier.contact_stiffening,
            resweep_strain: rapier.resweep_strain,
        }
    }
}

impl DynamicsSoftBodySettings {
    pub(crate) fn rapier(&self) -> rapier3d_f64::dynamics::SoftBodiesSettings {
        rapier3d_f64::dynamics::SoftBodiesSettings {
            max_extra_substeps: self.max_extra_substeps as usize,
            contact_stiffening: self.contact_stiffening,
            resweep_strain: self.resweep_strain,
            ..Default::default()
        }
    }

    fn valid(&self) -> bool {
        self.substeps >= 1 && positive(self.contact_stiffening) && positive(self.resweep_strain)
    }
}

/// A soft body's state after the latest step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicsSoftBodyReadout {
    pub particle_count: u32,
    /// Bodies a tear split off, read with this one.
    pub piece_count: u32,
    pub center_of_mass: [f64; 3],
    /// The mass-weighted mean particle velocity.
    pub linear_velocity: [f64; 3],
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
    /// The enclosed volume of its closed pieces, and that volume at rest
    /// (both zero for an open sheet).
    pub volume: f64,
    pub rest_volume: f64,
    /// The largest edge stretch, as a fraction over rest length.
    pub max_stretch: f64,
    /// The colliders (environment, bodies, other soft bodies, itself) its
    /// surface touches.
    pub contact_count: u32,
    pub sleeping: bool,
    /// Changes whenever a tear changes its particles or elements.
    pub topology_version: u32,
}

/// One particle of a soft body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsSoftBodyParticle {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub mass: f64,
    /// Pinned or driven to a target: moved, never pushed.
    pub kinematic: bool,
    pub on_surface: bool,
}

/// What to draw: triangles with smooth normals and UVs, in the world.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DynamicsSoftBodySurface {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

/// A particle held by a two-way constraint to a point of a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsSoftBodyBodyAttachment {
    pub particle: u32,
    pub body: DynamicsBodyId,
}

/// Where the drawn surface's UVs come from.
#[derive(Debug, Clone)]
enum SurfaceUvs {
    /// One per particle of each piece (as `handles`), carried through every
    /// tear: a split particle's copies take its UV.
    Particles(Vec<Vec<[f32; 2]>>),
    /// One per vertex of the skin.
    Skin(Vec<[f32; 2]>),
}

/// A particle of a soft body: the piece holding it (an index into the
/// body's `handles`) and its index there. A tear renumbers particles and
/// moves some into new pieces; these follow them.
type Particle = (usize, u32);

pub(crate) struct SolverSoftBody {
    /// The body, then every piece split off it, in the order they split.
    handles: Vec<SoftBodyHandle>,
    uvs: SurfaceUvs,
    pinned: BTreeSet<Particle>,
    /// Driven particles and where they reach by the end of the next step.
    targets: BTreeMap<Particle, [f64; 3]>,
    /// Particles driven by joint attachments, in attachment order, and
    /// where they were last driven.
    joints: Vec<(Particle, Option<[f64; 3]>)>,
    forces: DynamicsSoftBodyForces,
    /// Where each driven particle started the step.
    starts: Vec<(Particle, Vector, Vector)>,
    /// Whether the body moved or changed since its surface was last taken.
    moved: bool,
    contact_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsSoftBodyError {
    Duplicate {
        body: DynamicsSoftBodyId,
    },
    Unknown {
        body: DynamicsSoftBodyId,
    },
    Invalid {
        body: DynamicsSoftBodyId,
    },
    Particle {
        body: DynamicsSoftBodyId,
        particle: u32,
    },
    Settings,
}

impl DynamicsSoftBodyError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Duplicate { .. } => "duplicate-dynamics-soft-body",
            Self::Unknown { .. } => "unknown-dynamics-soft-body",
            Self::Invalid { .. } => "invalid-dynamics-soft-body",
            Self::Particle { .. } => "invalid-dynamics-soft-body-particle",
            Self::Settings => "invalid-dynamics-soft-body-settings",
        }
    }
}

impl From<DynamicsSoftBodyError> for DynamicsError {
    fn from(value: DynamicsSoftBodyError) -> Self {
        Self::SoftBody(value)
    }
}

impl DynamicsSolver {
    pub fn soft_body_settings(&self) -> DynamicsSoftBodySettings {
        self.soft_settings
    }

    pub fn configure_soft_bodies(
        &mut self,
        settings: DynamicsSoftBodySettings,
    ) -> Result<(), DynamicsError> {
        if !settings.valid() {
            return Err(DynamicsSoftBodyError::Settings.into());
        }
        self.soft_settings = settings;
        Ok(())
    }

    pub fn soft_body_count(&self) -> usize {
        self.soft_bodies.len()
    }

    pub fn insert_soft_body(&mut self, input: DynamicsSoftBodyInput) -> Result<(), DynamicsError> {
        let id = input.id;
        if self.soft_bodies.contains_key(&id) {
            return Err(DynamicsSoftBodyError::Duplicate { body: id }.into());
        }
        let invalid = || DynamicsError::from(DynamicsSoftBodyError::Invalid { body: id });
        if !valid_input(&input) {
            return Err(invalid());
        }
        let (builder, uvs) = shape_builder(&input.shape).ok_or_else(invalid)?;
        let count = builder.positions.len();
        if input
            .pinned
            .iter()
            .any(|&particle| particle as usize >= count)
        {
            return Err(invalid());
        }
        let mut builder = builder
            .mass(input.mass)
            .linear_damping(input.linear_damping)
            .gravity_scale(input.gravity_scale)
            .cell_model(match input.cell_model {
                DynamicsSoftBodyCellModel::Volume => SoftBodyCellModel::Volume,
                DynamicsSoftBodyCellModel::Corotational => SoftBodyCellModel::Corotational,
                DynamicsSoftBodyCellModel::NeoHookean => SoftBodyCellModel::NeoHookean,
            })
            .solver(match input.solver {
                DynamicsSoftBodySolver::Constraints => SoftBodySolver::Constraints,
                DynamicsSoftBodySolver::Fem => SoftBodySolver::Fem,
            })
            .material(input.material.rapier())
            .volume_preservation(input.volume_preservation)
            .volume_factor(input.volume_factor)
            .shape_matching(input.shape_matching)
            .self_contacts(input.self_contacts)
            .additional_solver_iterations(input.extra_substeps as usize)
            .additional_pgs_iterations(input.extra_iterations as usize)
            .can_sleep(input.can_sleep)
            .pinned_particles(input.pinned.iter().copied())
            .surface_collider(
                ColliderBuilder::ball(0.05)
                    .density(0.0)
                    .friction(input.friction)
                    .restitution(input.restitution)
                    .collision_groups(InteractionGroups::new(
                        Group::from_bits_retain(input.collision_groups),
                        Group::from_bits_retain(input.collision_mask),
                        InteractionTestMode::And,
                    ))
                    .user_data(SOFT_COLLIDER_TAG | u128::from(id.0)),
            );
        if let Some(radius) = input.particle_radius {
            builder = builder.particle_radius(radius);
        }
        let handle = self.world.insert_soft_body(builder);
        let uvs = match uvs {
            Uvs::Particles(uvs) => SurfaceUvs::Particles(vec![uvs]),
            Uvs::Skin(uvs) => SurfaceUvs::Skin(uvs),
        };
        self.soft_bodies.insert(
            id,
            SolverSoftBody {
                handles: vec![handle],
                uvs,
                pinned: input
                    .pinned
                    .into_iter()
                    .map(|particle| (0, particle))
                    .collect(),
                targets: BTreeMap::new(),
                joints: Vec::new(),
                forces: DynamicsSoftBodyForces::default(),
                starts: Vec::new(),
                moved: true,
                contact_count: 0,
            },
        );
        Ok(())
    }

    /// Remove a soft body and every piece split off it.
    pub fn remove_soft_body(&mut self, id: DynamicsSoftBodyId) -> bool {
        let Some(body) = self.soft_bodies.remove(&id) else {
            return false;
        };
        for handle in body.handles {
            self.world.remove_soft_body(handle);
        }
        true
    }

    /// Change the material, damping and volume settings of a live body in
    /// place. Its particles, pins, targets and attachments stay.
    pub fn update_soft_body_material(
        &mut self,
        id: DynamicsSoftBodyId,
        material: DynamicsSoftBodyMaterial,
        volume_preservation: bool,
        volume_factor: f64,
    ) -> Result<(), DynamicsError> {
        if !material.valid() || !positive(volume_factor) {
            return Err(DynamicsSoftBodyError::Invalid { body: id }.into());
        }
        let material = material.rapier();
        for handle in self.soft_handles(id)? {
            if let Some(body) = self.world.soft_bodies.get_mut(handle) {
                body.set_material(material);
                body.enable_volume_preservation(volume_preservation);
                body.set_volume_factor(volume_factor);
                body.wake_up();
            }
        }
        Ok(())
    }

    /// Hold `particles` where they are, and release the particles pinned
    /// before that are not listed (unless driven).
    pub fn set_soft_body_pins(
        &mut self,
        id: DynamicsSoftBodyId,
        particles: &[u32],
    ) -> Result<(), DynamicsError> {
        let (_, count) = self.soft_main(id)?;
        check_particles(id, count, particles.iter().copied())?;
        self.change_kinematic(id, |soft| {
            soft.pinned = particles.iter().map(|particle| (0, *particle)).collect();
        });
        Ok(())
    }

    /// Drive `targets`' particles to their positions by the end of the next
    /// step, and release driven particles that are not listed (unless
    /// pinned, which then hold where they are).
    pub fn set_soft_body_targets(
        &mut self,
        id: DynamicsSoftBodyId,
        targets: &[(u32, [f64; 3])],
    ) -> Result<(), DynamicsError> {
        let (_, count) = self.soft_main(id)?;
        check_particles(id, count, targets.iter().map(|(particle, _)| *particle))?;
        if !targets
            .iter()
            .all(|(_, at)| at.iter().copied().all(f64::is_finite))
        {
            return Err(DynamicsSoftBodyError::Invalid { body: id }.into());
        }
        self.change_kinematic(id, |soft| {
            soft.targets = targets
                .iter()
                .map(|(particle, at)| ((0, *particle), *at))
                .collect();
        });
        Ok(())
    }

    /// The particles joint attachments drive, in attachment order: driven
    /// to [`Self::set_soft_body_joint_targets`] from the next step on, until
    /// replaced. Particles no longer listed go free unless pinned or driven.
    pub fn set_soft_body_joint_particles(
        &mut self,
        id: DynamicsSoftBodyId,
        particles: &[u32],
    ) -> Result<(), DynamicsError> {
        let (_, count) = self.soft_main(id)?;
        check_particles(id, count, particles.iter().copied())?;
        self.change_kinematic(id, |soft| {
            soft.joints = particles
                .iter()
                .map(|particle| ((0, *particle), None))
                .collect();
        });
        Ok(())
    }

    /// Where each joint-driven particle goes by the end of the next step, in
    /// attachment order; `None` holds it where it was last driven.
    pub fn set_soft_body_joint_targets(
        &mut self,
        id: DynamicsSoftBodyId,
        targets: &[Option<[f64; 3]>],
    ) -> Result<(), DynamicsError> {
        let soft = self
            .soft_bodies
            .get_mut(&id)
            .ok_or(DynamicsSoftBodyError::Unknown { body: id })?;
        let finite = targets
            .iter()
            .flatten()
            .all(|at| at.iter().copied().all(f64::is_finite));
        if targets.len() != soft.joints.len() || !finite {
            return Err(DynamicsSoftBodyError::Invalid { body: id }.into());
        }
        for ((_, last), target) in soft.joints.iter_mut().zip(targets) {
            if target.is_some() {
                *last = *target;
            }
        }
        soft.moved = true;
        Ok(())
    }

    /// Change a body's pins and drives, then pin the particles that became
    /// kinematic and release the ones that stopped.
    fn change_kinematic(
        &mut self,
        id: DynamicsSoftBodyId,
        change: impl FnOnce(&mut SolverSoftBody),
    ) {
        let Some(soft) = self.soft_bodies.get_mut(&id) else {
            return;
        };
        let before = soft.kinematic();
        change(soft);
        let after = soft.kinematic();
        for particle in before.symmetric_difference(&after) {
            let Some(body) = soft
                .handles
                .get(particle.0)
                .and_then(|handle| self.world.soft_bodies.get_mut(*handle))
            else {
                continue;
            };
            if (particle.1 as usize) < body.num_particles() {
                body.set_particle_pinned(particle.1 as usize, after.contains(particle));
            }
        }
        for handle in &soft.handles {
            if let Some(body) = self.world.soft_bodies.get_mut(*handle) {
                body.wake_up();
            }
        }
        soft.moved = true;
    }

    /// Replace the body attachments: each particle is held to the point of
    /// its body where the particle is now, both ways. Attachments a tear
    /// carried into pieces are replaced too.
    pub fn set_soft_body_attachments(
        &mut self,
        id: DynamicsSoftBodyId,
        attachments: &[DynamicsSoftBodyBodyAttachment],
    ) -> Result<(), DynamicsError> {
        let (handle, count) = self.soft_main(id)?;
        check_particles(id, count, attachments.iter().map(|a| a.particle))?;
        let rigid = attachments
            .iter()
            .map(|attachment| self.handle(attachment.body))
            .collect::<Result<Vec<_>, _>>()?;
        let world = &mut self.world;
        for piece in &self.soft_bodies[&id].handles {
            if let Some(body) = world.soft_bodies.get_mut(*piece) {
                let attached: BTreeSet<u32> = body
                    .particle_attachments()
                    .iter()
                    .map(|attachment| attachment.particle)
                    .collect();
                for particle in attached {
                    body.detach_particle(particle as usize);
                }
            }
        }
        let body = world.soft_bodies.get_mut(handle).expect("live handle");
        for (attachment, rigid) in attachments.iter().zip(&rigid) {
            body.attach_particle(attachment.particle as usize, *rigid, &world.bodies);
        }
        body.wake_up();
        for rigid in rigid {
            world.wake_up(rigid, true);
        }
        Ok(())
    }

    pub fn set_soft_body_forces(
        &mut self,
        id: DynamicsSoftBodyId,
        forces: DynamicsSoftBodyForces,
    ) -> Result<(), DynamicsError> {
        if !forces.valid() {
            return Err(DynamicsSoftBodyError::Invalid { body: id }.into());
        }
        let soft = self
            .soft_bodies
            .get_mut(&id)
            .ok_or(DynamicsSoftBodyError::Unknown { body: id })?;
        soft.forces = forces;
        Ok(())
    }

    /// Change the velocity of every free particle within `radius` of
    /// `point` by `velocity_change`, scaled down linearly to nothing at
    /// `radius`; a radius of zero or less changes every free particle fully.
    pub fn apply_soft_body_velocity_change(
        &mut self,
        id: DynamicsSoftBodyId,
        point: [f64; 3],
        velocity_change: [f64; 3],
        radius: f64,
    ) -> Result<(), DynamicsError> {
        self.kick_soft_body(id, point, radius, |_| Vector::from_array(velocity_change))
    }

    /// Push every free particle within `radius` of `center` away from it at
    /// `speed`, scaled down linearly to nothing at `radius`.
    pub fn apply_soft_body_radial_velocity_change(
        &mut self,
        id: DynamicsSoftBodyId,
        center: [f64; 3],
        speed: f64,
        radius: f64,
    ) -> Result<(), DynamicsError> {
        let center = Vector::from_array(center);
        self.kick_soft_body(id, center.to_array(), radius, |position| {
            (position - center).try_normalize().unwrap_or(Vector::ZERO) * speed
        })
    }

    fn kick_soft_body(
        &mut self,
        id: DynamicsSoftBodyId,
        point: [f64; 3],
        radius: f64,
        change: impl Fn(Vector) -> Vector,
    ) -> Result<(), DynamicsError> {
        let handles = self.soft_handles(id)?;
        if !point.iter().copied().all(f64::is_finite) || !radius.is_finite() {
            return Err(DynamicsSoftBodyError::Invalid { body: id }.into());
        }
        let point = Vector::from_array(point);
        for handle in handles {
            let Some(body) = self.world.soft_bodies.get_mut(handle) else {
                continue;
            };
            for index in 0..body.num_particles() {
                let particle = &body.particles()[index];
                if particle.is_pinned() {
                    continue;
                }
                let position = particle.position();
                let scale = if radius > 0.0 {
                    1.0 - (position - point).length() / radius
                } else {
                    1.0
                };
                if scale > 0.0 {
                    let velocity = particle.velocity() + change(position) * scale;
                    if velocity.is_finite() {
                        body.set_particle_velocity(index, velocity);
                    }
                }
            }
            body.wake_up();
        }
        if let Some(soft) = self.soft_bodies.get_mut(&id) {
            soft.moved = true;
        }
        Ok(())
    }

    pub fn soft_body(&self, id: DynamicsSoftBodyId) -> Option<DynamicsSoftBodyReadout> {
        let soft = self.soft_bodies.get(&id)?;
        let mut readout = DynamicsSoftBodyReadout {
            piece_count: soft.handles.len().saturating_sub(1) as u32,
            contact_count: soft.contact_count,
            sleeping: true,
            bounds_min: [f64::INFINITY; 3],
            bounds_max: [f64::NEG_INFINITY; 3],
            ..Default::default()
        };
        let (mut mass, mut moment, mut momentum) = (0.0, Vector::ZERO, Vector::ZERO);
        for body in soft
            .handles
            .iter()
            .filter_map(|handle| self.world.soft_bodies.get(*handle))
        {
            readout.particle_count += body.num_particles() as u32;
            readout.sleeping &= body.is_sleeping();
            readout.topology_version = readout
                .topology_version
                .wrapping_add(body.topology_version());
            readout.volume += body.volume();
            readout.rest_volume += body.rest_volume();
            for particle in body.particles() {
                let position = particle.position();
                for axis in 0..3 {
                    readout.bounds_min[axis] = readout.bounds_min[axis].min(position[axis]);
                    readout.bounds_max[axis] = readout.bounds_max[axis].max(position[axis]);
                }
                mass += particle.mass();
                moment += position * particle.mass();
                momentum += particle.velocity() * particle.mass();
            }
            for edge in body.edges() {
                let [a, b] = edge.vertices;
                let length = (body.particle_position(a as usize)
                    - body.particle_position(b as usize))
                .length();
                if edge.rest_length > 0.0 {
                    readout.max_stretch = readout.max_stretch.max(length / edge.rest_length - 1.0);
                }
            }
        }
        if mass > 0.0 {
            readout.center_of_mass = (moment / mass).to_array();
            readout.linear_velocity = (momentum / mass).to_array();
        }
        if readout.particle_count == 0 {
            readout.bounds_min = [0.0; 3];
            readout.bounds_max = [0.0; 3];
        }
        Some(readout)
    }

    /// Every particle of the body (not of its pieces), in particle order.
    pub fn soft_body_particles(
        &self,
        id: DynamicsSoftBodyId,
    ) -> Option<Vec<DynamicsSoftBodyParticle>> {
        let soft = self.soft_bodies.get(&id)?;
        let body = self.world.soft_bodies.get(soft.handles[0])?;
        Some(
            body.particles()
                .iter()
                .map(|particle| DynamicsSoftBodyParticle {
                    position: particle.position().to_array(),
                    velocity: particle.velocity().to_array(),
                    mass: particle.mass(),
                    kinematic: particle.is_pinned(),
                    on_surface: particle.is_on_surface(),
                })
                .collect(),
        )
    }

    /// Whether the body moved or changed since its surface was last taken
    /// with [`Self::take_soft_body_surface`].
    pub fn soft_body_moved(&self, id: DynamicsSoftBodyId) -> bool {
        self.soft_bodies.get(&id).is_some_and(|soft| soft.moved)
    }

    /// The surface to draw, with `two_sided` adding each triangle's back as
    /// its own triangle with reversed normals (for cloth under a one-sided
    /// material). Marks the body as drawn.
    pub fn take_soft_body_surface(
        &mut self,
        id: DynamicsSoftBodyId,
        two_sided: bool,
    ) -> Option<DynamicsSoftBodySurface> {
        let surface = self.soft_body_surface(id, two_sided)?;
        if let Some(soft) = self.soft_bodies.get_mut(&id) {
            soft.moved = false;
        }
        Some(surface)
    }

    pub fn soft_body_surface(
        &self,
        id: DynamicsSoftBodyId,
        two_sided: bool,
    ) -> Option<DynamicsSoftBodySurface> {
        let soft = self.soft_bodies.get(&id)?;
        let mut surface = DynamicsSoftBodySurface::default();
        for (slot, body) in soft
            .handles
            .iter()
            .enumerate()
            .filter_map(|(slot, handle)| Some((slot, self.world.soft_bodies.get(*handle)?)))
        {
            let skin = match &soft.uvs {
                SurfaceUvs::Skin(uvs) => body
                    .meshes()
                    .find(|mesh| mesh.is_skinned())
                    .map(|mesh| (mesh, uvs)),
                SurfaceUvs::Particles(_) => None,
            };
            let base = surface.positions.len() as u32;
            match (skin, &soft.uvs) {
                (Some((mesh, uvs)), _) => {
                    for (index, vertex) in mesh.vertex_positions(body).enumerate() {
                        surface.positions.push(vertex.as_vec3().to_array());
                        surface
                            .uvs
                            .push(uvs.get(index).copied().unwrap_or_default());
                    }
                    surface.indices.extend(
                        mesh.indices()
                            .iter()
                            .flat_map(|triangle| triangle.map(|vertex| base + vertex)),
                    );
                }
                (None, uvs) => {
                    // Only the particles on the boundary are drawn.
                    let mut vertex_of = HashMap::new();
                    for triangle in body.boundary() {
                        for particle in triangle {
                            let vertex = *vertex_of.entry(*particle).or_insert_with(|| {
                                let at = body.particles()[*particle as usize].position();
                                surface.positions.push(at.as_vec3().to_array());
                                surface.uvs.push(match uvs {
                                    SurfaceUvs::Particles(pieces) => pieces
                                        .get(slot)
                                        .and_then(|piece| piece.get(*particle as usize))
                                        .copied()
                                        .unwrap_or_default(),
                                    SurfaceUvs::Skin(_) => [0.0; 2],
                                });
                                surface.positions.len() as u32 - 1
                            });
                            surface.indices.push(vertex);
                        }
                    }
                }
            }
        }
        surface.normals = smooth_normals(&surface.positions, &surface.indices);
        if two_sided {
            let count = surface.positions.len() as u32;
            surface.positions.extend_from_within(..);
            surface.uvs.extend_from_within(..);
            let backs: Vec<[f32; 3]> = surface
                .normals
                .iter()
                .map(|normal| normal.map(|component| -component))
                .collect();
            surface.normals.extend(backs);
            let fronts = surface.indices.len();
            for triangle in 0..fronts / 3 {
                let [a, b, c] = [0, 1, 2].map(|k| surface.indices[triangle * 3 + k] + count);
                surface.indices.extend([a, c, b]);
            }
        }
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for position in &surface.positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
        if surface.positions.is_empty() {
            (min, max) = ([0.0; 3], [0.0; 3]);
        }
        surface.bounds_min = min;
        surface.bounds_max = max;
        Some(surface)
    }

    fn soft_handles(&self, id: DynamicsSoftBodyId) -> Result<Vec<SoftBodyHandle>, DynamicsError> {
        Ok(self
            .soft_bodies
            .get(&id)
            .ok_or(DynamicsSoftBodyError::Unknown { body: id })?
            .handles
            .clone())
    }

    fn soft_main(&self, id: DynamicsSoftBodyId) -> Result<(SoftBodyHandle, usize), DynamicsError> {
        let soft = self
            .soft_bodies
            .get(&id)
            .ok_or(DynamicsSoftBodyError::Unknown { body: id })?;
        let body = self
            .world
            .soft_bodies
            .get(soft.handles[0])
            .ok_or(DynamicsSoftBodyError::Unknown { body: id })?;
        Ok((soft.handles[0], body.num_particles()))
    }

    /// Before a step: note where each driven particle starts and where it
    /// goes, a joint's target over the product's.
    pub(crate) fn begin_soft_step(&mut self) {
        for soft in self.soft_bodies.values_mut() {
            let driven: BTreeMap<Particle, [f64; 3]> = soft
                .targets
                .iter()
                .map(|(particle, target)| (*particle, *target))
                .chain(
                    soft.joints
                        .iter()
                        .filter_map(|(particle, target)| Some((*particle, (*target)?))),
                )
                .collect();
            soft.starts.clear();
            for (particle, target) in driven {
                let Some(body) = soft
                    .handles
                    .get(particle.0)
                    .and_then(|handle| self.world.soft_bodies.get(*handle))
                else {
                    continue;
                };
                if (particle.1 as usize) < body.num_particles() {
                    let start = body.particle_position(particle.1 as usize);
                    soft.starts
                        .push((particle, start, Vector::from_array(target)));
                }
            }
        }
    }

    /// Before each substep: move driven particles along their paths, `along`
    /// of the way by the end of it, and lay on this substep's forces.
    pub(crate) fn soft_substep(&mut self, along: f64) {
        for soft in self.soft_bodies.values_mut() {
            let forces = soft.forces;
            for (slot, handle) in soft.handles.iter().enumerate() {
                let Some(body) = self.world.soft_bodies.get_mut(*handle) else {
                    continue;
                };
                for (particle, start, target) in &soft.starts {
                    // Tears are reconciled after every substep.
                    if particle.0 == slot && (particle.1 as usize) < body.num_particles() {
                        body.set_particle_kinematic_target(
                            particle.1 as usize,
                            start.lerp(*target, along),
                        );
                    }
                }
                if forces.is_none() {
                    if body.particles().iter().any(|p| p.force() != Vector::ZERO) {
                        body.reset_forces(false);
                    }
                    continue;
                }
                apply_forces(body, &forces);
            }
        }
    }

    /// After a step: count contacts and note which bodies moved.
    pub(crate) fn end_soft_step(&mut self) {
        let mut touching: BTreeMap<u64, u32> = BTreeMap::new();
        for pair in self.world.contact_pairs() {
            let soft_touch = pair.soft().is_some_and(|soft| soft.is_touching());
            if !soft_touch && !pair.has_any_active_contact() {
                continue;
            }
            for collider in [pair.collider1, pair.collider2] {
                if let Some(id) = self
                    .world
                    .colliders
                    .get(collider)
                    .and_then(|collider| soft_id(collider.user_data))
                {
                    *touching.entry(id).or_insert(0) += 1;
                }
            }
        }
        for (id, soft) in &mut self.soft_bodies {
            soft.contact_count = touching.get(&id.0).copied().unwrap_or(0);
            soft.moved |= soft.handles.iter().any(|handle| {
                self.world
                    .soft_bodies
                    .get(*handle)
                    .is_some_and(|body| !body.is_sleeping())
            });
        }
    }

    /// Move every soft body's particles and drive targets by `delta`.
    pub(crate) fn translate_soft_bodies(&mut self, delta: Vector) {
        for soft in self.soft_bodies.values_mut() {
            for handle in &soft.handles {
                let Some(body) = self.world.soft_bodies.get_mut(*handle) else {
                    continue;
                };
                for index in 0..body.num_particles() {
                    let position = body.particle_position(index) + delta;
                    body.set_particle_position(index, position);
                }
            }
            let moved = |target: &mut [f64; 3]| {
                *target = (Vector::from_array(*target) + delta).to_array();
            };
            soft.targets.values_mut().for_each(moved);
            soft.joints
                .iter_mut()
                .filter_map(|(_, target)| target.as_mut())
                .for_each(moved);
            soft.moved = true;
        }
    }

    /// Drop attachments to a rigid body about to be removed; returns them
    /// so a replacement can take them again. Rapier detaches a particle
    /// from every body at once, so its other attachments are made again.
    pub(crate) fn detach_soft_bodies_from(
        &mut self,
        rigid: RigidBodyHandle,
    ) -> Vec<(SoftBodyHandle, u32)> {
        let mut detached = Vec::new();
        let world = &mut self.world;
        for handle in self.soft_bodies.values().flat_map(|soft| &soft.handles) {
            let Some(body) = world.soft_bodies.get_mut(*handle) else {
                continue;
            };
            let attachments = body.particle_attachments().to_vec();
            let particles: BTreeSet<u32> = attachments
                .iter()
                .filter(|attachment| attachment.body == rigid)
                .map(|attachment| attachment.particle)
                .collect();
            for particle in &particles {
                body.detach_particle(*particle as usize);
                detached.push((*handle, *particle));
            }
            for other in attachments.iter().filter(|attachment| {
                particles.contains(&attachment.particle) && attachment.body != rigid
            }) {
                body.attach_particle(other.particle as usize, other.body, &world.bodies);
            }
        }
        detached
    }

    pub(crate) fn reattach_soft_bodies(
        &mut self,
        id: DynamicsBodyId,
        attachments: Vec<(SoftBodyHandle, u32)>,
    ) {
        let Ok(rigid) = self.handle(id) else {
            return;
        };
        let world = &mut self.world;
        for (handle, particle) in attachments {
            if let Some(body) = world.soft_bodies.get_mut(handle) {
                body.attach_particle(particle as usize, rigid, &world.bodies);
            }
        }
    }

    /// After a substep: follow each tear's renumbering and new pieces, so
    /// pins, drives and UVs still name the particles they named.
    pub(crate) fn reconcile_soft_tears(&mut self, tears: Vec<SoftBodyTearEvent>) {
        for tear in tears {
            let Some(soft) = self
                .soft_bodies
                .values_mut()
                .find(|soft| soft.handles.contains(&tear.soft_body))
            else {
                continue;
            };
            soft.reconcile(&tear);
        }
    }
}

impl SolverSoftBody {
    /// The particles held kinematic: pinned, driven or joint-driven.
    fn kinematic(&self) -> BTreeSet<Particle> {
        self.pinned
            .iter()
            .chain(self.targets.keys())
            .chain(self.joints.iter().map(|(particle, _)| particle))
            .copied()
            .collect()
    }

    fn slot(&self, handle: SoftBodyHandle) -> Option<usize> {
        self.handles.iter().position(|known| *known == handle)
    }

    fn reconcile(&mut self, tear: &SoftBodyTearEvent) {
        let Some(torn) = self.slot(tear.soft_body) else {
            return;
        };
        for piece in &tear.pieces {
            if self.slot(piece.soft_body).is_none() {
                self.handles.push(piece.soft_body);
            }
        }
        let handles = self.handles.clone();
        let slot = |handle: SoftBodyHandle| {
            handles
                .iter()
                .position(|known| *known == handle)
                .expect("the tear's pieces are adopted")
        };
        // A tear keeps its particles' indices and appends the copies; a
        // split then numbers each piece's particles afresh.
        let remap = |particle: Particle| -> Particle {
            if particle.0 != torn {
                return particle;
            }
            match tear.particle_destination(particle.1) {
                Some((handle, index)) => (slot(handle), index),
                None => particle,
            }
        };
        self.pinned = self
            .pinned
            .iter()
            .map(|particle| remap(*particle))
            .collect();
        self.targets = self
            .targets
            .iter()
            .map(|(particle, target)| (remap(*particle), *target))
            .collect();
        for (particle, _) in &mut self.joints {
            *particle = remap(*particle);
        }
        for (particle, _, _) in &mut self.starts {
            *particle = remap(*particle);
        }
        if let SurfaceUvs::Particles(pieces) = &mut self.uvs {
            pieces.resize(handles.len(), Vec::new());
            let mut torn_uvs = std::mem::take(&mut pieces[torn]);
            for &(copy, source) in &tear.split_particles {
                let uv = torn_uvs.get(source as usize).copied().unwrap_or_default();
                if torn_uvs.len() <= copy as usize {
                    torn_uvs.resize(copy as usize + 1, [0.0; 2]);
                }
                torn_uvs[copy as usize] = uv;
            }
            if tear.pieces.is_empty() {
                pieces[torn] = torn_uvs;
            } else {
                for piece in &tear.pieces {
                    pieces[slot(piece.soft_body)] = piece
                        .particles
                        .iter()
                        .map(|particle| {
                            torn_uvs
                                .get(*particle as usize)
                                .copied()
                                .unwrap_or_default()
                        })
                        .collect();
                }
            }
        }
        self.moved = true;
    }
}

/// Collects a substep's tears for [`DynamicsSolver::reconcile_soft_tears`].
#[derive(Default)]
pub(crate) struct TearEvents(Mutex<Vec<SoftBodyTearEvent>>);

impl TearEvents {
    pub(crate) fn take(&self) -> Vec<SoftBodyTearEvent> {
        std::mem::take(&mut *self.0.lock().expect("tear events are only pushed"))
    }
}

impl EventHandler for TearEvents {
    fn handle_collision_event(
        &self,
        _: &RigidBodySet,
        _: &ColliderSet,
        _: CollisionEvent,
        _: Option<&ContactPair>,
    ) {
    }

    fn handle_contact_force_event(
        &self,
        _: f64,
        _: &RigidBodySet,
        _: &ColliderSet,
        _: &ContactPair,
        _: f64,
    ) {
    }

    fn handle_soft_body_tear_event(&self, _: &SoftBodySet, event: &SoftBodyTearEvent) {
        self.0
            .lock()
            .expect("tear events are only pushed")
            .push(event.clone());
    }
}

/// Uniform acceleration on every free particle, and the air's push on each
/// boundary triangle shared among its corners.
fn apply_forces(body: &mut SoftBody, forces: &DynamicsSoftBodyForces) {
    let count = body.num_particles();
    let mut force = vec![Vector::ZERO; count];
    let acceleration = Vector::from_array(forces.acceleration);
    if acceleration != Vector::ZERO {
        for (index, particle) in body.particles().iter().enumerate() {
            force[index] += acceleration * particle.mass();
        }
    }
    let wind = Vector::from_array(forces.wind);
    if forces.drag > 0.0 {
        let particles = body.particles();
        for triangle in body.boundary() {
            let [a, b, c] = triangle.map(|particle| particles[particle as usize].position());
            let cross = (b - a).cross(c - a);
            let twice_area = cross.length();
            if twice_area <= f64::EPSILON {
                continue;
            }
            let normal = cross / twice_area;
            let velocity = triangle
                .iter()
                .map(|particle| particles[*particle as usize].velocity())
                .sum::<Vector>()
                / 3.0;
            let through = (wind - velocity).dot(normal);
            let push = normal * (forces.drag * 0.5 * twice_area * through * through.abs());
            for particle in triangle {
                force[*particle as usize] += push / 3.0;
            }
        }
    }
    body.reset_forces(false);
    for (index, force) in force.into_iter().enumerate() {
        if force != Vector::ZERO {
            body.add_particle_force(index, force, true);
        }
    }
}

/// Area-weighted vertex normals; a vertex on no triangle points up.
fn smooth_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![rapier3d_f64::glamx::Vec3::ZERO; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle
            .map(|vertex| rapier3d_f64::glamx::Vec3::from_array(positions[vertex as usize]));
        let weighted = (b - a).cross(c - a);
        for vertex in triangle {
            normals[*vertex as usize] += weighted;
        }
    }
    normals
        .into_iter()
        .map(|normal| {
            normal
                .try_normalize()
                .unwrap_or(rapier3d_f64::glamx::Vec3::Y)
                .to_array()
        })
        .collect()
}

enum Uvs {
    Particles(Vec<[f32; 2]>),
    Skin(Vec<[f32; 2]>),
}

fn shape_builder(shape: &DynamicsSoftBodyShape) -> Option<(SoftBodyBuilder, Uvs)> {
    let vectors = |positions: &[[f64; 3]]| -> Option<Vec<Vector>> {
        positions
            .iter()
            .all(|position| position.iter().copied().all(f64::is_finite))
            .then(|| positions.iter().copied().map(Vector::from_array).collect())
    };
    let given_uvs = |uvs: &[[f32; 2]], count: usize| -> Option<Vec<[f32; 2]>> {
        match uvs.len() {
            0 => Some(vec![[0.0; 2]; count]),
            length if length == count => Some(uvs.to_vec()),
            _ => None,
        }
    };
    match shape {
        DynamicsSoftBodyShape::Cloth {
            origin,
            across,
            down,
            columns,
            rows,
        } => {
            if *columns < 2 || *rows < 2 {
                return None;
            }
            let (columns, rows) = (*columns as usize, *rows as usize);
            let builder = SoftBodyBuilder::cloth(
                Vector::from_array(*origin),
                Vector::from_array(*across),
                Vector::from_array(*down),
                columns,
                rows,
            );
            // Particle `column * rows + row`.
            let uvs = (0..columns)
                .flat_map(|column| {
                    (0..rows).map(move |row| {
                        [
                            column as f32 / (columns - 1) as f32,
                            row as f32 / (rows - 1) as f32,
                        ]
                    })
                })
                .collect();
            Some((builder, Uvs::Particles(uvs)))
        }
        DynamicsSoftBodyShape::Box {
            center,
            half_extents,
            particles,
        } => {
            if particles.iter().any(|count| *count < 2)
                || !half_extents.iter().copied().all(positive)
            {
                return None;
            }
            let builder = SoftBodyBuilder::cuboid(
                Vector::from_array(*center),
                Vector::from_array(*half_extents),
                particles[0] as usize,
                particles[1] as usize,
                particles[2] as usize,
            );
            let min = Vector::from_array(*center) - Vector::from_array(*half_extents);
            let size = Vector::from_array(*half_extents) * 2.0;
            let uvs = builder
                .positions
                .iter()
                .map(|position| {
                    let local = (*position - min) / size;
                    [local.x as f32, 1.0 - local.y as f32]
                })
                .collect();
            Some((builder, Uvs::Particles(uvs)))
        }
        DynamicsSoftBodyShape::Shell {
            center,
            radius,
            subdivisions,
        } => {
            if !positive(*radius) || *subdivisions > 6 {
                return None;
            }
            let center = Vector::from_array(*center);
            let builder = SoftBodyBuilder::sphere(center, *radius, *subdivisions as usize);
            let uvs = builder
                .positions
                .iter()
                .map(|position| {
                    let direction = (*position - center).normalize_or_zero();
                    [
                        (0.5 + direction.z.atan2(direction.x) / std::f64::consts::TAU) as f32,
                        (direction.y.clamp(-1.0, 1.0).acos() / std::f64::consts::PI) as f32,
                    ]
                })
                .collect();
            Some((builder, Uvs::Particles(uvs)))
        }
        DynamicsSoftBodyShape::Surface {
            positions,
            uvs,
            triangles,
        } => {
            let count = positions.len();
            if !in_range(triangles.iter().flatten(), count) {
                return None;
            }
            let builder = SoftBodyBuilder::trimesh(vectors(positions)?, triangles.clone())?
                // A sheet should drape; ask for shape matching to hold a shape.
                .shape_matching(false);
            Some((builder, Uvs::Particles(given_uvs(uvs, count)?)))
        }
        DynamicsSoftBodyShape::Solid {
            positions,
            uvs,
            tetrahedra,
        } => {
            let count = positions.len();
            if tetrahedra.is_empty() || !in_range(tetrahedra.iter().flatten(), count) {
                return None;
            }
            let builder = SoftBodyBuilder::new(vectors(positions)?).cells(tetrahedra.clone());
            let edges = builder.cell_edges();
            let mean = edges
                .iter()
                .map(|[a, b]| {
                    (builder.positions[*a as usize] - builder.positions[*b as usize]).length()
                })
                .sum::<f64>()
                / edges.len().max(1) as f64;
            let builder = builder.particle_radius(mean * 0.5);
            Some((builder, Uvs::Particles(given_uvs(uvs, count)?)))
        }
        DynamicsSoftBodyShape::Filled {
            positions,
            uvs,
            triangles,
            cell_size,
            mesh_collision,
        } => {
            let count = positions.len();
            if !positive(*cell_size) || !in_range(triangles.iter().flatten(), count) {
                return None;
            }
            let builder =
                SoftBodyBuilder::volumetric_skinned(&vectors(positions)?, triangles, *cell_size)?
                    .skin_collision(*mesh_collision);
            Some((builder, Uvs::Skin(given_uvs(uvs, count)?)))
        }
    }
}

fn in_range<'a>(mut indices: impl Iterator<Item = &'a u32>, count: usize) -> bool {
    indices.all(|index| (*index as usize) < count)
}

fn valid_input(input: &DynamicsSoftBodyInput) -> bool {
    positive(input.mass)
        && input.particle_radius.is_none_or(positive)
        && non_negative(input.linear_damping)
        && input.gravity_scale.is_finite()
        && input.material.valid()
        && positive(input.volume_factor)
        && non_negative(input.friction)
        && non_negative(input.restitution)
}

fn check_particles(
    id: DynamicsSoftBodyId,
    count: usize,
    particles: impl IntoIterator<Item = u32>,
) -> Result<(), DynamicsError> {
    match particles
        .into_iter()
        .find(|particle| *particle as usize >= count)
    {
        Some(particle) => Err(DynamicsSoftBodyError::Particle { body: id, particle }.into()),
        None => Ok(()),
    }
}

pub(crate) fn soft_id(user_data: u128) -> Option<u64> {
    (user_data & SOFT_COLLIDER_TAG != 0).then(|| (user_data & u128::from(u64::MAX)) as u64)
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn non_negative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

#[cfg(test)]
#[path = "soft_body_tests.rs"]
mod tests;
