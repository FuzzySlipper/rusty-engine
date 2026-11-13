//! An inline mesh's product data (#10803): four values per vertex that a
//! product shader's shade stage reads as `Surface.product_data`,
//! interpolated, whole or partitioned alike, and zero for a mesh without.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// Writes the product data as colour: red and green its x and y, blue one
/// less its w (the wall's is 1), so data reading zero draws blue.
const DATA: &str = "#import rusty::types::Surface

fn shade(surface: Surface) -> vec4<f32> {
    let data = surface.product_data;
    return vec4<f32>(data.x, data.y, 1.0 - data.w, 1.0);
}
";

/// The wall's cells across and up, and its corners (x, y) at z = -3.
const CELLS: (u32, u32) = (8, 4);
const MIN: [f32; 2] = [-2.0, -1.0];
const MAX: [f32; 2] = [2.0, 1.0];
const DEPTH: f32 = -3.0;

/// A grid wall facing the camera; with `data`, each vertex's product data is
/// its place on the wall, 0 to 1 across (x) and up (y).
fn wall(data: bool) -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut values, mut indices) =
        (vec![], vec![], vec![], vec![], vec![]);
    for j in 0..=CELLS.1 {
        for i in 0..=CELLS.0 {
            let (u, v) = (i as f32 / CELLS.0 as f32, j as f32 / CELLS.1 as f32);
            positions.extend_from_slice(&[
                MIN[0] + (MAX[0] - MIN[0]) * u,
                MIN[1] + (MAX[1] - MIN[1]) * v,
                DEPTH,
            ]);
            normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            uvs.extend_from_slice(&[u, 1.0 - v]);
            values.extend_from_slice(&[u, v, 0.0, 1.0]);
        }
    }
    let row = CELLS.0 + 1;
    for j in 0..CELLS.1 {
        for i in 0..CELLS.0 {
            let at = j * row + i;
            indices.extend_from_slice(&[at, at + 1, at + row + 1, at, at + row + 1, at + row]);
        }
    }
    let count = indices.len() as u32;
    let mut mesh = payload(positions, normals, uvs, indices, &[(0, count)]);
    mesh.product_data = data.then(|| values.into());
    mesh
}

fn render(meshes: Vec<MeshPayloadDescriptor>) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut material = material("material/data", [1.0; 4], None);
    material.shader = Some(MaterialShaderDescriptor {
        shader: "shader/data".to_owned(),
        parameters: [[0.0; 4]; 4],
        textures: [None, None],
        displacement_bound: 0.0,
    });
    let mut ops = vec![
        RenderDiff::DefineShader {
            shader: ShaderDescriptor {
                id: "shader/data".to_owned(),
                path: "shaders/data.wgsl".to_owned(),
                source: DATA.to_owned(),
                keywords: Vec::new(),
            },
        },
        RenderDiff::DefineMaterial { material },
    ];
    for (index, mesh) in meshes.into_iter().enumerate() {
        let asset = format!("mesh/wall-{index}");
        ops.push(static_mesh(&asset, mesh, "material/data"));
        ops.push(instance(
            index as u64 + 1,
            None,
            &asset,
            Transform::IDENTITY,
        ));
    }
    harness.apply(ops);
    harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0)).1
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * WIDTH + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

#[test]
fn a_product_shader_reads_the_product_data_whole_partitioned_and_absent() {
    let whole = render(vec![wall(true)]);
    // Along the middle row red grows to the right and green holds; down the
    // middle column green falls and red holds. Blue is zero.
    let row: Vec<[u8; 4]> = (WIDTH * 3 / 10..WIDTH * 7 / 10)
        .map(|x| pixel(&whole, x, HEIGHT / 2))
        .collect();
    let column: Vec<[u8; 4]> = (HEIGHT * 3 / 10..HEIGHT * 7 / 10)
        .map(|y| pixel(&whole, WIDTH / 2, y))
        .collect();
    assert!(
        row.windows(2).all(|pair| pair[1][0] >= pair[0][0]),
        "{row:?}"
    );
    assert!(row.last().unwrap()[0] > row[0][0] + 60, "{row:?}");
    assert!(
        column.windows(2).all(|pair| pair[1][1] <= pair[0][1]),
        "{column:?}"
    );
    assert!(column[0][1] > column.last().unwrap()[1] + 60, "{column:?}");
    for sample in row.iter().chain(&column) {
        assert_eq!(sample[2], 0, "{sample:?}");
    }
    let spread = |values: &mut dyn Iterator<Item = u8>| {
        let values: Vec<u8> = values.collect();
        values.iter().max().unwrap() - values.iter().min().unwrap()
    };
    assert!(spread(&mut row.iter().map(|sample| sample[1])) <= 1);
    assert!(spread(&mut column.iter().map(|sample| sample[0])) <= 1);
    // The middle of the wall is half way both ways.
    let middle = pixel(&whole, WIDTH / 2, HEIGHT / 2);
    assert!(middle[0] > 100 && middle[0] < 220, "{middle:?}");
    assert!(middle[0].abs_diff(middle[1]) <= 2, "{middle:?}");

    // Cut into eight parts, each carrying its vertices' data: the same
    // picture.
    let parts =
        partition_mesh_spatially(&wall(true), [MIN[0], MIN[1], -10.0], [1.0, 1.0, 20.0]).unwrap();
    assert_eq!(parts.len(), 8);
    assert!(parts.iter().all(|part| part.product_data.is_some()));
    let partitioned = render(parts);
    assert!(
        whole == partitioned,
        "the partitioned wall draws as the whole one"
    );

    // Without the stream the same shader reads zero.
    let absent = render(vec![wall(false)]);
    assert_eq!(pixel(&absent, WIDTH / 2, HEIGHT / 2), [0, 0, 255, 255]);
    assert_eq!(pixel(&absent, WIDTH * 2 / 3, HEIGHT / 3), [0, 0, 255, 255]);
    assert_ne!(
        pixel(&absent, 2, 2),
        [0, 0, 255, 255],
        "the wall, not the sky"
    );
}
