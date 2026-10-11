//! Heightfield terrain tiles as renderer nodes (#9842): a group node per
//! heightfield, placed in its layer and focused for the tiles' geomorph
//! (`RenderDiff::SetMorphFocus`), and beneath it one static mesh instance
//! per meshed tile, shown while the tile is drawn.

use std::collections::BTreeMap;

use render_model::{
    Geometry, MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
    MeshBufferLayout, MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
    MeshMorph, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, MeshTextureSpace,
    RenderDiff, RenderLayer, RenderMetadata, RenderNode, ShadowCasting, StaticMeshAsset,
    StaticMeshInstanceDescriptor, Transform,
};
use svc_mesh::heightfield::{TileKey, TileMesh};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

/// Where a heightfield stands: its point (0, 0, 0) and the layer units one
/// of its units spans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldPlacement {
    pub origin: [f32; 3],
    pub scale: f32,
    pub layer: RenderLayer,
    pub shadow_casting: ShadowCasting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NodeKey {
    Root(u64),
    Tile(u64, TileKey),
}

#[derive(Debug, Clone)]
struct Projected {
    placement: HeightfieldPlacement,
    focus: Option<[f64; 2]>,
    /// Each tile's node and whether it is shown.
    tiles: BTreeMap<TileKey, bool>,
}

/// The renderer nodes of every heightfield.
#[derive(Debug, Clone)]
pub struct HeightfieldProjector {
    registry: StableHandleRegistry<NodeKey>,
    heightfields: BTreeMap<u64, Projected>,
}

impl Default for HeightfieldProjector {
    fn default() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::HEIGHTFIELD),
            heightfields: BTreeMap::new(),
        }
    }
}

/// The static mesh a tile draws as.
pub fn heightfield_tile_asset(heightfield: u64, key: TileKey) -> String {
    format!(
        "mesh/heightfield-{heightfield}/{}/{}/{}",
        key.ring, key.x, key.z
    )
}

impl HeightfieldProjector {
    /// Create the heightfield's node, or move it.
    pub fn place(
        &mut self,
        heightfield: u64,
        placement: HeightfieldPlacement,
    ) -> Result<Vec<RenderDiff>, HandleAllocationError> {
        let transform = Transform {
            translation: placement.origin,
            scale: [placement.scale; 3],
            ..Transform::IDENTITY
        };
        match self.heightfields.get_mut(&heightfield) {
            Some(projected) => {
                if projected.placement == placement {
                    return Ok(Vec::new());
                }
                projected.placement = placement;
                let handle = self.registry.handle_of(&NodeKey::Root(heightfield));
                Ok(handle
                    .map(|handle| RenderDiff::Update {
                        handle,
                        transform: Some(transform),
                        material: None,
                        visible: None,
                        metadata: None,
                    })
                    .into_iter()
                    .collect())
            }
            None => {
                let handle = self.registry.allocate(NodeKey::Root(heightfield))?;
                self.heightfields.insert(
                    heightfield,
                    Projected {
                        placement,
                        focus: None,
                        tiles: BTreeMap::new(),
                    },
                );
                let mut node = RenderNode::new(Geometry::Group);
                node.transform = transform;
                node.layer = placement.layer;
                Ok(vec![RenderDiff::Create {
                    handle,
                    parent: None,
                    node,
                }])
            }
        }
    }

    /// Move the point the tiles morph about (heightfield units).
    pub fn focus(&mut self, heightfield: u64, focus: Option<[f64; 2]>) -> Vec<RenderDiff> {
        let Some(projected) = self.heightfields.get_mut(&heightfield) else {
            return Vec::new();
        };
        if projected.focus == focus {
            return Vec::new();
        }
        projected.focus = focus;
        self.registry
            .handle_of(&NodeKey::Root(heightfield))
            .map(|handle| RenderDiff::SetMorphFocus { handle, focus })
            .into_iter()
            .collect()
    }

    /// Define (or redefine) a tile's mesh, with its node where it stands,
    /// shown or hidden; without a mesh, remove the tile.
    pub fn mesh(
        &mut self,
        heightfield: u64,
        key: TileKey,
        origin: [f64; 2],
        mesh: Option<&TileMesh>,
        materials: &BTreeMap<u32, String>,
        shown: bool,
    ) -> Result<Vec<RenderDiff>, HandleAllocationError> {
        let Some(mesh) = mesh else {
            return Ok(self.remove_tile(heightfield, key));
        };
        let Some(projected) = self.heightfields.get_mut(&heightfield) else {
            return Ok(Vec::new());
        };
        let asset = heightfield_tile_asset(heightfield, key);
        let mut operations = vec![RenderDiff::DefineStaticMesh {
            asset: tile_asset(&asset, origin, mesh, materials),
        }];
        if let std::collections::btree_map::Entry::Vacant(entry) = projected.tiles.entry(key) {
            let parent = self.registry.handle_of(&NodeKey::Root(heightfield));
            let handle = self.registry.allocate(NodeKey::Tile(heightfield, key))?;
            entry.insert(shown);
            operations.push(RenderDiff::CreateStaticMeshInstance {
                handle,
                parent,
                instance: StaticMeshInstanceDescriptor {
                    asset,
                    transform: Transform {
                        translation: [origin[0] as f32, 0.0, origin[1] as f32],
                        ..Transform::IDENTITY
                    },
                    visible: shown,
                    material_overrides: Vec::new(),
                    metadata: RenderMetadata::default(),
                    layer: projected.placement.layer,
                    shadow_casting: projected.placement.shadow_casting,
                },
            });
        } else {
            operations.extend(self.show(heightfield, key, shown));
        }
        Ok(operations)
    }

    /// Show or hide a tile's node.
    pub fn show(&mut self, heightfield: u64, key: TileKey, shown: bool) -> Vec<RenderDiff> {
        let Some(current) = self
            .heightfields
            .get_mut(&heightfield)
            .and_then(|projected| projected.tiles.get_mut(&key))
        else {
            return Vec::new();
        };
        if *current == shown {
            return Vec::new();
        }
        *current = shown;
        self.registry
            .handle_of(&NodeKey::Tile(heightfield, key))
            .map(|handle| RenderDiff::Update {
                handle,
                transform: None,
                material: None,
                visible: Some(shown),
                metadata: None,
            })
            .into_iter()
            .collect()
    }

    /// Remove a tile's node and mesh.
    pub fn remove_tile(&mut self, heightfield: u64, key: TileKey) -> Vec<RenderDiff> {
        let removed = self
            .heightfields
            .get_mut(&heightfield)
            .and_then(|projected| projected.tiles.remove(&key));
        let handle = self.registry.remove(&NodeKey::Tile(heightfield, key));
        match (removed, handle) {
            (Some(_), Some(handle)) => vec![
                RenderDiff::Destroy { handle },
                RenderDiff::ReleaseStaticMesh {
                    asset: heightfield_tile_asset(heightfield, key),
                },
            ],
            _ => Vec::new(),
        }
    }

    /// Remove a heightfield: its tiles and its node.
    pub fn remove(&mut self, heightfield: u64) -> Vec<RenderDiff> {
        let Some(projected) = self.heightfields.get(&heightfield) else {
            return Vec::new();
        };
        let keys: Vec<TileKey> = projected.tiles.keys().copied().collect();
        let mut operations: Vec<RenderDiff> = keys
            .into_iter()
            .flat_map(|key| self.remove_tile(heightfield, key))
            .collect();
        self.heightfields.remove(&heightfield);
        if let Some(handle) = self.registry.remove(&NodeKey::Root(heightfield)) {
            operations.push(RenderDiff::Destroy { handle });
        }
        operations
    }
}

/// A tile's mesh as a static mesh asset: its groups' material indices as
/// slots bound to the heightfield's materials, its uvs and triplanar
/// positions in the heightfield's own units.
fn tile_asset(
    asset: &str,
    origin: [f64; 2],
    mesh: &TileMesh,
    materials: &BTreeMap<u32, String>,
) -> StaticMeshAsset {
    let vertex_count = (mesh.positions.len() / 3) as u32;
    let attribute = |name, components| MeshAttribute {
        name,
        components,
        kind: MeshAttributeKind::F32,
    };
    let mut attributes = vec![
        attribute(MeshAttributeName::Position, 3),
        attribute(MeshAttributeName::Normal, 3),
        attribute(MeshAttributeName::Uv, 2),
    ];
    if mesh.colors.is_some() {
        attributes.push(attribute(MeshAttributeName::Color, 4));
    }
    let groups: Vec<MeshGroupDescriptor> = mesh
        .groups
        .iter()
        .map(|(material, start, count)| MeshGroupDescriptor {
            material_slot: *material as u16,
            start: *start,
            count: *count,
        })
        .collect();
    let material_slots = mesh
        .groups
        .iter()
        .filter_map(|(material, _, _)| {
            Some(MeshMaterialSlot {
                slot: *material as u16,
                material: materials.get(material)?.clone(),
            })
        })
        .collect();
    StaticMeshAsset {
        asset: asset.to_owned(),
        payload: MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count,
                index_count: mesh.indices.len() as u32,
                index_width: MeshIndexWidth::U32,
                attributes,
            },
            groups,
            bounds: MeshBoundsDescriptor {
                min: mesh.min,
                max: mesh.max,
            },
            source: MeshPayloadSource::Inline {
                positions: mesh.positions.clone(),
                normals: mesh.normals.clone(),
                uvs: Some(mesh.uvs.clone()),
                colors: mesh.colors.clone(),
                indices: mesh.indices.clone(),
            },
            provenance: MeshProvenance::Generated,
            layer_weights: mesh.layer_weights,
            layer_palette: Vec::new(),
            vertex_occlusion: false,
            texture_space: Some(MeshTextureSpace {
                cell_size: 1.0,
                origin: [origin[0] as f32, 0.0, origin[1] as f32],
            }),
            distance_field: None,
            morph: mesh
                .morph
                .as_ref()
                .zip(mesh.morph_range)
                .map(|(targets, (start, end))| MeshMorph {
                    targets: targets.clone(),
                    start: start as f32,
                    end: end as f32,
                }),
        },
        material_slots,
        collision: MeshCollisionPolicy::VisualOnly,
    }
}
