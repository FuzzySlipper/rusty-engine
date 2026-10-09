//! The GPU verification lane (docs/verification.md#gpu-verification).
//!
//! ```text
//! rusty-gpu-lane run     --root DIR --candidate RENDERER --source TEXT --out DIR [--machine NAME] [--scene NAME]... [--repeats N] [--checkout DIR]
//! rusty-gpu-lane accept  --root DIR --renderer RENDERER --source TEXT --reason TEXT [--machine NAME]
//! rusty-gpu-lane status  --run DIR --repo OWNER/NAME --sha SHA [--review FILE] [--conclusion success|failure|neutral]
//! ```
//!
//! `run` renders every scene in `DIR/scenes.json` with this machine's accepted
//! baseline renderer and with the candidate `rusty-scene-render`, interleaved,
//! and writes `report.json`, `report.md`, each scene's baseline, candidate and
//! diff images and, when anything is flagged, `review-prompt.md` for a
//! reviewing agent. It exits 0 on pass, 1 when flagged, 2 when a candidate
//! render failed. `accept` makes a renderer this machine's baseline. `status`
//! posts the run as the GitHub check run `gpu-render/<machine>` on a commit,
//! through the `gpu-lane-status` workflow (a check run needs the Actions
//! token).

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use render_verify::lane::{self, RunRequest, Verdict};

const USAGE: &str = "usage: rusty-gpu-lane run --root DIR --candidate RENDERER --source TEXT --out DIR \
[--machine NAME] [--scene NAME]... [--repeats N] [--checkout DIR]\n       rusty-gpu-lane accept --root DIR --renderer RENDERER \
--source TEXT --reason TEXT [--machine NAME]\n       rusty-gpu-lane status --run DIR --repo OWNER/NAME --sha SHA \
[--review FILE] [--conclusion success|failure|neutral]";

/// The check-run summary GitHub accepts through a workflow input is bounded;
/// the report is cut to this many characters.
const SUMMARY_LIMIT: usize = 6000;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&arguments) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rusty-gpu-lane: {error}");
            ExitCode::from(3)
        }
    }
}

struct Options {
    values: Vec<(String, String)>,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut values = Vec::new();
        let mut iter = arguments.iter();
        while let Some(flag) = iter.next() {
            let name = flag
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument {flag}\n{USAGE}"))?;
            let value = iter
                .next()
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
            values.push((name.to_owned(), value.clone()));
        }
        Ok(Self { values })
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn require(&self, name: &str) -> Result<&str, String> {
        self.get(name)
            .ok_or_else(|| format!("--{name} is required\n{USAGE}"))
    }

    fn all(&self, name: &str) -> Vec<String> {
        self.values
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .collect()
    }
}

/// This machine's name for its baseline: `--machine`, else
/// `RUSTY_GPU_LANE_MACHINE`, else the host name.
fn machine_name(options: &Options) -> String {
    if let Some(name) = options.get("machine") {
        return name.to_owned();
    }
    if let Ok(name) = std::env::var("RUSTY_GPU_LANE_MACHINE") {
        if !name.is_empty() {
            return name;
        }
    }
    for variable in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(name) = std::env::var(variable) {
            if !name.is_empty() {
                return name.to_lowercase();
            }
        }
    }
    Command::new("hostname")
        .output()
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unnamed".to_owned())
}

fn dispatch(arguments: &[String]) -> Result<ExitCode, String> {
    let (command, rest) = arguments.split_first().ok_or_else(|| USAGE.to_owned())?;
    let options = Options::parse(rest)?;
    match command.as_str() {
        "run" => {
            let root = PathBuf::from(options.require("root")?);
            let machine = machine_name(&options);
            let candidate = PathBuf::from(options.require("candidate")?);
            let out = PathBuf::from(options.require("out")?);
            let only = options.all("scene");
            let checkout = options.get("checkout").map(PathBuf::from);
            let repeats = options
                .get("repeats")
                .map(|value| {
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("--repeats {value}: not a count"))
                })
                .transpose()?;
            let report = lane::run(&RunRequest {
                root: &root,
                machine: &machine,
                candidate: &candidate,
                candidate_source: options.require("source")?,
                out: &out,
                only: &only,
                repeats,
                checkout: checkout.as_deref(),
            })?;
            println!("{}", lane::markdown(&report));
            println!("report: {}", out.join("report.md").display());
            if report.verdict != Verdict::Pass {
                println!("review prompt: {}", out.join("review-prompt.md").display());
            }
            Ok(ExitCode::from(match report.verdict {
                Verdict::Pass => 0,
                Verdict::Flagged => 1,
                Verdict::Failed => 2,
            }))
        }
        "accept" => {
            let root = PathBuf::from(options.require("root")?);
            let machine = machine_name(&options);
            let baseline = lane::accept(
                &root,
                &machine,
                Path::new(options.require("renderer")?),
                options.require("source")?,
                options.require("reason")?,
            )?;
            println!(
                "accepted {} as the baseline of {} ({})",
                baseline.source,
                baseline.machine,
                baseline.renderer_path(&root).display()
            );
            Ok(ExitCode::SUCCESS)
        }
        "status" => {
            let run = PathBuf::from(options.require("run")?);
            let text = std::fs::read_to_string(run.join("report.json"))
                .map_err(|error| format!("{}: {error}", run.join("report.json").display()))?;
            let report: serde_json::Value =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            post_status(&options, &run, &report)?;
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn post_status(options: &Options, run: &Path, report: &serde_json::Value) -> Result<(), String> {
    let repo = options.require("repo")?;
    let sha = options.require("sha")?;
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("--sha {sha}: a full 40-character commit SHA"));
    }
    let machine = report["machine"].as_str().unwrap_or("unnamed");
    let verdict = report["verdict"].as_str().unwrap_or("failed");
    // An unreviewed flag is neutral, which a gate requiring success does not
    // pass; a reviewer posts success or failure with --conclusion and the
    // review attached. A software adapter never concludes success.
    let conclusion = lane::conclusion(
        verdict,
        report["softwareAdapter"].as_bool().unwrap_or(true),
        options.get("conclusion"),
    )?;
    let mut summary = std::fs::read_to_string(run.join("report.md")).unwrap_or_default();
    if let Some(review) = options.get("review") {
        let text = std::fs::read_to_string(review).map_err(|error| format!("{review}: {error}"))?;
        summary = format!("{text}\n\n---\n\n{summary}");
    }
    if summary.len() > SUMMARY_LIMIT {
        let mut cut = SUMMARY_LIMIT;
        while !summary.is_char_boundary(cut) {
            cut -= 1;
        }
        summary.truncate(cut);
        summary.push_str("\n\n(cut; the full report is on the lane machine)");
    }
    let name = format!("gpu-render/{machine}");
    let title = format!(
        "{verdict} on {}",
        report["adapter"].as_str().unwrap_or("an unknown adapter")
    );
    let status = Command::new("gh")
        .args(["workflow", "run", "gpu-lane-status.yml", "--repo", repo])
        .args(["-f", &format!("sha={sha}")])
        .args(["-f", &format!("name={name}")])
        .args(["-f", &format!("conclusion={conclusion}")])
        .args(["-f", &format!("title={title}")])
        .args(["-f", &format!("summary={summary}")])
        .status()
        .map_err(|error| format!("gh: {error}"))?;
    if !status.success() {
        return Err(format!("gh workflow run exited {status}"));
    }
    println!("requested check run {name} = {conclusion} on {sha}");
    Ok(())
}
