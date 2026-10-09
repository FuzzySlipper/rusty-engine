//! The feature gallery (docs/performance.md, "Feature gallery"): one scene
//! drawn as it is and once per renderer feature it leaves off, each turned on
//! at its Engine default, with what each changes in the picture and costs.
//! "How does this game look with X on", at a glance.
//!
//! Each variant renders in its own `rusty-scene-render` process, so its GPU
//! timings are its own. Features come from the renderer settings catalogue
//! (`RENDERER_SETTING_OPTIONS`), so a setting added there joins the gallery
//! with no change here.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use render_model::{
    AmbientOcclusionMode, RendererSettingKind, RendererSettingsDescriptor, VolumetricCloudsQuality,
    VolumetricFogQuality, RENDERER_SETTING_OPTIONS,
};

/// The stand-in fog medium for a scene with none: density and anisotropy.
const STAND_IN_FOG: &str = "0.015,0.6";
use serde::Serialize;
use serde_json::Value;

use crate::{
    font::{draw_text, LINE},
    image::{compare, Image},
};

/// What to draw and where.
pub struct GalleryRequest<'a> {
    /// The `rusty-scene-render` that draws each variant.
    pub renderer: &'a Path,
    pub snapshot: &'a Path,
    pub out: &'a Path,
    pub width: u32,
    pub height: u32,
    /// Frames each variant draws for its timings.
    pub frames: u32,
}

/// One variant: the flags that turn its feature on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryVariant {
    pub id: String,
    pub label: String,
    pub args: Vec<String>,
    /// Said beside the result, for a feature drawn with a stand-in setup.
    pub note: Option<String>,
}

/// What one variant drew and cost against the scene as it is.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryEntry {
    pub id: String,
    pub label: String,
    pub image: String,
    pub frame_ms: f64,
    /// Frame median against the baseline's; 0 for the baseline.
    pub frame_delta_ms: f64,
    /// Share of pixels changed by more than 2 levels against the baseline.
    pub changed_share: f64,
    /// GPU passes whose median rose most, with the rise.
    pub costlier_passes: Vec<(String, f64)>,
    /// What the device refused of it.
    pub refused: Vec<String>,
    pub note: Option<String>,
    pub error: Option<String>,
}

/// The gallery as written.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryReport {
    pub snapshot: String,
    pub adapter: String,
    pub software_adapter: bool,
    pub entries: Vec<GalleryEntry>,
    pub sheet: String,
}

struct Rendered {
    report: Value,
    image: PathBuf,
}

fn render(request: &GalleryRequest<'_>, id: &str, args: &[String]) -> Result<Rendered, String> {
    let image = request.out.join(format!("{id}.png"));
    let output = Command::new(request.renderer)
        .arg(request.snapshot)
        .arg(&image)
        .args(["--width", &request.width.to_string()])
        .args(["--height", &request.height.to_string()])
        .args(["--frames", &request.frames.to_string()])
        .args(args)
        .output()
        .map_err(|error| format!("{}: {error}", request.renderer.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().last().unwrap_or("failed").to_owned());
    }
    let report =
        serde_json::from_slice(&output.stdout).map_err(|error| format!("report: {error}"))?;
    let path = request.out.join(format!("{id}.json"));
    fs::write(&path, &output.stdout).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Rendered { report, image })
}

/// The GPU passes that ran in at least half the timed frames, with their
/// medians: a one-off build (the sky light's, a probe bake) is not a
/// per-frame cost.
fn passes(report: &Value) -> Vec<(String, f64)> {
    let frames = report["timing"]["frames"].as_u64().unwrap_or(0);
    report["gpu"]["passes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|pass| {
            let timed = pass["timedFrames"].as_u64().unwrap_or(0);
            timed > 0 && timed * 2 >= frames
        })
        .filter_map(|pass| {
            Some((
                pass["pass"].as_str()?.to_owned(),
                pass["medianGpuMs"].as_f64()?,
            ))
        })
        .collect()
}

fn choice_label(id: &str, value: &str) -> String {
    RENDERER_SETTING_OPTIONS
        .iter()
        .find(|option| option.id == id)
        .and_then(|option| match option.kind {
            RendererSettingKind::Choice { choices } => choices
                .iter()
                .find(|choice| choice.value == value)
                .map(|choice| choice.label.to_owned()),
            _ => None,
        })
        .unwrap_or_else(|| value.to_owned())
}

fn option_label(id: &str) -> String {
    RENDERER_SETTING_OPTIONS
        .iter()
        .find(|option| option.id == id)
        .map_or_else(|| id.to_owned(), |option| option.label.to_owned())
}

/// The variants for a scene drawn with `settings`: every feature it leaves
/// off or below its top quality, then everything on. Display pacing (vsync)
/// and tuning values (occlusion strength and radius) are not features.
pub fn plan(
    settings: &RendererSettingsDescriptor,
    camera: Option<[f64; 3]>,
    has_indirect_light: bool,
    has_fog: bool,
) -> Vec<GalleryVariant> {
    let choose = |id: &str, value: &str| vec!["--choose".to_owned(), format!("{id}={value}")];
    let labelled = |id: &str, value: &str| {
        let label = RENDERER_SETTING_OPTIONS
            .iter()
            .find(|option| option.id == id)
            .map_or(id, |option| option.label);
        format!("{label}: {}", choice_label(id, value))
    };
    let mut variants: Vec<GalleryVariant> = Vec::new();
    let mut everything: Vec<String> = Vec::new();
    let mut push = |id: String,
                    label: String,
                    args: Vec<String>,
                    everything_too: bool,
                    note: Option<String>| {
        if everything_too {
            everything.extend(args.iter().cloned());
        }
        variants.push(GalleryVariant {
            id,
            label,
            args,
            note,
        });
    };
    for option in RENDERER_SETTING_OPTIONS {
        let id = option.id;
        match id {
            "renderScale" if settings.render_scale < 1.0 => push(
                format!("{id}-1"),
                "Render scale 1×".to_owned(),
                choose(id, "1"),
                true,
                None,
            ),
            "antialiasing" if settings.antialiasing != 4 => push(
                format!("{id}-4x"),
                labelled(id, "4x"),
                choose(id, "4x"),
                true,
                None,
            ),
            "shadows" if !settings.shadows => push(
                format!("{id}-true"),
                "Shadows".to_owned(),
                choose(id, "true"),
                true,
                None,
            ),
            "shadowBudget" if settings.shadows && settings.shadow_budget.is_some() => push(
                format!("{id}-none"),
                labelled(id, "none"),
                choose(id, "none"),
                true,
                None,
            ),
            "ambientOcclusion" => {
                for (mode, value) in [
                    (AmbientOcclusionMode::ScreenSpace, "screenSpace"),
                    (AmbientOcclusionMode::DistanceField, "distanceField"),
                ] {
                    if settings.ambient_occlusion.mode != mode {
                        let everything_too = mode == AmbientOcclusionMode::ScreenSpace
                            && settings.ambient_occlusion.mode == AmbientOcclusionMode::Disabled;
                        push(
                            format!("{id}-{value}"),
                            labelled(id, value),
                            choose(id, value),
                            everything_too,
                            None,
                        );
                    }
                }
            }
            "volumetricFog" if settings.volumetric_fog == VolumetricFogQuality::Off => {
                // Volumetric fog lights the scene's fog; a scene without any
                // gets a thin stand-in haze to show what it would do.
                let mut args = choose(id, "low");
                let note = (!has_fog).then(|| {
                    args.extend([
                        "--volumetric-fog-medium".to_owned(),
                        STAND_IN_FOG.to_owned(),
                    ]);
                    "A thin stand-in haze (0.015 per metre): the game sets its own fog.".to_owned()
                });
                push(format!("{id}-low"), labelled(id, "low"), args, true, note);
            }
            "volumetricClouds" if settings.volumetric_clouds == VolumetricCloudsQuality::Off => {
                push(
                    format!("{id}-low"),
                    labelled(id, "low"),
                    choose(id, "low"),
                    true,
                    Some("Draws only where the scene has a cloud layer or regions.".to_owned()),
                )
            }
            "clusteredLighting" if !settings.clustered_lighting => push(
                format!("{id}-true"),
                option.label.to_owned(),
                choose(id, "true"),
                true,
                None,
            ),
            "gpuCulling" if !settings.gpu_culling => push(
                format!("{id}-true"),
                option.label.to_owned(),
                choose(id, "true"),
                true,
                None,
            ),
            _ => {}
        }
    }
    // Indirect light needs a volume the game places; the gallery puts one
    // around the camera to show what it would do.
    if let (false, Some([x, y, z])) = (has_indirect_light, camera) {
        let volume = format!("{x:.2},{y:.2},{z:.2},24,12,24,2,1");
        push(
            "indirectLight".to_owned(),
            "Indirect light".to_owned(),
            vec!["--indirect-light".to_owned(), volume],
            true,
            Some(
                "A 48×24×48 m probe volume around the camera: the game places its own.".to_owned(),
            ),
        );
    }
    if variants.len() > 1 {
        variants.push(GalleryVariant {
            id: "everything".to_owned(),
            label: "Everything on".to_owned(),
            args: everything,
            note: None,
        });
    }
    variants
}

/// Draws the gallery: `baseline.png`, one image per variant, `sheet.png`,
/// `gallery.json` and `gallery.md` in `request.out`.
pub fn run(request: &GalleryRequest<'_>) -> Result<GalleryReport, String> {
    fs::create_dir_all(request.out)
        .map_err(|error| format!("{}: {error}", request.out.display()))?;
    let baseline = render(request, "baseline", &[])?;
    let settings: RendererSettingsDescriptor =
        serde_json::from_value(baseline.report["settings"]["effective"].clone())
            .map_err(|error| format!("baseline settings: {error}"))?;
    let camera = baseline.report["camera"]["position"]
        .as_array()
        .and_then(|position| {
            Some([
                position.first()?.as_f64()?,
                position.get(1)?.as_f64()?,
                position.get(2)?.as_f64()?,
            ])
        });
    let has_indirect = !baseline.report["gpu"]["indirectLight"].is_null();
    let has_fog = baseline.report["gpu"]["volumetricFog"]["sceneHasFog"]
        .as_bool()
        .unwrap_or(false);
    let baseline_image = Image::read_png(&baseline.image)?;
    let baseline_frame = baseline.report["timing"]["medianMs"]
        .as_f64()
        .unwrap_or(0.0);
    let baseline_passes = passes(&baseline.report);

    let mut entries = vec![GalleryEntry {
        id: "baseline".to_owned(),
        label: "As the game draws it".to_owned(),
        image: baseline.image.display().to_string(),
        frame_ms: baseline_frame,
        frame_delta_ms: 0.0,
        changed_share: 0.0,
        costlier_passes: Vec::new(),
        refused: Vec::new(),
        note: None,
        error: None,
    }];
    for variant in plan(&settings, camera, has_indirect, has_fog) {
        eprintln!("rusty-scene-render gallery: {}", variant.label);
        let entry = match render(request, &variant.id, &variant.args) {
            Ok(rendered) => {
                let image = Image::read_png(&rendered.image)?;
                let diff = compare(&baseline_image, &image, 2);
                let frame_ms = rendered.report["timing"]["medianMs"]
                    .as_f64()
                    .unwrap_or(0.0);
                let mut costlier: Vec<(String, f64)> = passes(&rendered.report)
                    .into_iter()
                    .map(|(name, ms)| {
                        let before = baseline_passes
                            .iter()
                            .find(|(other, _)| *other == name)
                            .map_or(0.0, |(_, ms)| *ms);
                        (name, ms - before)
                    })
                    .filter(|(_, rise)| *rise > 0.005)
                    .collect();
                costlier.sort_by(|a, b| b.1.total_cmp(&a.1));
                costlier.truncate(3);
                let refused = rendered.report["settings"]["refused"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|refusal| {
                        Some(format!(
                            "{}: {}",
                            option_label(refusal["setting"].as_str()?),
                            refusal["refusal"].as_str()?
                        ))
                    })
                    .collect();
                GalleryEntry {
                    id: variant.id.clone(),
                    label: variant.label.clone(),
                    image: rendered.image.display().to_string(),
                    frame_ms,
                    frame_delta_ms: frame_ms - baseline_frame,
                    changed_share: diff.changed_share,
                    costlier_passes: costlier,
                    refused,
                    note: variant.note.clone(),
                    error: None,
                }
            }
            Err(error) => GalleryEntry {
                id: variant.id.clone(),
                label: variant.label.clone(),
                image: String::new(),
                frame_ms: 0.0,
                frame_delta_ms: 0.0,
                changed_share: 0.0,
                costlier_passes: Vec::new(),
                refused: Vec::new(),
                note: variant.note.clone(),
                error: Some(error),
            },
        };
        entries.push(entry);
    }

    let sheet = request.out.join("sheet.png");
    contact_sheet(&entries)?.write_png(&sheet)?;
    let report = GalleryReport {
        snapshot: request.snapshot.display().to_string(),
        adapter: baseline.report["adapter"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        software_adapter: baseline.report["softwareAdapter"]
            .as_bool()
            .unwrap_or(false),
        entries,
        sheet: sheet.display().to_string(),
    };
    let json = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    let path = request.out.join("gallery.json");
    fs::write(&path, json).map_err(|error| format!("{}: {error}", path.display()))?;
    let path = request.out.join("gallery.md");
    fs::write(&path, markdown(&report)).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(report)
}

/// One line of a sheet cell's caption: the cost and how much changed.
fn caption(entry: &GalleryEntry) -> String {
    if let Some(error) = &entry.error {
        return format!("failed: {error}");
    }
    if entry.id == "baseline" {
        return format!("{:.2} ms a frame", entry.frame_ms);
    }
    let change = if entry.changed_share < 0.0005 {
        "no visible change".to_owned()
    } else {
        format!("{:.1}% of pixels changed", entry.changed_share * 100.0)
    };
    format!("{:+.2} ms - {change}", entry.frame_delta_ms)
}

const CELL_WIDTH: u32 = 480;
const SHEET_COLUMNS: usize = 3;
const CAPTION_SCALE: u32 = 2;

fn contact_sheet(entries: &[GalleryEntry]) -> Result<Image, String> {
    let images: Vec<Option<Image>> = entries
        .iter()
        .map(|entry| {
            (!entry.image.is_empty())
                .then(|| {
                    Image::read_png(Path::new(&entry.image))
                        .map(|image| image.shrunk_to_width(CELL_WIDTH))
                })
                .transpose()
        })
        .collect::<Result<_, _>>()?;
    let picture_height = images
        .iter()
        .flatten()
        .map(|image| image.height)
        .max()
        .unwrap_or(CELL_WIDTH * 9 / 16);
    let caption_height = LINE * CAPTION_SCALE * 2 + 8;
    let cell_height = picture_height + caption_height;
    let rows = entries.len().div_ceil(SHEET_COLUMNS) as u32;
    let gap = 6;
    let mut sheet = Image::new(
        SHEET_COLUMNS as u32 * (CELL_WIDTH + gap) + gap,
        rows * (cell_height + gap) + gap,
        [18, 20, 26, 255],
    );
    for (index, (entry, image)) in entries.iter().zip(&images).enumerate() {
        let x = gap + (index % SHEET_COLUMNS) as u32 * (CELL_WIDTH + gap);
        let y = gap + (index / SHEET_COLUMNS) as u32 * (cell_height + gap);
        if let Some(image) = image {
            sheet.blit(image, x, y);
        }
        let label = if entry.refused.is_empty() {
            entry.label.clone()
        } else {
            format!("{} (refused)", entry.label)
        };
        let label = fit(&label, CELL_WIDTH - 4);
        let text_y = y + picture_height + 4;
        draw_text(
            &mut sheet,
            x + 2,
            text_y,
            &label,
            CAPTION_SCALE,
            [235, 240, 248, 255],
        );
        draw_text(
            &mut sheet,
            x + 2,
            text_y + LINE * CAPTION_SCALE,
            &fit(&caption(entry), CELL_WIDTH - 4),
            CAPTION_SCALE,
            [150, 170, 195, 255],
        );
    }
    Ok(sheet)
}

/// `text` cut to fit `width` pixels at the caption scale, ending in "..".
fn fit(text: &str, width: u32) -> String {
    let fits = (width / (crate::font::ADVANCE * CAPTION_SCALE)) as usize;
    if text.chars().count() <= fits {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(fits.saturating_sub(2)).collect();
    cut.push_str("..");
    cut
}

/// The gallery as a Markdown table.
pub fn markdown(report: &GalleryReport) -> String {
    let mut text = format!(
        "# Feature gallery\n\n`{}` on {}{}.\n\n![sheet]({})\n\n| Feature | Frame ms | Δ ms | Changed | Costlier passes | Notes |\n| --- | --- | --- | --- | --- | --- |\n",
        report.snapshot,
        report.adapter,
        if report.software_adapter { " (software adapter: not GPU evidence)" } else { "" },
        Path::new(&report.sheet).file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
    );
    for entry in &report.entries {
        let passes = entry
            .costlier_passes
            .iter()
            .map(|(name, rise)| format!("{name} +{rise:.3}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut notes: Vec<String> = entry.refused.clone();
        notes.extend(entry.note.clone());
        notes.extend(entry.error.clone());
        text.push_str(&format!(
            "| {} | {:.2} | {:+.2} | {:.2}% | {} | {} |\n",
            entry.label,
            entry.frame_ms,
            entry.frame_delta_ms,
            entry.changed_share * 100.0,
            passes,
            notes.join("; ")
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_scene_offers_every_feature_it_leaves_off_then_everything() {
        let settings = RendererSettingsDescriptor {
            antialiasing: 2,
            render_scale: 0.75,
            ..RendererSettingsDescriptor::DEFAULT
        };
        let ids: Vec<String> = plan(&settings, Some([1.0, 2.0, 3.0]), false, false)
            .into_iter()
            .map(|variant| variant.id)
            .collect();
        assert_eq!(
            ids,
            [
                "renderScale-1",
                "antialiasing-4x",
                "shadows-true",
                "ambientOcclusion-screenSpace",
                "ambientOcclusion-distanceField",
                "volumetricFog-low",
                "volumetricClouds-low",
                "clusteredLighting-true",
                "gpuCulling-true",
                "indirectLight",
                "everything",
            ]
        );
    }

    #[test]
    fn a_scene_with_everything_on_offers_only_the_other_occlusion_path() {
        let mut settings = RendererSettingsDescriptor {
            shadows: true,
            clustered_lighting: true,
            gpu_culling: true,
            volumetric_fog: VolumetricFogQuality::High,
            volumetric_clouds: VolumetricCloudsQuality::High,
            ..RendererSettingsDescriptor::DEFAULT
        };
        settings.ambient_occlusion.mode = AmbientOcclusionMode::ScreenSpace;
        let variants = plan(&settings, None, true, true);
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].id, "ambientOcclusion-distanceField");
        assert_eq!(
            variants[0].args,
            ["--choose", "ambientOcclusion=distanceField"]
        );
    }

    #[test]
    fn everything_on_takes_screen_space_occlusion_not_both_paths() {
        let variants = plan(&RendererSettingsDescriptor::DEFAULT, None, false, false);
        let everything = variants
            .iter()
            .find(|variant| variant.id == "everything")
            .unwrap();
        assert!(everything
            .args
            .contains(&"ambientOcclusion=screenSpace".to_owned()));
        assert!(!everything
            .args
            .contains(&"ambientOcclusion=distanceField".to_owned()));
    }

    #[test]
    fn captions_say_cost_and_change() {
        let mut entry = GalleryEntry {
            id: "shadows-true".to_owned(),
            label: "Shadows".to_owned(),
            image: String::new(),
            frame_ms: 2.0,
            frame_delta_ms: 0.4,
            changed_share: 0.123,
            costlier_passes: Vec::new(),
            refused: Vec::new(),
            note: None,
            error: None,
        };
        assert_eq!(caption(&entry), "+0.40 ms - 12.3% of pixels changed");
        entry.changed_share = 0.0;
        assert_eq!(caption(&entry), "+0.40 ms - no visible change");
    }
}
