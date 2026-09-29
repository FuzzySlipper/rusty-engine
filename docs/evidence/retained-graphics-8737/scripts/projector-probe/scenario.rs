// Shared #8737 probe scenario: a catalog with one static mesh and one cube,
// BULK unchanged mesh instances and MOVING cubes that move every frame.
use std::{collections::BTreeMap, sync::Arc, time::Instant};

use render_model::{
    Geometry, Material, MaterialUvStrategy, MeshAttribute, MeshAttributeKind, MeshAttributeName,
    MeshBoundsDescriptor, MeshBufferLayout, MeshCollisionPolicy, MeshGroupDescriptor,
    MeshIndexWidth, MeshMaterialSlot, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance,
    RenderLayer, RenderMaterialDescriptor, StaticMeshAsset, Transform,
};
use render_projection::{Appearance, AppearanceResources, RuntimeAppearanceCatalog};

pub const MOVING: u64 = 50;
pub const FRAMES: usize = 200;
pub const MESH: &str = "appearance/mesh";
pub const CUBE: &str = "appearance/cube";

pub fn catalog() -> RuntimeAppearanceCatalog {
    let material = RenderMaterialDescriptor {
        schema_version: 2,
        id: "material/plain".to_string(),
        color: [0.4, 0.5, 0.6, 1.0],
        texture: None,
        roughness: 1.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: Default::default(),
        double_sided: false,
        voxel_surface: None,
    };
    let mesh = StaticMeshAsset {
        asset: "mesh/triangle".to_string(),
        payload: MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count: 3,
                index_count: 3,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    MeshAttribute {
                        name: MeshAttributeName::Position,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                    MeshAttribute {
                        name: MeshAttributeName::Normal,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                ],
            },
            groups: vec![MeshGroupDescriptor {
                material_slot: 0,
                start: 0,
                count: 3,
            }],
            bounds: MeshBoundsDescriptor {
                min: [0.0; 3],
                max: [1.0, 1.0, 0.0],
            },
            source: MeshPayloadSource::Inline {
                positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                uvs: None,
                colors: None,
                indices: vec![0, 1, 2],
            },
            provenance: MeshProvenance::StaticAsset,
        },
        material_slots: vec![MeshMaterialSlot {
            slot: 0,
            material: "material/plain".to_string(),
        }],
        collision: MeshCollisionPolicy::VisualOnly,
    };
    RuntimeAppearanceCatalog {
        resources: AppearanceResources {
            materials: vec![material],
            static_meshes: vec![Arc::new(mesh)],
            ..AppearanceResources::default()
        },
        appearances: BTreeMap::from([
            (
                MESH.to_owned(),
                Appearance::StaticMesh {
                    asset: "mesh/triangle".to_owned(),
                    material_overrides: Vec::new(),
                },
            ),
            (
                CUBE.to_owned(),
                Appearance::Primitive {
                    geometry: Geometry::Cube,
                    material: Material::DEFAULT,
                },
            ),
        ]),
    }
}

/// Object `id`: the first MOVING are cubes, the rest mesh instances in a grid.
pub fn object(id: u64, frame: usize) -> (&'static str, Transform, bool, RenderLayer) {
    let mut transform = Transform::IDENTITY;
    if id <= MOVING {
        transform.translation = [id as f32, frame as f32 * 0.01, 0.0];
        (CUBE, transform, true, RenderLayer::Scene)
    } else {
        transform.translation = [(id % 100) as f32, 0.0, (id / 100) as f32];
        (MESH, transform, true, RenderLayer::Scene)
    }
}

pub fn measure(mut frame: impl FnMut(usize) -> usize) -> (u128, u128, usize) {
    let mut samples = Vec::with_capacity(FRAMES);
    let mut operations = 0;
    for index in 1..=FRAMES {
        let started = Instant::now();
        operations = frame(index);
        samples.push(started.elapsed().as_micros());
    }
    samples.sort_unstable();
    (samples[FRAMES / 2], samples[FRAMES * 95 / 100], operations)
}
