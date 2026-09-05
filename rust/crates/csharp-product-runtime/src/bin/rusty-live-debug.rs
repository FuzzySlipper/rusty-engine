//! Small agent-friendly client for an explicitly enabled Rusty product dev host.
//!
//! The client owns only HTTP transport, local transcript/history, and
//! descriptor-derived help. The generated product catalog remains the sole
//! authority for command meaning and dispatch.

use std::{
    collections::VecDeque,
    env,
    io::{self, BufRead, Read, Write},
    net::TcpStream,
};

use serde::Deserialize;

const MAX_SCROLLBACK: usize = 128;
const MAX_COMMAND_BYTES: usize = 64 * 1024;

fn main() {
    match Arguments::parse().and_then(run) {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("rusty-live-debug: {error}");
            std::process::exit(1);
        }
    }
}

fn run(arguments: Arguments) -> Result<i32, String> {
    match arguments {
        Arguments::Help => {
            println!("usage: rusty-live-debug --origin http://host:port [--command <line>]\nWithout --command, starts an interactive REPL. The dev host must be started with --live-debug.");
            Ok(0)
        }
        Arguments::Run { origin, command } => {
            let transport = HttpLiveDebugTransport::new(origin)?;
            match command {
                Some(command) => run_one(&transport, &command),
                None => run_repl(&transport),
            }
        }
    }
}

fn run_one(transport: &impl LiveDebugTransport, command: &str) -> Result<i32, String> {
    let response = transport.execute(command)?;
    Ok(print_outcome(&response).0)
}

fn print_outcome(response: &ExecuteResponse) -> (i32, String) {
    match response {
        ExecuteResponse::Completed { succeeded, message } => {
            if *succeeded {
                println!("{message}");
                (0, message.clone())
            } else {
                eprintln!("{message}");
                (2, message.clone())
            }
        }
        ExecuteResponse::TransportFailure { status, message } => {
            eprintln!("{message}");
            (if *status == 404 { 3 } else { 1 }, message.clone())
        }
    }
}

fn run_repl(transport: &impl LiveDebugTransport) -> Result<i32, String> {
    let catalog = match transport.catalog()? {
        CatalogResponse::Available(catalog) => Some(catalog),
        CatalogResponse::Unavailable => {
            eprintln!("Catalog unavailable; commands can still be entered directly.");
            None
        }
    };
    let mut history = VecDeque::with_capacity(MAX_SCROLLBACK);
    let mut scrollback = VecDeque::with_capacity(MAX_SCROLLBACK);
    let mut diagnostics_cursor = None;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    writeln!(
        stdout,
        "Rusty live debug. Type `help`, `complete <prefix>`, `diagnostics [cursor]`, `history`, `scrollback`, or `exit`."
    )
    .map_err(|error| error.to_string())?;
    loop {
        write!(stdout, "debug> ").map_err(|error| error.to_string())?;
        stdout.flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        if stdin
            .lock()
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            writeln!(stdout).map_err(|error| error.to_string())?;
            return Ok(0);
        }
        let command = line.trim();
        if command.is_empty() {
            continue;
        }
        if matches!(command, "exit" | "quit") {
            return Ok(0);
        }
        if command == "history" {
            for entry in &history {
                writeln!(stdout, "{entry}").map_err(|error| error.to_string())?;
            }
            continue;
        }
        if command == "scrollback" {
            for entry in &scrollback {
                writeln!(stdout, "{entry}").map_err(|error| error.to_string())?;
            }
            continue;
        }
        if command == "help" {
            print_catalog(&mut stdout, catalog.as_ref(), None)?;
            continue;
        }
        if let Some(prefix) = command.strip_prefix("complete ") {
            print_catalog(&mut stdout, catalog.as_ref(), Some(prefix.trim()))?;
            continue;
        }
        if command == "diagnostics" || command.starts_with("diagnostics ") {
            let supplied = command
                .strip_prefix("diagnostics")
                .unwrap_or_default()
                .trim();
            let after = if supplied.is_empty() {
                diagnostics_cursor
            } else {
                if supplied
                    .parse::<u64>()
                    .ok()
                    .is_none_or(|value| value.to_string() != supplied)
                {
                    return Err("diagnostics cursor must be a canonical u64".to_owned());
                }
                Some(supplied.to_owned())
            };
            let response = transport.diagnostics(after)?;
            diagnostics_cursor = Some(response.next_cursor);
            writeln!(stdout, "{}", response.body).map_err(|error| error.to_string())?;
            continue;
        }
        if command.len() > MAX_COMMAND_BYTES {
            eprintln!("command exceeds {MAX_COMMAND_BYTES} byte bound");
            continue;
        }
        let response = transport.execute(command)?;
        let (status, message) = print_outcome(&response);
        if scrollback.len() == MAX_SCROLLBACK {
            scrollback.pop_front();
        }
        scrollback.push_back(format!("[{status}] {command}\n{message}"));
        if history.len() == MAX_SCROLLBACK {
            history.pop_front();
        }
        history.push_back(command.to_owned());
    }
}

fn print_catalog(
    output: &mut impl Write,
    catalog: Option<&Catalog>,
    prefix: Option<&str>,
) -> Result<(), String> {
    let Some(catalog) = catalog else {
        writeln!(output, "No generated catalog is available.")
            .map_err(|error| error.to_string())?;
        return Ok(());
    };
    for command in &catalog.commands {
        if prefix.is_some_and(|value| !command.name.starts_with(value)) {
            continue;
        }
        write!(output, "{}", command.name).map_err(|error| error.to_string())?;
        for parameter in &command.parameters {
            write!(output, " <{}:{}>", parameter.name, parameter.type_name)
                .map_err(|error| error.to_string())?;
        }
        writeln!(output, " — {}", command.description).map_err(|error| error.to_string())?;
    }
    Ok(())
}

trait LiveDebugTransport {
    fn catalog(&self) -> Result<CatalogResponse, String>;
    fn execute(&self, command: &str) -> Result<ExecuteResponse, String>;
    fn diagnostics(&self, after: Option<String>) -> Result<DiagnosticsResponse, String>;
}

struct HttpLiveDebugTransport {
    authority: String,
}

impl HttpLiveDebugTransport {
    fn new(origin: String) -> Result<Self, String> {
        let authority = origin
            .strip_prefix("http://")
            .ok_or("--origin must be an http:// host:port origin")?;
        if authority.is_empty() || authority.contains('/') || authority.contains('@') {
            return Err("--origin must be an http:// host:port origin".to_owned());
        }
        Ok(Self {
            authority: authority.to_owned(),
        })
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<(&str, &str)>,
    ) -> Result<(u16, String), String> {
        let mut stream = TcpStream::connect(&self.authority)
            .map_err(|error| format!("could not connect to {}: {error}", self.authority))?;
        let (body, content_type) = body.unwrap_or(("", ""));
        let content_type = if method == "POST" {
            format!("Content-Type: {content_type}\r\n")
        } else {
            String::new()
        };
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n{}Content-Length: {}\r\n\r\n{}",
            self.authority,
            content_type,
            body.len(), body,
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|error| error.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;
        let (head, body) = response
            .split_once("\r\n\r\n")
            .ok_or("host returned a malformed HTTP response")?;
        let status = head
            .split_whitespace()
            .nth(1)
            .ok_or("host response lacks status")?
            .parse::<u16>()
            .map_err(|_| "host response has invalid status")?;
        Ok((status, body.to_owned()))
    }
}

impl LiveDebugTransport for HttpLiveDebugTransport {
    fn catalog(&self) -> Result<CatalogResponse, String> {
        let (status, body) = self.request("GET", "/__rusty/product/runtime/debug/catalog", None)?;
        if status == 404 {
            return Ok(CatalogResponse::Unavailable);
        }
        if status != 200 {
            return Err(format!("catalog request failed ({status}): {body}"));
        }
        let catalog: Catalog =
            serde_json::from_str(&body).map_err(|_| "catalog response is invalid".to_owned())?;
        if catalog.available {
            Ok(CatalogResponse::Available(catalog))
        } else {
            Ok(CatalogResponse::Unavailable)
        }
    }

    fn execute(&self, command: &str) -> Result<ExecuteResponse, String> {
        let (status, body) = self.request(
            "POST",
            "/__rusty/product/runtime/debug/execute",
            Some((command, "text/plain; charset=utf-8")),
        )?;
        if status == 200 {
            return Ok(ExecuteResponse::Completed {
                succeeded: true,
                message: body,
            });
        }
        if status == 422 {
            return Ok(ExecuteResponse::Completed {
                succeeded: false,
                message: body,
            });
        }
        Ok(ExecuteResponse::TransportFailure {
            status,
            message: body,
        })
    }

    fn diagnostics(&self, after: Option<String>) -> Result<DiagnosticsResponse, String> {
        let body = after.map_or_else(
            || "{}".to_owned(),
            |cursor| format!(r#"{{"after":"{cursor}"}}"#),
        );
        let (status, response) = self.request(
            "POST",
            "/__rusty/product/runtime/diagnostics/read",
            Some((&body, "application/json")),
        )?;
        if status != 200 {
            return Err(format!("diagnostics request failed ({status}): {response}"));
        }
        let decoded: DiagnosticsWire = serde_json::from_str(&response)
            .map_err(|_| "diagnostics response is invalid".to_owned())?;
        let _ = (
            &decoded.events,
            decoded.floor_sequence,
            decoded.through_sequence,
            decoded.read_monotonic_nanoseconds,
            decoded.lagged,
            decoded.warning_count,
            decoded.error_count,
            decoded.dropped_count,
            &decoded.telemetry,
        );
        Ok(DiagnosticsResponse {
            next_cursor: decoded.next_cursor,
            body: response,
        })
    }
}

enum CatalogResponse {
    Available(Catalog),
    Unavailable,
}
enum ExecuteResponse {
    Completed { succeeded: bool, message: String },
    TransportFailure { status: u16, message: String },
}
struct DiagnosticsResponse {
    next_cursor: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    available: bool,
    commands: Vec<CommandDescriptor>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandDescriptor {
    name: String,
    description: String,
    parameters: Vec<ParameterDescriptor>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParameterDescriptor {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiagnosticsWire {
    next_cursor: String,
    events: Vec<serde_json::Value>,
    floor_sequence: String,
    through_sequence: String,
    read_monotonic_nanoseconds: String,
    lagged: bool,
    warning_count: String,
    error_count: String,
    dropped_count: String,
    /// Hosts predating the product-lane telemetry snapshot omit this field.
    /// Keep the outer response backward compatible while still decoding the
    /// complete, closed telemetry shape when a current host supplies it.
    telemetry: Option<DiagnosticsTelemetryWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
// This is intentionally a decode-only schema; the CLI preserves and prints
// the original JSON so callers can select telemetry with ordinary JSON tools.
#[allow(dead_code)]
struct DiagnosticsTelemetryWire {
    in_flight_operation: Option<DiagnosticsOperationWire>,
    in_flight_age_ms: Option<String>,
    last_product_admission_latency_ms: Option<String>,
    last_input_admission_latency_ms: Option<String>,
    queued_input_batches: usize,
    queued_input_events: usize,
    input_batch_capacity: usize,
    oldest_input_age_ms: Option<String>,
    input_overflow_pending: bool,
    runtime_progress_rate_millihertz: Option<String>,
    runtime_progress_age_ms: Option<String>,
    connections: usize,
    subscribers: usize,
    output_queue_items: usize,
    output_queue_capacity: usize,
    output_queue_floor: String,
    output_binding_active: bool,
    update_attribution: Option<DiagnosticsUpdateAttributionWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code)]
struct DiagnosticsUpdateAttributionWire {
    sample_count: String,
    callback_duration_us_p50: String,
    callback_duration_us_p95: String,
    callback_duration_us_max: String,
    latest: DiagnosticsUpdateAttributionSampleWire,
    rolling_slowest: DiagnosticsUpdateAttributionSampleWire,
    rolling_slowest_age_ms: String,
    slowest: DiagnosticsUpdateAttributionSampleWire,
    slowest_age_ms: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code)]
struct DiagnosticsUpdateAttributionSampleWire {
    callback_duration_us: String,
    character_step_calls: String,
    character_step_duration_us: String,
    character_step_cast_count: String,
    character_step_candidate_count: String,
    character_step_narrow_phase_count: String,
    voxel_residency_calls: String,
    voxel_residency_duration_us: String,
    voxel_scene_presentation_calls: String,
    voxel_scene_presentation_duration_us: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DiagnosticsOperationWire {
    Connect,
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
    ReplaceControl,
    ReleaseControl,
    Input,
    AdvanceRealtime,
    AdmitDemandStep,
    AdmitExternalStep,
    CompleteTimeline,
    ReportAudioFeedback,
    ReportAnimationFeedback,
    ReportGhostPlateFeedback,
    ReportRendererDiagnostics,
    ExecuteDebug,
}
enum Arguments {
    Help,
    Run {
        origin: String,
        command: Option<String>,
    },
}
impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut origin = None;
        let mut command = None;
        let mut values = env::args().skip(1);
        while let Some(value) = values.next() {
            match value.as_str() {
                "--origin" => {
                    origin = Some(values.next().ok_or("--origin requires http://host:port")?)
                }
                "--command" => command = Some(values.next().ok_or("--command requires text")?),
                "--help" => return Ok(Self::Help),
                _ => return Err(format!("unknown argument `{value}`")),
            }
        }
        Ok(Self::Run {
            origin: origin.ok_or("--origin is required")?,
            command,
        })
    }
}
