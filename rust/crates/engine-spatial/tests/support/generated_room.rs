//! Exact regression fixture for the former product-shaped generated-room recipe.
//!
//! This fixture intentionally has no runtime owner. It preserves the old
//! seed/dimensions/material/pillar/accent/exit-aperture recipe so the spatial
//! projections remain covered while durable environment generation stays owned
//! by `environment-authoring`, whose tunnel semantics are intentionally
//! different.

use engine_spatial::{CollisionSceneError, MaterialVoxel, VoxelCollisionScene};
use svc_rng::{RngSeed, ScopedRng};

const GENERATED_ROOM_SCOPE: &str = "rusty-engine.generated-room.v1";
const GENERATED_EXIT_WIDTH: u32 = 3;
const GENERATED_EXIT_HEIGHT: u32 = 2;
const GENERATED_ROOM_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GeneratedRoomConfig {
    pub(crate) seed: u64,
    pub(crate) voxel_size: f64,
    pub(crate) chunk_size: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedRoomRecord {
    pub(crate) generator_version: u32,
    pub(crate) output_hash: u64,
    pub(crate) pillar_voxel: [i64; 3],
    pub(crate) accent_voxel: [i64; 3],
    pub(crate) exit_aperture_min: [i64; 3],
    pub(crate) exit_aperture_max_exclusive: [i64; 3],
    pub(crate) solid_voxel_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedRoomError {
    TooSmall,
    ExceedsChunk,
}

#[derive(Debug)]
pub(crate) enum GeneratedRoomFixtureError {
    Generation(GeneratedRoomError),
    Scene(CollisionSceneError),
}

impl std::fmt::Display for GeneratedRoomFixtureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generation(error) => write!(formatter, "generated room rejected: {error:?}"),
            Self::Scene(error) => write!(formatter, "generated room scene rejected: {error}"),
        }
    }
}

impl std::error::Error for GeneratedRoomFixtureError {}

pub(crate) struct GeneratedRoomFixture {
    pub(crate) record: GeneratedRoomRecord,
    pub(crate) scene: VoxelCollisionScene,
}

impl GeneratedRoomFixture {
    pub(crate) fn new(config: GeneratedRoomConfig) -> Result<Self, GeneratedRoomFixtureError> {
        let (voxels, record) =
            generate_room(config).map_err(GeneratedRoomFixtureError::Generation)?;
        let scene =
            VoxelCollisionScene::from_material_voxels(config.voxel_size, config.chunk_size, voxels)
                .map_err(GeneratedRoomFixtureError::Scene)?;
        Ok(Self { record, scene })
    }
}

pub(crate) fn room_config(seed: u64) -> GeneratedRoomConfig {
    GeneratedRoomConfig {
        seed,
        voxel_size: 1.0,
        chunk_size: 16,
        width: 7,
        height: 4,
        length: 10,
    }
}

fn generate_room(
    config: GeneratedRoomConfig,
) -> Result<(Vec<MaterialVoxel>, GeneratedRoomRecord), GeneratedRoomError> {
    if config.width < 5 || config.height < 3 || config.length < 8 {
        return Err(GeneratedRoomError::TooSmall);
    }
    let shell = [
        config
            .width
            .checked_add(2)
            .ok_or(GeneratedRoomError::ExceedsChunk)?,
        config
            .height
            .checked_add(2)
            .ok_or(GeneratedRoomError::ExceedsChunk)?,
        config
            .length
            .checked_add(2)
            .ok_or(GeneratedRoomError::ExceedsChunk)?,
    ];
    if !(1..=engine_spatial::MAX_CHUNK_SIZE).contains(&config.chunk_size)
        || shell.iter().any(|dimension| *dimension > config.chunk_size)
    {
        return Err(GeneratedRoomError::ExceedsChunk);
    }
    let mut rng = ScopedRng::new(RngSeed::new(config.seed), GENERATED_ROOM_SCOPE);
    let pillar_x = 2 + rng
        .next_bounded_u32(config.width - 2)
        .expect("validated pillar span");
    let pillar_z = 1 + config.length / 2;
    let accent_x = if rng.next_bool() { 0 } else { shell[0] - 1 };
    let accent_z = 1 + rng
        .next_bounded_u32(config.length)
        .expect("validated accent span");
    let exit_x_start = 1 + (config.width - GENERATED_EXIT_WIDTH) / 2;
    let exit_x_end = exit_x_start + GENERATED_EXIT_WIDTH;
    let exit_y_end = 1 + GENERATED_EXIT_HEIGHT;
    let exit_z = shell[2] - 1;
    let mut voxels = Vec::new();
    for z in 0..shell[2] {
        for y in 0..shell[1] {
            for x in 0..shell[0] {
                let in_exit_aperture = z == exit_z
                    && (exit_x_start..exit_x_end).contains(&x)
                    && (1..exit_y_end).contains(&y);
                if in_exit_aperture {
                    continue;
                }
                let on_shell = x == 0
                    || x + 1 == shell[0]
                    || y == 0
                    || y + 1 == shell[1]
                    || z == 0
                    || z + 1 == shell[2];
                let material_slot = if on_shell {
                    if x == accent_x && y == 1 && z == accent_z {
                        3
                    } else if y == 0 {
                        2
                    } else {
                        1
                    }
                } else if x == pillar_x && z == pillar_z {
                    3
                } else {
                    continue;
                };
                voxels.push(MaterialVoxel {
                    address: [i64::from(x), i64::from(y), i64::from(z)],
                    material_slot,
                });
            }
        }
    }
    let output_hash = hash_generated_room(config, &voxels);
    let record = GeneratedRoomRecord {
        generator_version: GENERATED_ROOM_VERSION,
        output_hash,
        pillar_voxel: [i64::from(pillar_x), 1, i64::from(pillar_z)],
        accent_voxel: [i64::from(accent_x), 1, i64::from(accent_z)],
        exit_aperture_min: [i64::from(exit_x_start), 1, i64::from(exit_z)],
        exit_aperture_max_exclusive: [
            i64::from(exit_x_end),
            i64::from(exit_y_end),
            i64::from(exit_z + 1),
        ],
        solid_voxel_count: voxels.len(),
    };
    Ok((voxels, record))
}

fn hash_generated_room(config: GeneratedRoomConfig, voxels: &[MaterialVoxel]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for value in [
        u64::from(GENERATED_ROOM_VERSION),
        config.seed,
        config.voxel_size.to_bits(),
        u64::from(config.chunk_size),
        u64::from(config.width),
        u64::from(config.height),
        u64::from(config.length),
    ] {
        feed_hash(&mut hash, &value.to_le_bytes());
    }
    for voxel in voxels {
        for coordinate in voxel.address {
            feed_hash(&mut hash, &coordinate.to_le_bytes());
        }
        feed_hash(&mut hash, &voxel.material_slot.to_le_bytes());
    }
    hash
}

fn feed_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
