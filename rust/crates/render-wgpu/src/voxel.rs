//! Voxel families: voxel object assets and instances, and the voxel surface
//! mapping that voxel scene chunk materials sample through.
//!
//! A voxel object asset is a set of meshes and named frames that each select
//! one mesh; an instance draws its current frame's mesh with the asset's slot
//! materials and its own overrides. Frames are switched by rebinding the
//! instance's parts, never by re-uploading geometry.
//!
//! Voxel scene chunks arrive as `ReplaceMeshPayload` on chunk nodes (only
//! changed chunks) and bind `voxel-material/<slot>` materials;
//! `apply.rs` realizes them as payload parts. A material with a voxel surface
//! samples its texture through [`VoxelSurfaceUniform`]: chunk UVs are tile
//! coordinates in cells, repeated by the tile scale from the tile origin and
//! mapped into the texture or its half-texel-inset atlas region.

use std::collections::BTreeMap;

use render_model::{
    RenderHandle, VoxelObjectRenderAsset, VoxelSurfaceDescriptor, VoxelSurfaceMappingDescriptor,
};

use crate::resources::{self, ResourceSource};
use crate::tables::{GpuMesh, NodeKind, Topology};
use crate::Renderer;

/// One admitted voxel object asset: its uploaded meshes and frame table.
pub(crate) struct VoxelObjectRow {
    pub meshes: Vec<GpuMesh>,
    /// Frame index to mesh index.
    pub frames: Vec<u32>,
    pub slots: BTreeMap<u16, String>,
}

impl VoxelObjectRow {
    /// The mesh a frame draws; an out-of-range frame draws the first frame.
    pub fn frame_mesh(&self, frame: u32) -> u32 {
        self.frames
            .get(frame as usize)
            .or_else(|| self.frames.first())
            .copied()
            .unwrap_or(0)
    }
}

/// Voxel surface sampling for the material uniform: `uv` is remapped to
/// `mix(uv_min, uv_max, fract((uv - tile_origin) / tile_scale))`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VoxelSurfaceUniform {
    pub tile_scale: [f32; 2],
    pub tile_origin: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

impl VoxelSurfaceUniform {
    /// Resolve the mapping against the texture the material binds. An atlas
    /// region is sampled half a texel inside its content edges, so filtering
    /// never reaches a neighbouring region; a repeat mapping covers the whole
    /// texture.
    pub fn resolve(surface: &VoxelSurfaceDescriptor, texture_size: Option<(u32, u32)>) -> Self {
        let (tile_scale, tile_origin, region) = match &surface.mapping {
            VoxelSurfaceMappingDescriptor::Repeat {
                tile_scale_cells,
                tile_origin_cells,
                ..
            } => (*tile_scale_cells, *tile_origin_cells, None),
            VoxelSurfaceMappingDescriptor::Atlas {
                region,
                tile_scale_cells,
                tile_origin_cells,
                ..
            } => (*tile_scale_cells, *tile_origin_cells, Some(region)),
        };
        let (uv_min, uv_max) = match (region, texture_size) {
            (Some(region), Some((width, height))) => {
                let (width, height) = (width as f32, height as f32);
                let [x, y] = region.content_min.map(|value| value as f32);
                let [w, h] = region.content_extent.map(|value| value as f32);
                (
                    [(x + 0.5) / width, (y + 0.5) / height],
                    [(x + w - 0.5) / width, (y + h - 0.5) / height],
                )
            }
            _ => ([0.0, 0.0], [1.0, 1.0]),
        };
        Self {
            tile_scale,
            tile_origin,
            uv_min,
            uv_max,
        }
    }

    /// CPU reference of the shader mapping, for tests.
    #[cfg(test)]
    pub fn sample(&self, uv: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|axis| {
            let scaled = (uv[axis] - self.tile_origin[axis]) / self.tile_scale[axis];
            let repeated = scaled - scaled.floor();
            self.uv_min[axis] + (self.uv_max[axis] - self.uv_min[axis]) * repeated
        })
    }
}

impl Renderer {
    pub(crate) fn define_voxel_object(
        &mut self,
        asset: &VoxelObjectRenderAsset,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        let mut meshes = Vec::with_capacity(asset.meshes.len());
        for (index, mesh) in asset.meshes.iter().enumerate() {
            let mut streams = resources::mesh_streams(&mesh.payload, resources)?;
            // Voxel object materials draw without vertex colours.
            streams.colors = None;
            meshes.push(
                self.upload_mesh(
                    &format!("{} mesh {index}", asset.asset),
                    &streams,
                    Topology::Triangles,
                    mesh.payload
                        .groups
                        .iter()
                        .map(|group| (group.material_slot, group.start, group.count))
                        .collect(),
                    BTreeMap::new(),
                ),
            );
        }
        let id = self.tables.names.id(&asset.asset);
        self.tables.voxel_objects.insert(
            id,
            VoxelObjectRow {
                meshes,
                frames: asset.frames.iter().map(|frame| frame.mesh).collect(),
                slots: asset
                    .material_slots
                    .iter()
                    .map(|slot| (slot.slot, slot.material.clone()))
                    .collect(),
            },
        );
        // Instances created before (or kept across) a redefinition bind the
        // new meshes.
        for handle in self.voxel_object_instances(&asset.asset) {
            self.rebuild_parts(handle);
        }
        Ok(())
    }

    pub(crate) fn release_voxel_object(&mut self, asset: &str) {
        if let Some(id) = self.tables.names.get(asset) {
            self.tables.voxel_objects.remove(id);
        }
        for handle in self.voxel_object_instances(asset) {
            self.rebuild_parts(handle);
        }
    }

    /// Switch an instance's frame: its parts rebind to that frame's mesh.
    pub(crate) fn set_voxel_object_frame(
        &mut self,
        handle: RenderHandle,
        frame: u32,
    ) -> Result<(), String> {
        let node = self
            .tables
            .nodes
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown node {}", handle.raw()))?;
        if let NodeKind::VoxelObject(instance) = &mut node.kind {
            if instance.frame == frame {
                return Ok(());
            }
            instance.frame = frame;
        }
        self.rebuild_parts(handle);
        Ok(())
    }

    fn voxel_object_instances(&self, asset: &str) -> Vec<RenderHandle> {
        let mut handles: Vec<RenderHandle> = self
            .tables
            .nodes
            .iter()
            .filter(|(_, node)| {
                matches!(&node.kind, NodeKind::VoxelObject(instance) if instance.asset == asset)
            })
            .map(|(handle, _)| *handle)
            .collect();
        handles.sort();
        handles
    }
}

#[cfg(test)]
mod tests {
    use render_model::{
        TextureFilter, TextureWrap, VoxelAtlasPaddingDescriptor, VoxelAtlasRegionDescriptor,
        VoxelSurfaceAlphaModeDescriptor,
    };

    use super::*;

    fn surface(mapping: VoxelSurfaceMappingDescriptor) -> VoxelSurfaceDescriptor {
        VoxelSurfaceDescriptor {
            schema_version: 1,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Clamp,
            alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
            mapping,
        }
    }

    #[test]
    fn repeat_mapping_tiles_the_whole_texture_by_scale_from_origin() {
        let uniform = VoxelSurfaceUniform::resolve(
            &surface(VoxelSurfaceMappingDescriptor::Repeat {
                texture: "texture/wall".to_owned(),
                texture_version: 1,
                texture_content_hash: "h".to_owned(),
                tile_scale_cells: [2.0, 4.0],
                tile_origin_cells: [1.0, 0.0],
            }),
            Some((64, 64)),
        );
        assert_eq!(uniform.sample([1.0, 0.0]), [0.0, 0.0]);
        assert_eq!(uniform.sample([2.0, 1.0]), [0.5, 0.25]);
        // Three cells from the origin at scale 2 wrap to the tile's middle.
        assert_eq!(uniform.sample([4.0, 6.0]), [0.5, 0.5]);
    }

    #[test]
    fn atlas_mapping_stays_half_a_texel_inside_the_region() {
        let uniform = VoxelSurfaceUniform::resolve(
            &surface(VoxelSurfaceMappingDescriptor::Atlas {
                atlas: "atlas/doom".to_owned(),
                atlas_version: 1,
                atlas_content_hash: "a".to_owned(),
                texture: "texture/doom".to_owned(),
                texture_version: 1,
                texture_content_hash: "h".to_owned(),
                region: VoxelAtlasRegionDescriptor {
                    id: "stone".to_owned(),
                    content_min: [16, 32],
                    content_extent: [16, 16],
                    padding: VoxelAtlasPaddingDescriptor {
                        left: 1,
                        right: 1,
                        bottom: 1,
                        top: 1,
                    },
                    inset: "halfTexel".to_owned(),
                },
                tile_scale_cells: [1.0, 1.0],
                tile_origin_cells: [0.0, 0.0],
            }),
            Some((64, 128)),
        );
        assert_eq!(uniform.uv_min, [16.5 / 64.0, 32.5 / 128.0]);
        assert_eq!(uniform.uv_max, [31.5 / 64.0, 47.5 / 128.0]);
        assert_eq!(uniform.sample([3.0, 7.0]), uniform.uv_min);
    }
}
