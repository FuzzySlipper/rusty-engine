//! Soft bodies: cloth and elastic volumes in a Dynamics world, their pins,
//! drive targets and attachments (to bodies and to an animated instance's
//! joints), their forces, and the appearances that draw them.
//!
//! Joint attachments are resolved before each step from the instance's
//! evaluated pose, carried forward by its latest motion. After each step a
//! soft body that moved reshapes its appearances.

use engine_spatial::{
    DynamicsSoftBodyBodyAttachment, DynamicsSoftBodyCellModel, DynamicsSoftBodyForces,
    DynamicsSoftBodyId, DynamicsSoftBodyInput, DynamicsSoftBodyMaterial, DynamicsSoftBodySettings,
    DynamicsSoftBodyShape, DynamicsSoftBodySolver, DynamicsSoftness,
};

use super::*;
use crate::appearance::DeformedShape;

pub(super) enum SoftBodySlot {
    Active(SoftBody),
    Tombstoned,
}

pub(super) struct SoftBody {
    world: u64,
    joints: Option<JointAttachments>,
    /// Appearances drawing it, and whether each draws triangles' backs.
    appearances: Vec<(u64, bool)>,
}

/// The joints and offsets of a soft body's joint attachments, in the order
/// the solver keeps their particles (which tears renumber).
struct JointAttachments {
    instance: u64,
    /// Joint and offset in the joint's frame.
    attachments: Vec<(u32, [f64; 3])>,
}

fn soft_error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_SOFT_BODY", message)
}

fn soft_solver_error(error: DynamicsError) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_SOFT_BODY", error.code())
}

fn softness(value: NativeDynamicsSoftness) -> DynamicsSoftness {
    DynamicsSoftness {
        frequency: f64::from(value.frequency),
        damping_ratio: f64::from(value.damping_ratio),
    }
}

fn material(value: &NativeDynamicsSoftBodyMaterial) -> DynamicsSoftBodyMaterial {
    let positive = |value: f32| (value > 0.0).then_some(f64::from(value));
    DynamicsSoftBodyMaterial {
        edge: softness(value.edge),
        bend: softness(value.bend),
        volume: softness(value.volume),
        shape_matching: softness(value.shape_matching),
        young_modulus: f64::from(value.young_modulus),
        poisson_ratio: f64::from(value.poisson_ratio),
        elastic_damping_ratio: f64::from(value.elastic_damping_ratio),
        deformation_damping: f64::from(value.deformation_damping),
        plastic_yield: f64::from(value.plastic_yield),
        plastic_creep: f64::from(value.plastic_creep),
        plastic_max: f64::from(value.plastic_max),
        edge_plastic_yield: f64::from(value.edge_plastic_yield),
        edge_plastic_creep: f64::from(value.edge_plastic_creep),
        edge_plastic_max: f64::from(value.edge_plastic_max),
        tear_strain: positive(value.tear_strain),
        tear_force: positive(value.tear_force),
        tear_smoothing: f64::from(value.tear_smoothing),
        interior_strength: f64::from(value.interior_strength),
        max_tears_per_step: if value.max_tears_per_step == 0 {
            u32::MAX
        } else {
            value.max_tears_per_step
        },
        min_piece: (value.min_piece > 0).then_some(value.min_piece),
    }
}

fn point(value: NativeVec3) -> [f64; 3] {
    vec3_f64(native_vec3_value(value))
}

/// Groups of `N` indices.
fn grouped<const N: usize>(indices: &[u32]) -> Result<Vec<[u32; N]>, CsharpEngineServicesError> {
    let (groups, rest) = indices.as_chunks::<N>();
    if !rest.is_empty() {
        return Err(soft_error(format!("indices come in groups of {N}")));
    }
    Ok(groups.to_vec())
}

/// `v` turned by the unit quaternion `q` (x, y, z, w).
fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let axis = [q[0], q[1], q[2]];
    let t = cross(axis, v).map(|value| value * 2.0);
    let turn = cross(axis, t);
    std::array::from_fn(|i| v[i] + q[3] * t[i] + turn[i])
}

fn rotation(joint: &render_model::Transform) -> [f64; 4] {
    let q = joint.rotation.map(f64::from);
    let length = q.iter().map(|value| value * value).sum::<f64>().sqrt();
    if length > 0.0 && length.is_finite() {
        q.map(|value| value / length)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// A placement's world transform of `offset`.
fn place(joint: &render_model::Transform, offset: [f64; 3]) -> [f64; 3] {
    let scaled = std::array::from_fn(|axis| offset[axis] * f64::from(joint.scale[axis]));
    let turned = rotate(rotation(joint), scaled);
    std::array::from_fn(|axis| turned[axis] + f64::from(joint.translation[axis]))
}

/// `point` in a placement's frame.
fn local(joint: &render_model::Transform, point: [f64; 3]) -> [f64; 3] {
    let [x, y, z, w] = rotation(joint);
    let relative = std::array::from_fn(|axis| point[axis] - f64::from(joint.translation[axis]));
    let unturned = rotate([-x, -y, -z, w], relative);
    std::array::from_fn(|axis| unturned[axis] / f64::from(joint.scale[axis]))
}

impl RuntimeDynamicsBridge {
    fn active_soft_body(&self, handle: u64) -> Result<&SoftBody, CsharpEngineServicesError> {
        match self.soft_bodies.get(&handle) {
            Some(SoftBodySlot::Active(body)) => Ok(body),
            Some(SoftBodySlot::Tombstoned) => Err(tombstoned("soft body")),
            None => Err(unknown("soft body", handle)),
        }
    }

    fn active_soft_body_mut(
        &mut self,
        handle: u64,
    ) -> Result<&mut SoftBody, CsharpEngineServicesError> {
        match self.soft_bodies.get_mut(&handle) {
            Some(SoftBodySlot::Active(body)) => Ok(body),
            Some(SoftBodySlot::Tombstoned) => Err(tombstoned("soft body")),
            None => Err(unknown("soft body", handle)),
        }
    }

    /// The soft body's world solver.
    fn soft_solver(
        &mut self,
        handle: u64,
    ) -> Result<&mut DynamicsSolver, CsharpEngineServicesError> {
        let world = self.active_soft_body(handle)?.world;
        Ok(&mut self.active_world_mut(world)?.solver)
    }

    fn create_soft_body(
        &mut self,
        request: &NativeDynamicsSoftBodyRequest,
    ) -> Result<NativeDynamicsSoftBodyHandle, CsharpEngineServicesError> {
        self.active_world(request.world.value)?;
        let positions: Vec<[f64; 3]> =
            unsafe { borrowed_slice(request.positions, request.positions_len, "positions") }?
                .iter()
                .map(|position| point(*position))
                .collect();
        let uvs: Vec<[f32; 2]> = unsafe { borrowed_slice(request.uvs, request.uvs_len, "uvs") }?
            .iter()
            .map(|uv| [uv.x, uv.y])
            .collect();
        let indices = unsafe { borrowed_slice(request.indices, request.indices_len, "indices") }?;
        let pinned = unsafe { borrowed_slice(request.pinned, request.pinned_len, "pinned") }?;
        let shape = match request.shape {
            NativeDynamicsSoftBodyShape::Cloth => DynamicsSoftBodyShape::Cloth {
                origin: point(request.origin),
                across: point(request.across),
                down: point(request.down),
                columns: request.resolution_x,
                rows: request.resolution_y,
            },
            NativeDynamicsSoftBodyShape::Box => DynamicsSoftBodyShape::Box {
                center: point(request.origin),
                half_extents: point(request.across),
                particles: [
                    request.resolution_x,
                    request.resolution_y,
                    request.resolution_z,
                ],
            },
            NativeDynamicsSoftBodyShape::Shell => DynamicsSoftBodyShape::Shell {
                center: point(request.origin),
                radius: f64::from(request.radius),
                subdivisions: request.resolution_x,
            },
            NativeDynamicsSoftBodyShape::Surface => DynamicsSoftBodyShape::Surface {
                positions,
                uvs,
                triangles: grouped(indices)?,
            },
            NativeDynamicsSoftBodyShape::Solid => DynamicsSoftBodyShape::Solid {
                positions,
                uvs,
                tetrahedra: grouped(indices)?,
            },
            NativeDynamicsSoftBodyShape::Filled => DynamicsSoftBodyShape::Filled {
                positions,
                uvs,
                triangles: grouped(indices)?,
                cell_size: f64::from(request.cell_size),
                mesh_collision: request.mesh_collision,
            },
        };
        let handle = Self::allocate(&mut self.next_soft_body);
        let input = DynamicsSoftBodyInput {
            id: DynamicsSoftBodyId(handle),
            shape,
            mass: f64::from(request.mass),
            particle_radius: (request.particle_radius > 0.0)
                .then_some(f64::from(request.particle_radius)),
            linear_damping: f64::from(request.linear_damping),
            gravity_scale: f64::from(request.gravity_scale),
            cell_model: match request.cell_model {
                NativeDynamicsSoftBodyCellModel::Volume => DynamicsSoftBodyCellModel::Volume,
                NativeDynamicsSoftBodyCellModel::Corotational => {
                    DynamicsSoftBodyCellModel::Corotational
                }
                NativeDynamicsSoftBodyCellModel::NeoHookean => {
                    DynamicsSoftBodyCellModel::NeoHookean
                }
            },
            solver: match request.solver {
                NativeDynamicsSoftBodySolver::Constraints => DynamicsSoftBodySolver::Constraints,
                NativeDynamicsSoftBodySolver::Fem => DynamicsSoftBodySolver::Fem,
            },
            material: material(&request.material),
            volume_preservation: request.volume_preservation,
            volume_factor: f64::from(request.volume_factor),
            shape_matching: request.shape_matching,
            self_contacts: request.self_contacts,
            collision_groups: request.collision_groups,
            collision_mask: request.collision_mask,
            friction: f64::from(request.friction),
            restitution: f64::from(request.restitution),
            extra_substeps: request.extra_substeps,
            extra_iterations: request.extra_iterations,
            can_sleep: request.can_sleep,
            pinned: pinned.to_vec(),
        };
        self.active_world_mut(request.world.value)?
            .solver
            .insert_soft_body(input)
            .map_err(soft_solver_error)?;
        self.soft_bodies.insert(
            handle,
            SoftBodySlot::Active(SoftBody {
                world: request.world.value,
                joints: None,
                appearances: Vec::new(),
            }),
        );
        Ok(NativeDynamicsSoftBodyHandle { value: handle })
    }

    /// Remove a soft body; its appearances keep the last shape drawn.
    pub(super) fn release_soft_body(&mut self, handle: u64) {
        let Some(SoftBodySlot::Active(body)) =
            self.soft_bodies.insert(handle, SoftBodySlot::Tombstoned)
        else {
            return;
        };
        if let Some(WorldSlot::Active(world)) = self.worlds.get_mut(&body.world) {
            world.solver.remove_soft_body(DynamicsSoftBodyId(handle));
        }
    }

    pub(super) fn soft_bodies_of(&self, world: u64) -> Vec<u64> {
        self.soft_bodies
            .iter()
            .filter_map(|(handle, slot)| match slot {
                SoftBodySlot::Active(body) if body.world == world => Some(*handle),
                _ => None,
            })
            .collect()
    }

    fn configure_soft_bodies(
        &mut self,
        request: NativeDynamicsSoftBodySettingsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        self.active_world_mut(request.world.value)?
            .solver
            .configure_soft_bodies(DynamicsSoftBodySettings {
                substeps: request.substeps,
                max_extra_substeps: request.max_extra_substeps,
                contact_stiffening: f64::from(request.contact_stiffening),
                resweep_strain: f64::from(request.resweep_strain),
            })
            .map_err(soft_solver_error)
    }

    fn update_soft_body_material(
        &mut self,
        request: NativeDynamicsSoftBodyMaterialRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .update_soft_body_material(
                DynamicsSoftBodyId(handle),
                material(&request.material),
                request.volume_preservation,
                f64::from(request.volume_factor),
            )
            .map_err(soft_solver_error)
    }

    fn set_soft_body_pins(
        &mut self,
        request: &NativeDynamicsSoftBodyPinsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let particles =
            unsafe { borrowed_slice(request.particles, request.particles_len, "pinned") }?;
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .set_soft_body_pins(DynamicsSoftBodyId(handle), particles)
            .map_err(soft_solver_error)
    }

    fn set_soft_body_targets(
        &mut self,
        request: &NativeDynamicsSoftBodyTargetsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let targets = unsafe { borrowed_slice(request.targets, request.targets_len, "targets") }?
            .iter()
            .map(|target| (target.particle, point(target.position)))
            .collect::<Vec<_>>();
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .set_soft_body_targets(DynamicsSoftBodyId(handle), &targets)
            .map_err(soft_solver_error)
    }

    fn set_soft_body_body_attachments(
        &mut self,
        request: &NativeDynamicsSoftBodyBodyAttachmentsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let attachments = unsafe {
            borrowed_slice(
                request.attachments,
                request.attachments_len,
                "body attachments",
            )
        }?;
        let handle = request.soft_body.value;
        let world = self.active_soft_body(handle)?.world;
        for attachment in attachments {
            self.world_body(world, attachment.body.value, "CSHARP_SOFT_BODY")?;
        }
        let attachments = attachments
            .iter()
            .map(|attachment| DynamicsSoftBodyBodyAttachment {
                particle: attachment.particle,
                body: DynamicsBodyId(attachment.body.value),
            })
            .collect::<Vec<_>>();
        self.soft_solver(handle)?
            .set_soft_body_attachments(DynamicsSoftBodyId(handle), &attachments)
            .map_err(soft_solver_error)
    }

    fn set_soft_body_joint_attachments(
        &mut self,
        request: &NativeDynamicsSoftBodyJointAttachmentsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let attachments = unsafe {
            borrowed_slice(
                request.attachments,
                request.attachments_len,
                "joint attachments",
            )
        }?;
        let handle = request.soft_body.value;
        let instance = request.instance.value;
        let mut joints = attachments
            .iter()
            .map(|attachment| {
                (
                    attachment.particle,
                    attachment.joint,
                    point(attachment.offset),
                )
            })
            .collect::<Vec<_>>();
        if request.capture_offsets && !joints.is_empty() {
            let source = self
                .appearance()?
                .ragdoll_source(instance, "CSHARP_SOFT_BODY_POSE")?;
            let particles = self
                .soft_solver(handle)?
                .soft_body_particles(DynamicsSoftBodyId(handle))
                .ok_or_else(|| unknown("soft body", handle))?;
            for (particle, joint, offset) in &mut joints {
                let at = particles
                    .get(*particle as usize)
                    .ok_or_else(|| soft_error(format!("no particle {particle}")))?;
                let posed = source
                    .latest
                    .joints
                    .get(*joint as usize)
                    .ok_or_else(|| soft_error(format!("no joint {joint}")))?;
                *offset = local(&posed.world, at.position);
            }
        }
        let particles: Vec<u32> = joints.iter().map(|(particle, _, _)| *particle).collect();
        self.soft_solver(handle)?
            .set_soft_body_joint_particles(DynamicsSoftBodyId(handle), &particles)
            .map_err(soft_solver_error)?;
        self.active_soft_body_mut(handle)?.joints =
            (!joints.is_empty()).then(|| JointAttachments {
                instance,
                attachments: joints
                    .into_iter()
                    .map(|(_, joint, offset)| (joint, offset))
                    .collect(),
            });
        // Driven from the next step on.
        self.drive_soft_body(handle, 0.0)
    }

    /// Before a step of `seconds` of `world`: drive each soft body's joint
    /// attachments where their joints will be.
    pub(super) fn drive_soft_bodies_of(
        &mut self,
        world: u64,
        seconds: f64,
    ) -> Result<(), CsharpEngineServicesError> {
        for handle in self.soft_bodies_of(world) {
            self.drive_soft_body(handle, seconds)?;
        }
        Ok(())
    }

    fn drive_soft_body(
        &mut self,
        handle: u64,
        seconds: f64,
    ) -> Result<(), CsharpEngineServicesError> {
        let Some(joints) = &self.active_soft_body(handle)?.joints else {
            return Ok(());
        };
        let instance = joints.instance;
        let attachments = joints.attachments.clone();
        let source = match self.appearance.is_some() {
            true => self
                .appearance()?
                .ragdoll_source(instance, "CSHARP_SOFT_BODY_POSE")
                .ok(),
            false => None,
        };
        // Where each offset is now, carried forward by the pose's latest
        // motion to the end of the step; with no pose, held where it was.
        let targets: Vec<Option<[f64; 3]>> = attachments
            .iter()
            .map(|(joint, offset)| {
                let source = source.as_ref()?;
                let now = place(&source.latest.joints.get(*joint as usize)?.world, *offset);
                let ahead = source.previous.as_ref().and_then(|previous| {
                    let interval = source.latest.seconds - previous.seconds;
                    let before = place(&previous.joints.get(*joint as usize)?.world, *offset);
                    (interval > 0.0).then_some((before, seconds / interval))
                });
                Some(match ahead {
                    Some((before, ahead)) => {
                        std::array::from_fn(|axis| now[axis] + (now[axis] - before[axis]) * ahead)
                    }
                    None => now,
                })
            })
            .collect();
        self.soft_solver(handle)?
            .set_soft_body_joint_targets(DynamicsSoftBodyId(handle), &targets)
            .map_err(soft_solver_error)
    }

    /// After a step of `world`: reshape the appearances of each soft body
    /// that moved.
    pub(super) fn draw_soft_bodies_of(
        &mut self,
        world: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        if self.appearance.is_none() {
            return Ok(());
        }
        for handle in self.soft_bodies_of(world) {
            let appearances = self.active_soft_body(handle)?.appearances.clone();
            if appearances.is_empty() {
                continue;
            }
            let solver = self.soft_solver(handle)?;
            let id = DynamicsSoftBodyId(handle);
            if !solver.soft_body_moved(id) {
                continue;
            }
            let one_sided = solver.take_soft_body_surface(id, false);
            let two_sided = appearances
                .iter()
                .any(|(_, two_sided)| *two_sided)
                .then(|| solver.soft_body_surface(id, true))
                .flatten();
            let mut live = Vec::new();
            for (appearance, sides) in appearances {
                let Some(surface) = (if sides { &two_sided } else { &one_sided }) else {
                    continue;
                };
                let shape = DeformedShape {
                    positions: &surface.positions,
                    normals: &surface.normals,
                    uvs: &surface.uvs,
                    indices: &surface.indices,
                    bounds_min: surface.bounds_min,
                    bounds_max: surface.bounds_max,
                };
                if self.appearance()?.reshape_deformed(appearance, &shape)? {
                    live.push((appearance, sides));
                }
            }
            self.active_soft_body_mut(handle)?.appearances = live;
        }
        Ok(())
    }

    fn create_soft_body_appearance(
        &mut self,
        request: NativeDynamicsSoftBodyAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let handle = request.soft_body.value;
        let surface = self
            .soft_solver(handle)?
            .soft_body_surface(DynamicsSoftBodyId(handle), request.two_sided)
            .ok_or_else(|| unknown("soft body", handle))?;
        let appearance = self
            .appearance
            .ok_or_else(|| soft_error("soft body appearances need the Graphics service"))?;
        // SAFETY: EngineServiceSet owns both bridges and refreshes this
        // pointer whenever it hands out the function tables.
        let appearance = unsafe { &mut *appearance }.create_deformed_appearance(
            request.material,
            &DeformedShape {
                positions: &surface.positions,
                normals: &surface.normals,
                uvs: &surface.uvs,
                indices: &surface.indices,
                bounds_min: surface.bounds_min,
                bounds_max: surface.bounds_max,
            },
        )?;
        self.active_soft_body_mut(handle)?
            .appearances
            .push((appearance.value, request.two_sided));
        Ok(appearance)
    }

    fn set_soft_body_forces(
        &mut self,
        request: NativeDynamicsSoftBodyForcesRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .set_soft_body_forces(
                DynamicsSoftBodyId(handle),
                DynamicsSoftBodyForces {
                    acceleration: point(request.acceleration),
                    wind: point(request.wind),
                    drag: f64::from(request.drag),
                },
            )
            .map_err(soft_solver_error)
    }

    fn apply_soft_body_velocity_change(
        &mut self,
        request: NativeDynamicsSoftBodyVelocityChangeRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .apply_soft_body_velocity_change(
                DynamicsSoftBodyId(handle),
                point(request.point),
                point(request.velocity_change),
                f64::from(request.radius),
            )
            .map_err(soft_solver_error)
    }

    fn apply_soft_body_radial_velocity_change(
        &mut self,
        request: NativeDynamicsSoftBodyRadialRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.soft_body.value;
        self.soft_solver(handle)?
            .apply_soft_body_radial_velocity_change(
                DynamicsSoftBodyId(handle),
                point(request.center),
                f64::from(request.speed),
                f64::from(request.radius),
            )
            .map_err(soft_solver_error)
    }

    fn read_soft_body(
        &mut self,
        handle: NativeDynamicsSoftBodyHandle,
    ) -> Result<NativeDynamicsSoftBodyReadout, CsharpEngineServicesError> {
        let readout = self
            .soft_solver(handle.value)?
            .soft_body(DynamicsSoftBodyId(handle.value))
            .ok_or_else(|| unknown("soft body", handle.value))?;
        let vector = |value: [f64; 3]| native_vec3(vec3_f32(value));
        Ok(NativeDynamicsSoftBodyReadout {
            particle_count: readout.particle_count,
            piece_count: readout.piece_count,
            center_of_mass: vector(readout.center_of_mass),
            linear_velocity: vector(readout.linear_velocity),
            bounds_min: vector(readout.bounds_min),
            bounds_max: vector(readout.bounds_max),
            volume: readout.volume as f32,
            rest_volume: readout.rest_volume as f32,
            max_stretch: readout.max_stretch as f32,
            contact_count: readout.contact_count,
            sleeping: readout.sleeping,
            topology_version: readout.topology_version,
        })
    }

    fn read_soft_body_particles(
        &mut self,
        handle: NativeDynamicsSoftBodyHandle,
    ) -> Result<NativeDynamicsSoftBodyParticlesResult, CsharpEngineServicesError> {
        let particles = self
            .soft_solver(handle.value)?
            .soft_body_particles(DynamicsSoftBodyId(handle.value))
            .ok_or_else(|| unknown("soft body", handle.value))?;
        self.soft_particles.clear();
        self.soft_particles.extend(particles.iter().map(|particle| {
            NativeDynamicsSoftBodyParticle {
                position: native_vec3(vec3_f32(particle.position)),
                velocity: native_vec3(vec3_f32(particle.velocity)),
                mass: particle.mass as f32,
                kinematic: particle.kinematic,
                on_surface: particle.on_surface,
            }
        }));
        Ok(NativeDynamicsSoftBodyParticlesResult {
            particles: self.soft_particles.as_ptr(),
            particles_len: self.soft_particles.len(),
        })
    }
}

fn soft_call<T>(
    context: *mut c_void,
    receipt: *mut NativeOperationErrorReceipt,
    output: *mut T,
    call: impl FnOnce(&mut RuntimeDynamicsBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    super::ragdoll::ragdoll_call(context, receipt, output, call)
}

pub(super) unsafe extern "C" fn create_soft_body(
    context: *mut c_void,
    request: *const NativeDynamicsSoftBodyRequest,
    handle: *mut NativeDynamicsSoftBodyHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    soft_call(context, receipt, handle, |bridge| {
        bridge.create_soft_body(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn destroy_soft_body(
    context: *mut c_void,
    handle: NativeDynamicsSoftBodyHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        match bridge.soft_bodies.get(&handle.value) {
            None => return Err(unknown("soft body", handle.value)),
            Some(SoftBodySlot::Tombstoned) => return Ok(()),
            Some(SoftBodySlot::Active(_)) => {}
        }
        bridge.release_soft_body(handle.value);
        Ok(())
    })
}

pub(super) unsafe extern "C" fn configure_soft_bodies(
    context: *mut c_void,
    request: NativeDynamicsSoftBodySettingsRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.configure_soft_bodies(request)
    })
}

pub(super) unsafe extern "C" fn update_soft_body_material(
    context: *mut c_void,
    request: NativeDynamicsSoftBodyMaterialRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.update_soft_body_material(request)
    })
}

pub(super) unsafe extern "C" fn set_soft_body_pins(
    context: *mut c_void,
    request: *const NativeDynamicsSoftBodyPinsRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.set_soft_body_pins(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn set_soft_body_targets(
    context: *mut c_void,
    request: *const NativeDynamicsSoftBodyTargetsRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.set_soft_body_targets(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn set_soft_body_body_attachments(
    context: *mut c_void,
    request: *const NativeDynamicsSoftBodyBodyAttachmentsRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.set_soft_body_body_attachments(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn set_soft_body_joint_attachments(
    context: *mut c_void,
    request: *const NativeDynamicsSoftBodyJointAttachmentsRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.set_soft_body_joint_attachments(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn set_soft_body_forces(
    context: *mut c_void,
    request: NativeDynamicsSoftBodyForcesRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.set_soft_body_forces(request)
    })
}

pub(super) unsafe extern "C" fn apply_soft_body_velocity_change(
    context: *mut c_void,
    request: NativeDynamicsSoftBodyVelocityChangeRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.apply_soft_body_velocity_change(request)
    })
}

pub(super) unsafe extern "C" fn apply_soft_body_radial_velocity_change(
    context: *mut c_void,
    request: NativeDynamicsSoftBodyRadialRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        bridge.apply_soft_body_radial_velocity_change(request)
    })
}

pub(super) unsafe extern "C" fn read_soft_body(
    context: *mut c_void,
    handle: NativeDynamicsSoftBodyHandle,
    result: *mut NativeDynamicsSoftBodyReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    soft_call(context, receipt, result, |bridge| {
        bridge.read_soft_body(handle)
    })
}

pub(super) unsafe extern "C" fn read_soft_body_particles(
    context: *mut c_void,
    handle: NativeDynamicsSoftBodyHandle,
    result: *mut NativeDynamicsSoftBodyParticlesResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    soft_call(context, receipt, result, |bridge| {
        bridge.read_soft_body_particles(handle)
    })
}

pub(super) unsafe extern "C" fn create_soft_body_appearance(
    context: *mut c_void,
    request: NativeDynamicsSoftBodyAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    soft_call(context, receipt, result, |bridge| {
        bridge.create_soft_body_appearance(request)
    })
}

pub(super) unsafe extern "C" fn destroy_soft_body_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    soft_call(context, receipt, &mut done, |bridge| {
        for slot in bridge.soft_bodies.values_mut() {
            if let SoftBodySlot::Active(body) = slot {
                body.appearances
                    .retain(|(drawn, _)| *drawn != appearance.value);
            }
        }
        let graphics = bridge
            .appearance
            .ok_or_else(|| soft_error("soft body appearances need the Graphics service"))?;
        // SAFETY: EngineServiceSet owns both bridges and refreshes this
        // pointer whenever it hands out the function tables.
        unsafe { &mut *graphics }.destroy_appearance(appearance)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::appearance::{
        character_for_test, end_call_posed, material_for_test, place_character_for_test,
        show_for_test, RuntimeAppearanceBridge,
    };

    const BODY_GLB: &[u8] =
        include_bytes!("../../../../../fixtures/csharp-joint-attachments/content/body.glb");
    const TICK: f32 = 1.0 / 60.0;
    const COLUMNS: u32 = 6;
    const ROWS: u32 = 4;

    fn vec3(x: f32, y: f32, z: f32) -> NativeVec3 {
        NativeVec3 { x, y, z }
    }

    fn softness(frequency: f32) -> NativeDynamicsSoftness {
        NativeDynamicsSoftness {
            frequency,
            damping_ratio: 1.0,
        }
    }

    fn cloth(
        world: NativeDynamicsWorldHandle,
        origin: NativeVec3,
        pinned: &[u32],
    ) -> NativeDynamicsSoftBodyRequest {
        NativeDynamicsSoftBodyRequest {
            world,
            shape: NativeDynamicsSoftBodyShape::Cloth,
            origin,
            across: vec3(0.1, 0.0, 0.0),
            down: vec3(0.0, -0.1, 0.0),
            radius: 0.0,
            cell_size: 0.0,
            resolution_x: COLUMNS,
            resolution_y: ROWS,
            resolution_z: 0,
            positions: std::ptr::null(),
            positions_len: 0,
            uvs: std::ptr::null(),
            uvs_len: 0,
            indices: std::ptr::null(),
            indices_len: 0,
            pinned: pinned.as_ptr(),
            pinned_len: pinned.len(),
            mass: 0.5,
            particle_radius: 0.0,
            linear_damping: 0.1,
            gravity_scale: 1.0,
            cell_model: NativeDynamicsSoftBodyCellModel::Volume,
            solver: NativeDynamicsSoftBodySolver::Constraints,
            material: NativeDynamicsSoftBodyMaterial {
                edge: softness(30.0),
                bend: softness(10.0),
                volume: softness(30.0),
                shape_matching: softness(10.0),
                young_modulus: 1.0e4,
                poisson_ratio: 0.3,
                elastic_damping_ratio: 1.0,
                deformation_damping: 0.0,
                plastic_yield: 0.0,
                plastic_creep: 1.0,
                plastic_max: 1.0,
                edge_plastic_yield: 0.0,
                edge_plastic_creep: 1.0,
                edge_plastic_max: 0.5,
                tear_strain: 0.0,
                tear_force: 0.0,
                tear_smoothing: 0.0,
                interior_strength: 1.0,
                max_tears_per_step: 0,
                min_piece: 0,
            },
            volume_preservation: false,
            volume_factor: 1.0,
            shape_matching: false,
            self_contacts: false,
            mesh_collision: false,
            collision_groups: u32::MAX,
            collision_mask: u32::MAX,
            friction: 0.5,
            restitution: 0.0,
            extra_substeps: 0,
            extra_iterations: 3,
            can_sleep: true,
        }
    }

    fn step(dynamics: &mut RuntimeDynamicsBridge, world: NativeDynamicsWorldHandle, steps: u32) {
        let actions: [NativeDynamicsAction; 0] = [];
        dynamics
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: TICK,
                steps,
                actions: actions.as_ptr(),
                actions_len: 0,
            })
            .unwrap();
    }

    fn payloads(
        appearance: &mut RuntimeAppearanceBridge,
    ) -> Vec<render_model::MeshPayloadDescriptor> {
        let call = appearance.take_staged_call();
        let payloads = call
            .render_ops()
            .into_iter()
            .filter_map(|op| match op {
                render_model::RenderDiff::ReplaceMeshPayload { payload, .. } => Some(payload),
                _ => None,
            })
            .collect();
        appearance.commit(call);
        appearance.begin_call();
        payloads
    }

    fn world(dynamics: &mut RuntimeDynamicsBridge) -> NativeDynamicsWorldHandle {
        dynamics
            .create_world(NativeDynamicsWorldConfig {
                gravity: vec3(0.0, -9.81, 0.0),
            })
            .unwrap()
    }

    #[test]
    fn a_soft_body_appearance_is_reshaped_by_the_steps_that_move_it() {
        let mut appearance = crate::appearance::create(Default::default(), BTreeMap::new());
        appearance.begin_call();
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut dynamics = RuntimeDynamicsBridge::new(spatial.collision_source());
        dynamics.bind_appearance(&mut appearance);
        let world = world(&mut dynamics);
        let pins: Vec<u32> = (0..COLUMNS).map(|column| column * ROWS).collect();
        let soft_body = dynamics
            .create_soft_body(&cloth(world, vec3(0.0, 2.0, 0.0), &pins))
            .unwrap();
        let material = material_for_test(&mut appearance);
        let drawn = dynamics
            .create_soft_body_appearance(NativeDynamicsSoftBodyAppearanceRequest {
                soft_body,
                material,
                two_sided: true,
            })
            .unwrap();
        show_for_test(&mut appearance, drawn);
        // Shown in its first shape: both sides of every particle.
        let first = payloads(&mut appearance);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].layout.vertex_count, COLUMNS * ROWS * 2);
        assert_eq!(first[0].bounds.max[1], 2.0);

        dynamics.bind_appearance(&mut appearance);
        step(&mut dynamics, world, 10);
        let moved = payloads(&mut appearance);
        assert_eq!(moved.len(), 1);
        assert_ne!(moved[0].source, first[0].source);

        let read = dynamics.read_soft_body(soft_body).unwrap();
        assert_eq!(read.particle_count, COLUMNS * ROWS);
        let particles = dynamics.read_soft_body_particles(soft_body).unwrap();
        let particles =
            unsafe { std::slice::from_raw_parts(particles.particles, particles.particles_len) }
                .to_vec();
        assert_eq!(particles.len(), (COLUMNS * ROWS) as usize);
        assert!(particles[0].kinematic && !particles[1].kinematic);

        // Gone, it is drawn as it was last.
        dynamics.bind_appearance(&mut appearance);
        let status = unsafe {
            destroy_soft_body(
                (&mut dynamics as *mut RuntimeDynamicsBridge).cast(),
                soft_body,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, ABI_OK);
        step(&mut dynamics, world, 10);
        assert!(payloads(&mut appearance).is_empty());
        assert!(dynamics.read_soft_body(soft_body).is_err());
    }

    #[test]
    fn joint_attachments_carry_cloth_with_the_posed_character() {
        let mut appearance = crate::appearance::create(
            Default::default(),
            BTreeMap::from([("body.glb".to_owned(), crate::ContentBytes::from(BODY_GLB))]),
        );
        let mut scene = render_presentation::PresentationWorld::default();
        scene.advance_elapsed(1.0);
        appearance.set_presentation_clock(1.0, 0.0);
        appearance.begin_call();
        let (instance, joints) = character_for_test(&mut appearance);
        end_call_posed(&mut appearance, &mut scene, 0.0);

        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut dynamics = RuntimeDynamicsBridge::new(spatial.collision_source());
        appearance.set_presentation_clock(1.0, TICK as f64);
        appearance.begin_call();
        dynamics.bind_appearance(&mut appearance);
        let world = world(&mut dynamics);
        let soft_body = dynamics
            .create_soft_body(&cloth(world, vec3(-0.25, 1.1, -0.2), &[]))
            .unwrap();
        let top: Vec<u32> = (0..COLUMNS).map(|column| column * ROWS).collect();
        let attachments: Vec<NativeDynamicsSoftBodyJointAttachment> = top
            .iter()
            .map(|particle| NativeDynamicsSoftBodyJointAttachment {
                particle: *particle,
                joint: joints["Spine"],
                offset: NativeVec3::default(),
            })
            .collect();
        dynamics
            .set_soft_body_joint_attachments(&NativeDynamicsSoftBodyJointAttachmentsRequest {
                soft_body,
                instance,
                attachments: attachments.as_ptr(),
                attachments_len: attachments.len(),
                capture_offsets: true,
            })
            .unwrap();
        let before = dynamics
            .active_world(world.value)
            .unwrap()
            .solver
            .soft_body_particles(DynamicsSoftBodyId(soft_body.value))
            .unwrap();
        step(&mut dynamics, world, 1);
        let held = dynamics
            .active_world(world.value)
            .unwrap()
            .solver
            .soft_body_particles(DynamicsSoftBodyId(soft_body.value))
            .unwrap();
        for particle in &top {
            let (a, b) = (
                before[*particle as usize].position,
                held[*particle as usize].position,
            );
            assert!(
                (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1.0e-4),
                "{a:?} {b:?}"
            );
            assert!(held[*particle as usize].kinematic);
        }
        // The character rises half a metre over a call: the top edge goes
        // with it, and the free cloth hangs below.
        place_character_for_test(&mut appearance, instance, 0.5);
        end_call_posed(&mut appearance, &mut scene, TICK as f64);
        appearance.set_presentation_clock(1.0, 2.0 * TICK as f64);
        appearance.begin_call();
        dynamics.bind_appearance(&mut appearance);
        step(&mut dynamics, world, 1);
        let lifted = dynamics
            .active_world(world.value)
            .unwrap()
            .solver
            .soft_body_particles(DynamicsSoftBodyId(soft_body.value))
            .unwrap();
        for particle in &top {
            let rise =
                lifted[*particle as usize].position[1] - held[*particle as usize].position[1];
            // The pose rose 0.5 m in a tick; carried forward a tick, the
            // edge meets it where it is drawn now and keeps up.
            assert!(rise > 0.45, "{rise}");
        }
        // Ending them frees the edge.
        dynamics
            .set_soft_body_joint_attachments(&NativeDynamicsSoftBodyJointAttachmentsRequest {
                soft_body,
                instance,
                attachments: std::ptr::null(),
                attachments_len: 0,
                capture_offsets: false,
            })
            .unwrap();
        let freed = dynamics
            .active_world(world.value)
            .unwrap()
            .solver
            .soft_body_particles(DynamicsSoftBodyId(soft_body.value))
            .unwrap();
        assert!(!freed[0].kinematic);
        // Destroying the world ends the soft body.
        dynamics.destroy_world(world).unwrap();
        assert!(dynamics.read_soft_body(soft_body).is_err());
    }
}
