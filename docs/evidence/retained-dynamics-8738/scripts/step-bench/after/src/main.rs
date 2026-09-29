//! The #8738 path: one retained Rapier world updated in place.
#[path = "../../scenario.rs"]
mod scenario;

use std::time::Instant;

use engine_spatial::{
    DynamicsBodyId, DynamicsBodyInput, DynamicsShape, DynamicsSolver, DynamicsTether,
    DynamicsTetherEndpoint, VoxelCollisionScene, VoxelEdit, VoxelEditService, VoxelEditTransaction,
};
use scenario::*;

fn body(id: u64, at: [f32; 3], shape: DynamicsShape, mass: f64) -> DynamicsBodyInput {
    // RigidBodyComponent::dynamic defaults, as the Dynamics bridge maps them.
    DynamicsBodyInput {
        id: DynamicsBodyId(id),
        translation: at.map(f64::from),
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
        friction: f64::from(0.5_f32),
        restitution: 0.0,
        collision_groups: u32::MAX,
        collision_mask: u32::MAX,
        enabled: true,
        sleeping: false,
        continuous_collision: false,
    }
}

fn main() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, floor()).unwrap();
    let mut solver = DynamicsSolver::new(GRAVITY.map(f64::from));
    scene.bind_dynamics_environment(&mut solver);
    let crate_shape = DynamicsShape::Cuboid {
        half_extents: [f64::from(CRATE_HALF); 3],
    };
    let ball_shape = DynamicsShape::Sphere {
        radius: f64::from(BALL_RADIUS),
    };
    let mut next = 1;
    let mut crates = Vec::new();
    for at in scenario::crates() {
        solver
            .insert_body(body(next, at, crate_shape, 10.0))
            .unwrap();
        crates.push(DynamicsBodyId(next));
        next += 1;
    }
    let mut balls = Vec::new();
    for at in scenario::balls() {
        solver.insert_body(body(next, at, ball_shape, 1.0)).unwrap();
        balls.push(DynamicsBodyId(next));
        next += 1;
    }
    let rope_ball = DynamicsBodyId(next);
    solver
        .insert_body(body(next, scenario::rope_ball(), ball_shape, 2.0))
        .unwrap();
    next += 1;
    solver
        .set_tether(DynamicsTether {
            id: 1,
            first: DynamicsTetherEndpoint::Fixed(ROPE_ANCHOR.map(f64::from)),
            second: DynamicsTetherEndpoint::Body {
                body: rope_ball,
                local_anchor: [0.0; 3],
            },
            maximum_length: f64::from(ROPE_LENGTH),
            target_length: f64::from(ROPE_LENGTH),
            reel_speed: 0.0,
            contacts_enabled: false,
        })
        .unwrap();

    let mut report = Report {
        label: "after: retained DynamicsSolver",
        step_us: Vec::with_capacity(STEPS),
        sleeping_before_edit: 0,
        sleeping_at_end: 0,
        contacts_at_end: 0,
        bodies_at_end: 0,
        last_tower_top_y: 0.0,
        dug_tower_top_y: 0.0,
        teleported_y: 0.0,
        rope_distance: 0.0,
        rope_catches: 0,
    };
    let all = |solver: &DynamicsSolver| {
        (1..next + 1)
            .filter_map(|id| solver.body(DynamicsBodyId(id)))
            .collect::<Vec<_>>()
    };
    let sleeping =
        |solver: &DynamicsSolver| all(solver).iter().filter(|body| body.sleeping).count();
    for step in 0..STEPS {
        if step == EDIT_STEP {
            report.sleeping_before_edit = sleeping(&solver);
            solver
                .set_body_motion(
                    balls[0],
                    [20.0, 5.0, 26.5],
                    [0.0, 0.0, 0.0, 1.0],
                    [0.0; 3],
                    [0.0; 3],
                    false,
                )
                .unwrap();
            solver.remove_body(balls[1]);
            solver
                .insert_body(body(next, [28.0, 2.0, 28.0], crate_shape, 10.0))
                .unwrap();
        }
        if step == DIG_STEP {
            let edits = dug()
                .into_iter()
                .map(|address| VoxelEdit::Clear { address })
                .collect::<Vec<_>>();
            let expected_revision = scene.source_revision();
            VoxelEditService::apply(
                &mut scene,
                VoxelEditTransaction {
                    expected_revision,
                    edits: &edits,
                },
            )
            .unwrap();
            let started = Instant::now();
            let receipt = scene.bind_dynamics_environment(&mut solver);
            eprintln!(
                "dig rebind: {receipt:?} in {:.1} us",
                started.elapsed().as_secs_f64() * 1e6
            );
        }
        let started = Instant::now();
        solver.step(f64::from(STEP_SECONDS), 1, &[]).unwrap();
        report.step_us.push(started.elapsed().as_secs_f64() * 1e6);
        report.rope_catches += solver
            .tether_readouts()
            .iter()
            .filter(|tether| tether.caught)
            .count();
    }
    report.sleeping_at_end = sleeping(&solver);
    report.contacts_at_end = solver.contacts().len();
    report.bodies_at_end = solver.body_count();
    let top = |id: DynamicsBodyId| solver.body(id).unwrap().translation[1] as f32;
    report.last_tower_top_y = top(crates[TOWERS * TOWER_HEIGHT - 1]);
    report.dug_tower_top_y = top(crates[TOWER_HEIGHT - 1]);
    report.teleported_y = top(balls[0]);
    report.rope_distance = solver.tether_readouts()[0].distance;
    report.print();
}
