//! Bounded build-time Product Kernel capability discovery.
//!
//! A product kernel is ordinary source-linked Rust, so reading its capability
//! declaration means compiling that source against the exact Engine facade and
//! calling the fixed `RustyProductRuntime::capabilities` contract.  This module
//! deliberately never scans Rust text for declarations or accepts a registry,
//! callback, or dynamic library.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use product_model::{
    CapabilityAccess, CapabilityAvailability, CapabilityBudget, CapabilityKind, CapabilityMetadata,
    CapabilityProvenance, CapabilityUses, ProductKernelCapabilityDescriptor, ProductPath,
    MAX_CAPABILITY_PROVENANCE_BYTES, MAX_PRODUCT_KERNEL_CAPABILITIES,
};
use serde::Deserialize;

const MAX_PROBE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_PROBE_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PROBE_STRING_BYTES: usize = 512;
const PROBE_SCHEMA: &str = "rusty.product-kernel-capability-probe.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KernelProbeError {
    code: &'static str,
    path: String,
    message: String,
}

impl KernelProbeError {
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for KernelProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for KernelProbeError {}

fn fail(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> KernelProbeError {
    KernelProbeError {
        code,
        path: path.into(),
        message: bound(message.into()),
    }
}

fn bound(value: String) -> String {
    if value.len() <= MAX_PROBE_OUTPUT_BYTES {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = MAX_PROBE_OUTPUT_BYTES - ELLIPSIS.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &value[..end])
}

/// Runs one bounded subprocess without a shell.  Both output streams are
/// drained concurrently, so a noisy compiler cannot deadlock the caller; an
/// overflow or timeout terminates the complete owned process group before
/// returning, so a compiler descendant cannot retain either diagnostic pipe.
pub(crate) fn run_bounded(
    mut command: Command,
    timeout: Duration,
) -> Result<Output, KernelProbeError> {
    if timeout.is_zero() || timeout > MAX_PROBE_TIMEOUT {
        return Err(fail(
            "RUSTY_PROCESS_TIMEOUT_BOUNDS",
            "process",
            "subprocess timeout must be within 1ms..=120s",
        ));
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|error| {
        fail(
            "RUSTY_PROCESS_START",
            "process",
            format!("could not start bounded subprocess: {error}"),
        )
    })?;
    let process_group = child.id();
    let mut stdout = child.stdout.take().expect("stdout pipe configured");
    let mut stderr = child.stderr.take().expect("stderr pipe configured");
    let (overflow_sender, overflow_receiver) = mpsc::channel();
    let stdout_sender = overflow_sender.clone();
    let stdout_reader = thread::spawn(move || read_bounded_stream(&mut stdout, stdout_sender));
    let stderr_reader = thread::spawn(move || read_bounded_stream(&mut stderr, overflow_sender));

    let started = Instant::now();
    loop {
        if overflow_receiver.try_recv().is_ok() {
            terminate_process_group(&mut child, process_group);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(fail(
                "RUSTY_PROCESS_OUTPUT_BOUNDS",
                "process",
                "subprocess stdout or stderr exceeded the 64 KiB diagnostic bound",
            ));
        }
        let observed = match child.try_wait() {
            Ok(observed) => observed,
            Err(error) => {
                terminate_process_group(&mut child, process_group);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(fail(
                    "RUSTY_PROCESS_WAIT",
                    "process",
                    format!("could not observe bounded subprocess: {error}"),
                ));
            }
        };
        match observed {
            Some(status) => {
                terminate_process_group(&mut child, process_group);
                let stdout = stdout_reader.join().unwrap_or_default();
                let stderr = stderr_reader.join().unwrap_or_default();
                if stdout.len() > MAX_PROBE_OUTPUT_BYTES || stderr.len() > MAX_PROBE_OUTPUT_BYTES {
                    return Err(fail(
                        "RUSTY_PROCESS_OUTPUT_BOUNDS",
                        "process",
                        "subprocess stdout or stderr exceeded the 64 KiB diagnostic bound",
                    ));
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            None if started.elapsed() > timeout => {
                terminate_process_group(&mut child, process_group);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(fail(
                    "RUSTY_PROCESS_TIMEOUT",
                    "process",
                    "subprocess exceeded its fixed 120 second timeout",
                ));
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn terminate_process_group(child: &mut std::process::Child, process_group: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("pkill")
            .arg("-KILL")
            .arg("-g")
            .arg(process_group.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn read_bounded_stream(stream: &mut impl Read, overflow: mpsc::Sender<()>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = stream
        .take((MAX_PROBE_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes);
    if bytes.len() > MAX_PROBE_OUTPUT_BYTES {
        let _ = overflow.send(());
    }
    bytes
}

/// Compiles and runs a fixed one-shot probe that invokes the source-linked
/// `RustyProductRuntime::capabilities` method.  It is intentionally a build
/// operation, never a generated-product runtime dependency.
pub(crate) fn probe_capabilities(
    product_root: &Path,
    kernel_entry: &ProductPath,
    engine_facade: &Path,
) -> Result<Vec<ProductKernelCapabilityDescriptor>, KernelProbeError> {
    let kernel_path = checked_kernel_entry(product_root, kernel_entry)?;
    let facade = fs::canonicalize(engine_facade).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_ENGINE",
            engine_facade.display().to_string(),
            format!("cannot resolve the current Rusty Engine facade for Product Kernel probing: {error}"),
        )
    })?;
    if !facade.join("Cargo.toml").is_file() {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_ENGINE",
            facade.display().to_string(),
            "current Rusty Engine facade has no Cargo.toml; rebuild the Engine checkout before probing Product Kernel capabilities",
        ));
    }

    let temporary = tempfile::Builder::new()
        .prefix("rusty-product-kernel-probe-")
        .tempdir()
        .map_err(|error| {
            fail(
                "RUSTY_KERNEL_PROBE_TEMP",
                "kernel.entry",
                format!("cannot create isolated Product Kernel probe: {error}"),
            )
        })?;
    let result = probe_in(temporary.path(), &kernel_path, &facade);
    let cleanup = temporary.close();
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(fail(
            "RUSTY_KERNEL_PROBE_CLEANUP",
            "kernel.entry",
            format!("Product Kernel probe succeeded but its temporary closure could not be removed: {error}"),
        )),
        (Err(error), Err(cleanup)) => Err(fail(
            "RUSTY_KERNEL_PROBE_CLEANUP",
            "kernel.entry",
            format!("{error}; temporary probe cleanup also failed: {cleanup}"),
        )),
    }
}

fn checked_kernel_entry(
    product_root: &Path,
    kernel_entry: &ProductPath,
) -> Result<PathBuf, KernelProbeError> {
    let root = fs::canonicalize(product_root).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_ROOT",
            product_root.display().to_string(),
            format!("cannot resolve product root for Product Kernel probing: {error}"),
        )
    })?;
    let path = product_root.join(kernel_entry.as_str());
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_ENTRY",
            kernel_entry.as_str(),
            format!("cannot inspect declared Product Kernel source: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_ENTRY",
            kernel_entry.as_str(),
            "Product Kernel entry must be a regular non-symlink Rust source file",
        ));
    }
    let canonical = fs::canonicalize(&path).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_ENTRY",
            kernel_entry.as_str(),
            format!("cannot resolve declared Product Kernel source: {error}"),
        )
    })?;
    if !canonical.starts_with(&root) {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_ENTRY",
            kernel_entry.as_str(),
            "Product Kernel entry resolves outside the explicit product root",
        ));
    }
    Ok(canonical)
}

fn probe_in(
    temporary: &Path,
    kernel_entry: &Path,
    engine_facade: &Path,
) -> Result<Vec<ProductKernelCapabilityDescriptor>, KernelProbeError> {
    let source = temporary.join("src");
    fs::create_dir_all(&source).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_WRITE",
            "kernel.entry",
            format!("cannot create Product Kernel probe source: {error}"),
        )
    })?;
    fs::write(temporary.join("Cargo.toml"), probe_manifest(engine_facade)?).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_WRITE",
            "kernel.entry",
            format!("cannot write Product Kernel probe manifest: {error}"),
        )
    })?;
    fs::write(source.join("main.rs"), probe_source(kernel_entry)?).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_WRITE",
            "kernel.entry",
            format!("cannot write Product Kernel probe source: {error}"),
        )
    })?;

    let mut command = Command::new("cargo");
    let target_directory = engine_target_directory(engine_facade)?;
    command
        .arg("run")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(temporary.join("Cargo.toml"))
        .current_dir(temporary)
        // The probe package itself remains disposable, but sharing the exact
        // current Engine checkout's Cargo target avoids recompiling the whole
        // facade (including optional desktop transitive crates) for every
        // `rusty check` or `rusty build` invocation.
        .env("CARGO_TARGET_DIR", target_directory)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, MAX_PROBE_TIMEOUT)?;
    if !output.status.success() {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_COMPILE",
            "kernel.entry",
            format!(
                "Product Kernel capability probe could not compile or run the fixed RustyProductRuntime contract: {}",
                text(&output.stderr)
            ),
        ));
    }
    decode_probe_output(&output.stdout)
}

fn engine_target_directory(engine_facade: &Path) -> Result<PathBuf, KernelProbeError> {
    let root = engine_facade.ancestors().nth(3).ok_or_else(|| {
        fail(
            "RUSTY_KERNEL_PROBE_ENGINE",
            engine_facade.display().to_string(),
            "current Rusty Engine facade does not have the expected checkout layout",
        )
    })?;
    let target = root.join("target");
    if !target.is_dir() {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_ENGINE",
            target.display().to_string(),
            "current Rusty Engine checkout has no Cargo target directory; build the Engine before probing Product Kernel capabilities",
        ));
    }
    Ok(target)
}

fn probe_manifest(engine_facade: &Path) -> Result<String, KernelProbeError> {
    let facade = engine_facade.to_str().ok_or_else(|| {
        fail(
            "RUSTY_KERNEL_PROBE_ENGINE",
            engine_facade.display().to_string(),
            "current Rusty Engine facade path must be UTF-8 for the bounded probe manifest",
        )
    })?;
    Ok(format!(
        "[package]\nname = \"rusty-product-kernel-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nrusty-engine = {{ path = {} }}\nserde_json = \"1\"\n",
        rust_string(facade)
    ))
}

fn probe_source(kernel_entry: &Path) -> Result<String, KernelProbeError> {
    let kernel = kernel_entry.to_str().ok_or_else(|| {
        fail(
            "RUSTY_KERNEL_PROBE_ENTRY",
            kernel_entry.display().to_string(),
            "Product Kernel entry path must be UTF-8 for the bounded probe source",
        )
    })?;
    Ok(format!(
        r#"#![forbid(unsafe_code)]

#[path = {kernel}]
mod product_kernel_source;

use rusty_engine::{{
    product_kernel::ProductKernelRuntimeDefinition,
    product_model::CapabilityUse,
}};

fn main() {{
    let capabilities = <product_kernel_source::RustyProductRuntime as ProductKernelRuntimeDefinition>::capabilities()
        .iter()
        .copied()
        .map(|capability| {{
            let metadata = capability.metadata();
            let availability = metadata.availability();
            serde_json::json!({{
                "identity": capability.identity(),
                "kind": metadata.kind().as_str(),
                "inputMap": metadata.uses().contains(CapabilityUse::InputMap),
                "schedule": metadata.uses().contains(CapabilityUse::Schedule),
                "timeline": metadata.uses().contains(CapabilityUse::Timeline),
                "availability": availability.as_str(),
                "unavailableReason": availability.reason(),
                "reads": metadata.access().reads(),
                "writes": metadata.access().writes(),
                "maximumCompactJsonPayloadBytes": metadata.budget().maximum_compact_json_payload_bytes(),
                "owner": metadata.provenance().owner(),
                "source": metadata.provenance().source(),
                "logicalPath": metadata.provenance().logical_path(),
            }})
        }})
        .collect::<Vec<_>>();
    println!("{{}}", serde_json::json!({{
        "schema": "{schema}",
        "capabilities": capabilities,
    }}));
}}
"#,
        kernel = rust_string(kernel),
        schema = PROBE_SCHEMA,
    ))
}

fn rust_string(value: &str) -> String {
    format!("\"{}\"", value.escape_default())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeOutput {
    schema: String,
    capabilities: Vec<ProbeCapability>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeCapability {
    identity: String,
    kind: String,
    input_map: bool,
    schedule: bool,
    timeline: bool,
    availability: String,
    unavailable_reason: Option<String>,
    reads: Vec<String>,
    writes: Vec<String>,
    maximum_compact_json_payload_bytes: usize,
    owner: String,
    source: String,
    logical_path: String,
}

fn decode_probe_output(
    bytes: &[u8],
) -> Result<Vec<ProductKernelCapabilityDescriptor>, KernelProbeError> {
    if bytes.len() > MAX_PROBE_OUTPUT_BYTES {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            "kernel.entry",
            "Product Kernel capability probe exceeded its 64 KiB result bound",
        ));
    }
    let output: ProbeOutput = serde_json::from_slice(bytes).map_err(|error| {
        fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            "kernel.entry",
            format!("Product Kernel capability probe did not emit one valid bounded JSON descriptor: {error}"),
        )
    })?;
    if output.schema != PROBE_SCHEMA {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            "kernel.entry",
            "Product Kernel capability probe schema did not match this Rusty CLI",
        ));
    }
    if output.capabilities.len() > MAX_PRODUCT_KERNEL_CAPABILITIES {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            "kernel.entry",
            format!(
                "Product Kernel declares more than {MAX_PRODUCT_KERNEL_CAPABILITIES} capabilities"
            ),
        ));
    }
    output
        .capabilities
        .into_iter()
        .enumerate()
        .map(|(index, capability)| descriptor_from_probe(index, capability))
        .collect()
}

fn descriptor_from_probe(
    index: usize,
    capability: ProbeCapability,
) -> Result<ProductKernelCapabilityDescriptor, KernelProbeError> {
    let path = format!("kernel.capabilities[{index}]");
    bounded_field(
        &capability.identity,
        &path,
        "identity",
        MAX_PROBE_STRING_BYTES,
    )?;
    bounded_field(
        &capability.owner,
        &path,
        "owner",
        MAX_CAPABILITY_PROVENANCE_BYTES,
    )?;
    bounded_field(
        &capability.source,
        &path,
        "source",
        MAX_CAPABILITY_PROVENANCE_BYTES,
    )?;
    bounded_field(
        &capability.logical_path,
        &path,
        "logicalPath",
        MAX_CAPABILITY_PROVENANCE_BYTES,
    )?;
    let kind = match capability.kind.as_str() {
        "system" => CapabilityKind::System,
        "operation" => CapabilityKind::Operation,
        "query" => CapabilityKind::Query,
        "projection" => CapabilityKind::Projection,
        "migration" => CapabilityKind::Migration,
        _ => {
            return Err(fail(
                "RUSTY_KERNEL_PROBE_OUTPUT",
                &path,
                "Product Kernel capability kind must be one of system, operation, query, projection, or migration",
            ))
        }
    };
    let mut uses = CapabilityUses::NONE;
    if capability.input_map {
        uses = uses.union(CapabilityUses::INPUT_MAP);
    }
    if capability.schedule {
        uses = uses.union(CapabilityUses::SCHEDULE);
    }
    if capability.timeline {
        uses = uses.union(CapabilityUses::TIMELINE);
    }
    let availability = match capability.availability.as_str() {
        "linkable" if capability.unavailable_reason.is_none() => CapabilityAvailability::Linkable,
        "unavailable" => {
            let reason = capability.unavailable_reason.ok_or_else(|| {
                fail(
                    "RUSTY_KERNEL_PROBE_OUTPUT",
                    &path,
                    "unavailable Product Kernel capability is missing its bounded reason",
                )
            })?;
            bounded_field(&reason, &path, "unavailableReason", MAX_PROBE_STRING_BYTES)?;
            CapabilityAvailability::Unavailable {
                reason: leak(reason),
            }
        }
        _ => return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            &path,
            "Product Kernel capability availability must be linkable or unavailable with a reason",
        )),
    };
    let reads = leak_strings(capability.reads, &path, "reads")?;
    let writes = leak_strings(capability.writes, &path, "writes")?;
    if capability.maximum_compact_json_payload_bytes == 0 {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            &path,
            "Product Kernel capability payload budget must be greater than zero",
        ));
    }
    Ok(ProductKernelCapabilityDescriptor::new(
        leak(capability.identity),
        CapabilityMetadata::new(
            kind,
            uses,
            availability,
            CapabilityAccess::new(reads, writes),
            CapabilityBudget::new(capability.maximum_compact_json_payload_bytes),
            CapabilityProvenance::new(
                leak(capability.owner),
                leak(capability.source),
                leak(capability.logical_path),
            ),
        ),
    ))
}

fn bounded_field(
    value: &str,
    path: &str,
    field: &str,
    maximum: usize,
) -> Result<(), KernelProbeError> {
    if value.is_empty()
        || value.len() > maximum
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            path,
            format!("Product Kernel capability {field} must contain 1..={maximum} non-control UTF-8 bytes"),
        ));
    }
    Ok(())
}

fn leak_strings(
    values: Vec<String>,
    path: &str,
    field: &str,
) -> Result<&'static [&'static str], KernelProbeError> {
    if values.len() > 128 {
        return Err(fail(
            "RUSTY_KERNEL_PROBE_OUTPUT",
            path,
            format!("Product Kernel capability {field} has more than 128 entries"),
        ));
    }
    let mut leaked = Vec::with_capacity(values.len());
    for value in values {
        bounded_field(&value, path, field, MAX_PROBE_STRING_BYTES)?;
        leaked.push(leak(value));
    }
    Ok(Box::leak(leaked.into_boxed_slice()))
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;

    #[test]
    fn bounded_process_reaps_pipe_holding_descendants_after_parent_exit() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "(sleep 30) & exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let output = run_bounded(command, Duration::from_secs(2)).expect("parent exits cleanly");
        assert!(output.status.success());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_process_timeout_reaps_descendant_tree() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "(sleep 30) & wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let error = run_bounded(command, Duration::from_millis(100)).expect_err("must time out");
        assert_eq!(error.code(), "RUSTY_PROCESS_TIMEOUT");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_process_rejects_fast_clean_output_overflow() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "head -c 65537 /dev/zero"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let error = run_bounded(command, Duration::from_secs(2)).expect_err("must reject output");
        assert_eq!(error.code(), "RUSTY_PROCESS_OUTPUT_BOUNDS");
    }
}
