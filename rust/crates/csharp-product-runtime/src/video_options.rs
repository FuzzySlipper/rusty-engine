//! The player's video options (docs/lighting-and-sky.md, "Video options"):
//! the renderer settings catalogue with the values in effect, served to the
//! Engine's options panel, and the player's choices, applied to the renderer
//! over every product request and stored for the install under the
//! persistence root.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use product_host::ProductHostVideoOptionsRequest;
use render_model::{
    renderer_setting_value, RendererSettingsOverrides, RENDERER_SETTING_OPTIONS,
    RENDERER_SETTING_PRESETS,
};
use render_wgpu::{RendererSettingsReadout, SceneDriver, SettingRefusal};
use serde_json::{json, Value};

/// The store's file under the persistence root.
pub(crate) const VIDEO_OPTIONS_FILE: &str = "engine-video-options.json";
/// The store's format.
const STORE_VERSION: u64 = 1;

/// The player's choices and where they are kept.
pub(crate) struct VideoOptions {
    file: Option<PathBuf>,
    choices: Mutex<RendererSettingsOverrides>,
    /// The world streams to a browser page: there is no display whose
    /// refresh vsync could wait for.
    streamed: bool,
}

impl VideoOptions {
    /// The choices stored under `persistence_root`, or none; a store that
    /// cannot be read is reported and ignored, so a damaged file never stops
    /// a game from starting.
    pub(crate) fn open(persistence_root: Option<&Path>, streamed: bool) -> Self {
        let file = persistence_root.map(|root| root.join(VIDEO_OPTIONS_FILE));
        let choices = file
            .as_deref()
            .and_then(|path| match read_store(path) {
                Ok(choices) => choices,
                Err(error) => {
                    eprintln!("rusty-product-host: video options: {error}; starting from the game's settings");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            file,
            choices: Mutex::new(choices),
            streamed,
        }
    }

    pub(crate) fn choices(&self) -> RendererSettingsOverrides {
        *self
            .choices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Answers the panel: the catalogue, or a change applied to `driver`,
    /// stored, then the catalogue as it now is.
    pub(crate) fn answer(
        &self,
        driver: &SceneDriver,
        request: ProductHostVideoOptionsRequest,
    ) -> Result<Value, String> {
        if let ProductHostVideoOptionsRequest::Change(change) = request {
            let mut choices = self
                .choices
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut next = *choices;
            apply_change(&mut next, &change)?;
            let readout = driver.settings_readout();
            if !next.apply(readout.product).valid() {
                return Err("those choices together are not a valid renderer setting".to_owned());
            }
            if let Some(file) = &self.file {
                write_store(file, &next)?;
            }
            *choices = next;
            drop(choices);
            driver.set_player_settings(next);
        }
        Ok(self.catalogue(&driver.settings_readout()))
    }

    /// Every option with its value in effect, what was asked, the game's own
    /// value, whether the player chose it and what the device refused.
    pub(crate) fn catalogue(&self, readout: &RendererSettingsReadout) -> Value {
        let options: Vec<Value> = RENDERER_SETTING_OPTIONS
            .iter()
            .map(|option| {
                let mut entry = serde_json::to_value(option).unwrap_or(Value::Null);
                if let Some(object) = entry.as_object_mut() {
                    object.insert(
                        "value".to_owned(),
                        renderer_setting_value(&readout.effective, option.id)
                            .unwrap_or(Value::Null),
                    );
                    object.insert(
                        "requested".to_owned(),
                        renderer_setting_value(&readout.requested, option.id)
                            .unwrap_or(Value::Null),
                    );
                    object.insert(
                        "gameDefault".to_owned(),
                        renderer_setting_value(&readout.product, option.id).unwrap_or(Value::Null),
                    );
                    object.insert(
                        "chosen".to_owned(),
                        Value::from(readout.player.chosen(option.id)),
                    );
                    object.insert(
                        "refused".to_owned(),
                        refusal(readout, option.id, self.streamed).map_or(Value::Null, Value::from),
                    );
                }
                entry
            })
            .collect();
        json!({
            "version": STORE_VERSION,
            "stored": self.file.is_some(),
            "presets": RENDERER_SETTING_PRESETS,
            "options": options,
        })
    }
}

/// Why the device does not draw option `id` as asked, in words for the
/// panel.
fn refusal(readout: &RendererSettingsReadout, id: &str, streamed: bool) -> Option<&'static str> {
    if id == "vsync" && streamed {
        return Some("The game is streamed to a browser page, which paces its own frames.");
    }
    let refused = match id {
        "ambientOcclusion" => readout.ambient_occlusion,
        "antialiasing" => readout.antialiasing,
        "vsync" => readout.vsync,
        "clusteredLighting" => readout.clustered_lighting,
        "gpuCulling" => readout.gpu_culling,
        "volumetricFog" => readout.volumetric_fog,
        _ => None,
    }?;
    Some(match refused {
        SettingRefusal::NoComputeShaders => "This graphics device has no compute shaders.",
        SettingRefusal::NoIndirectDraws => {
            "This graphics device cannot draw from GPU-written lists."
        }
        SettingRefusal::UnsupportedSampleCount => {
            "This graphics device cannot use that many samples."
        }
        SettingRefusal::VsyncOnly => "This display only presents in step with its refresh.",
        SettingRefusal::SoftwareAdapter => {
            "This is a software graphics adapter, which draws without it."
        }
    })
}

/// `{"choose": {"id", "value"}}`, `{"forget": id}`, `{"forget": null}` (every
/// choice) or `{"preset": id}`.
fn apply_change(choices: &mut RendererSettingsOverrides, change: &Value) -> Result<(), String> {
    let object = change
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or("a change is one of {choose}, {forget} or {preset}")?;
    let (verb, argument) = object.iter().next().ok_or("an empty change")?;
    match verb.as_str() {
        "choose" => {
            let id = argument["id"].as_str().ok_or("choose needs an id")?;
            choices.choose(id, &argument["value"])
        }
        "forget" => match argument {
            Value::Null => {
                *choices = RendererSettingsOverrides::default();
                Ok(())
            }
            Value::String(id) => choices.forget(id),
            _ => Err("forget takes an option id, or null for every choice".to_owned()),
        },
        "preset" => {
            let id = argument.as_str().ok_or("preset takes a preset id")?;
            let preset =
                RendererSettingsOverrides::preset(id).ok_or_else(|| format!("no preset {id}"))?;
            *choices = choices.with(&preset);
            Ok(())
        }
        other => Err(format!("no change {other}")),
    }
}

fn read_store(path: &Path) -> Result<Option<RendererSettingsOverrides>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if value["version"].as_u64() != Some(STORE_VERSION) {
        return Err(format!(
            "{}: not a version {STORE_VERSION} store",
            path.display()
        ));
    }
    serde_json::from_value(value["choices"].clone())
        .map(Some)
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// Written beside and renamed over, so a crash never leaves half a store.
fn write_store(path: &Path, choices: &RendererSettingsOverrides) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&json!({
        "version": STORE_VERSION,
        "choices": choices,
    }))
    .map_err(|error| error.to_string())?;
    let partial = path.with_extension("json.partial");
    fs::write(&partial, text).map_err(|error| format!("{}: {error}", partial.display()))?;
    fs::rename(&partial, path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Serves the panel from `driver` with the choices stored under
/// `persistence_root`, applying them now.
pub(crate) fn install(
    host: &product_host::ProductHostVideoOptions,
    driver: Arc<SceneDriver>,
    persistence_root: Option<&Path>,
    streamed: bool,
) {
    let options = VideoOptions::open(persistence_root, streamed);
    let choices = options.choices();
    if !choices.is_empty() {
        driver.set_player_settings(choices);
    }
    host.set_handler(move |request| options.answer(&driver, request));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("rusty-video-options-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn changes_choose_forget_and_lay_presets_over_other_choices() {
        let mut choices = RendererSettingsOverrides::default();
        apply_change(
            &mut choices,
            &json!({"choose": {"id": "vsync", "value": false}}),
        )
        .unwrap();
        apply_change(&mut choices, &json!({"preset": "low"})).unwrap();
        assert_eq!(choices.vsync, Some(false));
        assert_eq!(choices.antialiasing, Some(1));
        apply_change(&mut choices, &json!({"forget": "vsync"})).unwrap();
        assert_eq!(choices.vsync, None);
        apply_change(&mut choices, &json!({"forget": null})).unwrap();
        assert!(choices.is_empty());
        assert!(apply_change(&mut choices, &json!({"preset": "max"})).is_err());
        assert!(apply_change(
            &mut choices,
            &json!({"choose": {"id": "bloom", "value": 1}})
        )
        .is_err());
        assert!(apply_change(&mut choices, &json!({"choose": {}, "preset": "low"})).is_err());
    }

    #[test]
    fn the_store_round_trips_and_a_damaged_one_is_ignored() {
        let root = temporary_root("store");
        let path = root.join(VIDEO_OPTIONS_FILE);
        let mut choices = RendererSettingsOverrides::default();
        choices.choose("antialiasing", &json!("2x")).unwrap();
        write_store(&path, &choices).unwrap();
        assert_eq!(read_store(&path).unwrap(), Some(choices));
        assert_eq!(VideoOptions::open(Some(&root), false).choices(), choices);
        fs::write(
            &path,
            "{\"version\":1,\"choices\":{\"antialiasing\":\"lots\"}}",
        )
        .unwrap();
        assert!(read_store(&path).is_err());
        assert!(VideoOptions::open(Some(&root), false).choices().is_empty());
        assert_eq!(read_store(&root.join("absent.json")).unwrap(), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_catalogue_names_values_choices_and_refusals() {
        let options = VideoOptions::open(None, true);
        let product = render_model::RendererSettingsDescriptor::DEFAULT;
        let mut player = RendererSettingsOverrides::default();
        player.choose("antialiasing", &json!("off")).unwrap();
        let requested = player.apply(product);
        let readout = RendererSettingsReadout {
            requested,
            product,
            player,
            effective: requested,
            ambient_occlusion: None,
            antialiasing: None,
            vsync: None,
            clustered_lighting: Some(SettingRefusal::NoComputeShaders),
            gpu_culling: None,
            volumetric_fog: None,
        };
        let catalogue = options.catalogue(&readout);
        assert_eq!(catalogue["stored"], false);
        let entry = |id: &str| {
            catalogue["options"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["id"] == id)
                .unwrap()
                .clone()
        };
        assert_eq!(entry("antialiasing")["value"], "off");
        assert_eq!(entry("antialiasing")["gameDefault"], "4x");
        assert_eq!(entry("antialiasing")["chosen"], true);
        assert_eq!(entry("renderScale")["chosen"], false);
        assert!(entry("clusteredLighting")["refused"].is_string());
        assert!(
            entry("vsync")["refused"].is_string(),
            "streamed output has no display"
        );
        assert_eq!(catalogue["presets"][0]["id"], "low");
    }
}
