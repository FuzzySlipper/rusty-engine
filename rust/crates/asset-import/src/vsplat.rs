//! `VIGGCHAR` v1 `.vsplat` skinned Gaussian splat characters (the
//! Gaussian splat spike, docs/gaussian-splats.md).
//!
//! The layout is `@viggle/splat-engine` 0.3.2's MIT `decode.ts`: a JSON
//! block table, then component-major blocks, each quantised against the
//! bounds its own entry carries.

use serde_json::Value;

/// Why a `.vsplat` did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VsplatError(pub String);

impl std::fmt::Display for VsplatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VsplatError {}

/// One decoded Gaussian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VsplatSplat {
    pub position: [f32; 3],
    /// Unit quaternion, xyzw.
    pub rotation: [f32; 4],
    /// Per-axis standard deviation (the file stores its log).
    pub sigma: [f32; 3],
    /// sRGB colour (the file's SH DC term, already through `0.5 + C0 dc`).
    pub color: [u8; 3],
    /// Opacity, 0 to 1 (the file stores its logit).
    pub opacity: f32,
    /// Four global rig bone indices, through the file's palette, and their
    /// weights out of 255; the fourth weight is what the stored three leave.
    pub bones: [u16; 4],
    pub weights: [u8; 4],
}

/// A decoded character.
#[derive(Debug, Clone, PartialEq)]
pub struct VsplatCharacter {
    pub splats: Vec<VsplatSplat>,
    /// The `joints` block, one row per rig bone. Viggle's 441-bone
    /// characters store each bone's translation in its parent's frame (the
    /// SDK's rig tables give the rotations); rigs bound by
    /// `make_skinned_vsplat.py` store world rest positions.
    pub joints: Vec<[f32; 3]>,
}

/// Decode a `VIGGCHAR` v1 file.
pub fn decode_vsplat(bytes: &[u8]) -> Result<VsplatCharacter, VsplatError> {
    let fail = |message: &str| VsplatError(message.to_owned());
    if bytes.len() < 20 || &bytes[..8] != b"VIGGCHAR" {
        return Err(fail("not a VIGGCHAR file"));
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    if word(8) != 1 {
        return Err(fail("only VIGGCHAR version 1 decodes"));
    }
    let start = 20 + word(16);
    let header: Value = serde_json::from_slice(
        bytes
            .get(20..start)
            .ok_or_else(|| fail("header runs past the file"))?,
    )
    .map_err(|error| VsplatError(format!("header: {error}")))?;
    let count = header["numSplats"]
        .as_u64()
        .ok_or_else(|| fail("no numSplats"))? as usize;
    let blocks = &header["blocks"];
    let block = |name: &str| -> Result<(&Value, usize), VsplatError> {
        let entry = &blocks[name];
        let offset = entry["offset"]
            .as_u64()
            .ok_or_else(|| VsplatError(format!("no block {name}")))? as usize;
        let length = entry["length"].as_u64().unwrap_or(0) as usize;
        if start + offset + length > bytes.len() {
            return Err(VsplatError(format!("block {name} runs past the file")));
        }
        Ok((entry, start + offset))
    };
    let bound = |entry: &Value, key: &str, c: usize| entry[key][c].as_f64().unwrap_or(0.0) as f32;
    let u8_at = |base: usize, c: usize, i: usize| bytes[base + c * count + i];
    let u16_at = |base: usize, c: usize, i: usize| {
        let at = base + 2 * (c * count + i);
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    };
    // `min + q (max - min) / (levels - 1)`, as decode.ts dequantises.
    let dequantised = |entry: &Value, c: usize, q: u16, levels: f32| {
        let lo = bound(entry, "min", c);
        lo + f32::from(q) * (bound(entry, "max", c) - lo) / (levels - 1.0)
    };

    let (position, position_base) = block("position")?;
    let (scale, scale_base) = block("scale")?;
    let (_, rotation_base) = block("rotation")?;
    let (_, color_base) = block("color")?;
    let (opacity, opacity_base) = block("opacity")?;
    let skin = block("boneIndex").ok().zip(block("boneWeight").ok());
    let palette: Vec<u16> = skin
        .as_ref()
        .and_then(|((entry, _), _)| entry["palette"].as_array())
        .map(|palette| {
            palette
                .iter()
                .map(|bone| bone.as_u64().unwrap_or(0) as u16)
                .collect()
        })
        .unwrap_or_default();
    let wide_bones = skin
        .as_ref()
        .is_some_and(|((entry, _), _)| entry["dtype"] == "u16");
    if skin
        .as_ref()
        .is_some_and(|((entry, _), _)| entry["components"].as_u64() != Some(4))
    {
        return Err(fail("only four bones a splat decode"));
    }

    let splats = (0..count)
        .map(|i| {
            let [x, y, z]: [f32; 3] =
                std::array::from_fn(|c| f32::from(u8_at(rotation_base, c, i)) / 255.0 * 2.0 - 1.0);
            // The writer made w non-negative, so it is what xyz leave.
            let w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
            let length = (x * x + y * y + z * z + w * w).sqrt().max(f32::EPSILON);
            let logit = dequantised(opacity, 0, u16::from(u8_at(opacity_base, 0, i)), 256.0);
            let (bones, weights) = match &skin {
                Some(((_, index_base), (_, weight_base))) => {
                    let bone = |c| {
                        let stored = if wide_bones {
                            usize::from(u16_at(*index_base, c, i))
                        } else {
                            usize::from(u8_at(*index_base, c, i))
                        };
                        palette.get(stored).copied().unwrap_or(0)
                    };
                    let stored: [u8; 3] = std::array::from_fn(|c| u8_at(*weight_base, c, i));
                    let last = 255u32.saturating_sub(stored.iter().map(|&w| u32::from(w)).sum());
                    (
                        [bone(0), bone(1), bone(2), bone(3)],
                        [stored[0], stored[1], stored[2], last as u8],
                    )
                }
                // Unskinned: bone 0 whole.
                None => ([0; 4], [255, 0, 0, 0]),
            };
            VsplatSplat {
                position: std::array::from_fn(|c| {
                    dequantised(position, c, u16_at(position_base, c, i), 65536.0)
                }),
                rotation: [x / length, y / length, z / length, w / length],
                sigma: std::array::from_fn(|c| {
                    dequantised(scale, c, u16::from(u8_at(scale_base, c, i)), 256.0).exp()
                }),
                color: std::array::from_fn(|c| u8_at(color_base, c, i)),
                opacity: 1.0 / (1.0 + (-logit).exp()),
                bones,
                weights,
            }
        })
        .collect();

    let joints = match block("joints") {
        Ok((entry, base)) => {
            let rows = entry["count"].as_u64().unwrap_or(0) as usize;
            let f = |c: usize, j: usize| {
                let at = base + 4 * (c * rows + j);
                f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
            };
            (0..rows).map(|j| [f(0, j), f(1, j), f(2, j)]).collect()
        }
        Err(_) => vec![[0.0; 3]],
    };
    Ok(VsplatCharacter { splats, joints })
}
