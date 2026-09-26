//! Portable, file-relative asset semantics. Parsing deliberately permits drafts;
//! resolution validates the selected asset at its actual consumption boundary.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableAssets {
    pub schema_version: u32,
    #[serde(default)]
    pub assets: Vec<PortableAsset>,
    /// Optional authoring information. Never required to load an asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableAsset {
    pub id: String,
    #[serde(flatten)]
    pub definition: PortableDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortableDefinition {
    Attachment {
        target: String,
        child: String,
        joint: String,
        convention: String,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    },
    Texture {
        #[serde(default)]
        path: String,
    },
    Model {
        #[serde(default)]
        path: String,
        /// Local material IDs associated with named glTF material slots.
        #[serde(default)]
        materials: BTreeMap<String, String>,
        /// Local clip aliases refer to authored glTF clip names, not copied tracks.
        #[serde(default)]
        clips: BTreeMap<String, String>,
    },
    Material {
        #[serde(default)]
        textures: BTreeMap<String, String>,
    },
    Sprite {
        #[serde(default)]
        frames: Vec<PortableSpriteFrame>,
        #[serde(default)]
        animations: BTreeMap<String, PortableSpriteAnimation>,
        #[serde(default)]
        directions: Option<PortableDirections>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableSpriteFrame {
    pub id: String,
    /// Local texture asset ID; separate-frame sprites simply use different IDs.
    pub texture: String,
    #[serde(flatten)]
    pub region: PortableSpriteRegion,
    /// Original canvas dimensions, before trimming, in pixels.
    pub canvas: [u32; 2],
    /// Top-left trim offset within the original canvas, in pixels.
    #[serde(default)]
    pub trim: [u32; 2],
    /// Pivot measured from the original canvas top-left, in pixels.
    pub pivot: [f32; 2],
    #[serde(default)]
    pub anchors: BTreeMap<String, [f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "region", rename_all = "camelCase")]
pub enum PortableSpriteRegion {
    /// Entire separate image. Its dimensions equal the trimmed frame extent.
    Image { extent: [u32; 2] },
    /// Pixel rectangle in a texture, measured from its top-left.
    Rect { origin: [u32; 2], extent: [u32; 2] },
    /// Zero-based cell in a regular grid, measured from its top-left.
    Cell { cell: [u32; 2], extent: [u32; 2] },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableSpriteAnimation {
    pub frames: Vec<String>,
    pub timing: PortableSpriteTiming,
    pub looping: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortableSpriteTiming {
    Fps { fps: f64 },
    Durations { seconds: Vec<f64> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableDirections {
    /// Explicit convention: right-handed +Y up, yaw zero +Z, positive toward +X.
    pub convention: String,
    pub sectors: Vec<PortableDirection>,
    /// Each action maps sector IDs to animation IDs. Omitted directions stay missing.
    pub actions: BTreeMap<String, BTreeMap<String, String>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableDirection {
    pub id: String,
    pub yaw_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableAssetError(pub String);
impl std::fmt::Display for PortableAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PortableAssetError {}
fn error(message: impl Into<String>) -> PortableAssetError {
    PortableAssetError(message.into())
}

impl PortableAssets {
    pub fn decode(bytes: &[u8]) -> Result<Self, PortableAssetError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| error(e.to_string()))?;
        let version = value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64);
        if version != Some(1) {
            return Err(error(format!(
                "unsupported portable asset schemaVersion {version:?}; this Engine supports 1"
            )));
        }
        serde_json::from_value(value).map_err(|e| error(e.to_string()))
    }

    pub fn asset(&self, id: &str) -> Result<&PortableAsset, PortableAssetError> {
        let mut matches = self.assets.iter().filter(|a| a.id == id);
        let asset = matches
            .next()
            .ok_or_else(|| error(format!("missing asset '{id}'")))?;
        if id.is_empty() || matches.next().is_some() {
            return Err(error(format!("ambiguous or empty asset ID '{id}'")));
        }
        Ok(asset)
    }

    /// Resolve only the requested dependency closure. Unrelated unfinished assets
    /// do not prevent a valid selection from being loaded.
    pub fn resolve(&self, id: &str) -> Result<Vec<&PortableAsset>, PortableAssetError> {
        fn visit<'a>(
            doc: &'a PortableAssets,
            id: &str,
            visiting: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
            out: &mut Vec<&'a PortableAsset>,
        ) -> Result<(), PortableAssetError> {
            if done.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id.to_owned()) {
                return Err(error(format!("asset dependency cycle at '{id}'")));
            }
            let asset = doc.asset(id)?;
            let mut dependencies = Vec::new();
            match &asset.definition {
                PortableDefinition::Attachment {
                    target,
                    child,
                    joint,
                    convention,
                    translation,
                    rotation,
                    scale,
                } => {
                    if convention != "gltf-right-handed-y-up-meters"
                        || joint.trim().is_empty()
                        || translation
                            .iter()
                            .chain(rotation)
                            .chain(scale)
                            .any(|v| !v.is_finite())
                        || scale.contains(&0.0)
                        || rotation.iter().map(|v| v * v).sum::<f32>() <= f32::EPSILON
                    {
                        return Err(error(format!("attachment '{id}' requires a joint, finite nondegenerate TRS and gltf-right-handed-y-up-meters convention")));
                    }
                    if target == child {
                        return Err(error(format!(
                            "attachment '{id}' requires distinct target and child assets"
                        )));
                    }
                    dependencies.push((target, "model"));
                    dependencies.push((child, "model"));
                }
                PortableDefinition::Texture { path } => {
                    portable_relative_path(path)?;
                }
                PortableDefinition::Model {
                    path,
                    materials,
                    clips,
                } => {
                    portable_relative_path(path)?;
                    if clips
                        .iter()
                        .any(|(id, name)| id.is_empty() || name.is_empty())
                    {
                        return Err(error(format!(
                            "model '{id}' has an empty clip alias or source name"
                        )));
                    }
                    for dependency in materials.values() {
                        dependencies.push((dependency, "material"));
                    }
                }
                PortableDefinition::Material { textures } => {
                    for dependency in textures.values() {
                        dependencies.push((dependency, "texture"));
                    }
                }
                PortableDefinition::Sprite {
                    frames,
                    animations,
                    directions,
                } => {
                    validate_sprite(id, frames, animations, directions.as_ref())?;
                    for frame in frames {
                        dependencies.push((&frame.texture, "texture"));
                    }
                }
            }
            for (dependency, expected) in dependencies {
                let target = doc.asset(dependency)?;
                let correct = matches!(
                    (&target.definition, expected),
                    (PortableDefinition::Texture { .. }, "texture")
                        | (PortableDefinition::Material { .. }, "material")
                        | (PortableDefinition::Model { .. }, "model")
                );
                if !correct {
                    return Err(error(format!(
                        "asset '{id}' reference '{dependency}' requires {expected}"
                    )));
                }
                visit(doc, dependency, visiting, done, out)?;
            }
            visiting.remove(id);
            done.insert(id.to_owned());
            out.push(asset);
            Ok(())
        }
        let mut result = Vec::new();
        visit(
            self,
            id,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut result,
        )?;
        Ok(result)
    }
}

/// Paths are relative to the descriptor's directory, contained in its admitted
/// content context. This is a logical reference check, not filesystem access.
pub fn portable_relative_path(path: &str) -> Result<(), PortableAssetError> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(error(format!(
            "invalid descriptor-relative path '{path}'; use contained forward-slash relative paths"
        )));
    }
    Ok(())
}

impl PortableSpriteFrame {
    pub fn rectangle(&self) -> Result<([u32; 2], [u32; 2]), PortableAssetError> {
        let (origin, extent) = match self.region {
            PortableSpriteRegion::Image { extent } => ([0, 0], extent),
            PortableSpriteRegion::Rect { origin, extent } => (origin, extent),
            PortableSpriteRegion::Cell { cell, extent } => (
                [
                    cell[0]
                        .checked_mul(extent[0])
                        .ok_or_else(|| error("cell x overflow"))?,
                    cell[1]
                        .checked_mul(extent[1])
                        .ok_or_else(|| error("cell y overflow"))?,
                ],
                extent,
            ),
        };
        for axis in 0..2 {
            if extent[axis] == 0
                || self.canvas[axis] == 0
                || origin[axis].checked_add(extent[axis]).is_none()
                || self.trim[axis]
                    .checked_add(extent[axis])
                    .is_none_or(|end| end > self.canvas[axis])
            {
                return Err(error(format!(
                    "frame '{}' has invalid extent, canvas or trim",
                    self.id
                )));
            }
        }
        Ok((origin, extent))
    }
}
impl PortableSpriteAnimation {
    pub fn durations(&self) -> Result<Vec<f64>, PortableAssetError> {
        let values = match &self.timing {
            PortableSpriteTiming::Fps { fps } if fps.is_finite() && *fps > 0.0 => {
                vec![1.0 / fps; self.frames.len()]
            }
            PortableSpriteTiming::Durations { seconds } if seconds.len() == self.frames.len() => {
                seconds.clone()
            }
            _ => {
                return Err(error(
                    "animation requires positive finite FPS or one duration per frame",
                ))
            }
        };
        if values.is_empty() || values.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(error("animation durations must be positive and finite"));
        }
        Ok(values)
    }
}
fn validate_sprite(
    id: &str,
    frames: &[PortableSpriteFrame],
    animations: &BTreeMap<String, PortableSpriteAnimation>,
    directions: Option<&PortableDirections>,
) -> Result<(), PortableAssetError> {
    let mut ids = BTreeSet::new();
    if frames.is_empty() {
        return Err(error(format!("sprite '{id}' has no frames")));
    }
    for frame in frames {
        if frame.id.is_empty() || !ids.insert(frame.id.as_str()) {
            return Err(error(format!(
                "sprite '{id}' has duplicate or empty frame '{}'",
                frame.id
            )));
        }
        frame.rectangle()?;
        if frame
            .pivot
            .iter()
            .chain(frame.anchors.values().flatten())
            .any(|v| !v.is_finite())
        {
            return Err(error(format!(
                "frame '{}' has nonfinite pivot/anchor",
                frame.id
            )));
        }
    }
    for (name, animation) in animations {
        if name.is_empty() {
            return Err(error("empty animation name"));
        }
        animation.durations()?;
        for frame in &animation.frames {
            if !ids.contains(frame.as_str()) {
                return Err(error(format!(
                    "animation '{name}' references missing frame '{frame}'"
                )));
            }
        }
    }
    if let Some(directions) = directions {
        if directions.convention != "right-handed-y-up-yaw-zero-positive-z-toward-positive-x" {
            return Err(error("unsupported direction convention; expected right-handed-y-up-yaw-zero-positive-z-toward-positive-x"));
        }
        let mut sectors = BTreeSet::new();
        let mut yaws = BTreeSet::new();
        for sector in &directions.sectors {
            if sector.id.is_empty()
                || !sectors.insert(sector.id.as_str())
                || !sector.yaw_degrees.is_finite()
                || !(0.0..360.0).contains(&sector.yaw_degrees)
                || !yaws.insert(sector.yaw_degrees.to_bits())
            {
                return Err(error(
                    "direction IDs and yaw angles must be unique; yaw must be in [0,360)",
                ));
            }
        }
        for (action, mapping) in &directions.actions {
            for (sector, animation) in mapping {
                if !sectors.contains(sector.as_str()) || !animations.contains_key(animation) {
                    return Err(error(format!("action '{action}' has missing direction '{sector}' or animation '{animation}'")));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SPRITE: &str = r#"{
      "schemaVersion":1,
      "assets":[
        {"id":"sheet","kind":"texture","path":"images/walk.png"},
        {"id":"hero","kind":"sprite","frames":[
          {"id":"a","texture":"sheet","region":"cell","cell":[1,0],"extent":[16,24],"canvas":[20,30],"trim":[2,3],"pivot":[10,30]},
          {"id":"b","texture":"sheet","region":"rect","origin":[32,0],"extent":[16,24],"canvas":[20,30],"pivot":[10,30]}
        ],"animations":{"walk":{"frames":["b","a","b"],"timing":{"kind":"durations","seconds":[0.1,0.2,0.3]},"looping":true}},
        "directions":{"convention":"right-handed-y-up-yaw-zero-positive-z-toward-positive-x","sectors":[{"id":"front","yawDegrees":0},{"id":"right","yawDegrees":90}],"actions":{"walk":{"front":"walk"}}}},
        {"id":"unfinished","kind":"texture","path":""}
      ]
    }"#;
    #[test]
    fn resolves_only_selected_closure_and_preserves_order_timing_and_missing_directions() {
        let document = PortableAssets::decode(SPRITE.as_bytes()).unwrap();
        let selected = document.resolve("hero").unwrap();
        assert_eq!(
            selected.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            ["sheet", "hero"]
        );
        let PortableDefinition::Sprite {
            frames,
            animations,
            directions,
        } = &selected[1].definition
        else {
            panic!()
        };
        assert_eq!(frames[0].rectangle().unwrap(), ([16, 0], [16, 24]));
        assert_eq!(animations["walk"].frames, ["b", "a", "b"]);
        assert_eq!(animations["walk"].durations().unwrap(), [0.1, 0.2, 0.3]);
        assert!(!directions.as_ref().unwrap().actions["walk"].contains_key("right"));
        assert!(document
            .resolve("unfinished")
            .unwrap_err()
            .to_string()
            .contains("path"));
    }
    #[test]
    fn rejects_version_reference_role_and_path_errors_at_resolution() {
        assert!(PortableAssets::decode(br#"{"schemaVersion":42}"#)
            .unwrap_err()
            .to_string()
            .contains("supports 1"));
        let missing = SPRITE.replace("\"texture\":\"sheet\"", "\"texture\":\"missing\"");
        assert!(PortableAssets::decode(missing.as_bytes())
            .unwrap()
            .resolve("hero")
            .unwrap_err()
            .to_string()
            .contains("missing asset 'missing'"));
        for path in ["../escape", "/absolute", "C:/file", "a\\b", "a//b", "a/./b"] {
            assert!(portable_relative_path(path).is_err(), "{path}");
        }
        let invalid = SPRITE.replace("images/walk.png", "../walk.png");
        assert!(PortableAssets::decode(invalid.as_bytes())
            .unwrap()
            .resolve("hero")
            .is_err());
    }
    #[test]
    fn model_relationships_do_not_duplicate_gltf_tracks() {
        let doc = PortableAssets::decode(br#"{"schemaVersion":1,"assets":[{"id":"body","kind":"model","path":"body.glb","clips":{"idle":"Armature|Idle"},"materials":{"Body":"paint"}},{"id":"paint","kind":"material","textures":{"baseColor":"color"}},{"id":"color","kind":"texture","path":"color.png"}]}"#).unwrap();
        assert_eq!(doc.resolve("body").unwrap().len(), 3);
        assert!(PortableSpriteAnimation {
            frames: vec!["a".into()],
            timing: PortableSpriteTiming::Fps { fps: 0.0 },
            looping: false
        }
        .durations()
        .is_err());
    }
}
