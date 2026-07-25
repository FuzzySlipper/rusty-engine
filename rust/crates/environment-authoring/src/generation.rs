use core_math::Vec3;
use sha2::{Digest, Sha256};
use svc_rng::{RngSeed, ScopedRng};

pub const TUNNEL_GENERATOR_ID: &str = "rusty-engine.tunnel.enclosed";
pub const TUNNEL_GENERATOR_VERSION: u32 = 1;
pub const MAX_GENERATED_TUNNEL_VOXELS: usize = 1_000_000;
const SHELL_THICKNESS: u32 = 1;
const MAX_CHUNK_SIZE: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelPreset {
    TinyEnclosed,
}

impl TunnelPreset {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TinyEnclosed => "tiny-enclosed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TunnelGeneratorConfig {
    pub seed: u64,
    pub preset: TunnelPreset,
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub width: u32,
    pub height: u32,
    pub length: u32,
    pub wall_material: u16,
    pub floor_material: u16,
    pub accent_material: u16,
}

impl TunnelGeneratorConfig {
    pub const fn tiny_enclosed(seed: u64) -> Self {
        Self {
            seed,
            preset: TunnelPreset::TinyEnclosed,
            voxel_size: 1.0,
            chunk_size: 12,
            width: 5,
            height: 4,
            length: 9,
            wall_material: 1,
            floor_material: 2,
            accent_material: 3,
        }
    }

    pub const fn shell_dimensions(self) -> [u32; 3] {
        let padding = SHELL_THICKNESS * 2;
        [
            self.width.saturating_add(padding),
            self.height.saturating_add(padding),
            self.length.saturating_add(padding),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedVoxel {
    pub address: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratedCollisionAabb {
    pub address: [i64; 3],
    pub material_slot: u16,
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedSpawnMarker {
    pub source_id: &'static str,
    pub kind: &'static str,
    pub voxel: [i64; 3],
    pub local_position: Vec3,
    pub yaw_degrees: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratedTunnelFrame {
    pub local_offset: Vec3,
    pub playable_min: Vec3,
    pub playable_max: Vec3,
}

impl GeneratedTunnelFrame {
    pub fn canonical_to_centered(self, position: Vec3) -> Vec3 {
        position + self.local_offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTunnelProvenance {
    pub generator_id: &'static str,
    pub generator_version: u32,
    pub preset: &'static str,
    pub seed: u64,
    pub settings_sha256: String,
    pub voxel_data_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedTunnel {
    pub config: TunnelGeneratorConfig,
    pub voxels: Vec<GeneratedVoxel>,
    pub spawn_markers: Vec<GeneratedSpawnMarker>,
    pub frame: GeneratedTunnelFrame,
    pub provenance: GeneratedTunnelProvenance,
}

impl GeneratedTunnel {
    /// Canonical cells for direct admission to spatial voxel authority without
    /// introducing an authoring dependency into live spatial services.
    pub fn spatial_cells(&self) -> impl Iterator<Item = ([i64; 3], u16)> + '_ {
        self.voxels
            .iter()
            .map(|voxel| (voxel.address, voxel.material_slot))
    }

    pub fn collision_aabbs(&self) -> impl Iterator<Item = GeneratedCollisionAabb> + '_ {
        self.voxels.iter().map(|voxel| {
            let size = self.config.voxel_size;
            let min = [
                voxel.address[0] as f64 * size,
                voxel.address[1] as f64 * size,
                voxel.address[2] as f64 * size,
            ];
            GeneratedCollisionAabb {
                address: voxel.address,
                material_slot: voxel.material_slot,
                min,
                max: [min[0] + size, min[1] + size, min[2] + size],
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelGenerationError {
    InvalidVoxelSize {
        value: f64,
    },
    InvalidChunkSize {
        value: u32,
    },
    TooSmall {
        width: u32,
        height: u32,
        length: u32,
    },
    ExceedsChunk {
        shell: [u32; 3],
        chunk_size: u32,
    },
    DuplicateMaterial {
        material_slot: u16,
    },
    InvalidMaterial {
        material_slot: u16,
    },
    ResourceLimit {
        requested_cells: usize,
    },
}

impl std::fmt::Display for TunnelGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "tunnel generation rejected: {self:?}")
    }
}

impl std::error::Error for TunnelGenerationError {}

pub fn generate_tunnel(
    config: TunnelGeneratorConfig,
) -> Result<GeneratedTunnel, TunnelGenerationError> {
    validate_config(config)?;
    let shell = config.shell_dimensions();
    let mut rng = ScopedRng::new(RngSeed::new(config.seed), TUNNEL_GENERATOR_ID);
    let accent_x = if rng.next_bool() { shell[0] - 1 } else { 0 };
    let accent_z = 1 + rng
        .next_bounded_u32(config.length)
        .expect("validated non-zero tunnel length");
    let player_yaw = if rng.next_bool() { 0 } else { 90 };
    let mut voxels = Vec::new();
    for z in 0..shell[2] {
        for y in 0..shell[1] {
            for x in 0..shell[0] {
                let on_shell = x == 0
                    || x + 1 == shell[0]
                    || y == 0
                    || y + 1 == shell[1]
                    || z == 0
                    || z + 1 == shell[2];
                if !on_shell {
                    continue;
                }
                let material_slot = if x == accent_x && y == 1 && z == accent_z {
                    config.accent_material
                } else if y == 0 {
                    config.floor_material
                } else {
                    config.wall_material
                };
                voxels.push(GeneratedVoxel {
                    address: [i64::from(x), i64::from(y), i64::from(z)],
                    material_slot,
                });
            }
        }
    }

    let size = config.voxel_size as f32;
    let marker = |source_id, kind, voxel: [i64; 3], yaw_degrees| GeneratedSpawnMarker {
        source_id,
        kind,
        voxel,
        local_position: Vec3::new(
            (voxel[0] as f32 + 0.5) * size,
            (voxel[1] as f32 + 0.5) * size,
            (voxel[2] as f32 + 0.5) * size,
        ),
        yaw_degrees,
    };
    let spawn_markers = vec![
        marker("player_start", "player", [2, 2, 2], player_yaw),
        marker(
            "exit_hint",
            "navigation",
            [i64::from(config.width) - 1, 2, i64::from(config.length) - 1],
            180,
        ),
    ];
    let playable_width = config.width as f32 * size;
    let playable_height = config.height as f32 * size;
    let playable_length = config.length as f32 * size;
    let shell_size = SHELL_THICKNESS as f32 * size;
    let frame = GeneratedTunnelFrame {
        local_offset: Vec3::new(
            -(playable_width * 0.5 + shell_size),
            -shell_size,
            -(playable_length * 0.5 + shell_size),
        ),
        playable_min: Vec3::new(-playable_width * 0.5, 0.0, -playable_length * 0.5),
        playable_max: Vec3::new(playable_width * 0.5, playable_height, playable_length * 0.5),
    };
    let settings = settings_bytes(config);
    let provenance = GeneratedTunnelProvenance {
        generator_id: TUNNEL_GENERATOR_ID,
        generator_version: TUNNEL_GENERATOR_VERSION,
        preset: config.preset.label(),
        seed: config.seed,
        settings_sha256: sha256(&settings),
        voxel_data_sha256: hash_voxels(&voxels),
    };
    Ok(GeneratedTunnel {
        config,
        voxels,
        spawn_markers,
        frame,
        provenance,
    })
}

pub(crate) fn settings_bytes(config: TunnelGeneratorConfig) -> Vec<u8> {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        TUNNEL_GENERATOR_ID,
        TUNNEL_GENERATOR_VERSION,
        config.preset.label(),
        config.seed,
        config.voxel_size.to_bits(),
        config.chunk_size,
        config.width,
        config.height,
        config.length,
        config.wall_material,
        config.floor_material,
        config.accent_material,
    )
    .into_bytes()
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hash_voxels(voxels: &[GeneratedVoxel]) -> String {
    let mut cells = voxels.to_vec();
    cells.sort_by_key(|voxel| {
        (
            voxel.address[2],
            voxel.address[1],
            voxel.address[0],
            voxel.material_slot,
        )
    });
    let mut runs: Vec<([i64; 3], u32, u16)> = Vec::new();
    for voxel in cells {
        if let Some((start, length, material_slot)) = runs.last_mut() {
            if start[1] == voxel.address[1]
                && start[2] == voxel.address[2]
                && *material_slot == voxel.material_slot
                && start[0] + i64::from(*length) == voxel.address[0]
            {
                *length += 1;
                continue;
            }
        }
        runs.push((voxel.address, 1, voxel.material_slot));
    }
    runs.sort_by_key(|(start, length, material_slot)| (*start, *material_slot, *length));
    let mut hasher = Sha256::new();
    for (start, length, material_slot) in runs {
        for coordinate in start {
            hasher.update(coordinate.to_le_bytes());
        }
        hasher.update(length.to_le_bytes());
        hasher.update(material_slot.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn validate_config(config: TunnelGeneratorConfig) -> Result<(), TunnelGenerationError> {
    let public_voxel_size = config.voxel_size as f32;
    if !config.voxel_size.is_finite()
        || config.voxel_size <= 0.0
        || !public_voxel_size.is_finite()
        || public_voxel_size <= 0.0
    {
        return Err(TunnelGenerationError::InvalidVoxelSize {
            value: config.voxel_size,
        });
    }
    if !(1..=MAX_CHUNK_SIZE).contains(&config.chunk_size) {
        return Err(TunnelGenerationError::InvalidChunkSize {
            value: config.chunk_size,
        });
    }
    if config.width < 3 || config.height < 3 || config.length < 4 {
        return Err(TunnelGenerationError::TooSmall {
            width: config.width,
            height: config.height,
            length: config.length,
        });
    }
    let shell = config.shell_dimensions();
    let largest_public_extent = shell.into_iter().max().unwrap_or(0) as f32 * public_voxel_size;
    if !largest_public_extent.is_finite() {
        return Err(TunnelGenerationError::InvalidVoxelSize {
            value: config.voxel_size,
        });
    }
    if shell.iter().any(|dimension| *dimension > config.chunk_size) {
        return Err(TunnelGenerationError::ExceedsChunk {
            shell,
            chunk_size: config.chunk_size,
        });
    }
    let volume = shell
        .iter()
        .try_fold(1usize, |total, dimension| {
            total.checked_mul(*dimension as usize)
        })
        .ok_or(TunnelGenerationError::ResourceLimit {
            requested_cells: usize::MAX,
        })?;
    if volume > MAX_GENERATED_TUNNEL_VOXELS {
        return Err(TunnelGenerationError::ResourceLimit {
            requested_cells: volume,
        });
    }
    for slot in [
        config.wall_material,
        config.floor_material,
        config.accent_material,
    ] {
        if !(1..=4_095).contains(&slot) {
            return Err(TunnelGenerationError::InvalidMaterial {
                material_slot: slot,
            });
        }
    }
    for (left, right) in [
        (config.wall_material, config.floor_material),
        (config.wall_material, config.accent_material),
        (config.floor_material, config.accent_material),
    ] {
        if left == right {
            return Err(TunnelGenerationError::DuplicateMaterial {
                material_slot: left,
            });
        }
    }
    Ok(())
}
