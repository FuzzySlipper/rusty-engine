//! The binary encoding of the Spatial content artifact: the same facts as the
//! JSON schema, recognised by its magic, decoded into the same artifact and
//! validated by the same checks. A cell's level is not stored: validation
//! requires it to equal `round(supportHeight / levelQuantum)`, so it is
//! derived from the height.
//!
//! Little-endian, no padding:
//!
//! | field | encoding |
//! |---|---|
//! | magic | `RSPATIAL` (8 bytes) |
//! | schema version, navigation config schema version | u32, u32 |
//! | bounds min, max | 6 × f64 |
//! | cell size, level quantum, maximum slope degrees, required headroom, support probe drop | 5 × f64 |
//! | static mesh artifact id, navigation id | each u32 byte length + UTF-8 |
//! | position, triangle and cell counts | 3 × u64 |
//! | positions | count × 3 f64 |
//! | triangles | count × 3 u32 |
//! | cell columns, then rows | count × i32 each |
//! | cell support heights | count × f64 |
//! | cell walkable flags | ⌈count / 8⌉ bytes, cell `i` at bit `i % 8` of byte `i / 8` |

use super::{
    SpatialContentArtifact, SpatialContentBounds, SpatialContentCollision,
    SpatialContentNavigation, SpatialContentNavigationCell, SpatialContentNavigationConfig,
};

pub(super) const MAGIC: &[u8; 8] = b"RSPATIAL";

/// Decodes a binary artifact whose bytes begin with [`MAGIC`]. The error
/// says what was malformed; validation of the facts happens afterwards.
pub(super) fn decode(bytes: &[u8]) -> Result<SpatialContentArtifact, String> {
    let mut reader = Reader {
        bytes,
        at: MAGIC.len(),
    };
    let schema_version = reader.u32()?;
    let config_schema_version = reader.u32()?;
    let mut f64s = |count: usize| {
        (0..count)
            .map(|_| reader.f64())
            .collect::<Result<Vec<_>, _>>()
    };
    let bounds = f64s(6)?;
    let config = f64s(5)?;
    let static_mesh_artifact_id = reader.text()?;
    let navigation_id = reader.text()?;
    let positions = reader.count()?;
    let triangles = reader.count()?;
    let cells = reader.count()?;
    let positions = (0..positions)
        .map(|_| Ok([reader.f64()?, reader.f64()?, reader.f64()?]))
        .collect::<Result<Vec<_>, String>>()?;
    let triangles = (0..triangles)
        .map(|_| Ok([reader.u32()?, reader.u32()?, reader.u32()?]))
        .collect::<Result<Vec<_>, String>>()?;
    let columns = (0..cells)
        .map(|_| reader.i32())
        .collect::<Result<Vec<_>, _>>()?;
    let rows = (0..cells)
        .map(|_| reader.i32())
        .collect::<Result<Vec<_>, _>>()?;
    let heights = (0..cells)
        .map(|_| reader.f64())
        .collect::<Result<Vec<_>, _>>()?;
    let walkable = reader.take(cells.div_ceil(8))?;
    if reader.at != bytes.len() {
        return Err(format!(
            "{} bytes follow the last cell",
            bytes.len() - reader.at
        ));
    }
    let level_quantum = config[1];
    let cells = (0..cells)
        .map(|index| SpatialContentNavigationCell {
            column: i64::from(columns[index]),
            row: i64::from(rows[index]),
            level: (heights[index] / level_quantum).round() as i64,
            support_height: heights[index],
            walkable: walkable[index / 8] & (1 << (index % 8)) != 0,
        })
        .collect();
    Ok(SpatialContentArtifact {
        schema_version,
        static_mesh_artifact_id,
        bounds: SpatialContentBounds {
            min: [bounds[0], bounds[1], bounds[2]],
            max: [bounds[3], bounds[4], bounds[5]],
        },
        collision: SpatialContentCollision {
            positions,
            triangles,
        },
        navigation: SpatialContentNavigation {
            id: navigation_id,
            config: SpatialContentNavigationConfig {
                schema_version: config_schema_version,
                cell_size: config[0],
                level_quantum,
                maximum_slope_degrees: config[2],
                required_headroom: config[3],
                support_probe_drop: config[4],
            },
            cells,
        },
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| format!("truncated at byte {}", self.at))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("took N bytes"))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// A count of items still to read, each at least one byte, so a corrupt
    /// count cannot size an allocation beyond the file.
    fn count(&mut self) -> Result<usize, String> {
        let count = u64::from_le_bytes(self.array()?);
        usize::try_from(count)
            .ok()
            .filter(|&count| count <= self.bytes.len())
            .ok_or_else(|| format!("count {count} exceeds the artifact"))
    }

    fn text(&mut self) -> Result<String, String> {
        let length = self.u32()? as usize;
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| "an id was not UTF-8".to_owned())
    }
}

/// The binary encoding of `artifact`, for tests and fixtures.
#[cfg(test)]
pub(super) fn encode(artifact: &SpatialContentArtifact) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend(artifact.schema_version.to_le_bytes());
    let config = &artifact.navigation.config;
    out.extend(config.schema_version.to_le_bytes());
    for value in artifact
        .bounds
        .min
        .iter()
        .chain(&artifact.bounds.max)
        .chain(&[
            config.cell_size,
            config.level_quantum,
            config.maximum_slope_degrees,
            config.required_headroom,
            config.support_probe_drop,
        ])
    {
        out.extend(value.to_le_bytes());
    }
    for text in [&artifact.static_mesh_artifact_id, &artifact.navigation.id] {
        out.extend((text.len() as u32).to_le_bytes());
        out.extend(text.as_bytes());
    }
    let cells = &artifact.navigation.cells;
    for count in [
        artifact.collision.positions.len(),
        artifact.collision.triangles.len(),
        cells.len(),
    ] {
        out.extend((count as u64).to_le_bytes());
    }
    for value in artifact.collision.positions.iter().flatten() {
        out.extend(value.to_le_bytes());
    }
    for value in artifact.collision.triangles.iter().flatten() {
        out.extend(value.to_le_bytes());
    }
    for cell in cells {
        out.extend((cell.column as i32).to_le_bytes());
    }
    for cell in cells {
        out.extend((cell.row as i32).to_le_bytes());
    }
    for cell in cells {
        out.extend(cell.support_height.to_le_bytes());
    }
    let mut walkable = vec![0_u8; cells.len().div_ceil(8)];
    for (index, cell) in cells.iter().enumerate() {
        if cell.walkable {
            walkable[index / 8] |= 1 << (index % 8);
        }
    }
    out.extend(walkable);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::parse_spatial_content_artifact;

    fn fixture(name: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/csharp-spatial-artifact");
        std::fs::read(root.join(name)).unwrap()
    }

    fn refusal(bytes: &[u8]) -> String {
        parse_spatial_content_artifact("artifact", bytes)
            .unwrap_err()
            .code()
            .to_owned()
    }

    #[test]
    fn the_binary_fixture_holds_the_same_facts_as_the_json_one() {
        let json = parse_spatial_content_artifact("valid.json", &fixture("valid.json")).unwrap();
        let binary = fixture("valid.rspatial");
        assert_eq!(
            parse_spatial_content_artifact("valid.rspatial", &binary).unwrap(),
            json
        );
        // The fixture was written from the documented layout, independently.
        assert_eq!(encode(&json), binary);
    }

    #[test]
    fn a_damaged_binary_artifact_is_refused_as_schema_and_bad_facts_as_json_ones_are() {
        let binary = fixture("valid.rspatial");
        assert_eq!(
            refusal(&binary[..binary.len() - 1]),
            "CSHARP_SPATIAL_CONTENT_SCHEMA"
        );
        assert_eq!(
            refusal(&[binary.as_slice(), &[0]].concat()),
            "CSHARP_SPATIAL_CONTENT_SCHEMA"
        );
        // A cell count far beyond the file, at its fixed offset after the ids.
        let mut huge = binary.clone();
        let counts = MAGIC.len() + 8 + 11 * 8 + 4 + 20 + 4 + 26;
        huge[counts + 16..counts + 24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(refusal(&huge), "CSHARP_SPATIAL_CONTENT_SCHEMA");

        let mut artifact = parse_spatial_content_artifact("valid", &binary).unwrap();
        artifact.bounds.min[0] = 4.0;
        assert_eq!(refusal(&encode(&artifact)), "CSHARP_SPATIAL_CONTENT_BOUNDS");
        let mut artifact = parse_spatial_content_artifact("valid", &binary).unwrap();
        artifact.navigation.cells[1].column = 0;
        assert_eq!(
            refusal(&encode(&artifact)),
            "CSHARP_SPATIAL_CONTENT_NAVIGATION"
        );
    }

    /// A town-sized artifact (rusty-dagger's Charing: 544,563 cells and about
    /// 100,000 collision vertices), parsed and validated from each encoding.
    /// `cargo test --release -p csharp-engine-services --lib measure_artifact -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing measurement"]
    fn measure_artifact_encodings() {
        let side = 738_i64;
        let cells: Vec<serde_json::Value> = (0..544_563)
            .map(|index: i64| {
                let height = ((index % 37) as f64) * 0.25;
                serde_json::json!({"column": index / side, "row": index % side,
                    "level": (height / 0.25).round() as i64, "supportHeight": height,
                    "walkable": index % 11 != 0})
            })
            .collect();
        let positions: Vec<[f64; 3]> = (0..100_000)
            .map(|index| {
                [
                    (index % 700) as f64,
                    ((index % 37) as f64) * 0.25,
                    (index / 700) as f64,
                ]
            })
            .collect();
        let triangles: Vec<[u32; 3]> = (0..99_000)
            .map(|index| [index, index + 1, index + 2])
            .collect();
        let json = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1, "staticMeshArtifactId": "mesh/charing",
            "bounds": {"min": [0.0, 0.0, 0.0], "max": [740.0, 10.0, 740.0]},
            "collision": {"positions": positions, "triangles": triangles},
            "navigation": {"id": "navigation/charing", "config": {"schemaVersion": 1,
                "cellSize": 1.0, "levelQuantum": 0.25, "maximumSlopeDegrees": 45.0,
                "requiredHeadroom": 1.8, "supportProbeDrop": 0.1}, "cells": cells}
        }))
        .unwrap();
        let time = |bytes: &[u8]| {
            let started = std::time::Instant::now();
            let artifact = parse_spatial_content_artifact("charing", bytes).unwrap();
            (started.elapsed(), artifact)
        };
        let (json_time, artifact) = time(&json);
        let binary = encode(&artifact);
        let (binary_time, decoded) = time(&binary);
        assert_eq!(decoded, artifact);
        let started = std::time::Instant::now();
        let _ = decode(&binary).unwrap();
        println!("binary decode alone: {:?}", started.elapsed());
        println!(
            "JSON: {} bytes, parsed and validated in {json_time:?}",
            json.len()
        );
        println!(
            "binary: {} bytes, parsed and validated in {binary_time:?}",
            binary.len()
        );
    }
}
