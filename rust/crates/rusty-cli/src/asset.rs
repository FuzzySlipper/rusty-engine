//! `rusty asset check`: the runtime's admission of a standalone GLB, for
//! producers that write GLBs outside a product. It reads the file and never
//! writes.

use std::path::Path;

use asset_import::{
    admit_glb_source, import_animated_glb_asset, GlbSourceClosure, ImportContext, ImportDiagnostic,
    SourceUri,
};
use serde_json::{json, Value};

/// Admits `bytes` as a running product admits a GLB with no companion files:
/// the closure packer, then the animated-mesh import. The diagnostics are the
/// first step's refusal, or the import's diagnostics.
pub(crate) fn check_glb(file_name: &str, bytes: Vec<u8>) -> (bool, Vec<ImportDiagnostic>) {
    let packed = match admit_glb_source(&GlbSourceClosure {
        root_glb: bytes,
        resources: Vec::new(),
    }) {
        Ok(packed) => packed,
        Err(diagnostic) => return (false, vec![diagnostic]),
    };
    let outcome = import_animated_glb_asset(
        &SourceUri::RelativePath(file_name.to_owned()),
        &packed.glb_bytes,
        &ImportContext::default(),
    );
    (outcome.assets.is_some(), outcome.diagnostics)
}

pub(crate) fn report(path: &Path, admitted: bool, diagnostics: &[ImportDiagnostic]) -> Value {
    json!({
        "path": path.display().to_string(),
        "admitted": admitted,
        "diagnostics": diagnostics
            .iter()
            .map(|diagnostic| json!({
                "severity": diagnostic.severity.label(),
                "code": diagnostic.code.label(),
                "locus": diagnostic.locus,
                "message": diagnostic.message,
                "remedy": diagnostic.remedy,
            }))
            .collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHARACTER: &[u8] = include_bytes!(
        "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
    );

    /// The character GLB with its JSON chunk changed by `edit`.
    fn edited(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let json_length = u32::from_le_bytes(CHARACTER[12..16].try_into().unwrap()) as usize;
        let mut root: Value = serde_json::from_slice(&CHARACTER[20..20 + json_length]).unwrap();
        edit(&mut root);
        let mut json = serde_json::to_vec(&root).unwrap();
        json.resize(json.len().next_multiple_of(4), b' ');
        let rest = &CHARACTER[20 + json_length..];
        let total = 20 + json.len() + rest.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb.extend_from_slice(rest);
        glb
    }

    fn codes(diagnostics: &[ImportDiagnostic]) -> Vec<&'static str> {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error())
            .map(|diagnostic| diagnostic.code.label())
            .collect()
    }

    #[test]
    fn a_self_contained_glb_is_admitted() {
        let (admitted, diagnostics) = check_glb("character.glb", CHARACTER.to_vec());
        assert!(admitted, "{diagnostics:?}");
        assert!(codes(&diagnostics).is_empty());
        let report = report(Path::new("character.glb"), admitted, &diagnostics);
        assert_eq!(report["admitted"], true);
        assert_eq!(report["diagnostics"], json!([]));
    }

    #[test]
    fn an_external_image_uri_is_refused_as_an_external_resource() {
        let glb = edited(|root| {
            let image = root["images"][0].as_object_mut().unwrap();
            image.remove("bufferView");
            image.remove("mimeType");
            image.insert("uri".into(), json!("skin.png"));
        });
        let (admitted, diagnostics) = check_glb("character.glb", glb);
        assert!(!admitted);
        assert_eq!(codes(&diagnostics), ["externalResource"]);
        let report = report(Path::new("character.glb"), admitted, &diagnostics);
        assert_eq!(report["diagnostics"][0]["code"], "externalResource");
        assert_eq!(report["diagnostics"][0]["severity"], "error");
    }

    #[test]
    fn unadmitted_extensions_are_refused_with_the_runtime_codes() {
        // The Engine's allowlist refuses an extension the source uses...
        let used = edited(|root| root["extensionsUsed"] = json!(["KHR_materials_sheen"]));
        let (admitted, diagnostics) = check_glb("character.glb", used);
        assert!(!admitted);
        assert_eq!(codes(&diagnostics), ["unsupportedFeature"]);
        assert_eq!(diagnostics[0].locus, "source.extensionsUsed");
        // ...the glTF parser refuses an unknown extension it requires...
        let required = edited(|root| {
            root["extensionsUsed"] = json!(["KHR_draco_mesh_compression"]);
            root["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]);
        });
        let (admitted, diagnostics) = check_glb("character.glb", required);
        assert!(!admitted);
        assert_eq!(codes(&diagnostics), ["invalidContainer"]);
        assert!(diagnostics[0].message.contains("extensionsRequired"));
        // ...and an admitted one, declared, passes.
        let webp = edited(|root| root["extensionsUsed"] = json!(["EXT_texture_webp"]));
        assert!(check_glb("character.glb", webp).0);
    }
}
