//! Thin command contracts over the Product Model workflow owners.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use product_model::{
    admit_checked_product_composition, decode_compiled_composition,
    link_admitted_product_composition,
};

use crate::{
    args::OutputFormat,
    check as layout,
    inspect::{self as inspection, InspectSubject, InspectionRequest},
    kernel_probe::run_bounded,
    package as packaging,
    report::{Diagnostic, Fact, Report},
    workflow::{self, BuildProfile, GeneratedBinary},
    Execution, EXIT_CONFORMANCE,
};

const PRODUCT_START_TIMEOUT: Duration = Duration::from_secs(30);
const PRODUCT_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const BROWSER_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_HOST_DIAGNOSTIC_BYTES: usize = 64 * 1024;

pub(crate) fn check(start: PathBuf) -> Execution {
    let layout = layout::check(start.clone());
    if layout.exit_code != 0 {
        return layout;
    }
    let admitted = match workflow::admit_product(&start) {
        Ok(admitted) => admitted,
        Err(diagnostic) => return failed(diagnostic),
    };
    let generated = match workflow::verify_generated_product(&admitted) {
        Ok(value) => value,
        Err(diagnostic) => return failed(diagnostic),
    };
    let generated = generated
        .map(|receipt| format!("current; entries={}", receipt.entries().len()))
        .unwrap_or_else(|| "absent; no generated output was required".to_owned());
    Execution {
        report: Report::success().with_facts(vec![
            Fact::new("check.product", admitted.manifest().product_id()),
            Fact::new("check.authoring", "admitted"),
            Fact::new("check.capabilities", "compiled and linked"),
            Fact::new("check.content", "admitted"),
            Fact::new("check.generatedAssembly", generated),
        ]),
        exit_code: 0,
    }
}

pub(crate) fn doctor(start: PathBuf) -> Execution {
    let mut execution = check(start);
    if execution.exit_code == 0 {
        execution.report = execution.report.with_facts(vec![
            Fact::new("doctor.productModel", "ready"),
            Fact::new("doctor.authoringToolchain", "ready"),
            Fact::new("doctor.productAssembly", "ready"),
            Fact::new("doctor.browserProof", "available through rusty test"),
            Fact::new(
                "doctor.desktopWrapper",
                "selected wrapper realization is a separate workflow owner",
            ),
        ]);
    }
    execution
}

pub(crate) fn inspect(start: PathBuf, subject: &str) -> Execution {
    let subject = match parse_subject(subject) {
        Ok(subject) => subject,
        Err(diagnostic) => return usage_failed(diagnostic),
    };
    let admitted_product = match workflow::admit_product(start) {
        Ok(admitted) => admitted,
        Err(diagnostic) => return failed(diagnostic),
    };
    let compiled = match decode_compiled_composition(admitted_product.compiled_composition()) {
        Ok(compiled) => compiled,
        Err(error) => {
            return failed(Diagnostic::error(
                "RUSTY_INSPECT_COMPOSITION",
                error.diagnostic().path(),
                format!(
                    "Product Model owner could not decode the materialized composition: {}. Remedy: correct the Runtime Composition authoring source and rerun `rusty inspect`.",
                    error.diagnostic().message()
                ),
            ));
        }
    };
    let admitted = match admit_checked_product_composition(admitted_product.manifest(), compiled) {
        Ok(admitted) => admitted,
        Err(error) => return failed(model_error("RUSTY_INSPECT_ADMISSION", error)),
    };
    let linked =
        match link_admitted_product_composition(admitted, admitted_product.kernel_capabilities()) {
            Ok(linked) => linked,
            Err(error) => return failed(model_error("RUSTY_INSPECT_LINKAGE", error)),
        };
    let receipt = match workflow::verify_generated_product(&admitted_product) {
        Ok(receipt) => receipt,
        Err(diagnostic) => return failed(diagnostic),
    };
    let document = match inspection::inspect(
        subject,
        InspectionRequest {
            manifest: admitted_product.manifest(),
            admitted: linked.admitted(),
            linked: Some(&linked),
            assembly_receipt: receipt.as_ref(),
        },
    ) {
        Ok(document) => document,
        Err(error) => {
            let diagnostic = error.diagnostic();
            return failed(Diagnostic::error(
                diagnostic.code,
                diagnostic.source.clone(),
                format!(
                    "{} owner rejected the inspection request. Remedy: {}",
                    diagnostic.owner, diagnostic.remedy
                ),
            ));
        }
    };
    let facts = document
        .facts
        .into_iter()
        .map(|fact| {
            Fact::new(
                fact.path,
                format!(
                    "owner={}; source={}; {}",
                    fact.owner, fact.source, fact.value
                ),
            )
        })
        .collect();
    let diagnostics = document
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            let message = format!(
                "owner={}; source={}; remedy={}",
                diagnostic.owner, diagnostic.source, diagnostic.remedy
            );
            if diagnostic.level == "error" {
                Diagnostic::error(diagnostic.code, diagnostic.source, message)
            } else {
                Diagnostic::note(diagnostic.code, diagnostic.source, message)
            }
        })
        .collect::<Vec<_>>();
    let report = if diagnostics.is_empty() {
        Report::success()
    } else {
        Report::incomplete(diagnostics)
    }
    .with_facts(facts);
    Execution {
        exit_code: if report.has_errors() {
            EXIT_CONFORMANCE
        } else {
            0
        },
        report,
    }
}

pub(crate) fn build(start: PathBuf) -> Execution {
    match workflow::build_product(start, BuildProfile::Debug) {
        Ok(binary) => Execution {
            report: Report::success().with_facts(binary_facts(&binary)),
            exit_code: 0,
        },
        Err(diagnostic) => failed(diagnostic),
    }
}

pub(crate) fn package(start: PathBuf) -> Execution {
    let prepared = match workflow::prepare_product(start) {
        Ok(prepared) => prepared,
        Err(diagnostic) => return failed(diagnostic),
    };
    let binary = match workflow::build_prepared_product(&prepared, BuildProfile::Release) {
        Ok(binary) => binary,
        Err(diagnostic) => return failed(diagnostic),
    };
    let packaged = match packaging::package_product(
        prepared.root(),
        prepared.manifest(),
        binary.path(),
        prepared.receipt(),
    ) {
        Ok(packaged) => packaged,
        Err(error) => return package_failed(error),
    };
    let package_root = prepared.root().join(packaged.package_directory());
    if let Err(error) = packaging::verify_product_package(&package_root) {
        return package_failed(error);
    }
    Execution {
        report: Report::success().with_facts(vec![
            Fact::new("package.product", packaged.product()),
            Fact::new("package.directory", packaged.package_directory()),
            Fact::new("package.files", packaged.files().to_string()),
            Fact::new("package.sha256", packaged.package_sha256()),
            Fact::new(
                "package.desktopHost",
                "not-realized; selected wrapper generation and headed proof are owned by the desktop-wrapper workflow",
            ),
        ]),
        exit_code: 0,
    }
}

pub(crate) fn dev(start: PathBuf, port: u16, format: OutputFormat) -> Execution {
    let binary = match workflow::build_product(start, BuildProfile::Debug) {
        Ok(binary) => binary,
        Err(diagnostic) => return failed(diagnostic),
    };
    if format == OutputFormat::Json {
        return dev_captured(&binary, port);
    }
    let status = Command::new(binary.path())
        .arg("--port")
        .arg(port.to_string())
        .status();
    match status {
        Ok(status) if status.success() => Execution {
            report: Report::success().with_facts(binary_facts(&binary)),
            exit_code: 0,
        },
        Ok(status) => failed(Diagnostic::error(
            "RUSTY_DEV_HOST_EXIT",
            "generated/product-assembly",
            format!(
                "Product Dev Host owner exited with {status}. Remedy: inspect the generated runtime diagnostic and correct the admitted Product Kernel or host configuration."
            ),
        )),
        Err(error) => failed(Diagnostic::error(
            "RUSTY_DEV_HOST_START",
            binary.path().display().to_string(),
            format!(
                "Product Dev Host owner could not start the generated runtime: {error}. Remedy: rerun `rusty build` and verify the generated binary is executable."
            ),
        )),
    }
}

fn dev_captured(binary: &GeneratedBinary, port: u16) -> Execution {
    let host = match start_captured_host(binary, port, "RUSTY_DEV") {
        Ok(host) => host,
        Err(diagnostic) => return failed(diagnostic),
    };
    let origin = host.origin.clone();
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_line(&mut input) {
        drop(host);
        return failed(Diagnostic::error(
            "RUSTY_DEV_INPUT",
            "$stdin",
            format!(
                "Rusty CLI could not read the dev-session shutdown signal: {error}. Remedy: run the command from a terminal or pipe one newline after completing the session."
            ),
        ));
    }
    if let Err(diagnostic) = stop_test_host(host, "RUSTY_DEV") {
        return failed(diagnostic);
    }
    let mut facts = binary_facts(binary);
    facts.push(Fact::new("dev.origin", origin));
    Execution {
        report: Report::success().with_facts(facts),
        exit_code: 0,
    }
}

pub(crate) fn test(start: PathBuf) -> Execution {
    let prepared = match workflow::prepare_product(start) {
        Ok(prepared) => prepared,
        Err(diagnostic) => return failed(diagnostic),
    };
    let binary = match workflow::build_prepared_product(&prepared, BuildProfile::Debug) {
        Ok(binary) => binary,
        Err(diagnostic) => return failed(diagnostic),
    };
    let origin = match start_test_host(&binary) {
        Ok(origin) => origin,
        Err(diagnostic) => return failed(diagnostic),
    };
    if let Err(diagnostic) = browser_test(&origin.origin) {
        let _ = stop_test_host(origin, "RUSTY_TEST");
        return failed(diagnostic);
    }
    if let Err(diagnostic) = stop_test_host(origin, "RUSTY_TEST") {
        return failed(diagnostic);
    }
    let wrapper = if prepared.manifest().wrappers().is_empty() {
        "not-selected"
    } else {
        "selected-policy-present; packaged desktop host proof is a distinct layer"
    };
    Execution {
        report: Report::success().with_facts(vec![
            Fact::new("test.authoring", "pass; materialized and admitted"),
            Fact::new(
                "test.hostNeutral",
                "pass; generated runtime served its admitted bundle",
            ),
            Fact::new(
                "test.browser",
                "pass; real Chromium mounted exactly one Engine canvas",
            ),
            Fact::new("test.packagedHost", wrapper),
        ]),
        exit_code: 0,
    }
}

fn parse_subject(value: &str) -> Result<InspectSubject, Diagnostic> {
    match value {
        "all" => Ok(InspectSubject::All),
        "composition" => Ok(InspectSubject::Composition),
        "input" => Ok(InspectSubject::Input),
        "schedule" => Ok(InspectSubject::Schedule),
        "capability-bindings" => Ok(InspectSubject::CapabilityBindings),
        "timelines" => Ok(InspectSubject::Timelines),
        "lifecycle" => Ok(InspectSubject::Lifecycle),
        "mutation" => Ok(InspectSubject::Mutation),
        _ => Err(Diagnostic::error(
            "RUSTY_INSPECT_SUBJECT",
            "$",
            "inspect subject must be all, composition, input, schedule, capability-bindings, timelines, lifecycle, or mutation",
        )),
    }
}

fn model_error(code: &'static str, error: product_model::ProductModelError) -> Diagnostic {
    let diagnostic = error.diagnostic();
    Diagnostic::error(
        code,
        diagnostic.path(),
        format!(
            "Product Model owner rejected {} at {}: {}. Remedy: correct the authored declaration before inspecting it.",
            diagnostic.code(),
            diagnostic.source(),
            diagnostic.message()
        ),
    )
}

fn binary_facts(binary: &GeneratedBinary) -> Vec<Fact> {
    vec![
        Fact::new("build.profile", binary.profile().directory()),
        Fact::new("build.binary", binary.path().display().to_string()),
    ]
}

fn package_failed(error: packaging::PackageError) -> Execution {
    failed(Diagnostic::error(
        error.code(),
        error.path(),
        format!(
            "Product Package owner rejected the exact runtime closure: {}. Remedy: remove only a conflicting generated/product-package after preserving any needed evidence, then rerun `rusty package`.",
            error.detail()
        ),
    ))
}

struct TestHost {
    child: std::process::Child,
    origin: String,
    stdout_reader: Option<thread::JoinHandle<StreamCapture>>,
    stderr_reader: Option<thread::JoinHandle<StreamCapture>>,
}

fn start_test_host(binary: &GeneratedBinary) -> Result<TestHost, Diagnostic> {
    start_captured_host(binary, 0, "RUSTY_TEST")
}

fn start_captured_host(
    binary: &GeneratedBinary,
    port: u16,
    code_prefix: &str,
) -> Result<TestHost, Diagnostic> {
    let mut child = Command::new(binary.path())
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            Diagnostic::error(
                host_code(code_prefix, "START"),
                binary.path().display().to_string(),
                format!("Product Dev Host owner could not start: {error}. Remedy: rerun `rusty build` and inspect the generated runtime."),
            )
        })?;
    let stdout = child.stdout.take().expect("captured host stdout was piped");
    let stderr = child.stderr.take().expect("captured host stderr was piped");
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || capture_stdout(stdout, sender));
    let stderr_reader = thread::spawn(move || capture_stream(stderr));
    let mut host = TestHost {
        child,
        origin: String::new(),
        stdout_reader: Some(stdout_reader),
        stderr_reader: Some(stderr_reader),
    };
    let origin = match receiver.recv_timeout(PRODUCT_START_TIMEOUT) {
        Ok(Ok(origin)) if origin.trim().starts_with("http://127.0.0.1:") => {
            origin.trim().to_owned()
        }
        Ok(Ok(origin)) => {
            return Err(Diagnostic::error(
                host_code(code_prefix, "ORIGIN"),
                "generated/product-assembly",
                format!("Product Dev Host owner emitted an invalid origin `{}`. Remedy: regenerate the Engine-owned host.", origin.trim()),
            ));
        }
        Ok(Err(error)) => {
            return Err(Diagnostic::error(
                host_code(code_prefix, "OUTPUT"),
                "generated/product-assembly",
                format!("Product Dev Host owner output failed: {error}. Remedy: regenerate and rebuild the host."),
            ));
        }
        Err(_) => {
            return Err(Diagnostic::error(
                host_code(code_prefix, "TIMEOUT"),
                "generated/product-assembly",
                "Product Dev Host owner did not publish its loopback origin within 30 seconds. Remedy: inspect generated runtime startup diagnostics.",
            ));
        }
    };
    host.origin.clone_from(&origin);
    verify_http_root(&origin)?;
    Ok(host)
}

#[derive(Default)]
struct StreamCapture {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn capture_stdout(
    stdout: impl Read,
    origin: mpsc::Sender<std::io::Result<String>>,
) -> StreamCapture {
    let mut reader = BufReader::new(stdout);
    let mut first = Vec::new();
    let first_result = reader
        .by_ref()
        .take((MAX_HOST_DIAGNOSTIC_BYTES + 1) as u64)
        .read_until(b'\n', &mut first)
        .map(|_| String::from_utf8_lossy(&first).into_owned());
    let _ = origin.send(first_result);
    let mut capture = StreamCapture::default();
    append_bounded(&mut capture, &first);
    drain_bounded(&mut reader, &mut capture);
    capture
}

fn capture_stream(stream: impl Read) -> StreamCapture {
    let mut reader = BufReader::new(stream);
    let mut capture = StreamCapture::default();
    drain_bounded(&mut reader, &mut capture);
    capture
}

fn drain_bounded(reader: &mut impl Read, capture: &mut StreamCapture) {
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => append_bounded(capture, &buffer[..count]),
        }
    }
}

fn append_bounded(capture: &mut StreamCapture, bytes: &[u8]) {
    let available = MAX_HOST_DIAGNOSTIC_BYTES.saturating_sub(capture.bytes.len());
    capture
        .bytes
        .extend_from_slice(&bytes[..bytes.len().min(available)]);
    capture.overflowed |= bytes.len() > available;
}

fn verify_http_root(origin: &str) -> Result<(), Diagnostic> {
    let address = origin.strip_prefix("http://").ok_or_else(|| {
        Diagnostic::error(
            "RUSTY_TEST_HOST_ORIGIN",
            "generated/product-assembly",
            "Product Dev Host owner did not return an HTTP loopback origin. Remedy: regenerate the host.",
        )
    })?;
    let mut stream = TcpStream::connect(address).map_err(|error| {
        Diagnostic::error(
            "RUSTY_TEST_HOST_CONNECT",
            origin,
            format!("Product Dev Host owner refused its published origin: {error}. Remedy: inspect host startup and loopback policy."),
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| Diagnostic::error("RUSTY_TEST_HOST_HTTP", origin, error.to_string()))?;
    let mut response = Vec::new();
    stream
        .take(2 * 1024 * 1024)
        .read_to_end(&mut response)
        .map_err(|error| Diagnostic::error("RUSTY_TEST_HOST_HTTP", origin, error.to_string()))?;
    if !response.starts_with(b"HTTP/1.1 200") {
        return Err(Diagnostic::error(
            "RUSTY_TEST_HOST_HTTP",
            origin,
            "Product Dev Host owner did not serve the admitted browser root successfully. Remedy: inspect Product Assembly bundle publication.",
        ));
    }
    Ok(())
}

fn browser_test(origin: &str) -> Result<(), Diagnostic> {
    let script = engine_root().join("render/scripts/rusty-cli-browser-test.mjs");
    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg(origin)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, BROWSER_TIMEOUT).map_err(|error| {
        Diagnostic::error(
            "RUSTY_TEST_BROWSER_PROCESS",
            script.display().to_string(),
            format!("Browser proof owner could not run bounded Chromium evidence: {error}. Remedy: install the Engine render workspace and Chromium artifacts."),
        )
    })?;
    if !output.status.success() {
        return Err(Diagnostic::error(
            "RUSTY_TEST_BROWSER",
            script.display().to_string(),
            format!(
                "Browser proof owner rejected the generated product: {}. Remedy: fix UI mounting, browser-host integration, or the generated bundle and rerun `rusty test`.",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(())
}

fn stop_test_host(mut host: TestHost, code_prefix: &str) -> Result<(), Diagnostic> {
    if let Some(mut stdin) = host.child.stdin.take() {
        let _ = stdin.write_all(b"\n");
    }
    let started = Instant::now();
    loop {
        match host.child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let (stdout, stderr) = host.join_captures();
                if stdout.overflowed || stderr.overflowed {
                    return Err(Diagnostic::error(
                        host_code(code_prefix, "OUTPUT_BOUNDS"),
                        "generated/product-assembly",
                        "Product Dev Host owner exceeded the 64 KiB diagnostic bound. Remedy: inspect and bound generated runtime diagnostics.",
                    ));
                }
                return Ok(());
            }
            Ok(Some(status)) => {
                let (_, stderr) = host.join_captures();
                return Err(Diagnostic::error(
                    host_code(code_prefix, "EXIT"),
                    "generated/product-assembly",
                    format!(
                        "Product Dev Host owner exited with {status}: {}. Remedy: inspect generated runtime shutdown diagnostics.",
                        String::from_utf8_lossy(&stderr.bytes).trim()
                    ),
                ));
            }
            Ok(None) if started.elapsed() < PRODUCT_STOP_TIMEOUT => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = host.child.kill();
                let _ = host.child.wait();
                let _ = host.join_captures();
                return Err(Diagnostic::error(
                    host_code(code_prefix, "STOP_TIMEOUT"),
                    "generated/product-assembly",
                    "Product Dev Host owner did not stop within 10 seconds. Remedy: inspect host connection cleanup and shutdown ownership.",
                ));
            }
            Err(error) => {
                let _ = host.child.kill();
                let _ = host.child.wait();
                let _ = host.join_captures();
                return Err(Diagnostic::error(
                    host_code(code_prefix, "WAIT"),
                    "generated/product-assembly",
                    format!("Product Dev Host owner could not be observed: {error}. Remedy: rerun the isolated host proof."),
                ));
            }
        }
    }
}

impl TestHost {
    fn join_captures(&mut self) -> (StreamCapture, StreamCapture) {
        let stdout = self
            .stdout_reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        let stderr = self
            .stderr_reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        (stdout, stderr)
    }
}

impl Drop for TestHost {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        let _ = self.join_captures();
    }
}

fn host_code(prefix: &str, kind: &str) -> &'static str {
    match (prefix, kind) {
        ("RUSTY_DEV", "START") => "RUSTY_DEV_HOST_START",
        ("RUSTY_DEV", "ORIGIN") => "RUSTY_DEV_HOST_ORIGIN",
        ("RUSTY_DEV", "OUTPUT") => "RUSTY_DEV_HOST_OUTPUT",
        ("RUSTY_DEV", "TIMEOUT") => "RUSTY_DEV_HOST_TIMEOUT",
        ("RUSTY_DEV", "OUTPUT_BOUNDS") => "RUSTY_DEV_HOST_OUTPUT_BOUNDS",
        ("RUSTY_DEV", "EXIT") => "RUSTY_DEV_HOST_EXIT",
        ("RUSTY_DEV", "STOP_TIMEOUT") => "RUSTY_DEV_HOST_STOP_TIMEOUT",
        ("RUSTY_DEV", "WAIT") => "RUSTY_DEV_HOST_WAIT",
        ("RUSTY_TEST", "START") => "RUSTY_TEST_HOST_START",
        ("RUSTY_TEST", "ORIGIN") => "RUSTY_TEST_HOST_ORIGIN",
        ("RUSTY_TEST", "OUTPUT") => "RUSTY_TEST_HOST_OUTPUT",
        ("RUSTY_TEST", "TIMEOUT") => "RUSTY_TEST_HOST_TIMEOUT",
        ("RUSTY_TEST", "OUTPUT_BOUNDS") => "RUSTY_TEST_HOST_OUTPUT_BOUNDS",
        ("RUSTY_TEST", "EXIT") => "RUSTY_TEST_HOST_EXIT",
        ("RUSTY_TEST", "STOP_TIMEOUT") => "RUSTY_TEST_HOST_STOP_TIMEOUT",
        ("RUSTY_TEST", "WAIT") => "RUSTY_TEST_HOST_WAIT",
        _ => "RUSTY_HOST_INTERNAL",
    }
}

fn engine_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("rusty-cli remains inside the Engine checkout")
        .to_path_buf()
}

fn failed(diagnostic: Diagnostic) -> Execution {
    Execution {
        report: Report::failure("error", diagnostic),
        exit_code: EXIT_CONFORMANCE,
    }
}

fn usage_failed(diagnostic: Diagnostic) -> Execution {
    Execution {
        report: Report::failure("error", diagnostic),
        exit_code: crate::EXIT_USAGE,
    }
}
