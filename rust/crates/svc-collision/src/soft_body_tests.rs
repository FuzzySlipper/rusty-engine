use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::VoxelValue;
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

use super::*;
use crate::{CollisionProjection, DynamicsBodyInput, DynamicsShape, DynamicsSoftBodySkin};

const TICK: f64 = 1.0 / 60.0;
const GRAVITY: [f64; 3] = [0.0, -9.81, 0.0];
const SHEET: DynamicsSoftBodyId = DynamicsSoftBodyId(1);
const COLUMNS: u32 = 12;
const ROWS: u32 = 8;

/// A one-voxel-thick floor at y = -1..0 over x and z 0..8.
fn floor() -> CollisionProjection {
    let spec = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap();
    let mut world = VoxelWorld::new(spec);
    let mut chunk = VoxelChunk::from_spec(&spec);
    for x in 0..8 {
        for z in 0..8 {
            chunk
                .set(LocalVoxelCoord::new(x, 7, z), VoxelValue::solid_raw(1))
                .unwrap();
        }
    }
    world.insert(ChunkCoord::new(0, -1, 0), chunk);
    world.drain_dirty();
    CollisionProjection::build(&world)
}

fn input(id: DynamicsSoftBodyId, shape: DynamicsSoftBodyShape) -> DynamicsSoftBodyInput {
    DynamicsSoftBodyInput {
        id,
        shape,
        mass: 1.0,
        particle_radius: None,
        linear_damping: 0.1,
        gravity_scale: 1.0,
        cell_model: DynamicsSoftBodyCellModel::Volume,
        solver: DynamicsSoftBodySolver::Constraints,
        material: DynamicsSoftBodyMaterial::default(),
        volume_preservation: false,
        volume_factor: 1.0,
        shape_matching: false,
        self_contacts: false,
        collision_groups: u32::MAX,
        collision_mask: u32::MAX,
        friction: 0.5,
        restitution: 0.0,
        extra_substeps: 0,
        extra_iterations: 3,
        can_sleep: true,
        pinned: Vec::new(),
    }
}

/// A 1.1 m × 0.7 m banner hanging in the x/y plane from its top row at
/// y = 3, its particle `column * ROWS + row`.
fn banner() -> DynamicsSoftBodyInput {
    let mut banner = input(
        SHEET,
        DynamicsSoftBodyShape::Cloth {
            origin: [2.0, 3.0, 2.0],
            across: [0.1, 0.0, 0.0],
            down: [0.0, -0.1, 0.0],
            columns: COLUMNS,
            rows: ROWS,
        },
    );
    banner.pinned = (0..COLUMNS).map(|column| column * ROWS).collect();
    banner
}

fn jelly(center: [f64; 3]) -> DynamicsSoftBodyInput {
    let mut jelly = input(
        DynamicsSoftBodyId(2),
        DynamicsSoftBodyShape::Box {
            center,
            half_extents: [0.3; 3],
            particles: [5; 3],
        },
    );
    jelly.mass = 10.0;
    jelly.particle_radius = Some(0.02);
    jelly.cell_model = DynamicsSoftBodyCellModel::Corotational;
    jelly.material.young_modulus = 2.0e4;
    jelly.material.poisson_ratio = 0.4;
    jelly.volume_preservation = true;
    jelly
}

fn ball(id: u64, at: [f64; 3], radius: f64, held: bool) -> DynamicsBodyInput {
    DynamicsBodyInput {
        id: DynamicsBodyId(id),
        translation: at,
        rotation: [0.0, 0.0, 0.0, 1.0],
        shape: DynamicsShape::Sphere { radius },
        mass: 4.0,
        mass_properties: None,
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
        locked_translation_axes: [held; 3],
        locked_rotation_axes: [held; 3],
        linear_damping: 0.0,
        angular_damping: 0.0,
        gravity_scale: if held { 0.0 } else { 1.0 },
        friction: 0.5,
        restitution: 0.0,
        collision_groups: u32::MAX,
        collision_mask: u32::MAX,
        enabled: true,
        sleeping: false,
        continuous_collision: false,
    }
}

fn steps(solver: &mut DynamicsSolver, ticks: usize) {
    for _ in 0..ticks {
        solver.step(TICK, 1, &[]).unwrap();
    }
}

fn particles(solver: &DynamicsSolver, id: DynamicsSoftBodyId) -> Vec<DynamicsSoftBodyParticle> {
    solver.soft_body_particles(id).unwrap()
}

fn height(readout: &DynamicsSoftBodyReadout) -> f64 {
    readout.bounds_max[1] - readout.bounds_min[1]
}

#[test]
fn a_banner_hangs_from_its_pins_and_the_wind_lifts_it() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.insert_soft_body(banner()).unwrap();
    let before = particles(&solver, SHEET);
    steps(&mut solver, 120);
    let hung = particles(&solver, SHEET);
    for column in 0..COLUMNS {
        let pin = (column * ROWS) as usize;
        assert_eq!(hung[pin].position, before[pin].position);
        assert!(hung[pin].kinematic);
    }
    assert!(hung
        .iter()
        .all(|p| p.position.iter().all(|v| v.is_finite())));
    let still = solver.soft_body(SHEET).unwrap();
    assert!(still.max_stretch < 0.1, "{still:?}");
    assert!((still.center_of_mass[2] - 2.0).abs() < 0.01, "{still:?}");

    solver
        .set_soft_body_forces(
            SHEET,
            DynamicsSoftBodyForces {
                wind: [0.0, 0.0, 8.0],
                drag: 0.6,
                ..Default::default()
            },
        )
        .unwrap();
    // The banner swings downwind and keeps moving in the gusts it makes.
    let mut swings = Vec::new();
    for _ in 0..12 {
        steps(&mut solver, 15);
        swings.push(solver.soft_body(SHEET).unwrap().center_of_mass[2] - 2.0);
    }
    assert!(swings.iter().all(|z| *z > 0.05), "{swings:?}");
    let blown = solver.soft_body(SHEET).unwrap();
    assert!(!blown.sleeping);
    assert!(blown.max_stretch < 0.2, "{blown:?}");
    // The pins still hold.
    let pinned = particles(&solver, SHEET);
    assert_eq!(pinned[0].position, before[0].position);

    // Calm again: the forces are retained until replaced.
    solver
        .set_soft_body_forces(SHEET, DynamicsSoftBodyForces::default())
        .unwrap();
    // With the wind gone it swings back through hanging straight.
    let mut back = f64::INFINITY;
    for _ in 0..120 {
        steps(&mut solver, 1);
        back = back.min(solver.soft_body(SHEET).unwrap().center_of_mass[2] - 2.0);
    }
    assert!(back < 0.0, "{back}");
}

#[test]
fn cloth_drapes_over_a_body_and_the_floor_without_passing_through() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.bind_environment(&floor());
    solver
        .insert_body(ball(9, [3.0, 0.4, 3.0], 0.4, true))
        .unwrap();
    let mut sheet = input(
        SHEET,
        DynamicsSoftBodyShape::Cloth {
            origin: [2.2, 1.1, 2.2],
            across: [0.1, 0.0, 0.0],
            down: [0.0, 0.0, 0.1],
            columns: 17,
            rows: 17,
        },
    );
    sheet.particle_radius = Some(0.02);
    solver.insert_soft_body(sheet).unwrap();
    let mut closest = f64::INFINITY;
    let mut lowest = f64::INFINITY;
    for _ in 0..180 {
        steps(&mut solver, 1);
        for particle in particles(&solver, SHEET) {
            let p = particle.position;
            let d = ((p[0] - 3.0).powi(2) + (p[1] - 0.4).powi(2) + (p[2] - 3.0).powi(2)).sqrt();
            closest = closest.min(d);
            lowest = lowest.min(p[1]);
        }
    }
    let draped = solver.soft_body(SHEET).unwrap();
    assert!(closest > 0.4 - 0.03, "closest {closest}");
    assert!(lowest > -0.03, "lowest {lowest}");
    // Held up by the ball in the middle, lying on the floor at the edges.
    assert!(draped.bounds_max[1] > 0.75, "{draped:?}");
    assert!(draped.bounds_min[1] < 0.1, "{draped:?}");
    assert!(draped.contact_count >= 2, "{draped:?}");
    // The rigid contact list holds rigid bodies only.
    assert!(solver.contacts().iter().all(|c| c.first.0 == 9));
}

#[test]
fn a_jelly_squashes_on_landing_and_recovers_its_shape() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.bind_environment(&floor());
    solver.insert_soft_body(jelly([4.0, 1.5, 4.0])).unwrap();
    let rest = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
    let mut squashed = f64::INFINITY;
    let mut lowest = f64::INFINITY;
    for _ in 0..300 {
        steps(&mut solver, 1);
        let now = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
        if now.bounds_min[1] < 0.1 {
            squashed = squashed.min(height(&now));
        }
        lowest = lowest.min(now.bounds_min[1]);
    }
    let settled = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
    assert!(
        squashed < height(&rest) * 0.97,
        "{squashed} of {}",
        height(&rest)
    );
    assert!(
        (height(&settled) - height(&rest)).abs() < height(&rest) * 0.05,
        "{settled:?}"
    );
    assert!(settled.volume > settled.rest_volume * 0.97, "{settled:?}");
    assert!(lowest > -0.03, "{lowest}");
    assert!(settled.bounds_min[1] < 0.05);
}

#[test]
fn a_falling_ball_dents_a_jelly_that_springs_back() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.bind_environment(&floor());
    let mut soft = jelly([4.0, 0.32, 4.0]);
    soft.material.young_modulus = 5.0e3;
    solver.insert_soft_body(soft).unwrap();
    steps(&mut solver, 60);
    let rest = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
    let mut falling = ball(9, [4.0, 2.0, 4.0], 0.15, false);
    falling.linear_velocity = [0.0, -6.0, 0.0];
    solver.insert_body(falling).unwrap();
    let mut dented = f64::INFINITY;
    let mut touched = 0;
    for _ in 0..240 {
        steps(&mut solver, 1);
        let now = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
        dented = dented.min(now.bounds_max[1]);
        touched = touched.max(now.contact_count);
    }
    assert!(dented < rest.bounds_max[1] - 0.03, "{dented} {rest:?}");
    assert!(touched >= 2);
    let after = solver.soft_body(DynamicsSoftBodyId(2)).unwrap();
    assert!(after.volume > after.rest_volume * 0.95, "{after:?}");
}

#[test]
fn targets_drive_particles_while_pins_hold_and_release_frees_them() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.insert_soft_body(banner()).unwrap();
    let start = particles(&solver, SHEET);
    let corner = (COLUMNS - 1) * ROWS;
    let mut target = start[corner as usize].position;
    target[2] += 0.3;
    solver
        .set_soft_body_targets(SHEET, &[(corner, target)])
        .unwrap();
    steps(&mut solver, 1);
    let driven = particles(&solver, SHEET);
    for (driven, target) in driven[corner as usize].position.iter().zip(target) {
        assert!((driven - target).abs() < 1.0e-9);
    }
    // The corner was pinned too: dropping the target holds it where it is.
    solver.set_soft_body_targets(SHEET, &[]).unwrap();
    steps(&mut solver, 30);
    assert_eq!(particles(&solver, SHEET)[corner as usize].position, target);
    // Unpinning it lets it fall.
    let pins: Vec<u32> = (0..COLUMNS - 1).map(|column| column * ROWS).collect();
    solver.set_soft_body_pins(SHEET, &pins).unwrap();
    steps(&mut solver, 30);
    let freed = particles(&solver, SHEET);
    assert!(!freed[corner as usize].kinematic);
    assert!(freed[corner as usize].position[1] < target[1] - 0.01);
    // A driven particle that was never pinned goes free when released.
    let free = ROWS - 1;
    let mut low = freed[free as usize].position;
    low[1] -= 0.2;
    solver.set_soft_body_targets(SHEET, &[(free, low)]).unwrap();
    steps(&mut solver, 1);
    assert!(particles(&solver, SHEET)[free as usize].kinematic);
    solver.set_soft_body_targets(SHEET, &[]).unwrap();
    assert!(!particles(&solver, SHEET)[free as usize].kinematic);
    assert!(matches!(
        solver.set_soft_body_pins(SHEET, &[COLUMNS * ROWS]),
        Err(DynamicsError::SoftBody(
            DynamicsSoftBodyError::Particle { .. }
        ))
    ));
}

#[test]
fn a_body_attachment_pulls_both_ways_and_ends_with_the_body() {
    let mut solver = DynamicsSolver::new([0.0; 3]);
    let mut sheet = banner();
    sheet.pinned.clear();
    solver.insert_soft_body(sheet).unwrap();
    let corner = start_of(&solver, 0);
    let mut handle = ball(9, corner, 0.05, false);
    handle.collision_groups = 0;
    solver.insert_body(handle).unwrap();
    solver
        .set_soft_body_attachments(
            SHEET,
            &[DynamicsSoftBodyBodyAttachment {
                particle: 0,
                body: DynamicsBodyId(9),
            }],
        )
        .unwrap();
    solver
        .set_body_motion(
            DynamicsBodyId(9),
            corner,
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 2.0],
            [0.0; 3],
            false,
        )
        .unwrap();
    steps(&mut solver, 30);
    let pulled = solver.soft_body(SHEET).unwrap();
    let body = solver.body(DynamicsBodyId(9)).unwrap();
    // The cloth follows the body; the body is slowed by the cloth it drags.
    assert!(pulled.center_of_mass[2] > 2.05, "{pulled:?}");
    assert!(body.linear_velocity[2] < 1.9, "{body:?}");
    let attached = particles(&solver, SHEET)[0].position;
    let gap = (0..3)
        .map(|axis| (attached[axis] - body.translation[axis]).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(gap < 0.02, "{gap}");
    solver.remove_body(DynamicsBodyId(9));
    steps(&mut solver, 10);
    assert!(solver.soft_body(SHEET).is_some());
}

fn start_of(solver: &DynamicsSolver, particle: usize) -> [f64; 3] {
    particles(solver, SHEET)[particle].position
}

#[test]
fn a_rebase_moves_soft_bodies_and_their_targets() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.insert_soft_body(banner()).unwrap();
    steps(&mut solver, 20);
    let target = [5.0, 3.0, 2.0];
    solver.set_soft_body_targets(SHEET, &[(0, target)]).unwrap();
    let before = particles(&solver, SHEET);
    solver.translate([-16.0, 0.0, 8.0]);
    let after = particles(&solver, SHEET);
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(after.position[0], before.position[0] - 16.0);
        assert_eq!(after.position[2], before.position[2] + 8.0);
        assert_eq!(after.velocity, before.velocity);
    }
    steps(&mut solver, 1);
    let moved = particles(&solver, SHEET)[0].position;
    assert!(
        (moved[0] + 11.0).abs() < 1.0e-9 && (moved[2] - 10.0).abs() < 1.0e-9,
        "{moved:?}"
    );
}

#[test]
fn the_drawn_surface_has_uvs_unit_normals_bounds_and_an_optional_back() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.insert_soft_body(banner()).unwrap();
    assert!(solver.soft_body_moved(SHEET));
    let front = solver.take_soft_body_surface(SHEET, false).unwrap();
    assert!(!solver.soft_body_moved(SHEET));
    assert_eq!(front.positions.len(), (COLUMNS * ROWS) as usize);
    assert_eq!(
        front.indices.len(),
        ((COLUMNS - 1) * (ROWS - 1) * 6) as usize
    );
    let corner = front
        .positions
        .iter()
        .position(|p| p[0] == 2.0 && p[1] == 3.0)
        .unwrap();
    assert_eq!(front.uvs[corner], [0.0, 0.0]);
    let far = front
        .positions
        .iter()
        .position(|p| (p[0] - 3.1).abs() < 1.0e-5 && (p[1] - 2.3).abs() < 1.0e-5)
        .unwrap();
    assert_eq!(front.uvs[far], [1.0, 1.0]);
    for normal in &front.normals {
        let length = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1.0e-5);
        assert!(normal[2].abs() > 0.99);
    }
    assert_eq!(front.bounds_min, [2.0, 2.3, 2.0]);
    let both = solver.take_soft_body_surface(SHEET, true).unwrap();
    assert_eq!(both.positions.len(), front.positions.len() * 2);
    assert_eq!(both.indices.len(), front.indices.len() * 2);
    let back = front.positions.len();
    assert_eq!(both.normals[back], front.normals[0].map(|v| -v));
    // A sleeping body is not drawn again.
    steps(&mut solver, 1);
    assert!(solver.soft_body_moved(SHEET));
}

#[test]
fn a_torn_banner_draws_and_reads_every_piece() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    let mut torn = banner();
    torn.material.tear_strain = Some(0.4);
    solver.insert_soft_body(torn).unwrap();
    steps(&mut solver, 10);
    let before = solver.soft_body(SHEET).unwrap();
    // Yank the bottom middle down far faster than the cloth can follow.
    let bottom = (COLUMNS / 2) * ROWS + ROWS - 1;
    let mut far = particles(&solver, SHEET)[bottom as usize].position;
    far[1] -= 3.0;
    solver
        .set_soft_body_targets(SHEET, &[(bottom, far)])
        .unwrap();
    steps(&mut solver, 30);
    let after = solver.soft_body(SHEET).unwrap();
    assert_ne!(after.topology_version, before.topology_version, "{after:?}");
    assert!(after.particle_count > before.particle_count, "{after:?}");
    let surface = solver.soft_body_surface(SHEET, false).unwrap();
    assert!(surface.positions.iter().flatten().all(|v| v.is_finite()));
    assert!(surface
        .uvs
        .iter()
        .flatten()
        .all(|v| (0.0..=1.0).contains(v)));
    // Copies of split particles keep their UVs: nearly every vertex has one
    // away from the pinned corner's (0, 0).
    let unmapped = surface.uvs.iter().filter(|uv| **uv == [0.0, 0.0]).count();
    assert!(unmapped <= 2, "{unmapped} of {}", surface.uvs.len());
}

#[test]
fn a_filled_mesh_is_drawn_as_its_own_triangles() {
    // A 0.6 m cube, outward wound.
    let positions: Vec<[f64; 3]> = (0..8)
        .map(|corner| {
            [
                4.0 + 0.6 * f64::from(corner & 1),
                1.0 + 0.6 * f64::from((corner >> 1) & 1),
                4.0 + 0.6 * f64::from((corner >> 2) & 1),
            ]
        })
        .collect();
    let triangles = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    let uvs = (0..8).map(|corner| [corner as f32 / 7.0, 0.5]).collect();
    let mut filled = input(
        DynamicsSoftBodyId(3),
        DynamicsSoftBodyShape::Filled {
            positions,
            uvs,
            triangles,
            cell_size: 0.2,
            mesh_collision: false,
        },
    );
    filled.cell_model = DynamicsSoftBodyCellModel::Corotational;
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.bind_environment(&floor());
    solver.insert_soft_body(filled).unwrap();
    steps(&mut solver, 90);
    let surface = solver
        .soft_body_surface(DynamicsSoftBodyId(3), false)
        .unwrap();
    assert_eq!(surface.positions.len(), 8);
    assert_eq!(surface.indices.len(), 36);
    assert_eq!(surface.uvs[7], [1.0, 0.5]);
    // The cells' cage stands off the mesh, so it rests above the floor.
    assert!(
        surface.bounds_min[1] > -0.05 && surface.bounds_min[1] < 0.4,
        "{surface:?}"
    );
    assert!(
        solver
            .soft_body(DynamicsSoftBodyId(3))
            .unwrap()
            .particle_count
            > 8
    );
}

#[test]
fn self_contacts_and_fem_bodies_step_finite() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver.bind_environment(&floor());
    let mut folding = input(
        SHEET,
        DynamicsSoftBodyShape::Cloth {
            origin: [3.0, 0.6, 3.0],
            across: [0.1, 0.0, 0.0],
            down: [0.0, 0.0, 0.1],
            columns: 10,
            rows: 10,
        },
    );
    folding.self_contacts = true;
    folding.particle_radius = Some(0.02);
    solver.insert_soft_body(folding).unwrap();
    let mut stiff = jelly([5.5, 1.0, 5.5]);
    stiff.solver = DynamicsSoftBodySolver::Fem;
    stiff.cell_model = DynamicsSoftBodyCellModel::NeoHookean;
    stiff.id = DynamicsSoftBodyId(4);
    solver.insert_soft_body(stiff).unwrap();
    steps(&mut solver, 120);
    for id in [SHEET, DynamicsSoftBodyId(4)] {
        let readout = solver.soft_body(id).unwrap();
        assert!(
            readout.center_of_mass.iter().all(|v| v.is_finite()),
            "{readout:?}"
        );
        assert!(readout.bounds_min[1] > -0.05, "{readout:?}");
    }
}

#[test]
fn settings_shape_and_material_are_checked_and_soft_bodies_are_removed() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    let mut bad = banner();
    bad.mass = 0.0;
    assert!(solver.insert_soft_body(bad).is_err());
    let mut bad = banner();
    bad.material.poisson_ratio = 0.5;
    assert!(solver.insert_soft_body(bad).is_err());
    let mut bad = banner();
    bad.pinned = vec![COLUMNS * ROWS];
    assert!(solver.insert_soft_body(bad).is_err());
    assert!(solver
        .insert_soft_body(input(
            SHEET,
            DynamicsSoftBodyShape::Surface {
                positions: vec![[0.0; 3]; 3],
                uvs: Vec::new(),
                triangles: vec![[0, 1, 3]],
                skin: None,
            },
        ))
        .is_err());
    solver.insert_soft_body(banner()).unwrap();
    assert!(matches!(
        solver.insert_soft_body(banner()),
        Err(DynamicsError::SoftBody(
            DynamicsSoftBodyError::Duplicate { .. }
        ))
    ));
    assert!(solver
        .configure_soft_bodies(DynamicsSoftBodySettings {
            substeps: 0,
            ..Default::default()
        })
        .is_err());
    solver
        .configure_soft_bodies(DynamicsSoftBodySettings {
            substeps: 4,
            ..Default::default()
        })
        .unwrap();
    steps(&mut solver, 5);
    let mut softer = DynamicsSoftBodyMaterial::default();
    softer.edge.frequency = 5.0;
    solver
        .update_soft_body_material(SHEET, softer, false, 1.0)
        .unwrap();
    steps(&mut solver, 5);
    assert!(solver.remove_soft_body(SHEET));
    assert!(!solver.remove_soft_body(SHEET));
    assert_eq!(solver.soft_body_count(), 0);
    steps(&mut solver, 1);
}

#[test]
fn coincident_surface_vertices_keep_their_own_uvs() {
    // Two triangles meeting at one point, its two particles with UVs of
    // their own, as at a texture seam.
    let mut solver = DynamicsSolver::new([0.0; 3]);
    let uvs = vec![
        [0.0, 0.0],
        [0.5, 0.0],
        [0.0, 0.5],
        [1.0, 0.0],
        [0.5, 1.0],
        [1.0, 0.5],
    ];
    let shape = DynamicsSoftBodyShape::Surface {
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        uvs: uvs.clone(),
        triangles: vec![[0, 1, 2], [3, 4, 5]],
        skin: None,
    };
    solver.insert_soft_body(input(SHEET, shape)).unwrap();
    let surface = solver.soft_body_surface(SHEET, false).unwrap();
    for (corner, particle) in [0, 1, 2, 3, 4, 5].into_iter().enumerate() {
        assert_eq!(surface.uvs[surface.indices[corner] as usize], uvs[particle]);
    }
}

#[test]
fn a_driven_particle_stays_driven_while_tears_renumber_the_cloth() {
    let mut solver = DynamicsSolver::new(GRAVITY);
    solver
        .configure_soft_bodies(DynamicsSoftBodySettings {
            substeps: 4,
            ..Default::default()
        })
        .unwrap();
    let mut torn = banner();
    torn.material.tear_strain = Some(0.4);
    solver.insert_soft_body(torn).unwrap();
    steps(&mut solver, 10);
    let pins: Vec<[f64; 3]> = banner()
        .pinned
        .iter()
        .map(|pin| particles(&solver, SHEET)[*pin as usize].position)
        .collect();
    let before = solver.soft_body(SHEET).unwrap();
    // The bottom-right corner, dragged 3 m down over half a second.
    let bottom = COLUMNS * ROWS - 1;
    let start = particles(&solver, SHEET)[bottom as usize].position;
    let mut tracked = bottom;
    for tick in 1..=30 {
        let mut target = start;
        target[1] -= 3.0 * f64::from(tick.min(30)) / 30.0;
        solver
            .set_soft_body_targets(SHEET, &[(tracked, target)])
            .unwrap();
        steps(&mut solver, 1);
        // The driven particle is the one now at the target: a tear may
        // have renumbered it, but it was driven there all the same.
        let now = particles(&solver, SHEET);
        let at = now.iter().position(|particle| {
            particle.kinematic
                && (0..3).all(|axis| (particle.position[axis] - target[axis]).abs() < 1.0e-6)
        });
        match at {
            Some(index) => tracked = index as u32,
            // Torn off into a piece of its own: the lowest point of the body.
            None => {
                let lowest = solver.soft_body(SHEET).unwrap().bounds_min[1];
                assert!(
                    (lowest - target[1]).abs() < 1.0e-6,
                    "tick {tick}: {lowest} {target:?}"
                );
                break;
            }
        }
    }
    let after = solver.soft_body(SHEET).unwrap();
    assert_ne!(after.topology_version, before.topology_version, "{after:?}");
    // The pins still hold the same particles where they were.
    let now = particles(&solver, SHEET);
    for pin in pins {
        assert!(
            now.iter()
                .any(|particle| particle.kinematic && particle.position == pin),
            "{pin:?}"
        );
    }
    let surface = solver.soft_body_surface(SHEET, false).unwrap();
    assert!(surface.positions.iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn a_skinned_sheet_draws_its_skin_riding_the_swinging_sheet() {
    // A 1 m sheet of 5 × 5 particles hanging from its top row, and a slab
    // 2 cm thick over it as the skin: twice the vertices, closed at no edge.
    let mut positions = Vec::new();
    for row in 0..5 {
        for column in 0..5 {
            positions.push([column as f64 * 0.25, 2.0 - row as f64 * 0.25, 0.0]);
        }
    }
    let mut triangles = Vec::new();
    for row in 0..4u32 {
        for column in 0..4u32 {
            let a = row * 5 + column;
            triangles.extend([[a, a + 5, a + 1], [a + 1, a + 5, a + 6]]);
        }
    }
    let side = |z: f64| positions.iter().map(move |p| [p[0], p[1], z]);
    let skin = DynamicsSoftBodySkin {
        positions: side(0.01).chain(side(-0.01)).collect(),
        uvs: Vec::new(),
        triangles: triangles
            .iter()
            .copied()
            .chain(triangles.iter().map(|[a, b, c]| [a + 25, c + 25, b + 25]))
            .collect(),
    };
    let mut solver = DynamicsSolver::new(GRAVITY);
    let mut shape = input(
        SHEET,
        DynamicsSoftBodyShape::Surface {
            positions: positions.clone(),
            uvs: Vec::new(),
            triangles: triangles.clone(),
            skin: Some(skin.clone()),
        },
    );
    shape.pinned = (0..5).collect();
    solver.insert_soft_body(shape).unwrap();
    let rest = solver.soft_body_surface(SHEET, false).unwrap();
    assert_eq!(rest.positions.len(), 50);
    assert_eq!(rest.indices.len(), skin.triangles.len() * 3);
    for (drawn, given) in rest.positions.iter().zip(&skin.positions) {
        assert!((0..3).all(|axis| (f64::from(drawn[axis]) - given[axis]).abs() < 1e-5));
    }
    // Blown face-on, the sheet swings back; its skin goes with it, still
    // 2 cm thick.
    solver
        .set_soft_body_forces(
            SHEET,
            DynamicsSoftBodyForces {
                wind: [0.0, 0.0, -8.0],
                drag: 0.6,
                ..DynamicsSoftBodyForces::default()
            },
        )
        .unwrap();
    solver.step(TICK, 60, &[]).unwrap();
    let blown = solver.soft_body_surface(SHEET, false).unwrap();
    let particles = solver.soft_body_particles(SHEET).unwrap();
    let bottom = particles[22].position;
    assert!(bottom[2] < -0.2, "{bottom:?}");
    let front = blown.positions[22];
    let back = blown.positions[47];
    let thickness = (0..3)
        .map(|axis| (front[axis] - back[axis]).powi(2))
        .sum::<f32>()
        .sqrt();
    assert!((thickness - 0.02).abs() < 0.004, "{thickness}");
    let middle = [0, 1, 2].map(|axis| (front[axis] + back[axis]) / 2.0);
    assert!(
        (0..3).all(|axis| (f64::from(middle[axis]) - bottom[axis]).abs() < 0.01),
        "{middle:?} {bottom:?}"
    );
    // A skin must fit its sheet.
    let mut broken = skin;
    broken.triangles.push([0, 1, 50]);
    assert!(solver
        .insert_soft_body(input(
            DynamicsSoftBodyId(2),
            DynamicsSoftBodyShape::Surface {
                positions,
                uvs: Vec::new(),
                triangles,
                skin: Some(broken),
            },
        ))
        .is_err());
}
