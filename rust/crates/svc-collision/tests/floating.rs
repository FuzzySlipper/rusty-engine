//! Bodies floating in Dynamics water (#9883): drafts against Archimedes,
//! tipping under an offset load and righting, and a current carrying a raft.

use std::f64::consts::PI;
use svc_collision::{
    DynamicsAction, DynamicsBodyId, DynamicsBodyInput, DynamicsShape, DynamicsSolver,
    DynamicsWaterPlane,
};

const GRAVITY: f64 = 9.81;
const WATER_DENSITY: f64 = 1000.0;
const STEP: f64 = 1.0 / 60.0;
const SURFACE: f64 = 2.0;
const DRAFT_TOLERANCE: f64 = 0.005;
/// Strong enough to settle a light raft's heave within a few seconds.
const DRAG: f64 = 4.0;

fn body(id: u64, shape: DynamicsShape, mass: f64, translation: [f64; 3]) -> DynamicsBodyInput {
    DynamicsBodyInput {
        id: DynamicsBodyId(id),
        translation,
        rotation: [0.0, 0.0, 0.0, 1.0],
        shape,
        mass,
        mass_properties: None,
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
        locked_translation_axes: [false; 3],
        locked_rotation_axes: [false; 3],
        linear_damping: 0.0,
        angular_damping: 0.0,
        gravity_scale: 1.0,
        friction: 0.5,
        restitution: 0.0,
        collision_groups: u32::MAX,
        collision_mask: u32::MAX,
        enabled: true,
        sleeping: false,
        continuous_collision: false,
    }
}

fn lake(current: [f64; 2]) -> DynamicsWaterPlane {
    DynamicsWaterPlane {
        min: [-1000.0, -1000.0],
        max: [1000.0, 1000.0],
        surface: SURFACE,
        current,
        density: WATER_DENSITY,
        linear_drag: DRAG,
        angular_drag: DRAG,
    }
}

fn world(bodies: &[DynamicsBodyInput], current: [f64; 2]) -> DynamicsSolver {
    let mut solver = DynamicsSolver::new([0.0, -GRAVITY, 0.0]);
    for body in bodies {
        solver.insert_body(*body).unwrap();
    }
    solver
        .set_water(vec![lake(current)], bodies.iter().map(|body| body.id))
        .unwrap();
    solver
}

fn run(solver: &mut DynamicsSolver, seconds: f64, actions: &[DynamicsAction]) {
    for _ in 0..(seconds / STEP).round() as usize {
        solver.step(STEP, 1, actions).unwrap();
    }
}

/// The depth below the surface at which `volume` is displaced, by bisection
/// of a monotone submerged-volume function of depth.
fn depth_for(volume: f64, high: f64, submerged: impl Fn(f64) -> f64) -> f64 {
    let (mut low, mut high) = (0.0, high);
    for _ in 0..80 {
        let mid = 0.5 * (low + high);
        if submerged(mid) < volume {
            low = mid;
        } else {
            high = mid;
        }
    }
    0.5 * (low + high)
}

/// Area of a disk of radius `r` below a chord `d` above its lowest point.
fn segment(r: f64, d: f64) -> f64 {
    let t = d - r;
    r * r * (PI - (t / r).acos()) + t * (r * r - t * t).sqrt()
}

/// Volume of a sphere's cap of height `h`.
fn cap(r: f64, h: f64) -> f64 {
    PI * h * h * (3.0 * r - h) / 3.0
}

#[test]
fn a_box_raft_floats_at_the_archimedes_draft() {
    let half = [1.0, 0.15, 0.75];
    let mass = 180.0;
    let mut solver = world(
        &[body(
            1,
            DynamicsShape::Cuboid { half_extents: half },
            mass,
            [0.0, SURFACE + 0.5, 0.0],
        )],
        [0.0, 0.0],
    );
    run(&mut solver, 20.0, &[]);
    let raft = solver.body(DynamicsBodyId(1)).unwrap();
    let draft = SURFACE - (raft.translation[1] - half[1]);
    let expected = mass / (WATER_DENSITY * 4.0 * half[0] * half[2]);
    assert!(
        (draft - expected).abs() < DRAFT_TOLERANCE,
        "draft {draft}, Archimedes {expected}"
    );
    let water = solver.water_readout(DynamicsBodyId(1)).unwrap();
    assert!((water.submerged_fraction - expected / (2.0 * half[1])).abs() < 0.05);
    assert!(
        (water.buoyancy[1] - mass * GRAVITY).abs() < 0.02 * mass * GRAVITY,
        "{water:?}"
    );
}

#[test]
fn a_sphere_and_a_lying_capsule_float_at_their_drafts() {
    let radius = 0.3;
    let sphere_mass = 40.0;
    let (half_height, capsule_radius, capsule_mass) = (0.5, 0.2, 60.0);
    let mut solver = world(
        &[
            body(
                1,
                DynamicsShape::Sphere { radius },
                sphere_mass,
                [0.0, SURFACE + 0.4, 0.0],
            ),
            body(
                2,
                DynamicsShape::CapsuleY {
                    half_height,
                    radius: capsule_radius,
                },
                capsule_mass,
                [5.0, SURFACE + 0.3, 0.0],
            ),
        ],
        [0.0, 0.0],
    );
    // A capsule on end is unstable; tip it so it lies down.
    let tip = DynamicsAction {
        body: DynamicsBodyId(2),
        force: [0.0; 3],
        torque: [0.0; 3],
        impulse: [0.0; 3],
        torque_impulse: [0.0, 0.0, 2.0],
        wake: true,
    };
    solver.step(STEP, 1, &[tip]).unwrap();
    run(&mut solver, 30.0, &[]);

    let sphere = solver.body(DynamicsBodyId(1)).unwrap();
    let sphere_draft = SURFACE - (sphere.translation[1] - radius);
    let expected = depth_for(sphere_mass / WATER_DENSITY, 2.0 * radius, |h| {
        cap(radius, h)
    });
    assert!(
        (sphere_draft - expected).abs() < DRAFT_TOLERANCE,
        "sphere {sphere_draft} against {expected}"
    );

    let capsule = solver.body(DynamicsBodyId(2)).unwrap();
    let r = capsule_radius;
    let lying = |d: f64| 2.0 * half_height * segment(r, d) + cap(r, d);
    let expected = depth_for(capsule_mass / WATER_DENSITY, 2.0 * r, lying);
    let capsule_draft = SURFACE - (capsule.translation[1] - r);
    let [x, y, z, w] = capsule.rotation;
    let axis_y = 1.0 - 2.0 * (x * x + z * z);
    assert!(
        axis_y.abs() < 0.05,
        "the capsule lies down: axis y {axis_y}, rotation {:?}",
        [x, y, z, w]
    );
    assert!(
        (capsule_draft - expected).abs() < DRAFT_TOLERANCE,
        "capsule {capsule_draft} against {expected}"
    );
}

/// The raft's roll about z, from its rotation.
fn roll(solver: &DynamicsSolver) -> f64 {
    let [x, y, z, w] = solver.body(DynamicsBodyId(1)).unwrap().rotation;
    (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (z * z + x * x))
}

#[test]
fn an_offset_load_tips_the_raft_and_it_rights_itself() {
    let half = [1.0, 0.15, 0.75];
    let mass = 180.0;
    let load = 60.0;
    let arm = 0.8;
    let mut solver = world(
        &[body(
            1,
            DynamicsShape::Cuboid { half_extents: half },
            mass,
            [0.0, SURFACE + 0.1, 0.0],
        )],
        [0.0, 0.0],
    );
    run(&mut solver, 10.0, &[]);
    assert!(roll(&solver).abs() < 0.01);
    // The load's weight on the raft's +x side: a force at the centre and its
    // moment about z.
    let loaded = DynamicsAction {
        body: DynamicsBodyId(1),
        force: [0.0, -load * GRAVITY, 0.0],
        torque: [0.0, 0.0, -load * GRAVITY * arm],
        impulse: [0.0; 3],
        torque_impulse: [0.0; 3],
        wake: true,
    };
    run(&mut solver, 10.0, &[loaded]);
    let tipped = roll(&solver);
    // Hydrostatics: the loaded raft displaces (raft + load) / density, so
    // its metacentre stands BM = I / V above the centre of buoyancy, with I
    // the waterplane's second moment about z; the load's moment balances
    // W GM sin(roll).
    let displaced = (mass + load) / WATER_DENSITY;
    let draft = displaced / (4.0 * half[0] * half[2]);
    let second_moment = 2.0 * half[2] * (2.0 * half[0]).powi(3) / 12.0;
    let metacentric_height = second_moment / displaced + draft / 2.0 - half[1];
    let expected = -(load * arm / ((mass + load) * metacentric_height)).asin();
    assert!(
        (tipped - expected).abs() < 0.2 * expected.abs(),
        "roll {tipped}, hydrostatics {expected}"
    );
    let readout = solver.water_readout(DynamicsBodyId(1)).unwrap();
    assert!(readout.torque[2] > 0.0, "buoyancy pushes back: {readout:?}");
    run(&mut solver, 15.0, &[]);
    assert!(
        roll(&solver).abs() < 0.01,
        "unloaded, it rights itself: roll {}",
        roll(&solver)
    );
}

#[test]
fn a_current_carries_a_raft_downstream() {
    let half = [1.0, 0.15, 0.75];
    let current = [0.8, -0.3];
    let mut solver = world(
        &[body(
            1,
            DynamicsShape::Cuboid { half_extents: half },
            180.0,
            [0.0, SURFACE + 0.1, 0.0],
        )],
        current,
    );
    run(&mut solver, 30.0, &[]);
    let raft = solver.body(DynamicsBodyId(1)).unwrap();
    assert!(
        (raft.linear_velocity[0] - current[0]).abs() < 0.05,
        "{raft:?}"
    );
    assert!(
        (raft.linear_velocity[2] - current[1]).abs() < 0.05,
        "{raft:?}"
    );
}

#[test]
fn bodies_out_of_the_water_or_not_named_fall() {
    let sphere = DynamicsShape::Sphere { radius: 0.3 };
    let mut solver = DynamicsSolver::new([0.0, -GRAVITY, 0.0]);
    solver
        .insert_body(body(1, sphere, 10.0, [0.0, SURFACE, 0.0]))
        .unwrap();
    solver
        .insert_body(body(2, sphere, 10.0, [5000.0, SURFACE, 0.0]))
        .unwrap();
    solver
        .insert_body(body(3, sphere, 10.0, [3.0, SURFACE, 0.0]))
        .unwrap();
    solver
        .set_water(
            vec![lake([0.0, 0.0])],
            [DynamicsBodyId(1), DynamicsBodyId(2)],
        )
        .unwrap();
    run(&mut solver, 2.0, &[]);
    let y = |id| solver.body(DynamicsBodyId(id)).unwrap().translation[1];
    assert!(y(1) > SURFACE - 0.3, "the named body in the lake floats");
    assert!(y(2) < SURFACE - 10.0, "outside every plane it falls");
    assert!(y(3) < SURFACE - 10.0, "an unnamed body falls");
    assert_eq!(
        solver
            .water_readout(DynamicsBodyId(2))
            .unwrap()
            .submerged_fraction,
        0.0
    );
    assert!(solver.water_readout(DynamicsBodyId(3)).is_none());
}
