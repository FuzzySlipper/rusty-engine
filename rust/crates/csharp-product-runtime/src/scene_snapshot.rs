//! The committed scene in one file, drawn again without the product.
//!
//! `engine.renderer.snapshot <path>` writes the baseline a fresh renderer is
//! built from (the same one a rebaseline applies: the retained world frame,
//! the presentation frames and the view composition, in order), the Engine
//! state it reaches, and every renderer resource the Engine holds. The file
//! is a Product container (`product_container`): `scene.json` names where it
//! came from, `changes.json` is the baseline as serde of the existing types,
//! and each resource is `resources/<identity>`. Inline mesh streams are moved
//! into packed binary mesh resources on the way (`pack_mesh_resources`), so
//! geometry is binary and the descriptors are ones the renderer already
//! accepts. `rusty-scene-render` draws a snapshot on a fresh renderer.

use std::{borrow::Cow, collections::BTreeMap, path::Path, sync::Arc, time::Instant};

use product_container::{Body, Container, NewEntry};
use product_host::RuntimePublication;
use render_host_contracts::RendererViewComposition;
use render_model::{
    pack_mesh_resources, MeshPayloadSource, RenderFrameDiff, MAX_MESH_RESOURCE_BYTES,
};
use render_presentation::{GhostPlateProjectionOp, PresentationFrameDiff, PresentationOp};
use render_wgpu::{ResourceSource, SceneChange, SceneState};
use serde::{Deserialize, Serialize};

const METADATA: &str = "scene.json";
const CHANGES: &str = "changes.json";
const RESOURCES: &str = "resources";

/// Where a snapshot came from and the state its baseline reaches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotMetadata {
    pub product: Option<SceneSnapshotProduct>,
    pub host: SceneSnapshotHost,
    /// The adapter the live renderer drew with, when there was one.
    pub adapter: Option<String>,
    pub written_at_unix_ms: u64,
    pub state: SceneSnapshotState,
    pub options: SceneSnapshotOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSnapshotProduct {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotHost {
    pub version: String,
    pub abi_fingerprint: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotState {
    pub elapsed_seconds: f64,
    pub world_revision: u64,
    pub step: u64,
    pub held: bool,
}

impl From<SceneSnapshotState> for SceneState {
    fn from(state: SceneSnapshotState) -> Self {
        Self {
            elapsed_seconds: state.elapsed_seconds,
            world_revision: state.world_revision,
            step: state.step,
            held: state.held,
        }
    }
}

impl From<SceneState> for SceneSnapshotState {
    fn from(state: SceneState) -> Self {
        Self {
            elapsed_seconds: state.elapsed_seconds,
            world_revision: state.world_revision,
            step: state.step,
            held: state.held,
        }
    }
}

/// The renderer options the product's manifest selected.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotOptions {
    pub default_world_lights: bool,
    pub default_viewmodel_lights: bool,
    pub shadows: bool,
}

impl From<SceneSnapshotOptions> for render_wgpu::RendererOptions {
    fn from(options: SceneSnapshotOptions) -> Self {
        Self {
            default_world_lights: options.default_world_lights,
            default_viewmodel_lights: options.default_viewmodel_lights,
            shadows: options.shadows,
        }
    }
}

impl From<render_wgpu::RendererOptions> for SceneSnapshotOptions {
    fn from(options: render_wgpu::RendererOptions) -> Self {
        Self {
            default_world_lights: options.default_world_lights,
            default_viewmodel_lights: options.default_viewmodel_lights,
            shadows: options.shadows,
        }
    }
}

/// One baseline change, in the order a rebaseline applies it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SceneSnapshotChange {
    Frame(RenderFrameDiff),
    Presentation(PresentationFrameDiff),
    ViewComposition(RendererViewComposition),
}

impl SceneSnapshotChange {
    pub fn scene_change(&self) -> SceneChange<'_> {
        match self {
            Self::Frame(frame) => SceneChange::Frame(frame),
            Self::Presentation(frame) => SceneChange::Presentation(frame),
            Self::ViewComposition(composition) => SceneChange::ViewComposition(composition),
        }
    }
}

/// What a write produced, as the debug command answers it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotReport {
    pub path: String,
    pub bytes: u64,
    pub entries: usize,
    pub resources: usize,
    pub resource_bytes: u64,
    pub packed_meshes: usize,
    pub packed_mesh_bytes: u64,
    pub changes_json_bytes: u64,
    pub write_ms: f64,
}

/// Writes the baseline `publications` (the renderer's changes among them) and
/// `resources` (identity, bytes) to `path`, replacing any file there.
pub fn write_scene_snapshot<'a>(
    path: &Path,
    metadata: &SceneSnapshotMetadata,
    publications: &[RuntimePublication],
    resources: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<SceneSnapshotReport, String> {
    let started = Instant::now();
    let mut changes: Vec<SceneSnapshotChange> = publications
        .iter()
        .filter_map(|publication| match publication {
            RuntimePublication::Frame(frame) => Some(SceneSnapshotChange::Frame(frame.clone())),
            RuntimePublication::Presentation(frame) => {
                Some(SceneSnapshotChange::Presentation(frame.clone()))
            }
            RuntimePublication::ViewComposition(composition) => {
                Some(SceneSnapshotChange::ViewComposition(composition.clone()))
            }
            _ => None,
        })
        .collect();
    let mut packed = BTreeMap::new();
    for change in &mut changes {
        match change {
            SceneSnapshotChange::Frame(frame) => pack_meshes(frame, &mut packed)?,
            SceneSnapshotChange::Presentation(frame) => pack_ghost_captures(frame, &mut packed)?,
            SceneSnapshotChange::ViewComposition(_) => {}
        }
    }
    let metadata_json = serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?;
    let changes_json = serde_json::to_vec(&changes).map_err(|error| error.to_string())?;
    let mut entries = vec![
        NewEntry {
            path: METADATA.to_owned(),
            bundle: None,
            body: Body::Bytes(&metadata_json),
        },
        NewEntry {
            path: CHANGES.to_owned(),
            bundle: None,
            body: Body::Bytes(&changes_json),
        },
    ];
    let mut all: BTreeMap<&str, &[u8]> = resources.into_iter().collect();
    all.extend(
        packed
            .iter()
            .map(|(id, bytes)| (id.as_str(), bytes.as_slice())),
    );
    let resource_bytes = all.values().map(|bytes| bytes.len() as u64).sum();
    for (identity, bytes) in &all {
        entries.push(NewEntry {
            path: format!("{RESOURCES}/{identity}"),
            bundle: None,
            body: Body::Bytes(bytes),
        });
    }
    // Raw, so a reader borrows every resource from the map.
    let written = product_container::write(path, entries, Vec::new(), false)
        .map_err(|error| error.to_string())?;
    Ok(SceneSnapshotReport {
        path: path.display().to_string(),
        bytes: written.bytes,
        entries: written.entries,
        resources: all.len(),
        resource_bytes,
        packed_meshes: packed.len(),
        packed_mesh_bytes: packed.values().map(|bytes| bytes.len() as u64).sum(),
        changes_json_bytes: changes_json.len() as u64,
        write_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

/// Moves every inline mesh payload in `frame` into a packed mesh resource.
fn pack_meshes(
    frame: &mut RenderFrameDiff,
    packed: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    for payload in frame.mesh_payloads_mut() {
        if !matches!(payload.source, MeshPayloadSource::Inline { .. }) {
            continue;
        }
        let mut set = pack_mesh_resources(std::slice::from_ref(payload), MAX_MESH_RESOURCE_BYTES)
            .map_err(|error| format!("mesh payload could not be packed: {error:?}"))?;
        *payload = set.payloads.remove(0);
        for resource in set.resources {
            packed.insert(resource.resource, resource.bytes);
        }
    }
    Ok(())
}

/// Ghost plates carry the world capture their bank was drawn from.
fn pack_ghost_captures(
    frame: &mut PresentationFrameDiff,
    packed: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    for op in &mut frame.ops {
        let PresentationOp::GhostPlate { op, .. } = op else {
            continue;
        };
        let scene = match op {
            GhostPlateProjectionOp::Create { descriptor, .. } => &mut descriptor.captured_scene,
            GhostPlateProjectionOp::Recapture { captured_scene, .. } => captured_scene,
            GhostPlateProjectionOp::Update { .. } | GhostPlateProjectionOp::Destroy { .. } => {
                continue
            }
        };
        if let Some(scene) = scene {
            pack_meshes(Arc::make_mut(scene), packed)?;
        }
    }
    Ok(())
}

/// An open snapshot: its metadata and baseline, with resources borrowed from
/// the mapped file.
pub struct SceneSnapshot {
    container: Container,
    pub metadata: SceneSnapshotMetadata,
    pub changes: Vec<SceneSnapshotChange>,
}

impl SceneSnapshot {
    pub fn open(path: &Path) -> Result<Self, String> {
        let container = Container::open(path).map_err(|error| error.to_string())?;
        let json = |name: &str| {
            container
                .get(name)
                .map_err(|error| format!("{}: {error}", path.display()))
        };
        let metadata = serde_json::from_slice(&json(METADATA)?)
            .map_err(|error| format!("{METADATA}: {error}"))?;
        let changes = serde_json::from_slice(&json(CHANGES)?)
            .map_err(|error| format!("{CHANGES}: {error}"))?;
        Ok(Self {
            container,
            metadata,
            changes,
        })
    }

    /// The baseline's changes, as a rebaseline applies them.
    pub fn scene_changes(&self) -> impl Iterator<Item = SceneChange<'_>> {
        self.changes.iter().map(SceneSnapshotChange::scene_change)
    }

    pub fn resource_count(&self) -> usize {
        self.container
            .entries()
            .iter()
            .filter(|entry| entry.path.starts_with("resources/"))
            .count()
    }
}

impl ResourceSource for SceneSnapshot {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.container.get(&format!("{RESOURCES}/{identity}")).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{
        MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
        MeshBufferLayout, MeshGroupDescriptor, MeshIndexWidth, MeshPayloadDescriptor,
        MeshProvenance, RenderDiff, StaticMeshAsset,
    };

    fn inline_mesh() -> MeshPayloadDescriptor {
        let attribute = |name| MeshAttribute {
            name,
            components: 3,
            kind: MeshAttributeKind::F32,
        };
        MeshPayloadDescriptor {
            texture_space: None,
            layout: MeshBufferLayout {
                vertex_count: 3,
                index_count: 3,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    attribute(MeshAttributeName::Position),
                    attribute(MeshAttributeName::Normal),
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
        }
    }

    #[test]
    fn inline_meshes_become_binary_resources_and_read_back() {
        let directory = tempfile::tempdir().unwrap();
        let frame = RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::DefineStaticMesh {
                asset: StaticMeshAsset {
                    asset: "mesh/triangle".into(),
                    payload: inline_mesh(),
                    material_slots: Vec::new(),
                    collision: Default::default(),
                },
            }],
        };
        let metadata = SceneSnapshotMetadata {
            product: Some(SceneSnapshotProduct {
                id: "fixture.product".into(),
                title: "Fixture".into(),
            }),
            host: SceneSnapshotHost {
                version: "0.1.0".into(),
                abi_fingerprint: "ab".into(),
            },
            adapter: None,
            written_at_unix_ms: 1,
            state: SceneSnapshotState {
                elapsed_seconds: 2.5,
                world_revision: 7,
                step: 150,
                held: true,
            },
            options: render_wgpu::RendererOptions::default().into(),
        };
        let path = directory.path().join("scene.rscene");
        let report = write_scene_snapshot(
            &path,
            &metadata,
            &[RuntimePublication::Frame(frame)],
            [("texture-resource/abc", b"png".as_slice())],
        )
        .unwrap();
        assert_eq!((report.resources, report.packed_meshes), (2, 1));

        let snapshot = SceneSnapshot::open(&path).unwrap();
        assert_eq!(snapshot.metadata.state.step, 150);
        assert_eq!(snapshot.resource_count(), 2);
        assert_eq!(
            snapshot.bytes("texture-resource/abc").unwrap().as_ref(),
            b"png"
        );
        let [SceneSnapshotChange::Frame(frame)] = snapshot.changes.as_slice() else {
            panic!("one frame change");
        };
        let RenderDiff::DefineStaticMesh { asset } = &frame.ops[0] else {
            panic!("static mesh");
        };
        let MeshPayloadSource::Resource { resource, .. } = &asset.payload.source else {
            panic!("the mesh is a resource");
        };
        let bytes = snapshot.bytes(resource).expect("the packed mesh is stored");
        let decoded = render_model::decode_mesh_resource_payload(&asset.payload, &bytes).unwrap();
        assert_eq!(decoded, inline_mesh());
    }
}
