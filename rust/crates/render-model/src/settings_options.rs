//! The renderer settings as options a player chooses (docs/lighting-and-sky.md,
//! "Video options"): a catalogue that names, groups and bounds every setting
//! the renderer has, presets over it, and the player's choices, which apply
//! over whatever the product asks for.
//!
//! A new renderer setting adds its catalogue entry, its override field and
//! its value mapping here in the commit that adds it, so the video options
//! panel and the feature gallery offer it with no product change.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AmbientOcclusionMode, RendererSettingsDescriptor};

/// One choice of a [`RendererSettingKind::Choice`] option.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettingChoice {
    pub value: &'static str,
    pub label: &'static str,
}

/// How an option is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RendererSettingKind {
    /// On or off: a JSON boolean.
    Toggle,
    /// One of named values: a JSON string.
    Choice {
        choices: &'static [RendererSettingChoice],
    },
    /// A number within a range, in steps: a JSON number.
    Range {
        min: f64,
        max: f64,
        step: f64,
        unit: &'static str,
    },
}

/// One option of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettingOption {
    /// Stable identity, also the key of the player's stored choice.
    pub id: &'static str,
    pub label: &'static str,
    /// Display, Quality, Lighting or Advanced.
    pub group: &'static str,
    pub description: &'static str,
    #[serde(flatten)]
    pub kind: RendererSettingKind,
}

const ANTIALIASING_CHOICES: &[RendererSettingChoice] = &[
    RendererSettingChoice {
        value: "off",
        label: "Off",
    },
    RendererSettingChoice {
        value: "2x",
        label: "2× MSAA",
    },
    RendererSettingChoice {
        value: "4x",
        label: "4× MSAA",
    },
];

const SHADOW_BUDGET_CHOICES: &[RendererSettingChoice] = &[
    RendererSettingChoice {
        value: "4",
        label: "4 layers",
    },
    RendererSettingChoice {
        value: "8",
        label: "8 layers",
    },
    RendererSettingChoice {
        value: "16",
        label: "16 layers",
    },
    RendererSettingChoice {
        value: "32",
        label: "32 layers",
    },
    RendererSettingChoice {
        value: "none",
        label: "No limit",
    },
];

const AMBIENT_OCCLUSION_CHOICES: &[RendererSettingChoice] = &[
    RendererSettingChoice {
        value: "disabled",
        label: "Off",
    },
    RendererSettingChoice {
        value: "screenSpace",
        label: "Screen space",
    },
    RendererSettingChoice {
        value: "distanceField",
        label: "Distance field (voxel worlds)",
    },
];

/// Every renderer setting, in the order a menu shows them.
pub const RENDERER_SETTING_OPTIONS: &[RendererSettingOption] = &[
    RendererSettingOption {
        id: "renderScale",
        label: "Render scale",
        group: "Display",
        description: "Draws the world at a fraction of the screen's resolution and scales it up: lower is faster and softer.",
        kind: RendererSettingKind::Range {
            min: RendererSettingsDescriptor::MIN_RENDER_SCALE as f64,
            max: 1.0,
            step: 0.05,
            unit: "×",
        },
    },
    RendererSettingOption {
        id: "antialiasing",
        label: "Anti-aliasing",
        group: "Display",
        description: "Smooths jagged edges by taking several samples a pixel.",
        kind: RendererSettingKind::Choice {
            choices: ANTIALIASING_CHOICES,
        },
    },
    RendererSettingOption {
        id: "vsync",
        label: "Vertical sync",
        group: "Display",
        description: "Waits for the display's refresh before showing a frame: no tearing, frame rate capped at the refresh rate.",
        kind: RendererSettingKind::Toggle,
    },
    RendererSettingOption {
        id: "shadows",
        label: "Shadows",
        group: "Quality",
        description: "Lights that the game asks to cast shadows do.",
        kind: RendererSettingKind::Toggle,
    },
    RendererSettingOption {
        id: "shadowBudget",
        label: "Shadow budget",
        group: "Quality",
        description: "How many shadow maps render at once; the nearest and most important lights keep theirs.",
        kind: RendererSettingKind::Choice {
            choices: SHADOW_BUDGET_CHOICES,
        },
    },
    RendererSettingOption {
        id: "ambientOcclusion",
        label: "Ambient occlusion",
        group: "Lighting",
        description: "Darkens corners, creases and the ground under things, where ambient light reaches less.",
        kind: RendererSettingKind::Choice {
            choices: AMBIENT_OCCLUSION_CHOICES,
        },
    },
    RendererSettingOption {
        id: "ambientOcclusionStrength",
        label: "Occlusion strength",
        group: "Lighting",
        description: "How dark ambient occlusion makes what it darkens.",
        kind: RendererSettingKind::Range {
            min: 0.0,
            max: 2.0,
            step: 0.1,
            unit: "",
        },
    },
    RendererSettingOption {
        id: "ambientOcclusionRadius",
        label: "Occlusion radius",
        group: "Lighting",
        description: "How far from a surface ambient occlusion looks for what shades it.",
        kind: RendererSettingKind::Range {
            min: 0.25,
            max: 2.0,
            step: 0.05,
            unit: "m",
        },
    },
    RendererSettingOption {
        id: "clusteredLighting",
        label: "Clustered lighting",
        group: "Advanced",
        description: "Sorts lights into screen clusters first: faster with many lights in view, a little slower with few.",
        kind: RendererSettingKind::Toggle,
    },
    RendererSettingOption {
        id: "gpuCulling",
        label: "GPU culling",
        group: "Advanced",
        description: "Decides what is in view on the graphics card instead of the processor: helps only very full scenes.",
        kind: RendererSettingKind::Toggle,
    },
];

/// A named set of choices the player can apply at once.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettingPreset {
    pub id: &'static str,
    pub label: &'static str,
}

pub const RENDERER_SETTING_PRESETS: &[RendererSettingPreset] = &[
    RendererSettingPreset {
        id: "low",
        label: "Low",
    },
    RendererSettingPreset {
        id: "medium",
        label: "Medium",
    },
    RendererSettingPreset {
        id: "high",
        label: "High",
    },
    RendererSettingPreset {
        id: "ultra",
        label: "Ultra",
    },
];

/// The player's choices: each set field replaces the product's request for
/// that setting. Stored per install, so they hold across sessions and over
/// every `RendererSettings.Set` the product makes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct RendererSettingsOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub antialiasing: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vsync: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadows: Option<bool>,
    /// `Some(None)` is a chosen "no limit".
    #[serde(skip_serializing_if = "Option::is_none", with = "double_option")]
    pub shadow_budget: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambient_occlusion: Option<AmbientOcclusionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambient_occlusion_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambient_occlusion_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clustered_lighting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_culling: Option<bool>,
}

/// `Option<Option<T>>` as absent / `null` / a value.
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(
        value: &Option<Option<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

impl RendererSettingsOverrides {
    /// Whether the player has chosen nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The product's request with the player's choices over it.
    pub fn apply(&self, base: RendererSettingsDescriptor) -> RendererSettingsDescriptor {
        let mut settings = base;
        if let Some(value) = self.render_scale {
            settings.render_scale = value;
        }
        if let Some(value) = self.antialiasing {
            settings.antialiasing = value;
        }
        if let Some(value) = self.vsync {
            settings.vsync = value;
        }
        if let Some(value) = self.shadows {
            settings.shadows = value;
        }
        if let Some(value) = self.shadow_budget {
            settings.shadow_budget = value;
        }
        if let Some(value) = self.ambient_occlusion {
            settings.ambient_occlusion.mode = value;
        }
        if let Some(value) = self.ambient_occlusion_strength {
            settings.ambient_occlusion.strength = value;
        }
        if let Some(value) = self.ambient_occlusion_radius {
            settings.ambient_occlusion.radius = value;
        }
        if let Some(value) = self.clustered_lighting {
            settings.clustered_lighting = value;
        }
        if let Some(value) = self.gpu_culling {
            settings.gpu_culling = value;
        }
        settings
    }

    /// Whether the player has chosen the option `id`.
    pub fn chosen(&self, id: &str) -> bool {
        match id {
            "renderScale" => self.render_scale.is_some(),
            "antialiasing" => self.antialiasing.is_some(),
            "vsync" => self.vsync.is_some(),
            "shadows" => self.shadows.is_some(),
            "shadowBudget" => self.shadow_budget.is_some(),
            "ambientOcclusion" => self.ambient_occlusion.is_some(),
            "ambientOcclusionStrength" => self.ambient_occlusion_strength.is_some(),
            "ambientOcclusionRadius" => self.ambient_occlusion_radius.is_some(),
            "clusteredLighting" => self.clustered_lighting.is_some(),
            "gpuCulling" => self.gpu_culling.is_some(),
            _ => false,
        }
    }

    /// Chooses `value` for the option `id`, or says why not.
    pub fn choose(&mut self, id: &str, value: &Value) -> Result<(), String> {
        let option = RENDERER_SETTING_OPTIONS
            .iter()
            .find(|option| option.id == id)
            .ok_or_else(|| format!("no renderer setting {id}"))?;
        let toggle = || {
            value
                .as_bool()
                .ok_or_else(|| format!("{id} takes true or false"))
        };
        let number = || -> Result<f32, String> {
            let RendererSettingKind::Range { min, max, .. } = option.kind else {
                return Err(format!("{id} is not a range"));
            };
            value
                .as_f64()
                .filter(|number| number.is_finite() && (min..=max).contains(number))
                .map(|number| number as f32)
                .ok_or_else(|| format!("{id} takes a number from {min} to {max}"))
        };
        let choice = || -> Result<&str, String> {
            let RendererSettingKind::Choice { choices } = option.kind else {
                return Err(format!("{id} is not a choice"));
            };
            value
                .as_str()
                .filter(|text| choices.iter().any(|choice| choice.value == *text))
                .ok_or_else(|| {
                    let names: Vec<&str> = choices.iter().map(|choice| choice.value).collect();
                    format!("{id} takes one of {}", names.join(", "))
                })
        };
        match id {
            "renderScale" => self.render_scale = Some(number()?),
            "antialiasing" => {
                self.antialiasing = Some(match choice()? {
                    "off" => 1,
                    "2x" => 2,
                    _ => 4,
                })
            }
            "vsync" => self.vsync = Some(toggle()?),
            "shadows" => self.shadows = Some(toggle()?),
            "shadowBudget" => {
                let text = choice()?;
                self.shadow_budget = Some(text.parse::<u32>().ok());
            }
            "ambientOcclusion" => {
                self.ambient_occlusion = Some(match choice()? {
                    "screenSpace" => AmbientOcclusionMode::ScreenSpace,
                    "distanceField" => AmbientOcclusionMode::DistanceField,
                    _ => AmbientOcclusionMode::Disabled,
                })
            }
            "ambientOcclusionStrength" => self.ambient_occlusion_strength = Some(number()?),
            "ambientOcclusionRadius" => {
                let radius = number()?;
                if radius <= 0.0 {
                    return Err(format!("{id} must be above 0"));
                }
                self.ambient_occlusion_radius = Some(radius);
            }
            "clusteredLighting" => self.clustered_lighting = Some(toggle()?),
            "gpuCulling" => self.gpu_culling = Some(toggle()?),
            _ => return Err(format!("no renderer setting {id}")),
        }
        Ok(())
    }

    /// Forgets the player's choice for `id`, so the product's request holds.
    pub fn forget(&mut self, id: &str) -> Result<(), String> {
        match id {
            "renderScale" => self.render_scale = None,
            "antialiasing" => self.antialiasing = None,
            "vsync" => self.vsync = None,
            "shadows" => self.shadows = None,
            "shadowBudget" => self.shadow_budget = None,
            "ambientOcclusion" => self.ambient_occlusion = None,
            "ambientOcclusionStrength" => self.ambient_occlusion_strength = None,
            "ambientOcclusionRadius" => self.ambient_occlusion_radius = None,
            "clusteredLighting" => self.clustered_lighting = None,
            "gpuCulling" => self.gpu_culling = None,
            _ => return Err(format!("no renderer setting {id}")),
        }
        Ok(())
    }

    /// The choices a preset makes, over the player's others.
    pub fn preset(id: &str) -> Option<Self> {
        let (scale, antialiasing, budget, occlusion) = match id {
            "low" => (0.75, 1, Some(4), AmbientOcclusionMode::Disabled),
            "medium" => (1.0, 2, Some(8), AmbientOcclusionMode::ScreenSpace),
            "high" => (1.0, 4, Some(16), AmbientOcclusionMode::ScreenSpace),
            "ultra" => (1.0, 4, None, AmbientOcclusionMode::ScreenSpace),
            _ => return None,
        };
        Some(Self {
            render_scale: Some(scale),
            antialiasing: Some(antialiasing),
            shadows: Some(true),
            shadow_budget: Some(budget),
            ambient_occlusion: Some(occlusion),
            ..Self::default()
        })
    }

    /// `preset`'s choices laid over these.
    pub fn with(&self, preset: &Self) -> Self {
        Self {
            render_scale: preset.render_scale.or(self.render_scale),
            antialiasing: preset.antialiasing.or(self.antialiasing),
            vsync: preset.vsync.or(self.vsync),
            shadows: preset.shadows.or(self.shadows),
            shadow_budget: preset.shadow_budget.or(self.shadow_budget),
            ambient_occlusion: preset.ambient_occlusion.or(self.ambient_occlusion),
            ambient_occlusion_strength: preset
                .ambient_occlusion_strength
                .or(self.ambient_occlusion_strength),
            ambient_occlusion_radius: preset
                .ambient_occlusion_radius
                .or(self.ambient_occlusion_radius),
            clustered_lighting: preset.clustered_lighting.or(self.clustered_lighting),
            gpu_culling: preset.gpu_culling.or(self.gpu_culling),
        }
    }
}

/// The value of option `id` in `settings`, as the catalogue writes it.
pub fn renderer_setting_value(settings: &RendererSettingsDescriptor, id: &str) -> Option<Value> {
    Some(match id {
        "renderScale" => Value::from(round_to(settings.render_scale as f64, 0.01)),
        "antialiasing" => Value::from(match settings.antialiasing {
            1 => "off",
            2 => "2x",
            _ => "4x",
        }),
        "vsync" => Value::from(settings.vsync),
        "shadows" => Value::from(settings.shadows),
        "shadowBudget" => Value::from(
            settings
                .shadow_budget
                .map_or_else(|| "none".to_owned(), |budget| budget.to_string()),
        ),
        "ambientOcclusion" => Value::from(match settings.ambient_occlusion.mode {
            AmbientOcclusionMode::Disabled => "disabled",
            AmbientOcclusionMode::ScreenSpace => "screenSpace",
            AmbientOcclusionMode::DistanceField => "distanceField",
        }),
        "ambientOcclusionStrength" => {
            Value::from(round_to(settings.ambient_occlusion.strength as f64, 0.01))
        }
        "ambientOcclusionRadius" => {
            Value::from(round_to(settings.ambient_occlusion.radius as f64, 0.01))
        }
        "clusteredLighting" => Value::from(settings.clustered_lighting),
        "gpuCulling" => Value::from(settings.gpu_culling),
        _ => return None,
    })
}

fn round_to(value: f64, step: f64) -> f64 {
    (value / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_option_has_a_value_and_can_be_chosen_and_forgotten() {
        let settings = RendererSettingsDescriptor::DEFAULT;
        for option in RENDERER_SETTING_OPTIONS {
            let value = renderer_setting_value(&settings, option.id)
                .unwrap_or_else(|| panic!("{} has no value", option.id));
            let mut overrides = RendererSettingsOverrides::default();
            overrides
                .choose(option.id, &value)
                .unwrap_or_else(|error| panic!("{}: {error}", option.id));
            assert!(overrides.chosen(option.id), "{}", option.id);
            assert_eq!(overrides.apply(settings), settings, "{}", option.id);
            overrides.forget(option.id).unwrap();
            assert!(overrides.is_empty(), "{}", option.id);
        }
    }

    #[test]
    fn choices_apply_over_the_product_request() {
        let mut overrides = RendererSettingsOverrides::default();
        overrides.choose("antialiasing", &json!("off")).unwrap();
        overrides
            .choose("ambientOcclusion", &json!("screenSpace"))
            .unwrap();
        overrides.choose("shadowBudget", &json!("none")).unwrap();
        overrides.choose("renderScale", &json!(0.75)).unwrap();
        let base = RendererSettingsDescriptor {
            shadow_budget: Some(8),
            ..RendererSettingsDescriptor::DEFAULT
        };
        let applied = overrides.apply(base);
        assert_eq!(applied.antialiasing, 1);
        assert_eq!(
            applied.ambient_occlusion.mode,
            AmbientOcclusionMode::ScreenSpace
        );
        assert_eq!(applied.shadow_budget, None);
        assert_eq!(applied.render_scale, 0.75);
        assert_eq!(applied.vsync, base.vsync);
        assert!(applied.valid());
    }

    #[test]
    fn out_of_range_and_unknown_choices_are_refused() {
        let mut overrides = RendererSettingsOverrides::default();
        assert!(overrides.choose("renderScale", &json!(0.25)).is_err());
        assert!(overrides.choose("antialiasing", &json!("8x")).is_err());
        assert!(overrides.choose("vsync", &json!("yes")).is_err());
        assert!(overrides.choose("bloom", &json!(true)).is_err());
        assert!(overrides.is_empty());
    }

    #[test]
    fn the_store_round_trips_including_a_chosen_no_limit() {
        let mut overrides = RendererSettingsOverrides::default();
        overrides.choose("shadowBudget", &json!("none")).unwrap();
        overrides.choose("vsync", &json!(false)).unwrap();
        let text = serde_json::to_string(&overrides).unwrap();
        assert_eq!(text, r#"{"vsync":false,"shadowBudget":null}"#);
        let read: RendererSettingsOverrides = serde_json::from_str(&text).unwrap();
        assert_eq!(read, overrides);
        assert_eq!(
            serde_json::from_str::<RendererSettingsOverrides>("{}").unwrap(),
            RendererSettingsOverrides::default()
        );
    }

    #[test]
    fn presets_lay_over_other_choices() {
        let mut overrides = RendererSettingsOverrides::default();
        overrides.choose("vsync", &json!(false)).unwrap();
        let low = RendererSettingsOverrides::preset("low").unwrap();
        let chosen = overrides.with(&low);
        assert_eq!(chosen.vsync, Some(false));
        assert_eq!(chosen.antialiasing, Some(1));
        for preset in RENDERER_SETTING_PRESETS {
            let applied = RendererSettingsOverrides::preset(preset.id)
                .unwrap()
                .apply(RendererSettingsDescriptor::DEFAULT);
            assert!(applied.valid(), "{}", preset.id);
        }
    }

    #[test]
    fn the_catalogue_serializes_its_kinds() {
        let json = serde_json::to_value(RENDERER_SETTING_OPTIONS).unwrap();
        assert_eq!(json[0]["id"], "renderScale");
        assert_eq!(json[0]["kind"], "range");
        assert_eq!(json[1]["kind"], "choice");
        assert_eq!(json[1]["choices"][0]["value"], "off");
        assert_eq!(json[2]["kind"], "toggle");
    }
}
