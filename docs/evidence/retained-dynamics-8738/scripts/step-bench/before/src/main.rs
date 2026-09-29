//! The pre-#8738 path: EntityState holds every body, and each step rebuilds a
//! Rapier world, simulates, then publishes through revision-checked
//! replacements.
#[path = "../../scenario.rs"]
mod scenario;

use std::time::Instant;

use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{
    DynamicsBodyId, DynamicsTether, DynamicsTetherEndpoint, RigidBodyService, RigidBodyStepRequest,
    VoxelCollisionScene, VoxelEdit, VoxelEditService, VoxelEditTransaction,
};
use entity_state::{
    replace_rigid_body_states, EntityAuthoringService, EntityDefinition, EntityState,
    EntityTransform, Quat, RigidBodyComponent, RigidBodyShape, RigidBodyStateReplacement,
    TransformComponent,
};
use scenario::*;

fn insert(state: &mut EntityState, id: u64, at: [f32; 3], body: RigidBodyComponent) -> EntityId {
    let entity = EntityId::new(id);
    EntityAuthoringService
        .admit(
            state,
            state.revision(),
            [
                EntityDefinition::new(entity, format!("body-{id}")).with_full_transform(
                    EntityTransform {
                        translation: Vec3::new(at[0], at[1], at[2]),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                    },
                ),
            ],
        )
        .unwrap();
    let revision = state
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, body)
        .unwrap();
    entity
}

fn main() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, floor()).unwrap();
    let mut state = EntityState::from_definitions(std::iter::empty::<EntityDefinition>()).unwrap();
    let mut service = RigidBodyService::default();
    let crate_body = RigidBodyComponent::dynamic(
        RigidBodyShape::Cuboid {
            half_extents: Vec3::new(CRATE_HALF, CRATE_HALF, CRATE_HALF),
        },
        10.0,
    );
    let ball_body = RigidBodyComponent::dynamic(
        RigidBodyShape::Sphere {
            radius: BALL_RADIUS,
        },
        1.0,
    );
    let mut next = 1;
    let mut crates = Vec::new();
    for at in scenario::crates() {
        crates.push(insert(&mut state, next, at, crate_body));
        next += 1;
    }
    let mut balls = Vec::new();
    for at in scenario::balls() {
        balls.push(insert(&mut state, next, at, ball_body));
        next += 1;
    }
    let mut rope_ball_body = ball_body;
    rope_ball_body.mass = 2.0;
    let rope_ball = insert(&mut state, next, scenario::rope_ball(), rope_ball_body);
    next += 1;
    service
        .replace_tethers(
            &state,
            vec![DynamicsTether {
                id: 1,
                first: DynamicsTetherEndpoint::Fixed(ROPE_ANCHOR.map(f64::from)),
                second: DynamicsTetherEndpoint::Body {
                    body: DynamicsBodyId(rope_ball.raw()),
                    local_anchor: [0.0; 3],
                },
                maximum_length: f64::from(ROPE_LENGTH),
                target_length: f64::from(ROPE_LENGTH),
                reel_speed: 0.0,
                was_taut: false,
                wake: true,
                contacts_enabled: false,
            }],
        )
        .unwrap();

    let mut report = Report {
        label: "before: EntityState + rebuilt Rapier world",
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
    let sleeping = |state: &EntityState| {
        state
            .rigid_bodies()
            .filter(|(_, body)| body.sleeping)
            .count()
    };
    let mut last_receipt = None;
    for step in 0..STEPS {
        if step == EDIT_STEP {
            report.sleeping_before_edit = sleeping(&state);
            // Teleport a ball, remove another, add a crate.
            let teleported = balls[0];
            let replacement = RigidBodyStateReplacement {
                entity: teleported,
                expected_transform_revision: state
                    .component_revision::<TransformComponent>(teleported)
                    .unwrap(),
                expected_rigid_body_revision: state
                    .component_revision::<RigidBodyComponent>(teleported)
                    .unwrap(),
                transform: TransformComponent::from_transform(EntityTransform {
                    translation: Vec3::new(20.0, 5.0, 26.5),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                }),
                rigid_body: ball_body,
            };
            replace_rigid_body_states(&mut state, vec![replacement]).unwrap();
            let revision = state.revision();
            EntityAuthoringService
                .destroy(&mut state, revision, balls[1])
                .unwrap();
            insert(&mut state, next, [28.0, 2.0, 28.0], crate_body);
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
        }
        let started = Instant::now();
        let receipt = service
            .step(
                &mut state,
                &scene,
                RigidBodyStepRequest {
                    step_seconds: STEP_SECONDS,
                    steps: 1,
                    gravity: Vec3::new(GRAVITY[0], GRAVITY[1], GRAVITY[2]),
                    actions: Vec::new(),
                },
            )
            .unwrap();
        report.step_us.push(started.elapsed().as_secs_f64() * 1e6);
        report.rope_catches += receipt
            .tethers
            .iter()
            .filter(|tether| tether.caught)
            .count();
        last_receipt = Some(receipt);
    }
    let receipt = last_receipt.unwrap();
    report.sleeping_at_end = sleeping(&state);
    report.contacts_at_end = receipt.contacts.len();
    report.bodies_at_end = receipt.bodies_considered;
    let top = |entity: EntityId| state.transform(entity).unwrap().translation.y;
    report.last_tower_top_y = top(crates[TOWERS * TOWER_HEIGHT - 1]);
    report.dug_tower_top_y = top(crates[TOWER_HEIGHT - 1]);
    report.teleported_y = top(balls[0]);
    report.rope_distance = receipt.tethers[0].distance;
    report.print();
}
