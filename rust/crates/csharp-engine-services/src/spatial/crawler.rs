//! Surface crawlers (`ProposeCrawlerStep`): a ball-bodied mover whose up
//! direction follows the floor, wall or ceiling it is on. The session
//! retains nothing; the product holds the crawler's motion.

use super::*;
use engine_spatial::{
    step_crawler, CrawlerCommand, CrawlerConfig, CrawlerContact, CrawlerMotion, CrawlerStepReceipt,
    CrawlerSurface, CrawlerTransition,
};

fn vec3(value: NativeVec3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

pub(super) fn config(value: NativeCrawlerConfig) -> CrawlerConfig {
    let mut config = CrawlerConfig::default();
    config.radius = value.radius;
    config.contact_skin = value.contact_skin;
    config.speed = value.speed;
    config.acceleration = value.acceleration;
    config.braking = value.braking;
    config.gravity = value.gravity;
    config.terminal_fall_speed = value.terminal_fall_speed;
    config.stick_distance = value.stick_distance;
    config.floor_maximum_slope_radians = value.floor_maximum_slope_radians;
    config
}

pub(super) fn native_config(value: CrawlerConfig) -> NativeCrawlerConfig {
    NativeCrawlerConfig {
        radius: value.radius,
        contact_skin: value.contact_skin,
        speed: value.speed,
        acceleration: value.acceleration,
        braking: value.braking,
        gravity: value.gravity,
        terminal_fall_speed: value.terminal_fall_speed,
        stick_distance: value.stick_distance,
        floor_maximum_slope_radians: value.floor_maximum_slope_radians,
    }
}

/// A zero up or heading, as in a default motion, reads as upright facing +Z.
fn motion(value: NativeCrawlerMotion) -> CrawlerMotion {
    let fallback = CrawlerMotion::default();
    let or = |value: NativeVec3, fallback: Vec3| {
        if [value.x, value.y, value.z] == [0.0; 3] {
            fallback
        } else {
            vec3(value)
        }
    };
    CrawlerMotion {
        attached: value.attached,
        up: or(value.up, fallback.up),
        forward: or(value.forward, fallback.forward),
        velocity: vec3(value.velocity),
        last_sequence: value.last_sequence,
    }
}

fn native_motion(value: CrawlerMotion) -> NativeCrawlerMotion {
    NativeCrawlerMotion {
        attached: value.attached,
        up: native_vec3(value.up),
        forward: native_vec3(value.forward),
        velocity: native_vec3(value.velocity),
        last_sequence: value.last_sequence,
    }
}

pub(super) fn native_contact(value: Option<CrawlerContact>) -> NativeCrawlerContact {
    let Some(contact) = value else {
        return NativeCrawlerContact::default();
    };
    let mut native = NativeCrawlerContact {
        present: true,
        point: native_vec3(contact.point),
        normal: native_vec3(contact.normal),
        distance: contact.distance,
        ..NativeCrawlerContact::default()
    };
    match contact.source {
        CharacterCollisionSource::VoxelChunk(chunk) => {
            native.source_kind = NativeCharacterCollisionSourceKind::VoxelChunk;
            native.source_chunk_x = chunk.x;
            native.source_chunk_y = chunk.y;
            native.source_chunk_z = chunk.z;
        }
        CharacterCollisionSource::StaticMesh {
            instance,
            asset,
            geometry_hash,
        } => {
            native.source_kind = NativeCharacterCollisionSourceKind::StaticMesh;
            native.source_instance = instance.0;
            native.source_asset = asset.0;
            native.source_geometry_hash = geometry_hash;
        }
        // A crawler steps against retained collision only.
        CharacterCollisionSource::ActiveEntity(_) => {}
    }
    native
}

pub(super) fn native_surface(value: CrawlerSurface) -> NativeCrawlerSurface {
    match value {
        CrawlerSurface::None => NativeCrawlerSurface::None,
        CrawlerSurface::Floor => NativeCrawlerSurface::Floor,
        CrawlerSurface::Wall => NativeCrawlerSurface::Wall,
        CrawlerSurface::Ceiling => NativeCrawlerSurface::Ceiling,
    }
}

fn native_receipt(value: CrawlerStepReceipt) -> NativeCrawlerStepReceipt {
    NativeCrawlerStepReceipt {
        position: native_vec3(value.position),
        rotation: native_quat(value.rotation),
        surface: native_surface(value.surface),
        transition: match value.transition {
            CrawlerTransition::None => NativeCrawlerTransition::None,
            CrawlerTransition::Concave => NativeCrawlerTransition::Concave,
            CrawlerTransition::Convex => NativeCrawlerTransition::Convex,
            CrawlerTransition::Detached => NativeCrawlerTransition::Detached,
            CrawlerTransition::Landed => NativeCrawlerTransition::Landed,
            CrawlerTransition::Lost => NativeCrawlerTransition::Lost,
        },
        contact: native_contact(value.contact),
        motion: native_motion(value.motion),
        displacement: native_vec3(value.displacement),
        sub_steps: value.sub_steps,
    }
}

impl RuntimeSpatialBridge {
    fn propose_crawler(
        &mut self,
        request: NativeCrawlerStepRequest,
    ) -> Result<NativeCrawlerStepReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let command = CrawlerCommand {
            direction: vec3(request.command.direction),
            cling: request.command.cling,
            step_seconds: request.command.step_seconds,
            sequence: request.command.sequence,
        };
        step_crawler(
            &session.scene,
            vec3(request.position),
            motion(request.motion),
            &config(request.config),
            command,
        )
        .map(native_receipt)
        .map_err(|error| CsharpEngineServicesError::new(error.code(), error.to_string()))
    }
}

pub(super) unsafe extern "C" fn default_config(
    context: *mut c_void,
    config: *mut NativeCrawlerConfig,
) -> i32 {
    if context.is_null() || config.is_null() {
        return 0;
    }
    unsafe { *config = native_config(CrawlerConfig::default()) };
    ABI_OK
}

pub(super) unsafe extern "C" fn propose_step(
    context: *mut c_void,
    request: *const NativeCrawlerStepRequest,
    receipt: *mut NativeCrawlerStepReceipt,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.propose_crawler(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> NativeVec3 {
        NativeVec3 { x, y, z }
    }

    fn bridge_with_scene(
        scene: VoxelCollisionScene,
    ) -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 8,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        bridge.sessions.get_mut(&session.value).unwrap().scene = Arc::new(scene);
        (bridge, session)
    }

    /// A floor and a wall at x = 2 through the session's ABI: the default
    /// motion lands on the floor, and driving +X carries the crawler up the
    /// wall, the receipt naming each surface and its voxel chunk.
    #[test]
    fn a_default_crawler_lands_and_climbs_a_wall_through_the_abi() {
        let mut voxels = Vec::new();
        for x in -4..=2 {
            for z in -4..4 {
                voxels.push([x, 0, z]);
                if x == 2 {
                    for y in 1..12 {
                        voxels.push([x, y, z]);
                    }
                }
            }
        }
        let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
        let (mut bridge, session) = bridge_with_scene(scene);
        let context = (&mut bridge as *mut RuntimeSpatialBridge).cast();
        let mut native_config = NativeCrawlerConfig::default();
        assert_eq!(
            unsafe { default_config(context, &mut native_config) },
            ABI_OK
        );
        assert_eq!(native_config.radius, 0.25);
        let mut position = v(0.0, 1.5, 0.5);
        let mut motion = NativeCrawlerMotion::default();
        let mut surfaces = Vec::new();
        for sequence in 1..=120 {
            let direction = if sequence <= 10 {
                v(0.0, 0.0, 0.0)
            } else {
                v(1.0, 1.0, 0.0)
            };
            let request = NativeCrawlerStepRequest {
                session,
                position,
                motion,
                config: native_config,
                command: NativeCrawlerCommand {
                    direction,
                    cling: true,
                    step_seconds: 1.0 / 60.0,
                    sequence,
                },
            };
            let mut receipt = NativeCrawlerStepReceipt::default();
            let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
            assert_eq!(
                unsafe { propose_step(context, &request, &mut receipt, &mut error) },
                ABI_OK
            );
            position = receipt.position;
            motion = receipt.motion;
            if surfaces.last() != Some(&receipt.surface) {
                surfaces.push(receipt.surface);
            }
            if receipt.surface == NativeCrawlerSurface::Wall {
                assert!(receipt.contact.present);
                assert!((receipt.contact.normal.x + 1.0).abs() < 1.0e-3);
                assert!(matches!(
                    receipt.contact.source_kind,
                    NativeCharacterCollisionSourceKind::VoxelChunk
                ));
            }
        }
        assert_eq!(
            surfaces,
            [
                NativeCrawlerSurface::None,
                NativeCrawlerSurface::Floor,
                NativeCrawlerSurface::Wall
            ]
        );
        assert!(position.y > 2.0, "climbed: {position:?}");
    }

    #[test]
    fn an_invalid_config_is_refused_with_its_code() {
        let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
        let (mut bridge, session) = bridge_with_scene(scene);
        let context = (&mut bridge as *mut RuntimeSpatialBridge).cast();
        let request = NativeCrawlerStepRequest {
            session,
            position: v(0.5, 1.5, 0.5),
            motion: NativeCrawlerMotion::default(),
            config: NativeCrawlerConfig::default(),
            command: NativeCrawlerCommand {
                step_seconds: 1.0 / 60.0,
                ..NativeCrawlerCommand::default()
            },
        };
        let mut receipt = NativeCrawlerStepReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe { propose_step(context, &request, &mut receipt, &mut error) },
            0
        );
        assert_eq!(
            crate::operation_diagnostics::receipt_codes(&error),
            ["invalid-crawler-config"]
        );
    }
}
