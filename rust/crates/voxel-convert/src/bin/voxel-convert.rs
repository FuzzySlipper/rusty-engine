use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use voxel_convert::{
    convert_and_install, decode_conversion_request, MAX_CONVERSION_REQUEST_BYTES,
    MAX_CONVERSION_SOURCE_BYTES,
};

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
    let request_bytes = read_bounded(
        &arguments.request,
        MAX_CONVERSION_REQUEST_BYTES as u64,
        "request",
    )?;
    let request_text = String::from_utf8(request_bytes)
        .map_err(|_| format!("{}: request is not UTF-8", arguments.request.display()))?;
    let request = decode_conversion_request(&request_text).map_err(|error| error.to_string())?;
    let source = read_bounded(&arguments.source, MAX_CONVERSION_SOURCE_BYTES, "source")?;
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

fn read_bounded(path: &Path, max_bytes: u64, input_name: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "conversion.resourceLimit at {}: {input_name} exceeds the {max_bytes}-byte input limit",
            path.display()
        ));
    }
    Ok(bytes)
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
