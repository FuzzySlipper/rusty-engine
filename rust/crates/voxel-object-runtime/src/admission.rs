use std::{collections::BTreeMap, sync::Arc};

use svc_mesh::{mesh_cells_standalone, MeshError, MeshVoxelCell};
use voxel_asset::{
    canonicalize_voxel_object, decode_voxel_object, VoxelFrameCell, VoxelObjectAsset,
    VoxelObjectError, VoxelObjectFrameSelectionError,
};

use crate::{
    AdmittedVoxelObject, VoxelObjectFrameSource, VoxelObjectRuntimeClip, VoxelObjectRuntimeFrame,
    VoxelObjectRuntimeLimits,
};

#[derive(Debug)]
pub enum VoxelObjectAdmissionError {
    Asset(VoxelObjectError),
    Frame(VoxelObjectFrameSelectionError),
    Mesh(MeshError),
    FrameLimit { count: u64, limit: u32 },
    ResolvedVoxelLimit { count: u64, limit: u64 },
    MeshFaceLimit { count: u64, limit: u64 },
    DurationOverflow { clip: String },
}

impl std::fmt::Display for VoxelObjectAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asset(error) => error.fmt(formatter),
            Self::Frame(error) => error.fmt(formatter),
            Self::Mesh(error) => error.fmt(formatter),
            Self::FrameLimit { count, limit } => {
                write!(
                    formatter,
                    "voxel object resolves {count} frames; limit is {limit}"
                )
            }
            Self::ResolvedVoxelLimit { count, limit } => write!(
                formatter,
                "voxel object resolves {count} aggregate cells; runtime limit is {limit}"
            ),
            Self::MeshFaceLimit { count, limit } => write!(
                formatter,
                "voxel object creates {count} unique mesh faces; runtime limit is {limit}"
            ),
            Self::DurationOverflow { clip } => {
                write!(
                    formatter,
                    "voxel object clip `{clip}` duration exceeds u64 microseconds"
                )
            }
        }
    }
}

impl std::error::Error for VoxelObjectAdmissionError {}

pub fn admit_voxel_object_json(
    input: &str,
    limits: VoxelObjectRuntimeLimits,
) -> Result<AdmittedVoxelObject, VoxelObjectAdmissionError> {
    let object = decode_voxel_object(input).map_err(VoxelObjectAdmissionError::Asset)?;
    admit_voxel_object(&object, limits)
}

pub fn admit_voxel_object(
    object: &VoxelObjectAsset,
    limits: VoxelObjectRuntimeLimits,
) -> Result<AdmittedVoxelObject, VoxelObjectAdmissionError> {
    let object = canonicalize_voxel_object(object).map_err(VoxelObjectAdmissionError::Asset)?;
    let frame_count = object
        .clips
        .iter()
        .try_fold(1_u64, |count, clip| {
            count.checked_add(clip.frames.len() as u64)
        })
        .unwrap_or(u64::MAX);
    if frame_count > u64::from(limits.max_frames) {
        return Err(VoxelObjectAdmissionError::FrameLimit {
            count: frame_count,
            limit: limits.max_frames,
        });
    }

    let mut builder = AdmissionBuilder::new(&object, limits);
    let default_cells = object
        .resolve_default_frame()
        .map_err(VoxelObjectAdmissionError::Frame)?;
    builder.push_frame(
        VoxelObjectFrameSource::Default,
        object.default_frame.voxel_data_hash.clone(),
        default_cells,
    )?;

    let mut clips = Vec::with_capacity(object.clips.len());
    for clip in &object.clips {
        let mut frame_indices = Vec::with_capacity(clip.frames.len());
        let mut frame_durations_micros = Vec::with_capacity(clip.frames.len());
        let default_duration = seconds_to_micros(1.0 / clip.frames_per_second);
        let mut duration_micros = 0_u64;
        for (frame_index, animation_frame) in clip.frames.iter().enumerate() {
            let cells = object
                .resolve_clip_frame(&clip.id, frame_index)
                .map_err(VoxelObjectAdmissionError::Frame)?;
            let runtime_index = builder.push_frame(
                VoxelObjectFrameSource::Clip {
                    clip: clip.id.clone(),
                    frame: frame_index as u32,
                },
                animation_frame.frame.voxel_data_hash.clone(),
                cells,
            )?;
            frame_indices.push(runtime_index);
            let frame_duration = animation_frame
                .duration_seconds
                .map(seconds_to_micros)
                .unwrap_or(default_duration);
            duration_micros = duration_micros.checked_add(frame_duration).ok_or_else(|| {
                VoxelObjectAdmissionError::DurationOverflow {
                    clip: clip.id.clone(),
                }
            })?;
            frame_durations_micros.push(frame_duration);
        }
        clips.push(VoxelObjectRuntimeClip {
            id: clip.id.clone(),
            name: clip.name.clone(),
            frame_indices,
            frame_durations_micros,
            duration_micros,
        });
    }

    let (frames, meshes) = builder.finish();
    Ok(AdmittedVoxelObject::new(object, frames, clips, meshes))
}

fn seconds_to_micros(seconds: f64) -> u64 {
    (seconds * 1_000_000.0).round().max(1.0) as u64
}

struct AdmissionBuilder<'a> {
    object: &'a VoxelObjectAsset,
    limits: VoxelObjectRuntimeLimits,
    frames: Vec<VoxelObjectRuntimeFrame>,
    meshes: Vec<Arc<svc_mesh::MeshPayload>>,
    meshes_by_hash: BTreeMap<String, u32>,
    resolved_voxels: u64,
    unique_mesh_faces: u64,
}

impl<'a> AdmissionBuilder<'a> {
    fn new(object: &'a VoxelObjectAsset, limits: VoxelObjectRuntimeLimits) -> Self {
        Self {
            object,
            limits,
            frames: Vec::new(),
            meshes: Vec::new(),
            meshes_by_hash: BTreeMap::new(),
            resolved_voxels: 0,
            unique_mesh_faces: 0,
        }
    }

    fn push_frame(
        &mut self,
        source: VoxelObjectFrameSource,
        voxel_data_hash: String,
        cells: Vec<VoxelFrameCell>,
    ) -> Result<u32, VoxelObjectAdmissionError> {
        self.resolved_voxels = self.resolved_voxels.saturating_add(cells.len() as u64);
        if self.resolved_voxels > self.limits.max_resolved_voxels {
            return Err(VoxelObjectAdmissionError::ResolvedVoxelLimit {
                count: self.resolved_voxels,
                limit: self.limits.max_resolved_voxels,
            });
        }

        let cells: Arc<[VoxelFrameCell]> = cells.into();
        let mesh_index = if let Some(index) = self.meshes_by_hash.get(&voxel_data_hash) {
            *index
        } else {
            let remaining = self
                .limits
                .max_unique_mesh_faces
                .saturating_sub(self.unique_mesh_faces)
                .min(u64::from(u32::MAX)) as u32;
            let mesh_cells = cells
                .iter()
                .map(|cell| MeshVoxelCell {
                    coordinate: cell.coordinate,
                    material_slot: cell.material_slot,
                })
                .collect::<Vec<_>>();
            let mesh = mesh_cells_standalone(
                self.object.grid.cell_size,
                self.object.grid.pivot,
                &mesh_cells,
                remaining,
            )
            .map_err(|error| match error {
                MeshError::TooManyFaces { faces, .. } => VoxelObjectAdmissionError::MeshFaceLimit {
                    count: self.unique_mesh_faces.saturating_add(faces),
                    limit: self.limits.max_unique_mesh_faces,
                },
                other => VoxelObjectAdmissionError::Mesh(other),
            })?;
            self.unique_mesh_faces = self
                .unique_mesh_faces
                .saturating_add(u64::from(mesh.stats.source_faces));
            if self.unique_mesh_faces > self.limits.max_unique_mesh_faces {
                return Err(VoxelObjectAdmissionError::MeshFaceLimit {
                    count: self.unique_mesh_faces,
                    limit: self.limits.max_unique_mesh_faces,
                });
            }
            let index = self.meshes.len() as u32;
            self.meshes.push(Arc::new(mesh));
            self.meshes_by_hash.insert(voxel_data_hash.clone(), index);
            index
        };

        let index = self.frames.len() as u32;
        self.frames.push(VoxelObjectRuntimeFrame {
            index,
            source,
            voxel_data_hash,
            cells,
            mesh_index,
        });
        Ok(index)
    }

    fn finish(
        self,
    ) -> (
        Vec<VoxelObjectRuntimeFrame>,
        Vec<Arc<svc_mesh::MeshPayload>>,
    ) {
        (self.frames, self.meshes)
    }
}
