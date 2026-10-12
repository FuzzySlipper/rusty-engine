//! Water field surfaces as renderer nodes (#9884): one static mesh instance
//! per drawn field, its mesh redefined from each new snapshot.
//!
//! [`water_surface_mesh`] builds the mesh from a snapshot's cells: each wet
//! cell's two triangles over corners at the mean surface of the wet cells
//! around them, so neighbouring cells join without cracks, clipped where that
//! surface falls below the bed (each corner's bed the mean of its cells, as
//! ground interpolated between cell centres runs). The shore is the wet
//! cells' edge, or where a bank rises through the water within a wet cell.
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

/// The surface of every cell deeper than the drawn depth, trimmed to the
/// bed along the shore, or `None` when no cell is that deep.
pub fn water_surface_mesh(cells: WaterSurfaceCells<'_>) -> Option<WaterSurfaceMesh> {
    let WaterSurfaceCells {
        width,
        length,
        cell_size,
        bed,
        depth,
        drawn_depth,
    } = cells;
    let wet = |x: usize, z: usize| depth[z * width + x] > drawn_depth;
    let corners = width + 1;
    let corner_count = corners * (length + 1);
    // The cells touching a corner.
    let around = |corner: usize| {
        let (cx, cz) = ((corner % corners) as isize, (corner / corners) as isize);
        [(-1, -1), (0, -1), (-1, 0), (0, 0)]
            .into_iter()
            .map(move |(dx, dz)| (cx + dx, cz + dz))
            .filter(|&(x, z)| x >= 0 && z >= 0 && (x as usize) < width && (z as usize) < length)
            .map(|(x, z)| (x as usize, z as usize))
    };
    // Each corner's bed: the mean of the cells around it. Its surface: the
    // mean surface of the wet cells around it (every corner of a wet cell has
    // one).
    let mut corner_bed = vec![0.0f32; corner_count];
    let mut surface = vec![f32::NAN; corner_count];
    let mut drawn_cells = 0;
    for (corner, (bed_slot, surface_slot)) in
        corner_bed.iter_mut().zip(surface.iter_mut()).enumerate()
    {
        let (mut bed_sum, mut beds, mut wet_sum, mut wets) = (0.0, 0.0, 0.0, 0.0);
        for (x, z) in around(corner) {
            let i = z * width + x;
            bed_sum += bed[i];
            beds += 1.0;
            if wet(x, z) {
                wet_sum += bed[i] + depth[i];
                wets += 1.0;
            }
        }
        *bed_slot = bed_sum / beds;
        if wets > 0.0 {
            *surface_slot = wet_sum / wets;
        }
    }
    for z in 0..length {
        for x in 0..width {
            if wet(x, z) {
                drawn_cells += 1;
            }
        }
    }
    if drawn_cells == 0 {
        return None;
    }
    let water_depth = |corner: usize| surface[corner] - corner_bed[corner];
    let normal_at = |corner: usize| {
        let (cx, cz) = ((corner % corners) as isize, (corner / corners) as isize);
        let own = surface[corner];
        let at = |x: isize, z: isize| {
            let inside = x >= 0 && z >= 0 && (x as usize) <= width && (z as usize) <= length;
            let level = if inside {
                surface[z as usize * corners + x as usize]
            } else {
                f32::NAN
            };
            if level.is_nan() {
                own
            } else {
                level
            }
        };
        let dx = (at(cx + 1, cz) - at(cx - 1, cz)) / (2.0 * cell_size);
        let dz = (at(cx, cz + 1) - at(cx, cz - 1)) / (2.0 * cell_size);
        normalize([-dx, 1.0, -dz])
    };

    let mut mesh = WaterSurfaceMesh {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
        drawn_cells,
        ..WaterSurfaceMesh::default()
    };
    // Vertices by corner, and by the edge between two corners where the
    // shore crosses it, shared by the triangles on either side.
    let mut vertices: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    let mut vertex = |mesh: &mut WaterSurfaceMesh, a: usize, b: usize| -> u32 {
        let key = (a.min(b), a.max(b));
        *vertices.entry(key).or_insert_with(|| {
            let (da, db) = (water_depth(a), water_depth(b));
            let t = if a == b { 0.0 } else { da / (da - db) };
            let place = |corner: usize| {
                [
                    (corner % corners) as f32 * cell_size,
                    surface[corner],
                    (corner / corners) as f32 * cell_size,
                ]
            };
            let (pa, pb) = (place(a), place(b));
            let mut position: [f32; 3] =
                std::array::from_fn(|axis| pa[axis] + (pb[axis] - pa[axis]) * t);
            if a != b {
                // On the shore the water meets the bed.
                position[1] = corner_bed[a] + (corner_bed[b] - corner_bed[a]) * t;
            }
            let (na, nb) = (normal_at(a), normal_at(b));
            let normal = normalize(std::array::from_fn(|axis| {
                na[axis] + (nb[axis] - na[axis]) * t
            }));
            mesh.min = std::array::from_fn(|axis| mesh.min[axis].min(position[axis]));
            mesh.max = std::array::from_fn(|axis| mesh.max[axis].max(position[axis]));
            let index = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend_from_slice(&position);
            mesh.normals.extend_from_slice(&normal);
            mesh.uvs.extend_from_slice(&[position[0], position[2]]);
            index
        })
    };
    for z in 0..length {
        for x in 0..width {
            let a = z * corners + x;
            let (b, c, d) = (a + 1, a + corners, a + corners + 1);
            // Only wet cells hold water: a dry cell is dry however low its
            // ground, as the field's samples say.
            if !wet(x, z) {
                continue;
            }
            for triangle in [[a, c, b], [b, c, d]] {
                // The triangle's part where the water stands above the bed,
                // as corners and shore crossings in order.
                let mut polygon: Vec<(usize, usize)> = Vec::with_capacity(4);
                for k in 0..3 {
                    let (p, q) = (triangle[k], triangle[(k + 1) % 3]);
                    let (dp, dq) = (water_depth(p), water_depth(q));
                    if dp >= 0.0 {
                        polygon.push((p, p));
                    }
                    if (dp >= 0.0) != (dq >= 0.0) {
                        polygon.push((p, q));
                    }
                }
                if polygon.len() < 3 {
                    continue;
                }
                let ids: Vec<u32> = polygon
                    .iter()
                    .map(|&(p, q)| vertex(&mut mesh, p, q))
                    .collect();
                for k in 1..ids.len() - 1 {
                    if ids[0] != ids[k] && ids[k] != ids[k + 1] && ids[0] != ids[k + 1] {
                        mesh.indices
                            .extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
                    }
                }
            }
        }
    }
    if mesh.indices.is_empty() {
        return None;
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
    fn a_wet_flat_field_is_one_level_surface() {
        let bed = [0.0; 6];
        let depth = [0.7; 6];
        let mesh = water_surface_mesh(cells(&bed, &depth)).expect("wet");
        assert_eq!(mesh.drawn_cells, 6);
        assert_eq!(mesh.positions.len() / 3, 4 * 3);
        assert_eq!(mesh.indices.len(), 6 * 6);
        assert!(mesh
            .positions
            .chunks(3)
            .all(|p| (p[1] - 0.7).abs() < 1.0e-6));
        assert_eq!(mesh.min, [0.0, 0.7, 0.0]);
        assert_eq!(mesh.max, [6.0, 0.7, 4.0]);
    }

    /// R9884-4: one wet cell on flat ground draws that cell and nothing of
    /// its dry neighbours.
    #[test]
    fn an_isolated_wet_cell_draws_only_itself() {
        let bed = [0.0; 25];
        let mut depth = [0.0; 25];
        depth[12] = 1.0;
        let cells = WaterSurfaceCells {
            width: 5,
            length: 5,
            cell_size: 1.0,
            bed: &bed,
            depth: &depth,
            drawn_depth: 0.01,
        };
        let mesh = water_surface_mesh(cells).expect("one wet cell");
        assert_eq!(mesh.drawn_cells, 1);
        assert_eq!(mesh.min, [2.0, 1.0, 2.0]);
        assert_eq!(mesh.max, [3.0, 1.0, 3.0]);
        assert!((projected_area(&mesh) - 1.0).abs() < 1.0e-6);
    }

    /// Water standing at 0.6 over cells whose ground, interpolated between
    /// cell centres as ground drawn from them is, rises through 0.6 within
    /// the last wet cell: the shore is that crossing, with nothing drawn
    /// under the bank. Where the bank rises only in the dry cell beyond, the
    /// shore is the wet cells' edge.
    #[test]
    fn a_shore_ends_where_the_bank_rises_through_the_water_or_at_the_wet_edge() {
        const LEVEL: f32 = 0.6;
        for (bed, shore) in [
            // Cell 2 (x 4..6) is wet 0.1 deep; ground through the centres
            // (x = 1, 3, 5, ...) reaches 0.6 at x = 5 + 2 * 0.1 / 0.5 = 5.4.
            ([0.0, 0.0, 0.5, 1.0, 2.0, 3.0], 5.4),
            // The bank rises in dry cell 3: the water ends at its edge, x = 6.
            ([0.0, 0.0, 0.0, 1.0, 2.0, 3.0], 6.0),
        ] {
            let depth: Vec<f32> = bed.iter().map(|b| (LEVEL - b).max(0.0)).collect();
            let cells = WaterSurfaceCells {
                width: 6,
                length: 1,
                cell_size: 2.0,
                bed: &bed,
                depth: &depth,
                drawn_depth: 0.01,
            };
            let mesh = water_surface_mesh(cells).expect("wet cells");
            assert!(
                (mesh.max[0] - shore).abs() < 1.0e-4,
                "shore at x = {}, not {shore}",
                mesh.max[0]
            );
            let ground = |x: f32| {
                let centre = (x / 2.0 - 0.5).clamp(0.0, 5.0);
                let low = centre.floor() as usize;
                let high = (low + 1).min(5);
                bed[low] + (bed[high] - bed[low]) * (centre - low as f32)
            };
            for p in mesh.positions.chunks(3) {
                assert!(
                    p[1] >= ground(p[0]) - 1.0e-5,
                    "water under the bank at {p:?}"
                );
                assert!(p[1] <= LEVEL + 1.0e-5);
            }
            for normal in mesh.normals.chunks(3) {
                assert!(normal[1] > 0.9, "the surface faces up: {normal:?}");
            }
        }
    }

    /// The mesh's area seen from above.
    fn projected_area(mesh: &WaterSurfaceMesh) -> f32 {
        mesh.indices
            .chunks(3)
            .map(|t| {
                let p = |i: u32| {
                    [
                        mesh.positions[i as usize * 3],
                        mesh.positions[i as usize * 3 + 2],
                    ]
                };
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs()
            })
            .sum()
    }

    #[test]
    fn a_dry_field_has_no_surface() {
        let bed = [0.0; 6];
        let depth = [0.005; 6];
        assert!(water_surface_mesh(cells(&bed, &depth)).is_none());
    }
}
