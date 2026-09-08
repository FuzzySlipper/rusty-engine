use super::*;
use crate::{
    MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
    MeshGroupDescriptor, MeshIndexWidth, MeshProvenance,
};

fn attributed_mesh() -> MeshPayloadDescriptor {
    let positions = vec![
        -3.0, -1.0, 0.0, -1.0, -1.0, 0.0, -2.0, 1.0, 0.0, // crosses the [-2, 0] cell boundary
        1.0, -1.0, 0.0, 3.0, -1.0, 0.0, 2.0, 1.0, 0.0, 5.0, -1.0, 0.0, 7.0, -1.0, 0.0, 6.0, 1.0,
        0.0,
    ];
    let normals = (0..9)
        .flat_map(|vertex| {
            [
                vertex as f32 + 0.25,
                vertex as f32 + 0.5,
                vertex as f32 + 0.75,
            ]
        })
        .collect();
    let uvs = Some(
        (0..9)
            .flat_map(|vertex| [vertex as f32 / 10.0, vertex as f32 / 20.0])
            .collect(),
    );
    let colors = Some(
        (0..9)
            .flat_map(|vertex| [vertex as f32 / 10.0, 1.0 - vertex as f32 / 10.0, 0.5, 1.0])
            .collect(),
    );
    MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count: 9,
            index_count: 9,
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
                MeshAttribute {
                    name: MeshAttributeName::Uv,
                    components: 2,
                    kind: MeshAttributeKind::F32,
                },
                MeshAttribute {
                    name: MeshAttributeName::Color,
                    components: 4,
                    kind: MeshAttributeKind::F32,
                },
            ],
        },
        groups: vec![
            MeshGroupDescriptor {
                material_slot: 3,
                start: 0,
                count: 3,
            },
            MeshGroupDescriptor {
                material_slot: 9,
                start: 3,
                count: 6,
            },
        ],
        bounds: MeshBoundsDescriptor {
            min: [-3.0, -1.0, 0.0],
            max: [7.0, 1.0, 0.0],
        },
        source: MeshPayloadSource::Inline {
            positions,
            normals,
            uvs,
            colors,
            indices: (0..9).collect(),
        },
        provenance: MeshProvenance::StaticAsset,
    }
}

fn triangle_signatures(mesh: &MeshPayloadDescriptor) -> Vec<(u16, Vec<u32>)> {
    let MeshPayloadSource::Inline {
        positions,
        normals,
        uvs,
        colors,
        indices,
    } = &mesh.source
    else {
        panic!("test fixtures use inline meshes");
    };
    let mut result = Vec::new();
    for group in &mesh.groups {
        for triangle in indices[group.start as usize..(group.start + group.count) as usize]
            .as_chunks::<3>()
            .0
        {
            let mut attributes = Vec::new();
            for &index in triangle {
                let index = index as usize;
                attributes.extend(
                    positions[index * 3..index * 3 + 3]
                        .iter()
                        .map(|value| value.to_bits()),
                );
                attributes.extend(
                    normals[index * 3..index * 3 + 3]
                        .iter()
                        .map(|value| value.to_bits()),
                );
                if let Some(uvs) = uvs {
                    attributes.extend(
                        uvs[index * 2..index * 2 + 2]
                            .iter()
                            .map(|value| value.to_bits()),
                    );
                }
                if let Some(colors) = colors {
                    attributes.extend(
                        colors[index * 4..index * 4 + 4]
                            .iter()
                            .map(|value| value.to_bits()),
                    );
                }
            }
            result.push((group.material_slot, attributes));
        }
    }
    result.sort();
    result
}

#[test]
fn partitions_conserve_attributed_triangles_bitwise_across_bins() {
    let mesh = attributed_mesh();
    let parts = partition_mesh_spatially(&mesh, [0.0; 3], [2.0, 10.0, 10.0]).unwrap();

    assert_eq!(parts.len(), 3);
    assert_eq!(
        parts
            .iter()
            .flat_map(triangle_signatures)
            .collect::<Vec<_>>(),
        triangle_signatures(&mesh)
    );
    for part in &parts {
        assert_eq!(part.provenance, mesh.provenance);
        assert_eq!(part.layout.attributes, mesh.layout.attributes);
        assert_eq!(
            part.layout.index_count as usize,
            match &part.source {
                MeshPayloadSource::Inline { indices, .. } => indices.len(),
                _ => unreachable!(),
            }
        );
        part.validate().unwrap();
    }
}

#[test]
fn negative_coordinates_use_the_supplied_shifted_origin() {
    let mesh = attributed_mesh();
    let parts = partition_mesh_spatially(&mesh, [-10.0, -10.0, -10.0], [2.0, 10.0, 10.0]).unwrap();

    assert_eq!(parts.len(), 3);
    assert!(parts
        .iter()
        .any(|part| part.bounds.min == [-3.0, -1.0, 0.0]));
}

#[test]
fn crossing_triangle_uses_its_actual_vertex_bounds() {
    let mesh = attributed_mesh();
    let parts = partition_mesh_spatially(&mesh, [0.0; 3], [2.0, 10.0, 10.0]).unwrap();

    let crossing = parts
        .iter()
        .find(|part| part.bounds.min[0] == -3.0)
        .unwrap();
    assert_eq!(
        crossing.bounds,
        MeshBoundsDescriptor {
            min: [-3.0, -1.0, 0.0],
            max: [-1.0, 1.0, 0.0]
        }
    );
}

#[test]
fn preserves_absent_optional_channels() {
    let mut mesh = attributed_mesh();
    let MeshPayloadSource::Inline { uvs, colors, .. } = &mut mesh.source else {
        unreachable!()
    };
    *uvs = None;
    *colors = None;
    mesh.layout.attributes.retain(|attribute| {
        !matches!(
            attribute.name,
            MeshAttributeName::Uv | MeshAttributeName::Color
        )
    });

    for part in partition_mesh_spatially(&mesh, [0.0; 3], [2.0, 10.0, 10.0]).unwrap() {
        let MeshPayloadSource::Inline { uvs, colors, .. } = part.source else {
            unreachable!()
        };
        assert_eq!(uvs, None);
        assert_eq!(colors, None);
    }
}

#[test]
fn rejects_invalid_cells_and_non_inline_sources() {
    let mesh = attributed_mesh();
    assert!(partition_mesh_spatially(&mesh, [f32::NAN, 0.0, 0.0], [1.0; 3]).is_err());
    assert!(partition_mesh_spatially(&mesh, [0.0; 3], [1.0, 0.0, 1.0]).is_err());

    let mut shared = mesh;
    shared.source = MeshPayloadSource::SharedBuffer {
        buffer: 1,
        positions_byte_offset: 0,
        normals_byte_offset: 108,
        uvs_byte_offset: Some(216),
        colors_byte_offset: Some(288),
        indices_byte_offset: 432,
    };
    assert_eq!(
        partition_mesh_spatially(&shared, [0.0; 3], [1.0; 3]),
        Err("spatial mesh partition requires an inline mesh")
    );
}
