use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use game_host::{
    admit_stored_project_with_document, decode_game_snapshot, decode_project_document,
    encode_game_snapshot, materialize_stored_project_voxels, GameRuntime, ProjectSaveMode,
    ProjectStore, StoredVoxelEnvironment, VoxelEdit, VoxelEditTransaction, VoxelSourceRevision,
    GAME_SNAPSHOT_SCHEMA_VERSION,
};

const PROJECT: &str = include_str!("../../../../content/projects/loading-bay.project.json");
const PILLAR: [i64; 3] = [4, 1, 6];

#[test]
fn edited_authority_reopens_from_snapshot_and_explicit_static_project_save() {
    let decoded = decode_project_document(PROJECT).unwrap();
    let (authored, admitted) =
        admit_stored_project_with_document(decoded.project).expect("admit source project");
    let mut runtime = GameRuntime::from_admitted_project(admitted);
    let before_count = runtime.collision_scene().unwrap().solid_voxel_count();
    runtime
        .apply_voxel_edits(VoxelEditTransaction {
            expected_revision: VoxelSourceRevision::INITIAL,
            edits: &[VoxelEdit::Clear { address: PILLAR }],
        })
        .expect("clear generated pillar voxel");

    let edited = runtime.collision_scene().unwrap();
    assert_eq!(edited.source_revision().raw(), 1);
    assert_eq!(edited.solid_voxel_count(), before_count - 1);
    assert!(edited.generated_room().is_none());
    let edited_hash = edited.authority_hash();
    let edited_navigation = edited.navigation_hash();
    let edited_mesh = edited.mesh_chunks().to_vec();
    let edited_voxels = edited.material_voxels().to_vec();

    let snapshot = encode_game_snapshot(&runtime).expect("encode edited runtime");
    let snapshot_json: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(snapshot_json["schemaVersion"], GAME_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(snapshot_json["voxelCollision"]["sourceRevision"], 1);
    assert_eq!(
        snapshot_json["voxelCollision"]["authorityHash"],
        edited_hash
    );
    assert_eq!(
        snapshot_json["voxelCollision"]["generatedRoom"],
        serde_json::Value::Null
    );
    assert_eq!(
        snapshot_json["voxelCollision"]["materialVoxels"]
            .as_array()
            .unwrap()
            .len(),
        before_count - 1
    );
    for forbidden in ["voxelEdit", "changedVoxels", "editHistory", "events"] {
        assert!(!snapshot.contains(forbidden), "snapshot leaked {forbidden}");
    }

    let reopened = decode_game_snapshot(&snapshot).expect("reopen edited runtime snapshot");
    let reopened_scene = reopened.collision_scene().unwrap();
    assert_eq!(reopened_scene.source_revision().raw(), 1);
    assert_eq!(reopened_scene.authority_hash(), edited_hash);
    assert_eq!(reopened_scene.material_voxels(), edited_voxels);
    assert_eq!(reopened_scene.navigation_hash(), edited_navigation);
    assert_eq!(reopened_scene.mesh_chunks(), edited_mesh);
    assert!(!reopened_scene.contains_point([4.5, 1.5, 6.5]));

    let saved = materialize_stored_project_voxels(&authored, edited)
        .expect("materialize edited static authority");
    let directory = TestDirectory::new();
    let project_path = directory.path().join("edited.project.json");
    let store = ProjectStore::default();
    store
        .save(&project_path, &saved, ProjectSaveMode::CreateNew)
        .expect("save edited authored project");
    let bytes = fs::read_to_string(&project_path).unwrap();
    for forbidden in [
        "sourceRevision",
        "authorityHash",
        "voxelEdit",
        "changedVoxels",
        "editHistory",
        "events",
    ] {
        assert!(!bytes.contains(forbidden), "project leaked {forbidden}");
    }

    let loaded = store
        .load(&project_path)
        .expect("load edited authored project");
    assert!(matches!(
        loaded.project.scenes[0].voxel_environment,
        Some(StoredVoxelEnvironment::Material(_))
    ));
    let (_, admitted) = admit_stored_project_with_document(loaded.project).unwrap();
    let project_runtime = GameRuntime::from_admitted_project(admitted);
    let project_scene = project_runtime.collision_scene().unwrap();
    assert_eq!(project_scene.source_revision().raw(), 0);
    assert_eq!(project_scene.authority_hash(), edited_hash);
    assert_eq!(project_scene.material_voxels(), edited_voxels);
    assert_eq!(project_scene.navigation_hash(), edited_navigation);
    assert_eq!(project_scene.mesh_chunks(), edited_mesh);
    assert!(!project_scene.contains_point([4.5, 1.5, 6.5]));
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "rusty-engine-voxel-edit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
