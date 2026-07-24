use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use voxel_asset::VoxelConversionRequest;

use crate::{convert_glb, ConversionError, ConversionReceipt};

pub const MAX_CONVERSION_REQUEST_BYTES: usize = 1024 * 1024;

pub fn decode_conversion_request(input: &str) -> Result<VoxelConversionRequest, ConversionError> {
    if input.len() > MAX_CONVERSION_REQUEST_BYTES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "$",
            format!(
                "request has {} bytes; limit is {MAX_CONVERSION_REQUEST_BYTES}",
                input.len()
            ),
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let request = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        ConversionError::one(
            "conversion.requestDecode",
            json_path(&error.path().to_string()),
            error.inner().to_string(),
        )
    })?;
    deserializer.end().map_err(|error| {
        ConversionError::one(
            "conversion.requestDecode",
            "$",
            format!(
                "{} at line {}, column {}",
                error,
                error.line(),
                error.column()
            ),
        )
    })?;
    Ok(request)
}

pub fn convert_and_install(
    request: &VoxelConversionRequest,
    source: &[u8],
    output: &Path,
) -> Result<ConversionReceipt, ConversionError> {
    let receipt = convert_glb(request, source)?;
    install(&receipt.canonical_json, output)?;
    Ok(receipt)
}

fn install(contents: &str, output: &Path) -> Result<(), ConversionError> {
    let pending = pending_path(output)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&pending)
            .map_err(|error| io_error(&pending, "open pending output", error))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| io_error(&pending, "write pending output", error))?;
        file.sync_all()
            .map_err(|error| io_error(&pending, "sync pending output", error))?;
        fs::rename(&pending, output)
            .map_err(|error| io_error(output, "install converted output", error))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

fn pending_path(output: &Path) -> Result<PathBuf, ConversionError> {
    let Some(file_name) = output.file_name() else {
        return Err(ConversionError::one(
            "conversion.io",
            output.display().to_string(),
            "output path must name a file",
        ));
    };
    let mut pending_name = file_name.to_os_string();
    pending_name.push(".pending");
    Ok(output.with_file_name(pending_name))
}

fn io_error(path: &Path, operation: &str, error: std::io::Error) -> ConversionError {
    ConversionError::one(
        "conversion.io",
        path.display().to_string(),
        format!("failed to {operation}: {error}"),
    )
}

fn json_path(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}
