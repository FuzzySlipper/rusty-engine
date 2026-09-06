//! A scene-scale constructive input for measuring the geometry kernel alone.
use svc_implicit::{Bounds, Field, GenerateOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut field = Field::new();
    let mut wall = field.box_shape(Bounds {
        min: [0.0, 0.0, -0.45],
        max: [24.0, 5.0, 0.0],
    })?;
    for course in 0..8 {
        for column in 0..30 {
            let x = column as f32 * 0.8;
            let y = course as f32 * 0.6;
            let stone = field.box_shape(Bounds {
                min: [x + 0.025, y + 0.025, -0.1],
                max: [x + 0.775, y + 0.575, 0.1],
            })?;
            wall = field.union(wall, stone)?;
        }
    }
    let door = field.box_shape(Bounds {
        min: [10.0, -0.1, -1.0],
        max: [13.0, 3.5, 1.0],
    })?;
    wall = field.difference(wall, door)?;
    let root = field.capsule([3.0, 0.1, 0.25], [6.0, 2.0, 0.25], 0.2)?;
    wall = field.union(wall, root)?;
    let geometry = field.generate(
        wall,
        GenerateOptions {
            bounds: Bounds {
                min: [-0.1, -0.2, -0.6],
                max: [24.1, 5.2, 0.6],
            },
            cell_size: 0.08,
            max_vertices: 1_000_000,
            max_triangles: 2_000_000,
        },
    )?;
    println!("nodes={} vertices={} triangles={} depth={} spacing={:?} extraction_seconds={:.3} reoriented={} degenerate={}",
        field.node_count(), geometry.positions.len(), geometry.triangles.len(), geometry.depth,
        geometry.cell_size, geometry.generation_seconds, geometry.reoriented_triangles, geometry.degenerate_triangles);
    Ok(())
}
