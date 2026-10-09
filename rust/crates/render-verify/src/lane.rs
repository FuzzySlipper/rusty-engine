//! The GPU verification lane (docs/verification.md#gpu-verification): a fixed
//! set of scene snapshots rendered on one machine by its accepted baseline
//! renderer and by a candidate, interleaved, with the images and GPU pass
//! timings compared. Images are deterministic on one adapter, so any change
//! beyond the thresholds is flagged for review; timings flag and never fail.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::image::{compare, heat_map, Image, ImageDiff};

/// The lane's file at the root of its directory.
pub const MANIFEST_FILE: &str = "scenes.json";
/// A machine's baseline record, under `machines/<machine>/`.
pub const BASELINE_FILE: &str = "baseline.json";

/// How scenes render and when a difference is flagged, with per-scene
/// overrides.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaneSettings {
    pub width: u32,
    pub height: u32,
    /// Frames timed per render, their medians the timings. The image is
    /// drawn first, before them: a view seen once, so a feature filtered
    /// over frames (volumetric fog, volumetric clouds) shows its first
    /// frame, with no history.
    pub frames: u32,
    /// Degrees the camera turns each frame, so timings cover more than one
    /// view, as `rusty-scene-render --turn`.
    pub turn: f64,
    /// Renders per renderer, alternating which renderer goes first.
    pub repeats: u32,
    /// A pixel counts as changed when a colour channel moves more than this.
    pub changed_above: u8,
    /// Flag when more than this share of pixels changed.
    pub changed_share_flag: f64,
    /// Flag when the structural similarity falls below this.
    pub ssim_flag: f64,
    /// Flag a GPU pass slower by both this ratio and this many milliseconds.
    pub pass_ratio_flag: f64,
    pub pass_delta_flag_ms: f64,
    /// Flag the frame median slower by both this ratio and this many
    /// milliseconds.
    pub frame_ratio_flag: f64,
    pub frame_delta_flag_ms: f64,
}

impl Default for LaneSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            frames: 30,
            turn: 0.25,
            repeats: 3,
            changed_above: 2,
            changed_share_flag: 0.0005,
            ssim_flag: 0.999,
            pass_ratio_flag: 1.15,
            pass_delta_flag_ms: 0.05,
            frame_ratio_flag: 1.10,
            frame_delta_flag_ms: 0.2,
        }
    }
}

/// One scene: a snapshot, the extra `rusty-scene-render` flags it renders
/// with, and settings it overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneScene {
    pub name: String,
    /// The `.rscene` file, relative to the lane directory.
    pub file: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// What the scene is there to show, for the review.
    #[serde(default)]
    pub notes: String,
    /// Settings this scene overrides, as in `defaults`.
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// The lane's `scenes.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneManifest {
    #[serde(default)]
    pub defaults: LaneSettings,
    pub scenes: Vec<LaneScene>,
}

impl LaneManifest {
    pub fn read(root: &Path) -> Result<Self, String> {
        let path = root.join(MANIFEST_FILE);
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The settings a scene renders and is judged with.
    pub fn settings_for(&self, scene: &LaneScene) -> Result<LaneSettings, String> {
        if scene.settings.is_empty() {
            return Ok(self.defaults);
        }
        let mut merged = serde_json::to_value(self.defaults).map_err(|error| error.to_string())?;
        if let Some(object) = merged.as_object_mut() {
            for (key, value) in &scene.settings {
                if !object.contains_key(key) {
                    return Err(format!("scene {}: unknown setting {key}", scene.name));
                }
                object.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(merged).map_err(|error| format!("scene {}: {error}", scene.name))
    }
}

/// A machine's accepted baseline: the renderer copied into its directory,
/// and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineBaseline {
    pub machine: String,
    /// The renderer's file name in the machine's directory.
    pub renderer: String,
    /// What was accepted: a commit, a pair version.
    pub source: String,
    /// Why, for the history.
    pub reason: String,
    /// Seconds since the Unix epoch.
    pub accepted_at: u64,
}

pub fn machine_directory(root: &Path, machine: &str) -> PathBuf {
    root.join("machines").join(machine)
}

impl MachineBaseline {
    pub fn read(root: &Path, machine: &str) -> Result<Self, String> {
        let path = machine_directory(root, machine).join(BASELINE_FILE);
        let text = fs::read_to_string(&path).map_err(|error| {
            format!(
                "{}: {error} (accept a baseline on this machine first: rusty-gpu-lane accept)",
                path.display()
            )
        })?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn renderer_path(&self, root: &Path) -> PathBuf {
        machine_directory(root, &self.machine).join(&self.renderer)
    }
}

/// Makes `renderer` this machine's baseline: copies it in (so the baseline
/// survives rebuilds) and records where it came from. The previous record is
/// kept beside it, numbered.
pub fn accept(
    root: &Path,
    machine: &str,
    renderer: &Path,
    source: &str,
    reason: &str,
) -> Result<MachineBaseline, String> {
    let directory = machine_directory(root, machine);
    fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let record = directory.join(BASELINE_FILE);
    if record.exists() {
        let previous = directory.join(format!("baseline-{}.json", unix_seconds()));
        fs::rename(&record, &previous)
            .map_err(|error| format!("{}: {error}", previous.display()))?;
    }
    let file_name = renderer
        .file_name()
        .ok_or_else(|| format!("{}: not a file", renderer.display()))?
        .to_string_lossy()
        .into_owned();
    let copied = directory.join(&file_name);
    fs::copy(renderer, &copied)
        .map_err(|error| format!("{} -> {}: {error}", renderer.display(), copied.display()))?;
    let baseline = MachineBaseline {
        machine: machine.to_owned(),
        renderer: file_name,
        source: source.to_owned(),
        reason: reason.to_owned(),
        accepted_at: unix_seconds(),
    };
    let text = serde_json::to_string_pretty(&baseline).map_err(|error| error.to_string())?;
    fs::write(&record, text).map_err(|error| format!("{}: {error}", record.display()))?;
    Ok(baseline)
}

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// What one render reported.
#[derive(Debug, Clone, PartialEq)]
struct RenderRun {
    adapter: String,
    software: bool,
    frame_ms: f64,
    passes: BTreeMap<String, f64>,
}

fn render(
    renderer: &Path,
    snapshot: &Path,
    image: &Path,
    settings: &LaneSettings,
    args: &[String],
) -> Result<RenderRun, String> {
    let output = Command::new(renderer)
        .arg(snapshot)
        .arg(image)
        .args(["--width", &settings.width.to_string()])
        .args(["--height", &settings.height.to_string()])
        .args(["--frames", &settings.frames.to_string()])
        .args(["--turn", &settings.turn.to_string()])
        .args(args)
        .output()
        .map_err(|error| format!("{}: {error}", renderer.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(6).collect();
        return Err(format!(
            "{} exited {}: {}",
            renderer.display(),
            output.status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        ));
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("{}: report: {error}", renderer.display()))?;
    let frame_ms = report["timing"]["medianMs"].as_f64().unwrap_or(0.0);
    let mut passes = BTreeMap::new();
    for pass in report["gpu"]["passes"].as_array().into_iter().flatten() {
        if pass["timedFrames"].as_u64().unwrap_or(0) == 0 {
            continue;
        }
        if let (Some(name), Some(ms)) = (pass["pass"].as_str(), pass["medianGpuMs"].as_f64()) {
            passes.insert(name.to_owned(), ms);
        }
    }
    Ok(RenderRun {
        adapter: report["adapter"].as_str().unwrap_or("unknown").to_owned(),
        software: report["softwareAdapter"].as_bool().unwrap_or(false),
        frame_ms,
        passes,
    })
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

/// A timing compared: baseline and candidate medians over the repeats.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingDelta {
    pub name: String,
    pub baseline_ms: f64,
    pub candidate_ms: f64,
    pub flagged: bool,
}

impl TimingDelta {
    /// Flagged when the candidate's median is slower than the baseline's by
    /// both the ratio and the amount, and every candidate repeat is slower
    /// than every baseline repeat: a one-off stall on a shared machine does
    /// not flag.
    fn new(
        name: &str,
        baseline: &[f64],
        candidate: &[f64],
        ratio_flag: f64,
        delta_flag: f64,
    ) -> Self {
        let baseline_ms = median(&mut baseline.to_vec());
        let candidate_ms = median(&mut candidate.to_vec());
        let fastest_candidate = candidate.iter().copied().fold(f64::INFINITY, f64::min);
        let slowest_baseline = baseline.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let flagged = candidate_ms - baseline_ms > delta_flag
            && candidate_ms > baseline_ms * ratio_flag
            && fastest_candidate > slowest_baseline;
        Self {
            name: name.to_owned(),
            baseline_ms,
            candidate_ms,
            flagged,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Pass,
    Flagged,
    Failed,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Flagged => "flagged",
            Verdict::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneResult {
    pub name: String,
    pub notes: String,
    pub verdict: Verdict,
    /// Why it is flagged or failed, one line each.
    pub reasons: Vec<String>,
    pub image: Option<ImageDiff>,
    /// How much each renderer's own repeats differed from its first: a
    /// non-deterministic scene, judged above that noise.
    pub self_noise_share: f64,
    pub frame: Option<TimingDelta>,
    pub passes: Vec<TimingDelta>,
    pub baseline_image: String,
    pub candidate_image: String,
    pub diff_image: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub machine: String,
    /// Every adapter either renderer reported, joined: more than one means
    /// a renderer fell back to another.
    pub adapter: String,
    /// Any render, baseline or candidate, ran on a software adapter: the run
    /// is not GPU evidence, and its status cannot be success.
    pub software_adapter: bool,
    /// Scenes the selection named that the manifest does not have: the run
    /// fails, so a misspelt name never passes unrendered.
    pub unmatched: Vec<String>,
    pub baseline_source: String,
    pub candidate_source: String,
    /// The Engine checkout both sources are commits of, for the review's
    /// `git log` and `git diff`.
    pub checkout: Option<String>,
    pub started_at: u64,
    pub verdict: Verdict,
    pub scenes: Vec<SceneResult>,
}

/// What a run compares.
pub struct RunRequest<'a> {
    pub root: &'a Path,
    pub machine: &'a str,
    pub candidate: &'a Path,
    pub candidate_source: &'a str,
    pub out: &'a Path,
    /// Only the scenes named here, when not empty.
    pub only: &'a [String],
    /// Overrides every scene's repeat count.
    pub repeats: Option<u32>,
    /// The Engine checkout the sources are commits of, when they are.
    pub checkout: Option<&'a Path>,
}

pub fn run(request: &RunRequest<'_>) -> Result<RunReport, String> {
    let manifest = LaneManifest::read(request.root)?;
    let baseline = MachineBaseline::read(request.root, request.machine)?;
    let baseline_renderer = baseline.renderer_path(request.root);
    fs::create_dir_all(request.out)
        .map_err(|error| format!("{}: {error}", request.out.display()))?;
    let started_at = unix_seconds();
    let mut adapters = Vec::new();
    let mut software_adapter = false;
    let mut scenes = Vec::new();
    let unmatched: Vec<String> = request
        .only
        .iter()
        .filter(|name| !manifest.scenes.iter().any(|scene| &scene.name == *name))
        .cloned()
        .collect();
    for scene in &manifest.scenes {
        if !request.only.is_empty() && !request.only.contains(&scene.name) {
            continue;
        }
        let mut settings = manifest.settings_for(scene)?;
        if let Some(repeats) = request.repeats {
            settings.repeats = repeats;
        }
        let result = run_scene(
            request,
            scene,
            &settings,
            &baseline_renderer,
            &mut adapters,
            &mut software_adapter,
        )?;
        eprintln!("rusty-gpu-lane: {} {}", scene.name, result.verdict.word());
        scenes.push(result);
    }
    // Nothing rendered, or a name that matched nothing, fails: an empty run
    // is no evidence.
    let verdict = if scenes.is_empty() || !unmatched.is_empty() {
        Verdict::Failed
    } else {
        scenes
            .iter()
            .map(|scene| scene.verdict)
            .max()
            .unwrap_or(Verdict::Failed)
    };
    let report = RunReport {
        machine: request.machine.to_owned(),
        adapter: adapters.join(" / "),
        software_adapter,
        unmatched,
        baseline_source: baseline.source,
        candidate_source: request.candidate_source.to_owned(),
        checkout: request.checkout.map(|path| path.display().to_string()),
        started_at,
        verdict,
        scenes,
    };
    write_reports(&report, request.out)?;
    Ok(report)
}

fn run_scene(
    request: &RunRequest<'_>,
    scene: &LaneScene,
    settings: &LaneSettings,
    baseline_renderer: &Path,
    adapters: &mut Vec<String>,
    software_adapter: &mut bool,
) -> Result<SceneResult, String> {
    let directory = request.out.join(&scene.name);
    fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let snapshot = request.root.join(&scene.file);
    let repeats = settings.repeats.max(1);
    let mut runs: [Vec<RenderRun>; 2] = [Vec::new(), Vec::new()];
    let mut images: [Vec<PathBuf>; 2] = [Vec::new(), Vec::new()];
    let mut reasons = Vec::new();
    let mut failed = [false, false];
    let renderers = [baseline_renderer, request.candidate];
    let labels = ["baseline", "candidate"];
    for repeat in 0..repeats {
        // Alternate which renderer goes first, so warm-up and load drift
        // fall on both.
        let order: [usize; 2] = if repeat % 2 == 0 { [0, 1] } else { [1, 0] };
        for which in order {
            if failed[which] {
                continue;
            }
            let image = directory.join(format!("{}-{repeat}.png", labels[which]));
            match render(renderers[which], &snapshot, &image, settings, &scene.args) {
                Ok(run) => {
                    // Every render counts: a candidate falling back to a
                    // software adapter after a hardware baseline is caught.
                    if !adapters.contains(&run.adapter) {
                        adapters.push(run.adapter.clone());
                    }
                    *software_adapter |= run.software;
                    runs[which].push(run);
                    images[which].push(image);
                }
                Err(error) => {
                    failed[which] = true;
                    reasons.push(format!("{} failed: {error}", labels[which]));
                }
            }
        }
    }
    let path_text = |path: &Path| path.display().to_string();
    let baseline_image = directory.join("baseline.png");
    let candidate_image = directory.join("candidate.png");
    let diff_image = directory.join("diff.png");
    let mut result = SceneResult {
        name: scene.name.clone(),
        notes: scene.notes.clone(),
        verdict: Verdict::Pass,
        reasons: Vec::new(),
        image: None,
        self_noise_share: 0.0,
        frame: None,
        passes: Vec::new(),
        baseline_image: path_text(&baseline_image),
        candidate_image: path_text(&candidate_image),
        diff_image: path_text(&diff_image),
    };
    if failed[1] {
        result.verdict = Verdict::Failed;
        result.reasons = reasons;
        return Ok(result);
    }
    if failed[0] {
        // A baseline that cannot render this scene (a newer snapshot format,
        // a scene added since) is not the candidate's fault, but nothing was
        // compared: someone must look.
        result.verdict = Verdict::Flagged;
        result.reasons = reasons;
        if let Some(first) = images[1].first() {
            let _ = fs::copy(first, &candidate_image);
        }
        return Ok(result);
    }

    let read = |path: &PathBuf| Image::read_png(path);
    let baseline_first = read(&images[0][0])?;
    let candidate_first = read(&images[1][0])?;
    let mut self_noise = 0.0f64;
    for (first, paths) in [
        (&baseline_first, &images[0]),
        (&candidate_first, &images[1]),
    ] {
        for other in paths.iter().skip(1) {
            let diff = compare(first, &read(other)?, settings.changed_above);
            self_noise = self_noise.max(diff.changed_share);
        }
    }
    let diff = compare(&baseline_first, &candidate_first, settings.changed_above);
    baseline_first.write_png(&baseline_image)?;
    candidate_first.write_png(&candidate_image)?;
    heat_map(&baseline_first, &candidate_first).write_png(&diff_image)?;
    for path in images.iter().flatten() {
        let _ = fs::remove_file(path);
    }
    let share_flag = settings.changed_share_flag.max(self_noise * 2.0);
    if diff.size_mismatch {
        reasons.push("image size differs".to_owned());
    } else if diff.changed_share > share_flag {
        reasons.push(format!(
            "{:.3}% of pixels changed by more than {} levels (max {}, SSIM {:.4})",
            diff.changed_share * 100.0,
            settings.changed_above,
            diff.max_channel,
            diff.ssim
        ));
    } else if diff.ssim < settings.ssim_flag && self_noise == 0.0 {
        reasons.push(format!(
            "SSIM {:.4} below {}",
            diff.ssim, settings.ssim_flag
        ));
    }
    if self_noise > 0.0 {
        reasons.push(format!(
            "not deterministic: repeats differ in {:.3}% of pixels; judged above twice that",
            self_noise * 100.0
        ));
    }

    let frame_ms = |which: usize| {
        runs[which]
            .iter()
            .map(|run| run.frame_ms)
            .collect::<Vec<_>>()
    };
    let frame = TimingDelta::new(
        "frame",
        &frame_ms(0),
        &frame_ms(1),
        settings.frame_ratio_flag,
        settings.frame_delta_flag_ms,
    );
    if frame.flagged {
        reasons.push(format!(
            "frame median {:.3} -> {:.3} ms",
            frame.baseline_ms, frame.candidate_ms
        ));
    }
    let mut names: Vec<String> = runs
        .iter()
        .flatten()
        .flat_map(|run| run.passes.keys().cloned())
        .collect();
    names.sort();
    names.dedup();
    let mut passes = Vec::new();
    for name in names {
        let samples = |which: usize| {
            runs[which]
                .iter()
                .map(|run| run.passes.get(&name).copied().unwrap_or(0.0))
                .collect::<Vec<_>>()
        };
        let delta = TimingDelta::new(
            &name,
            &samples(0),
            &samples(1),
            settings.pass_ratio_flag,
            settings.pass_delta_flag_ms,
        );
        if delta.flagged {
            reasons.push(format!(
                "pass {name} {:.3} -> {:.3} ms",
                delta.baseline_ms, delta.candidate_ms
            ));
        }
        passes.push(delta);
    }
    let image_flagged = diff.size_mismatch
        || diff.changed_share > share_flag
        || (diff.ssim < settings.ssim_flag && self_noise == 0.0);
    let timing_flagged = frame.flagged || passes.iter().any(|pass| pass.flagged);
    result.verdict = if image_flagged || timing_flagged {
        Verdict::Flagged
    } else {
        Verdict::Pass
    };
    result.reasons = reasons;
    result.image = Some(diff);
    result.self_noise_share = self_noise;
    result.frame = Some(frame);
    result.passes = passes;
    Ok(result)
}

fn write_reports(report: &RunReport, out: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    let path = out.join("report.json");
    fs::write(&path, json).map_err(|error| format!("{}: {error}", path.display()))?;
    let path = out.join("report.md");
    fs::write(&path, markdown(report)).map_err(|error| format!("{}: {error}", path.display()))?;
    let flagged: Vec<&SceneResult> = report
        .scenes
        .iter()
        .filter(|scene| scene.verdict != Verdict::Pass)
        .collect();
    if !flagged.is_empty() {
        let path = out.join("review-prompt.md");
        fs::write(&path, review_prompt(report, &flagged))
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

/// The report as a Markdown table: also the check run's summary.
pub fn markdown(report: &RunReport) -> String {
    let mut text = format!(
        "GPU lane on **{}** ({}{}): **{}**\n\nBaseline `{}`, candidate `{}`.\n\n",
        report.machine,
        report.adapter,
        if report.software_adapter {
            ", software adapter: not GPU evidence"
        } else {
            ""
        },
        report.verdict.word(),
        report.baseline_source,
        report.candidate_source
    );
    if !report.unmatched.is_empty() {
        text.push_str(&format!(
            "**No such scene:** {}. The run fails until the selection names only scenes in `scenes.json`.\n\n",
            report.unmatched.join(", ")
        ));
    }
    if report.scenes.is_empty() {
        text.push_str("**Nothing was rendered.**\n\n");
    }
    text.push_str("| Scene | Verdict | Changed | SSIM | Frame ms | Slowest pass change |\n| --- | --- | --- | --- | --- | --- |\n");
    for scene in &report.scenes {
        let (changed, ssim) = scene
            .image
            .map_or(("-".to_owned(), "-".to_owned()), |diff| {
                (
                    format!("{:.3}%", diff.changed_share * 100.0),
                    format!("{:.4}", diff.ssim),
                )
            });
        let frame = scene.frame.as_ref().map_or("-".to_owned(), |frame| {
            format!("{:.2} → {:.2}", frame.baseline_ms, frame.candidate_ms)
        });
        let pass = scene
            .passes
            .iter()
            .max_by(|a, b| {
                (a.candidate_ms - a.baseline_ms).total_cmp(&(b.candidate_ms - b.baseline_ms))
            })
            .map_or("-".to_owned(), |pass| {
                format!(
                    "{} {:+.3} ms",
                    pass.name,
                    pass.candidate_ms - pass.baseline_ms
                )
            });
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            scene.name,
            scene.verdict.word(),
            changed,
            ssim,
            frame,
            pass
        ));
    }
    for scene in report
        .scenes
        .iter()
        .filter(|scene| !scene.reasons.is_empty())
    {
        text.push_str(&format!("\n**{}**\n", scene.name));
        for reason in &scene.reasons {
            text.push_str(&format!("- {reason}\n"));
        }
    }
    text
}

fn review_prompt(report: &RunReport, flagged: &[&SceneResult]) -> String {
    let mut text = format!(
        "# Review GPU lane differences\n\n\
         The GPU verification lane on {machine} ({adapter}) rendered each scene with the machine's \
         accepted baseline renderer (`{baseline}`) and a candidate (`{candidate}`). Rendering is \
         deterministic on one adapter, so the differences below come from the change under test, \
         unless a scene says it is not deterministic.\n\n\
         For each scene, open the baseline, candidate and diff images (the diff shows the baseline \
         in dim grey and changed pixels from dark red, a level or two, to white, 32 levels or more). \
         Read the change's commit messages and diff (`git log` and `git diff` between the two \
         sources). Then classify the scene:\n\n\
         - **intended**: the change says it alters this look or cost, and the images show that;\n\
         - **regression**: something changed that the change does not account for (missing or \
         black geometry, wrong colours, shadows or lighting gone, artifacts, a cost the change \
         does not explain);\n\
         - **noise**: a difference too small or too scattered to matter.\n\n\
         Answer with one line per scene, `name: intended|regression|noise - evidence`, naming \
         where in the image you looked. Do not change files.\n\n",
        machine = report.machine,
        adapter = report.adapter,
        baseline = report.baseline_source,
        candidate = report.candidate_source,
    );
    if let (Some(checkout), Some(base), Some(head)) = (
        report.checkout.as_deref(),
        commit_of(&report.baseline_source),
        commit_of(&report.candidate_source),
    ) {
        text.push_str(&format!(
            "The change under test, read-only:\n\n```sh\ngit -C {checkout} log --format='%H%n%B' {base}..{head}\ngit -C {checkout} diff {base} {head}\n```\n\n"
        ));
    }
    for scene in flagged {
        text.push_str(&format!("## {}\n\n", scene.name));
        if !scene.notes.is_empty() {
            text.push_str(&format!("{}\n\n", scene.notes));
        }
        for reason in &scene.reasons {
            text.push_str(&format!("- {reason}\n"));
        }
        text.push_str(&format!(
            "\n- baseline: `{}`\n- candidate: `{}`\n- diff: `{}`\n\n",
            scene.baseline_image, scene.candidate_image, scene.diff_image
        ));
    }
    text
}

/// The check run's conclusion for a run: `requested` (a reviewer's), or
/// from the verdict (an unreviewed flag is neutral). A run on a software
/// adapter is never success: it is not GPU evidence.
pub fn conclusion(
    verdict: &str,
    software_adapter: bool,
    requested: Option<&str>,
) -> Result<&'static str, String> {
    let wanted = match requested {
        Some("success") => "success",
        Some("failure") => "failure",
        Some("neutral") => "neutral",
        Some(other) => return Err(format!("--conclusion {other}: success, failure or neutral")),
        None => match verdict {
            "pass" => "success",
            "flagged" => "neutral",
            _ => "failure",
        },
    };
    if software_adapter && wanted == "success" {
        if requested.is_some() {
            return Err(
                "the run used a software adapter: it is not GPU evidence and cannot conclude success"
                    .to_owned(),
            );
        }
        return Ok("neutral");
    }
    Ok(wanted)
}

/// The commit a source names, when it starts with a full SHA.
fn commit_of(source: &str) -> Option<&str> {
    let first = source.split_whitespace().next()?;
    (first.len() == 40 && first.chars().all(|c| c.is_ascii_hexdigit())).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_settings_override_the_defaults() {
        let manifest: LaneManifest = serde_json::from_str(
            r#"{"defaults":{"frames":10},"scenes":[{"name":"a","file":"a.rscene","settings":{"turn":0,"changedShareFlag":0.01}}]}"#,
        )
        .unwrap();
        let settings = manifest.settings_for(&manifest.scenes[0]).unwrap();
        assert_eq!(settings.frames, 10);
        assert_eq!(settings.turn, 0.0);
        assert_eq!(settings.changed_share_flag, 0.01);
        assert_eq!(settings.width, 1280);
    }

    #[test]
    fn an_unknown_scene_setting_is_refused() {
        let manifest: LaneManifest = serde_json::from_str(
            r#"{"scenes":[{"name":"a","file":"a.rscene","settings":{"frame":3}}]}"#,
        )
        .unwrap();
        assert!(manifest.settings_for(&manifest.scenes[0]).is_err());
    }

    #[test]
    fn timings_flag_only_when_slower_by_both_ratio_and_amount() {
        let flag = |baseline: &[f64], candidate: &[f64]| {
            TimingDelta::new("p", baseline, candidate, 1.15, 0.05).flagged
        };
        assert!(!flag(&[0.01], &[0.03]));
        assert!(!flag(&[2.0], &[2.1]));
        assert!(flag(&[0.2, 0.21, 0.19], &[0.3, 0.29, 0.31]));
        assert!(!flag(&[0.3], &[0.2]));
        // One slow candidate repeat among fast ones does not flag.
        assert!(!flag(&[0.2, 0.21, 0.26], &[0.3, 0.29, 0.22]));
    }

    #[test]
    fn a_source_names_its_commit_by_a_full_sha() {
        let sha = "960a946b9be39b65550813ebf2e550e8765304d2";
        assert_eq!(commit_of(&format!("{sha} (main)")), Some(sha));
        assert_eq!(commit_of("pair 0.1.0-dev.60822a9d4389"), None);
        assert_eq!(commit_of("960a946b9"), None);
    }

    #[test]
    fn medians_take_the_middle() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(median(&mut []), 0.0);
    }

    /// A lane directory with one scene, `a`, and a baseline renderer that
    /// reports `baseline_adapter`; returns it and a candidate that reports
    /// `candidate_adapter`. Each renderer is a script that copies a small
    /// PNG to its output and prints a report.
    #[cfg(unix)]
    fn stub_lane(
        name: &str,
        baseline_adapter: (&str, bool),
        candidate_adapter: (&str, bool),
    ) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("render-verify-lane-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("scenes")).unwrap();
        fs::write(root.join("scenes/a.rscene"), b"{}").unwrap();
        fs::write(
            root.join(MANIFEST_FILE),
            r#"{"defaults":{"repeats":2},"scenes":[{"name":"a","file":"scenes/a.rscene"}]}"#,
        )
        .unwrap();
        let png = root.join("frame.png");
        Image::new(8, 8, [40, 80, 120, 255])
            .write_png(&png)
            .unwrap();
        let stub = |file: &str, (adapter, software): (&str, bool)| {
            let path = root.join(file);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\ncp '{}' \"$2\"\nprintf '%s' '{{\"adapter\":\"{adapter}\",\"softwareAdapter\":{software},\"timing\":{{\"medianMs\":1.0}},\"gpu\":{{\"passes\":[]}}}}'\n",
                    png.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let baseline = stub("baseline.sh", baseline_adapter);
        let candidate = stub("candidate.sh", candidate_adapter);
        accept(&root, "m", &baseline, "base", "test").unwrap();
        (root, candidate)
    }

    #[cfg(unix)]
    fn run_stub(root: &Path, candidate: &Path, only: &[String]) -> RunReport {
        run(&RunRequest {
            root,
            machine: "m",
            candidate,
            candidate_source: "head",
            out: &root.join("out"),
            only,
            repeats: None,
            checkout: None,
        })
        .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn a_selection_that_names_no_scene_fails_and_names_what_it_missed() {
        let gpu = ("Radeon", false);
        let (root, candidate) = stub_lane("selection", gpu, gpu);
        let everything = run_stub(&root, &candidate, &[]);
        assert_eq!(everything.verdict, Verdict::Pass, "the stubs agree");
        let unknown = run_stub(&root, &candidate, &["does-not-exist".to_owned()]);
        assert_eq!(
            unknown.verdict,
            Verdict::Failed,
            "nothing rendered is no pass"
        );
        assert!(unknown.scenes.is_empty());
        assert_eq!(unknown.unmatched, ["does-not-exist"]);
        assert!(markdown(&unknown).contains("does-not-exist"));
        let mixed = run_stub(&root, &candidate, &["a".to_owned(), "aa".to_owned()]);
        assert_eq!(mixed.scenes.len(), 1, "the named scene still renders");
        assert_eq!(mixed.scenes[0].verdict, Verdict::Pass);
        assert_eq!(
            mixed.verdict,
            Verdict::Failed,
            "but the misspelt one fails the run"
        );
        assert_eq!(mixed.unmatched, ["aa"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_software_adapter_on_either_side_is_reported_and_never_concludes_success() {
        let gpu = ("Radeon", false);
        let software = ("llvmpipe", true);
        for (name, baseline, candidate) in
            [("both", software, software), ("fallback", gpu, software)]
        {
            let (root, stub) = stub_lane(name, baseline, candidate);
            let report = run_stub(&root, &stub, &[]);
            assert_eq!(report.verdict, Verdict::Pass, "{name}: the images agree");
            assert!(
                report.software_adapter,
                "{name}: the software render is kept"
            );
            if name == "fallback" {
                assert_eq!(
                    report.adapter, "Radeon / llvmpipe",
                    "both adapters are named"
                );
            }
            assert_eq!(
                conclusion(report.verdict.word(), report.software_adapter, None),
                Ok("neutral"),
                "{name}: a software run posts no success"
            );
            assert!(
                conclusion(
                    report.verdict.word(),
                    report.software_adapter,
                    Some("success")
                )
                .is_err(),
                "{name}: nor can a reviewer post one"
            );
            let _ = fs::remove_dir_all(&root);
        }
        assert_eq!(conclusion("pass", false, None), Ok("success"));
        assert_eq!(conclusion("flagged", false, None), Ok("neutral"));
        assert_eq!(conclusion("flagged", false, Some("success")), Ok("success"));
        assert_eq!(conclusion("pass", true, Some("failure")), Ok("failure"));
    }
}
