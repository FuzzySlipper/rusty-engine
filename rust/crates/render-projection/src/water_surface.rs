//! Water field surfaces as renderer nodes (#9884): one static mesh instance
//! per drawn field, its mesh redefined from each new snapshot.
//!
//! [`water_surface_mesh`] builds the mesh from a snapshot's cells: one quad
//! per wet cell, its corners at the mean surface of the wet cells around
//! them, so neighbouring cells join without cracks. A shoreline quad meets
//! the bank at the cell's edge; the ground drawn above it hides the rest.
//!
//! Removals follow the heightfield's rule ([`HeightfieldRemovals`]): what was
//! made before this call goes `early`, ahead of the call's Graphics work.

use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
    MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
    MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, RenderDiff, RenderLayer,
    RenderMetadata, ShadowCasting, StaticMeshAsset, StaticMeshInstanceDescriptor, Transform,
};

use crate::{
    HandleAllocationError, HeightfieldRemovals, RenderHandleNamespace, StableHandleRegistry,
};

/// The cells of a field's snapshot a surface is drawn from.
#[derive(Debug, Clone, Copy)]
pub struct WaterSurfaceCells<'a> {
    pub width: usize,
    pub length: usize,
    pub cell_size: f32,
    pub bed: &'a [f32],
    pub depth: &'a [f32],
    /// Cells shallower than this are not drawn.
    pub drawn_depth: f32,
}

/// A water surface in the field's own frame (its corner at the origin).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterSurfaceMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub drawn_cells: u32,
}

/// The surface of every cell deeper than the drawn depth, or `None` when no
/// cell is.
pub fn water_surface_mesh(cells: WaterSurfaceCells<'_>) -> Option<WaterSurfaceMesh> {
    let WaterSurfaceCells {
        width,
        length,
        cell_size,
        bed,
        depth,
        drawn_depth,
    } = cells;
    let drawn = |x: usize, z: usize| depth[z * width + x] > drawn_depth;
    let corners = width + 1;
    // Each corner's height: the mean surface of the drawn cells touching it.
    let mut sum = vec![0.0f32; corners * (length + 1)];
    let mut count = vec![0u8; corners * (length + 1)];
    let mut drawn_cells = 0;
    for z in 0..length {
        for x in 0..width {
            if !drawn(x, z) {
                continue;
            }
            drawn_cells += 1;
            let i = z * width + x;
            let surface = bed[i] + depth[i];
            for corner in [
                z * corners + x,
                z * corners + x + 1,
                (z + 1) * corners + x,
                (z + 1) * corners + x + 1,
            ] {
                sum[corner] += surface;
                count[corner] += 1;
            }
        }
    }
    if drawn_cells == 0 {
        return None;
    }
    let height = |corner: usize| sum[corner] / f32::from(count[corner]);
    let mut vertex = vec![u32::MAX; sum.len()];
    let mut mesh = WaterSurfaceMesh {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
        drawn_cells,
        ..WaterSurfaceMesh::default()
    };
    for (corner, slot) in vertex.iter_mut().enumerate() {
        if count[corner] == 0 {
            continue;
        }
        let (cx, cz) = (corner % corners, corner / corners);
        let y = height(corner);
        // Central differences over the neighbouring drawn corners; a missing
        // neighbour stands at this corner's height.
        let at = |x: isize, z: isize| {
            let inside = x >= 0 && z >= 0 && (x as usize) <= width && (z as usize) <= length;
            let corner = inside.then(|| z as usize * corners + x as usize);
            corner.filter(|&c| count[c] > 0).map_or(y, height)
        };
        let (ix, iz) = (cx as isize, cz as isize);
        let dx = (at(ix + 1, iz) - at(ix - 1, iz)) / (2.0 * cell_size);
        let dz = (at(ix, iz + 1) - at(ix, iz - 1)) / (2.0 * cell_size);
        let normal = normalize([-dx, 1.0, -dz]);
        let position = [cx as f32 * cell_size, y, cz as f32 * cell_size];
        mesh.min = std::array::from_fn(|axis| mesh.min[axis].min(position[axis]));
        mesh.max = std::array::from_fn(|axis| mesh.max[axis].max(position[axis]));
        *slot = (mesh.positions.len() / 3) as u32;
        mesh.positions.extend_from_slice(&position);
        mesh.normals.extend_from_slice(&normal);
        mesh.uvs.extend_from_slice(&[position[0], position[2]]);
    }
    for z in 0..length {
        for x in 0..width {
            if !drawn(x, z) {
                continue;
            }
            let a = vertex[z * corners + x];
            let b = vertex[z * corners + x + 1];
            let c = vertex[(z + 1) * corners + x];
            let d = vertex[(z + 1) * corners + x + 1];
            mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Some(mesh)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|component| component / length)
}

/// Where a field's surface is drawn: its corner, in a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterSurfacePlacement {
    pub origin: [f32; 3],
    pub layer: RenderLayer,
}

#[derive(Debug, Clone)]
struct Projected {
    placement: WaterSurfacePlacement,
    visible: bool,
}

/// The drawn surfaces of every presented water field.
#[derive(Debug, Clone)]
pub struct WaterSurfaceProjector {
    registry: StableHandleRegistry<u64>,
    fields: BTreeMap<u64, Projected>,
    /// Fields whose node was made since [`Self::begin_call`].
    fresh: BTreeSet<u64>,
}

impl Default for WaterSurfaceProjector {
    fn default() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::WATER_SURFACE),
            fields: BTreeMap::new(),
            fresh: BTreeSet::new(),
        }
    }
}

/// The static mesh a field's surface draws as.
pub fn water_surface_asset(field: u64) -> String {
    format!("mesh/water-field-{field}")
}

impl WaterSurfaceProjector {
    /// A product call begins: what it makes is fresh until the next.
    pub fn begin_call(&mut self) {
        self.fresh.clear();
    }

    /// Draw `mesh` for the field with the material `material` (a catalog
    /// id), or hide its surface when there is none.
    pub fn draw(
        &mut self,
        field: u64,
        placement: WaterSurfacePlacement,
        mesh: Option<&WaterSurfaceMesh>,
        material: &str,
    ) -> Result<Vec<RenderDiff>, HandleAllocationError> {
        let mut operations = Vec::new();
        let Some(mesh) = mesh else {
            operations.extend(self.show(field, false));
            return Ok(operations);
        };
        let asset = water_surface_asset(field);
        operations.push(RenderDiff::DefineStaticMesh {
            asset: surface_asset(&asset, mesh, material),
        });
        match self.fields.get_mut(&field) {
            Some(_) => {
                operations.extend(self.place(field, placement));
                operations.extend(self.show(field, true));
            }
            None => {
                let handle = self.registry.allocate(field)?;
                self.fresh.insert(field);
                self.fields.insert(
                    field,
                    Projected {
                        placement,
                        visible: true,
                    },
                );
                operations.push(RenderDiff::CreateStaticMeshInstance {
                    handle,
                    parent: None,
                    instance: StaticMeshInstanceDescriptor {
                        asset,
                        transform: transform(placement),
                        visible: true,
                        material_overrides: Vec::new(),
                        metadata: RenderMetadata::default(),
                        layer: placement.layer,
                        shadow_casting: ShadowCasting::None,
                    },
                });
            }
        }
        Ok(operations)
    }

    /// Move a drawn field's surface (a world-origin rebase moved it).
    pub fn place(&mut self, field: u64, placement: WaterSurfacePlacement) -> Vec<RenderDiff> {
        let Some(projected) = self.fields.get_mut(&field) else {
            return Vec::new();
        };
        if projected.placement == placement {
            return Vec::new();
        }
        projected.placement = placement;
        self.registry
            .handle_of(&field)
            .map(|handle| RenderDiff::Update {
                handle,
                transform: Some(transform(placement)),
                material: None,
                visible: None,
                metadata: None,
            })
            .into_iter()
            .collect()
    }

    fn show(&mut self, field: u64, visible: bool) -> Vec<RenderDiff> {
        let Some(projected) = self.fields.get_mut(&field) else {
            return Vec::new();
        };
        if projected.visible == visible {
            return Vec::new();
        }
        projected.visible = visible;
        self.registry
            .handle_of(&field)
            .map(|handle| RenderDiff::Update {
                handle,
                transform: None,
                material: None,
                visible: Some(visible),
                metadata: None,
            })
            .into_iter()
            .collect()
    }

    /// Stop drawing a field: its node and mesh.
    pub fn remove(&mut self, field: u64) -> HeightfieldRemovals {
        let mut removals = HeightfieldRemovals::default();
        if self.fields.remove(&field).is_none() {
            return removals;
        }
        if let Some(handle) = self.registry.remove(&field) {
            let operations = [
                RenderDiff::Destroy { handle },
                RenderDiff::ReleaseStaticMesh {
                    asset: water_surface_asset(field),
                },
            ];
            if self.fresh.remove(&field) {
                removals.late.extend(operations);
            } else {
                removals.early.extend(operations);
            }
        }
        removals
    }
}

fn transform(placement: WaterSurfacePlacement) -> Transform {
    Transform {
        translation: placement.origin,
        ..Transform::IDENTITY
    }
}

fn surface_asset(asset: &str, mesh: &WaterSurfaceMesh, material: &str) -> StaticMeshAsset {
    let attribute = |name, components| MeshAttribute {
        name,
        components,
        kind: MeshAttributeKind::F32,
    };
    StaticMeshAsset {
        asset: asset.to_owned(),
        payload: MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count: (mesh.positions.len() / 3) as u32,
                index_count: mesh.indices.len() as u32,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    attribute(MeshAttributeName::Position, 3),
                    attribute(MeshAttributeName::Normal, 3),
                    attribute(MeshAttributeName::Uv, 2),
                ],
            },
            groups: vec![MeshGroupDescriptor {
                material_slot: 0,
                start: 0,
                count: mesh.indices.len() as u32,
            }],
            bounds: MeshBoundsDescriptor {
                min: mesh.min,
                max: mesh.max,
            },
            source: MeshPayloadSource::Inline {
                positions: mesh.positions.clone(),
                normals: mesh.normals.clone(),
                uvs: Some(mesh.uvs.clone()),
                colors: None,
                indices: mesh.indices.clone(),
            },
            provenance: MeshProvenance::Generated,
            layer_weights: false,
            layer_palette: Vec::new(),
            vertex_occlusion: false,
            texture_space: None,
            distance_field: None,
            morph: None,
        },
        material_slots: vec![MeshMaterialSlot {
            slot: 0,
            material: material.to_owned(),
        }],
        collision: MeshCollisionPolicy::VisualOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells<'a>(bed: &'a [f32], depth: &'a [f32]) -> WaterSurfaceCells<'a> {
        WaterSurfaceCells {
            width: 3,
            length: 2,
            cell_size: 2.0,
            bed,
            depth,
            drawn_depth: 0.01,
        }
    }

    #[test]
    fn wet_cells_join_at_their_mean_surface_and_dry_ones_are_left_out() {
        let bed = [0.0, 0.5, 3.0, 0.0, 0.5, 3.0];
        let depth = [1.0, 0.7, 0.0, 1.0, 0.7, 0.005];
        let mesh = water_surface_mesh(cells(&bed, &depth)).expect("two columns are wet");
        assert_eq!(mesh.drawn_cells, 4);
        // Three corner columns of three rows; the dry column adds none.
        assert_eq!(mesh.positions.len() / 3, 9);
        assert_eq!(mesh.indices.len(), 4 * 6);
        let height_at = |x: f32, z: f32| {
            mesh.positions
                .chunks(3)
                .find(|p| p[0] == x && p[2] == z)
                .map(|p| p[1])
                .expect("corner drawn")
        };
        assert_eq!(height_at(0.0, 0.0), 1.0);
        // Between surfaces 1.0 and 1.2.
        assert!((height_at(2.0, 2.0) - 1.1).abs() < 1.0e-6);
        assert_eq!(height_at(4.0, 4.0), 1.2);
        assert_eq!(mesh.min, [0.0, 1.0, 0.0]);
        assert_eq!(mesh.max, [4.0, 1.2, 4.0]);
        for normal in mesh.normals.chunks(3) {
            assert!(normal[1] > 0.9, "the surface faces up: {normal:?}");
        }
    }

    #[test]
    fn a_dry_field_has_no_surface() {
        let bed = [0.0; 6];
        let depth = [0.005; 6];
        assert!(water_surface_mesh(cells(&bed, &depth)).is_none());
    }
}
