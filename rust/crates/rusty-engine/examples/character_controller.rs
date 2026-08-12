use rusty_engine::core_ids::EntityId;
use rusty_engine::core_math::{Vec2, Vec3};
use rusty_engine::engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
    FirstPersonLookCommand, FirstPersonLookConfig, FirstPersonLookService, VoxelCollisionScene,
};
use rusty_engine::entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-4..=4).flat_map(|x| (-4..=4).map(move |z| [x, 0, z])),
    )?;
    let player = EntityId::new(1);
    let mut entities = EntityState::from_definitions([EntityDefinition::new(player, "player")
        .with_transform(Vec3::new(0.0, 1.9, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(1.9))])?;

    let config = CharacterControllerConfig::responsive_fps();
    let mut controller = CharacterControllerService::default();
    let receipt = controller.step(
        &mut entities,
        &scene,
        player,
        &config,
        CharacterControllerCommand {
            planar_intent: Vec2::new(0.0, 1.0),
            ..CharacterControllerCommand::idle(1.0 / 60.0, 1)
        },
    )?;

    let look = FirstPersonLookService.integrate(
        &FirstPersonLookConfig::default(),
        Default::default(),
        FirstPersonLookCommand {
            delta: Vec2::new(0.016, -0.004),
        },
    )?;
    println!(
        "position={:?} grounded={} heading={:?}",
        receipt.transform_after.translation, receipt.motion_after.grounded, look.forward
    );
    Ok(())
}
