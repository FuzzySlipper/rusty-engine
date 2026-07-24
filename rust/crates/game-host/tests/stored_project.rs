use game_host::{decode_stored_project, diagnostic_code};

const PROJECT: &str = include_str!("../../../../content/projects/loading-bay.project.json");

#[test]
fn hand_authored_project_is_static_typed_multi_family_content() {
    let project = decode_stored_project(PROJECT).expect("stored project");
    assert_eq!(project.project_id, "loading-bay");
    assert_eq!(project.entry_scene, "scene/loading-bay");
    assert_eq!(project.assets.len(), 5);
    assert_eq!(project.scenes.len(), 1);

    let entities = &project.scenes[0].entities;
    assert!(entities
        .iter()
        .any(|entity| entity.player_controller.is_some()));
    assert!(entities.iter().any(|entity| entity.weapon.is_some()));
    assert!(entities.iter().any(|entity| entity.navigation.is_some()));
    assert!(entities.iter().any(|entity| entity.health.is_some()));
    assert!(entities.iter().any(|entity| entity.encounter.is_some()));
    assert!(entities.iter().any(|entity| entity.door.is_some()));
    assert!(entities.iter().any(|entity| entity.switch.is_some()));
}

#[test]
fn invalid_asset_identity_reports_the_exact_catalog_path() {
    let invalid = mutate(|project| project["assets"][0]["id"] = "primitive/panel".into());
    let error = decode_stored_project(&invalid).unwrap_err();

    assert_eq!(error.diagnostic().code, diagnostic_code::INVALID_ASSET_ID);
    assert_eq!(error.diagnostic().path, "assets[0].id");
    assert!(error.diagnostic().message.contains("unknown kind"));
}

#[test]
fn duplicate_asset_identity_reports_both_declarations() {
    let invalid = mutate(|project| {
        project["assets"][1]["id"] = project["assets"][0]["id"].clone();
    });
    let error = decode_stored_project(&invalid).unwrap_err();

    assert_eq!(error.diagnostic().code, diagnostic_code::DUPLICATE_ASSET);
    assert_eq!(error.diagnostic().path, "assets[1].id");
    assert!(error.diagnostic().message.contains("assets[0].id"));
}

#[test]
fn entry_scene_requires_a_scene_identity_and_declared_document() {
    let wrong_kind = mutate(|project| project["entryScene"] = "mesh/player-marker".into());
    let wrong_kind = decode_stored_project(&wrong_kind).unwrap_err();
    assert_eq!(
        wrong_kind.diagnostic().code,
        diagnostic_code::WRONG_ASSET_KIND
    );
    assert_eq!(wrong_kind.diagnostic().path, "entryScene");

    let missing = mutate(|project| project["entryScene"] = "scene/not-declared".into());
    let missing = decode_stored_project(&missing).unwrap_err();
    assert_eq!(
        missing.diagnostic().code,
        diagnostic_code::MISSING_ENTRY_SCENE
    );
    assert_eq!(missing.diagnostic().path, "entryScene");
}

#[test]
fn structural_decode_error_retains_the_scene_source_path() {
    let invalid = mutate(|project| project["scenes"][0]["unexpected"] = true.into());
    let error = decode_stored_project(&invalid).unwrap_err();

    assert_eq!(error.diagnostic().code, diagnostic_code::DECODE);
    assert!(error.diagnostic().path.starts_with("scenes[0]"));
}

fn mutate(change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut project: serde_json::Value = serde_json::from_str(PROJECT).unwrap();
    change(&mut project);
    serde_json::to_string(&project).unwrap()
}
