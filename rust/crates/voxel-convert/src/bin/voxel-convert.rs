use std::path::PathBuf;
use std::process::ExitCode;

use voxel_convert::{convert_and_install, decode_conversion_request};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let request_text = std::fs::read_to_string(&arguments.request)
        .map_err(|error| format!("{}: {error}", arguments.request.display()))?;
    let request = decode_conversion_request(&request_text).map_err(|error| error.to_string())?;
    let source = std::fs::read(&arguments.source)
        .map_err(|error| format!("{}: {error}", arguments.source.display()))?;
    let receipt = convert_and_install(&request, &source, &arguments.output)
        .map_err(|error| error.to_string())?;
    println!(
        "asset={} sourceSha256={} settingsSha256={} contentHash={} vertices={} triangles={} voxels={} runs={} output={}",
        receipt.asset.asset_id,
        receipt.source_sha256,
        receipt.settings_sha256,
        receipt.content_hash,
        receipt.source_vertices,
        receipt.source_triangles,
        receipt.output_voxels,
        receipt.sparse_runs,
        arguments.output.display()
    );
    Ok(())
}

struct Arguments {
    request: PathBuf,
    source: PathBuf,
    output: PathBuf,
}

impl Arguments {
    fn parse(mut values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut request = None;
        let mut source = None;
        let mut output = None;
        while let Some(flag) = values.next() {
            let value = values
                .next()
                .ok_or_else(|| format!("missing value after {flag}"))?;
            match flag.as_str() {
                "--request" => request = Some(PathBuf::from(value)),
                "--source" => source = Some(PathBuf::from(value)),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {flag}")),
            }
        }
        Ok(Self {
            request: request.ok_or("required --request PATH")?,
            source: source.ok_or("required --source PATH")?,
            output: output.ok_or("required --output PATH")?,
        })
    }
}
