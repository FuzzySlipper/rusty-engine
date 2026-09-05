use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use product_dev_host::{
    CanonicalU64, ProductDevAudioFeedback, ProductDevAudioFeedbackResult, ProductDevBundle,
    ProductDevBundleEntry, ProductDevDebugCatalog, ProductDevDebugResult, ProductDevHost,
    ProductDevHostConfig, ProductDevInputBatch, ProductDevInputResult,
    ProductDevLifecycleOperation, ProductDevLog, ProductDevOperationKind,
    ProductDevOperationResult, ProductDevRuntime, ProductDevRuntimeBinding, ProductDevRuntimeMode,
    ProductDevRuntimeOutput, ProductDevRuntimeReadout, ProductDevRuntimeReceipt,
    ProductDevRuntimeState, ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};

#[derive(Default)]
struct FixtureRuntime {
    recovery_calls: Arc<AtomicUsize>,
}

struct ReconnectRuntime {
    started: bool,
    starts: Arc<AtomicUsize>,
    attaches: Arc<AtomicUsize>,
    fail_connect: bool,
}

struct OutputFailureRuntime {
    inputs: Arc<AtomicUsize>,
}

impl FixtureRuntime {
    fn binding() -> ProductDevRuntimeBinding {
        ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(2),
        }
    }

    fn readout() -> ProductDevRuntimeReadout {
        ProductDevRuntimeReadout::new(
            Self::binding(),
            ProductDevRuntimeMode::Realtime,
            ProductDevRuntimeState::Running,
        )
    }

    fn operation(
        operation: ProductDevOperationKind,
    ) -> ProductDevRuntimeReceipt<ProductDevOperationResult> {
        ProductDevRuntimeReceipt::new(
            ProductDevOperationResult::accepted(
                operation,
                Self::binding(),
                CanonicalU64::new(0),
                Self::readout(),
            )
            .unwrap(),
            vec![
                ProductDevRuntimeOutput::binding(Self::binding(), CanonicalU64::new(0)),
                ProductDevRuntimeOutput::complete_baseline(Self::binding()),
            ],
        )
        .unwrap()
    }
}

impl ReconnectRuntime {
    fn operation(
        &self,
        operation: ProductDevOperationKind,
        baseline: bool,
    ) -> ProductDevRuntimeReceipt<ProductDevOperationResult> {
        let mut outputs = vec![ProductDevRuntimeOutput::runtime_readout(
            FixtureRuntime::readout(),
        )];
        if baseline {
            outputs.insert(
                0,
                ProductDevRuntimeOutput::binding(FixtureRuntime::binding(), CanonicalU64::new(0)),
            );
            outputs.push(ProductDevRuntimeOutput::complete_baseline(
                FixtureRuntime::binding(),
            ));
        }
        ProductDevRuntimeReceipt::new(
            ProductDevOperationResult::accepted(
                operation,
                FixtureRuntime::binding(),
                CanonicalU64::new(0),
                FixtureRuntime::readout(),
            )
            .unwrap(),
            outputs,
        )
        .unwrap()
    }
}

impl ProductDevRuntime for ReconnectRuntime {
    fn connect(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        if self.fail_connect {
            return Err(product_dev_host::ProductDevRuntimeError::new(
                "FIXTURE_CONNECT_TAINTED",
                "fixture start callback escaped after entry",
            )
            .unwrap());
        }
        let operation = if self.started {
            self.attaches.fetch_add(1, Ordering::SeqCst);
            ProductDevOperationKind::Connect
        } else {
            self.started = true;
            self.starts.fetch_add(1, Ordering::SeqCst);
            ProductDevOperationKind::Start
        };
        Ok(self.operation(operation, true))
    }

    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(self.operation(operation.operation_kind(), true))
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevInputResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevInputResult::accepted(
                batch.events().len(),
                FixtureRuntime::binding(),
                FixtureRuntime::readout(),
            )
            .unwrap(),
            Vec::new(),
        )
        .unwrap())
    }

    fn advance_realtime(
        &mut self,
        _observed_time_ns: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(self.operation(ProductDevOperationKind::AdvanceRealtime, false))
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(self.operation(ProductDevOperationKind::AdmitDemandStep, false))
    }

    fn admit_external_step(
        &mut self,
        _step: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(self.operation(ProductDevOperationKind::AdmitExternalStep, false))
    }

    fn complete_timeline(
        &mut self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevTimelineCompletionResult::accepted(
                CanonicalU64::new(completion.envelope().ticket().value()),
                FixtureRuntime::binding(),
                FixtureRuntime::readout(),
            )
            .unwrap(),
            Vec::new(),
        )
        .unwrap())
    }
}

impl ProductDevRuntime for FixtureRuntime {
    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(Self::operation(operation.operation_kind()))
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevInputResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevInputResult::accepted(batch.events().len(), Self::binding(), Self::readout())
                .unwrap(),
            Vec::new(),
        )
        .unwrap())
    }

    fn recover_input_overflow(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        self.recovery_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Self::operation(ProductDevOperationKind::ReplaceControl))
    }

    fn describe_debug(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevDebugCatalog>,
        product_dev_host::ProductDevRuntimeError,
    > {
        let catalog = ProductDevDebugCatalog::decode_json(
            br#"{"available":true,"commands":[{"name":"fixture.echo","description":"Echoes a fixture value.","parameters":[{"name":"value","type":"string"}]}]}"#,
        )
        .unwrap();
        Ok(ProductDevRuntimeReceipt::new(catalog, Vec::new()).unwrap())
    }

    fn execute_debug(
        &mut self,
        command: &str,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevDebugResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        let result = match command {
            "fixture.fail" => {
                ProductDevDebugResult::new(false, "fixture semantic failure".to_owned()).unwrap()
            }
            "fixture.runtime" => {
                return Err(product_dev_host::ProductDevRuntimeError::new(
                    "FIXTURE_DEBUG_RUNTIME",
                    "fixture runtime failure",
                )
                .unwrap())
            }
            _ => ProductDevDebugResult::new(true, format!("executed {command}")).unwrap(),
        };
        Ok(ProductDevRuntimeReceipt::new(result, Vec::new()).unwrap())
    }

    fn advance_realtime(
        &mut self,
        _observed_time_ns: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(Self::operation(ProductDevOperationKind::AdvanceRealtime))
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(Self::operation(ProductDevOperationKind::AdmitDemandStep))
    }

    fn admit_external_step(
        &mut self,
        _step: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(Self::operation(ProductDevOperationKind::AdmitExternalStep))
    }

    fn complete_timeline(
        &mut self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevTimelineCompletionResult::accepted(
                CanonicalU64::new(completion.envelope().ticket().value()),
                Self::binding(),
                Self::readout(),
            )
            .unwrap(),
            Vec::new(),
        )
        .unwrap())
    }

    fn report_audio_feedback(
        &mut self,
        feedback: ProductDevAudioFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevAudioFeedbackResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        if feedback.runtime != Self::binding() {
            return Err(product_dev_host::ProductDevRuntimeError::new(
                "FIXTURE_AUDIO_BINDING",
                "audio feedback does not name the current binding",
            )
            .unwrap());
        }
        let accepted_through = feedback.facts.last().map(|fact| fact.fact_id());
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevAudioFeedbackResult::accepted(Self::binding(), accepted_through),
            Vec::new(),
        )
        .unwrap())
    }
}

impl ProductDevRuntime for OutputFailureRuntime {
    fn connect(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(FixtureRuntime::operation(ProductDevOperationKind::Connect))
    }

    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(FixtureRuntime::operation(operation.operation_kind()))
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevInputResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        let call = self.inputs.fetch_add(1, Ordering::SeqCst);
        let outputs = if call == 0 {
            // This receipt is valid, but it cannot be attached to a retained
            // stream until a binding baseline exists. It models publication
            // failure after the authoritative input call has consumed once.
            vec![ProductDevRuntimeOutput::runtime_readout(
                FixtureRuntime::readout(),
            )]
        } else {
            Vec::new()
        };
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevInputResult::accepted(
                batch.events().len(),
                FixtureRuntime::binding(),
                FixtureRuntime::readout(),
            )
            .unwrap(),
            outputs,
        )
        .unwrap())
    }

    fn advance_realtime(
        &mut self,
        _observed_time_ns: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(FixtureRuntime::operation(
            ProductDevOperationKind::AdvanceRealtime,
        ))
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(FixtureRuntime::operation(
            ProductDevOperationKind::AdmitDemandStep,
        ))
    }

    fn admit_external_step(
        &mut self,
        _step: CanonicalU64,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevOperationResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(FixtureRuntime::operation(
            ProductDevOperationKind::AdmitExternalStep,
        ))
    }

    fn complete_timeline(
        &mut self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>,
        product_dev_host::ProductDevRuntimeError,
    > {
        Ok(ProductDevRuntimeReceipt::new(
            ProductDevTimelineCompletionResult::accepted(
                CanonicalU64::new(completion.envelope().ticket().value()),
                FixtureRuntime::binding(),
                FixtureRuntime::readout(),
            )
            .unwrap(),
            Vec::new(),
        )
        .unwrap())
    }
}

fn start() -> product_dev_host::RunningProductDevHost {
    let bundle = ProductDevBundle::new(vec![
        ProductDevBundleEntry::new(
            "index.html",
            "text/html; charset=utf-8",
            b"<!doctype html><title>Rusty Product</title>".to_vec(),
        )
        .unwrap(),
        ProductDevBundleEntry::new(
            "main.js",
            "text/javascript; charset=utf-8",
            b"export {};".to_vec(),
        )
        .unwrap(),
    ])
    .unwrap();
    ProductDevHost::start(
        FixtureRuntime::default(),
        ProductDevHostConfig::new(0, bundle),
    )
    .unwrap()
}

fn start_debug() -> product_dev_host::RunningProductDevHost {
    start_debug_with_recovery_calls().0
}

fn start_debug_with_recovery_calls() -> (product_dev_host::RunningProductDevHost, Arc<AtomicUsize>)
{
    let bundle = ProductDevBundle::new(vec![ProductDevBundleEntry::new(
        "index.html",
        "text/html; charset=utf-8",
        b"<!doctype html>".to_vec(),
    )
    .unwrap()])
    .unwrap();
    let recovery_calls = Arc::new(AtomicUsize::new(0));
    let host = ProductDevHost::start(
        FixtureRuntime {
            recovery_calls: Arc::clone(&recovery_calls),
        },
        ProductDevHostConfig::new(0, bundle).with_live_debug(true),
    )
    .unwrap();
    (host, recovery_calls)
}

fn start_reconnect(
    starts: Arc<AtomicUsize>,
    attaches: Arc<AtomicUsize>,
) -> product_dev_host::RunningProductDevHost {
    start_reconnect_with_failure(starts, attaches, false)
}

fn start_reconnect_with_failure(
    starts: Arc<AtomicUsize>,
    attaches: Arc<AtomicUsize>,
    fail_connect: bool,
) -> product_dev_host::RunningProductDevHost {
    let bundle = ProductDevBundle::new(vec![ProductDevBundleEntry::new(
        "index.html",
        "text/html; charset=utf-8",
        b"<!doctype html>".to_vec(),
    )
    .unwrap()])
    .unwrap();
    ProductDevHost::start(
        ReconnectRuntime {
            started: false,
            starts,
            attaches,
            fail_connect,
        },
        ProductDevHostConfig::new(0, bundle),
    )
    .unwrap()
}

fn open_sse(address: std::net::SocketAddr, path: &str) -> TcpStream {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream
}

fn read_until(stream: &mut TcpStream, marker: &str) -> String {
    let mut bytes = Vec::new();
    while !String::from_utf8_lossy(&bytes).contains(marker) {
        let mut buffer = [0_u8; 1024];
        let count = stream.read(&mut buffer).unwrap();
        assert_ne!(count, 0, "SSE stream closed before {marker}");
        bytes.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8(bytes).unwrap()
}

fn read_through_marker(stream: &mut TcpStream, marker: &str) -> String {
    let mut bytes = Vec::new();
    while !String::from_utf8_lossy(&bytes).ends_with(marker) {
        let mut byte = [0_u8; 1];
        let count = stream.read(&mut byte).unwrap();
        assert_ne!(count, 0, "SSE stream closed before {marker}");
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).unwrap()
}

fn output_batch(event: &str) -> serde_json::Value {
    let data = event
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("SSE data");
    let batch: serde_json::Value = serde_json::from_str(data).expect("JSON output batch");
    assert_eq!(batch["kind"], "runtime-output-batch");
    batch
}

fn request(origin: &str, raw: &str) -> String {
    request_bytes(origin, raw.as_bytes())
}

fn request_bytes(origin: &str, raw: &[u8]) -> String {
    let address = origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let marker = b"Host: 127.0.0.1";
    let mut request = raw.to_vec();
    if let Some(position) = raw
        .windows(marker.len())
        .position(|window| window == marker)
    {
        request.splice(
            position..position + marker.len(),
            format!("Host: {address}").bytes(),
        );
    }
    stream.write_all(&request).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn retries_one_injected_listener_accept_error_then_serves_the_next_connection() {
    let diagnostics = ProductDevLog::new(Default::default()).unwrap();
    let decisions = Arc::new(AtomicUsize::new(0));
    let hook_decisions = Arc::clone(&decisions);
    let bundle = ProductDevBundle::new(vec![ProductDevBundleEntry::new(
        "index.html",
        "text/html; charset=utf-8",
        b"<!doctype html>".to_vec(),
    )
    .unwrap()])
    .unwrap();
    let host = ProductDevHost::start(
        FixtureRuntime::default(),
        ProductDevHostConfig::new(0, bundle)
            .with_diagnostics(diagnostics.clone())
            .with_test_accept_decision_hook(move || {
                (hook_decisions.fetch_add(1, Ordering::SeqCst) == 0)
                    .then_some(std::io::ErrorKind::Interrupted)
            }),
    )
    .unwrap();
    let response = request(
        &host.origin(),
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(decisions.load(Ordering::SeqCst) >= 2);
    let snapshot = diagnostics.snapshot();
    assert!(snapshot
        .events
        .iter()
        .any(|event| event.code() == "DEV_HOST_LISTENER_ACCEPT_RETRY"));
    assert!(format!("{:?}", snapshot.events).contains("RejectedRecoverable"));
    host.shutdown().unwrap();
}

#[test]
fn output_publication_failure_returns_the_consumed_receipt_for_resync_without_replay() {
    let inputs = Arc::new(AtomicUsize::new(0));
    let bundle = ProductDevBundle::new(vec![ProductDevBundleEntry::new(
        "index.html",
        "text/html; charset=utf-8",
        b"<!doctype html>".to_vec(),
    )
    .unwrap()])
    .unwrap();
    let host = ProductDevHost::start(
        OutputFailureRuntime {
            inputs: Arc::clone(&inputs),
        },
        ProductDevHostConfig::new(0, bundle),
    )
    .unwrap();
    let origin = host.origin();
    let input = "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"batch\":[]}";
    let first = request(&origin, input);
    assert!(first.starts_with("HTTP/1.1 200 OK\r\n"), "{first}");
    assert!(first.contains("X-Rusty-Commit-Disposition: resync-required\r\n"));
    assert!(first.contains("X-Rusty-Resync-Outputs: fresh\r\n"));
    assert!(first.contains("\"accepted\":true"));
    assert!(first.contains("\"binding\":{\"instanceId\":\"7\""));
    assert!(first.contains("\"readout\":{\"artifact\":\"rusty.product.runtime-readout\""));
    assert_eq!(inputs.load(Ordering::SeqCst), 1);

    let mut fresh = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let baseline = read_until(&mut fresh, "event: rusty-output-baseline");
    assert!(baseline.contains("\"operation\":\"connect\""));

    let second = request(&origin, input);
    assert!(second.starts_with("HTTP/1.1 200 OK\r\n"), "{second}");
    assert!(second.contains("X-Rusty-Commit-Disposition: committed\r\n"));
    assert_eq!(
        inputs.load(Ordering::SeqCst),
        2,
        "resync readout did not replay the already-consumed input",
    );
    host.shutdown().unwrap();
}

#[test]
fn serves_only_admitted_bundle_and_fixed_runtime_routes() {
    let host = start();
    let origin = host.origin();
    let index = request(
        &origin,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(index.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(index.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert!(index.contains("Cache-Control: no-store\r\n"));
    assert!(index.ends_with("<!doctype html><title>Rusty Product</title>"));

    let start = request(
        &origin,
        "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(start.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(start.contains("X-Rusty-Output-Through: 1\r\n"));
    assert!(start.contains("\"operation\":\"start\""));
    assert!(start.contains("\"instanceId\":\"7\""));
    assert!(start.contains("\"nextInputSequence\":\"0\""));

    let missing = request(
        &origin,
        "GET /not-admitted.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(missing.starts_with("HTTP/1.1 404 Not Found\r\n"));
    host.shutdown().unwrap();
}

#[test]
fn malformed_pointer_batch_resynchronizes_without_closing_the_host() {
    let (host, recovery_calls) = start_debug_with_recovery_calls();
    let origin = host.origin();
    let malformed_body = r#"{"batch":[{"runtime":{"instanceId":"7","generation":"1","controlRevision":"2"},"sequence":"0","context":"gameplay.default","fact":{"kind":"pointer-button","button":"primary","edge":"held"}}]}"#;
    let malformed = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{malformed_body}",
            malformed_body.len(),
        ),
    );
    assert!(malformed.starts_with("HTTP/1.1 200 OK\r\n"), "{malformed}");
    assert!(malformed.contains("\"code\":\"DEV_HOST_INPUT_DECODE\""));
    assert!(malformed.contains("\"disposition\":\"resync-required\""));
    assert!(malformed.contains("\"droppedCount\":1"));
    assert_eq!(recovery_calls.load(Ordering::SeqCst), 1);

    let pointer_body = r#"{"batch":[{"runtime":{"instanceId":"7","generation":"1","controlRevision":"2"},"sequence":"0","context":"gameplay.default","fact":{"kind":"pointer-button","button":"primary","edge":"pressed"}},{"runtime":{"instanceId":"7","generation":"1","controlRevision":"2"},"sequence":"1","context":"gameplay.default","fact":{"kind":"pointer-button","button":"secondary","edge":"pressed"}}]}"#;
    let pointer = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{pointer_body}",
            pointer_body.len(),
        ),
    );
    assert!(pointer.starts_with("HTTP/1.1 200 OK\r\n"), "{pointer}");
    assert!(pointer.contains("\"accepted\":true"));
    assert!(pointer.contains("\"count\":2"));

    let diagnostics = request(
        &origin,
        "POST /__rusty/product/runtime/diagnostics/read HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(
        diagnostics.contains("\"DEV_HOST_INPUT_DECODE\""),
        "{diagnostics}"
    );
    assert!(diagnostics.contains("\"resync-required\""), "{diagnostics}");
    host.shutdown().unwrap();
}

#[test]
fn over_limit_malformed_batch_resynchronizes_once_without_becoming_terminal() {
    let (host, recovery_calls) = start_debug_with_recovery_calls();
    let origin = host.origin();
    let batch = vec!["{}"; 1_025].join(",");
    let malformed_body = format!(r#"{{"batch":[{batch}]}}"#);
    let malformed = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{malformed_body}",
            malformed_body.len(),
        ),
    );

    assert!(malformed.starts_with("HTTP/1.1 200 OK\r\n"), "{malformed}");
    assert!(malformed.contains("\"code\":\"DEV_HOST_INPUT_DECODE\""));
    assert!(malformed.contains("\"disposition\":\"resync-required\""));
    assert!(malformed.contains("\"count\":1025"));
    assert!(malformed.contains("\"droppedCount\":1025"));
    assert!(!malformed.contains("\"disposition\":\"terminal\""));
    assert_eq!(recovery_calls.load(Ordering::SeqCst), 1);
    host.shutdown().unwrap();
}

#[test]
fn browser_diagnostics_readback_preserves_closed_terminal_facts() {
    let host = start_debug();
    let origin = host.origin();
    let report_body = r#"{"hostState":"failed","runtimeProgress":"9","transportState":"closed","outputState":"closed","lastRendererSequence":"60","rendererObservationAgeMs":"100","firstTerminal":{"code":"BROWSER_HOST_TRANSPORT_FAILED","message":"transport closed"},"recoverableEvent":{"code":"CSHARP_LIFECYCLE_CLOCK_REGRESSION","message":"dropped clock observation"},"pageEvents":[]}"#;
    let reported = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/browser-diagnostics HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{report_body}",
            report_body.len(),
        ),
    );
    assert!(reported.starts_with("HTTP/1.1 200 OK\r\n"), "{reported}");
    let read = request(
        &origin,
        "POST /__rusty/product/runtime/diagnostics/read HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(read.starts_with("HTTP/1.1 200 OK\r\n"), "{read}");
    assert!(read.contains("\"BROWSER_HOST_TRANSPORT_FAILED\""), "{read}");
    assert!(
        read.contains("\"CSHARP_LIFECYCLE_CLOCK_REGRESSION\""),
        "{read}"
    );
    assert!(read.contains("\"rejected-recoverable\""), "{read}");
    assert!(
        read.contains("\"transport\",\"value\":\"closed\""),
        "{read}"
    );
    assert!(
        read.contains("\"message\":\"Product Browser Host transition snapshot\""),
        "{read}"
    );
    assert!(
        read.contains("\"scope\",\"value\":\"transition\""),
        "{read}"
    );
    assert!(
        read.contains("\"renderer-sequence\",\"value\":\"60\""),
        "{read}"
    );
    assert!(read.contains("\"nextCursor\":\"3\""), "{read}");
    assert!(read.contains("\"telemetry\":{"), "{read}");
    assert!(
        read.contains("\"runtimeProgressRateMillihertz\":null"),
        "{read}"
    );
    host.shutdown().unwrap();
}

#[test]
fn browser_diagnostics_accepts_auxiliary_renderer_failure_as_recoverable() {
    let host = start_debug();
    let origin = host.origin();
    let report_body = r#"{"hostState":"ready","runtimeProgress":"9","transportState":"open","outputState":"open","recoverableEvent":{"code":"BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE","message":"renderer diagnostics reporting was temporarily unavailable: failed to fetch"},"pageEvents":[]}"#;
    let reported = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/browser-diagnostics HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{report_body}",
            report_body.len(),
        ),
    );
    assert!(reported.starts_with("HTTP/1.1 200 OK\r\n"), "{reported}");
    let read = request(
        &origin,
        "POST /__rusty/product/runtime/diagnostics/read HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(read.starts_with("HTTP/1.1 200 OK\r\n"), "{read}");
    assert!(
        read.contains("\"BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE\""),
        "{read}"
    );
    assert!(read.contains("\"warningCount\":\"1\""), "{read}");
    assert!(read.contains("\"errorCount\":\"0\""), "{read}");
    assert!(read.contains("\"rejected-recoverable\""), "{read}");
    host.shutdown().unwrap();
}

#[test]
fn browser_diagnostics_preserves_a_degraded_local_request_observation() {
    let host = start_debug();
    let origin = host.origin();
    let report_body = r#"{"hostState":"degraded","runtimeProgress":"9","transportState":"open","outputState":"open","recoverableEvent":{"code":"BROWSER_LOCAL_REQUEST_UNAVAILABLE","message":"Product Browser local runtime request failed for input: Failed to fetch"},"pageEvents":[]}"#;
    let reported = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/browser-diagnostics HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{report_body}",
            report_body.len(),
        ),
    );
    assert!(reported.starts_with("HTTP/1.1 200 OK\r\n"), "{reported}");
    let read = request(
        &origin,
        "POST /__rusty/product/runtime/diagnostics/read HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(
        read.contains("\"host-state\",\"value\":\"degraded\""),
        "{read}"
    );
    assert!(
        read.contains("\"BROWSER_LOCAL_REQUEST_UNAVAILABLE\""),
        "{read}"
    );
    assert!(read.contains("\"rejected-recoverable\""), "{read}");
    host.shutdown().unwrap();
}

#[test]
fn rejects_nonclosed_routes_headers_bodies_and_canonical_integers() {
    let host = start();
    let origin = host.origin();
    let generic = request(&origin, "POST /__rusty/product/runtime/call HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
    assert!(generic.starts_with("HTTP/1.1 404 Not Found\r\n"));
    let bad_realtime = request(&origin, "POST /__rusty/product/runtime/advance-realtime HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 24\r\n\r\n{\"observedTimeNs\":\"01\"}");
    assert!(bad_realtime.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    let oversized = request(&origin, "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 999999999\r\n\r\n");
    assert!(oversized.starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
    host.shutdown().unwrap();
}

#[test]
fn accepts_only_bounded_exact_binding_audio_feedback_on_its_fixed_route() {
    let host = start();
    let origin = host.origin();
    let accepted_body = r#"{"runtime":{"instanceId":"7","generation":"1","controlRevision":"2"},"replaceOwner":true,"evictedFactCount":"0","facts":[{"kind":"naturalCompletion","factId":"9","sequence":3,"source":"oneShot","signalHandle":"4"}]}"#;
    let decoded = serde_json::from_str::<ProductDevAudioFeedback>(accepted_body).unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), accepted_body);
    let accepted = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/audio-feedback HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{accepted_body}",
            accepted_body.len()
        ),
    );
    assert!(accepted.starts_with("HTTP/1.1 200 OK\r\n"), "{accepted}");
    assert!(accepted.contains("\"accepted\":true"));
    assert!(accepted.contains("\"acceptedThroughFactId\":\"9\""));

    let stale_body = accepted_body.replace("\"generation\":\"1\"", "\"generation\":\"2\"");
    let stale = request(
        &origin,
        &format!(
            "POST /__rusty/product/runtime/audio-feedback HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{stale_body}",
            stale_body.len()
        ),
    );
    assert!(stale.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(stale.contains("\"accepted\":false"));
    assert!(stale.contains("\"runtime\":{\"instanceId\":\"7\""));
    host.shutdown().unwrap();
}

#[test]
fn sse_receives_runtime_receipt_outputs_without_blocking_post() {
    let host = start();
    let origin = host.origin();
    let address = origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let sse = format!("GET /__rusty/product/runtime/outputs HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n");
    stream.write_all(sse.as_bytes()).unwrap();
    let headers = read_through_marker(&mut stream, "\r\n\r\n");
    assert!(headers.contains("HTTP/1.1 200 OK\r\n"));
    assert!(headers.contains("Content-Type: text/event-stream\r\n"));
    let response = request(&origin, "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    let event = read_through_marker(&mut stream, "\n\n");
    let batch = output_batch(&event);
    assert_eq!(batch["outputs"][0]["kind"], "binding");
    assert_eq!(batch["outputs"][0]["nextInputSequence"], "0");
    host.shutdown().unwrap();
}

#[test]
fn sse_delivers_realtime_progress_at_publication_cadence() {
    let host = start_reconnect(Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let mut stream = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let baseline = read_until(&mut stream, "\"operation\":\"start\"");
    assert!(baseline.contains("HTTP/1.1 200 OK\r\n"), "{baseline}");

    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let origin = host.origin();
    let publisher = thread::spawn(move || {
        for observed_time in 1..=7 {
            thread::sleep(Duration::from_micros(16_667));
            let body = format!("{{\"observedTimeNs\":\"{observed_time}\"}}");
            let response = request(
                &origin,
                &format!(
                    "POST /__rusty/product/runtime/advance-realtime HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
            );
            assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        }
    });
    let mut pending = String::new();
    let mut arrivals = Vec::new();
    while arrivals.len() < 7 {
        let mut bytes = [0_u8; 2_048];
        let count = stream.read(&mut bytes).unwrap();
        assert_ne!(count, 0, "SSE stream closed during cadence observation");
        pending.push_str(std::str::from_utf8(&bytes[..count]).unwrap());
        while let Some(end) = pending.find("\n\n") {
            let record = pending[..end].to_owned();
            pending.drain(..end + 2);
            if record.contains("\"kind\":\"runtime-readout\"") {
                arrivals.push(Instant::now());
            }
        }
    }
    publisher.join().unwrap();

    let mut intervals = arrivals
        .windows(2)
        .map(|pair| pair[1].duration_since(pair[0]))
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    let median = intervals[intervals.len() / 2];
    assert!(
        median < Duration::from_millis(22),
        "60 Hz publications were still delivered in poll-sized bursts: {intervals:?}"
    );
    drop(stream);
    host.shutdown().unwrap();
}

#[test]
fn fresh_sse_connects_once_then_attaches_after_retained_outputs_are_evicted() {
    let starts = Arc::new(AtomicUsize::new(0));
    let attaches = Arc::new(AtomicUsize::new(0));
    let host = start_reconnect(Arc::clone(&starts), Arc::clone(&attaches));
    let origin = host.origin();

    let mut first = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let first_baseline = read_until(&mut first, "\"operation\":\"start\"");
    assert!(first_baseline.contains("\"operation\":\"start\""));
    drop(first);

    for index in 0..=product_dev_host::MAX_OUTPUT_QUEUE_ITEMS {
        let body = format!("{{\"observedTimeNs\":\"{}\"}}", index + 1);
        let response = request(
            &origin,
            &format!("POST /__rusty/product/runtime/advance-realtime HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()),
        );
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    }

    let mut second = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let second_baseline = read_until(&mut second, "\"operation\":\"connect\"");
    assert!(second_baseline.contains("\"operation\":\"connect\""));
    assert!(!second_baseline.contains("event: rusty-output-lag"));

    let mut simultaneous = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let simultaneous_baseline = read_until(&mut simultaneous, "\"operation\":\"connect\"");
    assert!(simultaneous_baseline.contains("\"operation\":\"connect\""));
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(attaches.load(Ordering::SeqCst), 2);
    drop(second);
    drop(simultaneous);
    host.shutdown().unwrap();
}

#[test]
fn partial_fresh_baseline_reconnects_without_a_cursor_or_second_start() {
    let starts = Arc::new(AtomicUsize::new(0));
    let attaches = Arc::new(AtomicUsize::new(0));
    let host = start_reconnect(Arc::clone(&starts), Arc::clone(&attaches));

    let mut interrupted = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let headers = read_through_marker(&mut interrupted, "\r\n\r\n");
    assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"), "{headers}");
    let first_private_event = read_through_marker(&mut interrupted, "\n\n");
    assert_eq!(
        output_batch(&first_private_event)["outputs"][0]["kind"],
        "binding"
    );
    assert!(!first_private_event.contains("id: "));
    drop(interrupted);

    let mut retry = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let completed = read_until(&mut retry, "\"operation\":\"connect\"");
    assert!(completed.contains("event: rusty-output-baseline"));
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(attaches.load(Ordering::SeqCst), 1);
    drop(retry);
    host.shutdown().unwrap();
}

#[test]
fn fresh_sse_connect_replacement_error_requests_host_termination() {
    let host = start_reconnect_with_failure(
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        true,
    );
    let origin = host.origin();
    let response = request(
        &origin,
        "GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 500"), "{response}");
    assert!(host.termination_requested());
    host.shutdown().unwrap();
}

#[test]
fn interrupted_baseline_completion_has_no_cursor_and_reattaches_without_reset() {
    let starts = Arc::new(AtomicUsize::new(0));
    let attaches = Arc::new(AtomicUsize::new(0));
    let host = start_reconnect(Arc::clone(&starts), Arc::clone(&attaches));

    let mut interrupted = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let through_event = read_through_marker(&mut interrupted, "event: rusty-output-baseline\n");
    assert!(through_event.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(!through_event.contains("id: "));
    let incomplete_completion = read_through_marker(&mut interrupted, "\n");
    assert!(incomplete_completion.contains("\"operation\":\"start\""));
    assert!(!incomplete_completion.ends_with("\n\n"));
    drop(interrupted);

    let mut retry = open_sse(host.address(), "/__rusty/product/runtime/outputs/fresh");
    let completed = read_until(&mut retry, "\"operation\":\"connect\"");
    assert!(completed.contains("event: rusty-output-baseline"));
    assert!(!completed.contains("id: "));
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(attaches.load(Ordering::SeqCst), 1);
    drop(retry);
    host.shutdown().unwrap();
}

#[test]
fn idle_sse_disconnects_release_subscriber_slots() {
    let host = start();
    for _ in 0..product_dev_host::MAX_SSE_SUBSCRIBERS {
        let mut stream = open_sse(host.address(), "/__rusty/product/runtime/outputs");
        let response = read_until(&mut stream, "\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        drop(stream);
    }
    // With output-driven delivery an idle subscriber is intentionally observed
    // through failed heartbeat writes instead of a 25 ms socket poll. A TCP
    // stack may accept the first write after the peer closes, so allow two
    // heartbeat intervals for bounded reclamation.
    thread::sleep(Duration::from_millis(2_200));
    let mut final_stream = open_sse(host.address(), "/__rusty/product/runtime/outputs");
    let response = read_until(&mut final_stream, "\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(!response.contains("DEV_HOST_SSE_BOUNDS"));
    drop(final_stream);
    host.shutdown().unwrap();
}

#[test]
fn loopback_port_zero_relocates_and_shutdown_releases_the_listener() {
    let first = start();
    let second = start();
    assert!(first.address().ip().is_loopback());
    assert!(second.address().ip().is_loopback());
    assert_ne!(first.address().port(), second.address().port());
    let old = first.address();
    first.shutdown().unwrap();
    assert!(TcpStream::connect_timeout(&old, Duration::from_millis(100)).is_err());
    second.shutdown().unwrap();
}

#[test]
fn slow_header_times_out_with_explicit_close_response() {
    let host = start();
    let address = host.address();
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 408 Request Timeout\r\n"));
    assert!(response.contains("Connection: close\r\n"));
    host.shutdown().unwrap();
}

#[test]
fn slow_body_and_lagging_sse_cursor_fail_closed_while_runtime_continues() {
    let host = start();
    let origin = host.origin();
    let address = host.address();
    let mut slow = TcpStream::connect(address).unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    slow.write_all(b"POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{").unwrap();
    let mut slow_response = String::new();
    slow.read_to_string(&mut slow_response).unwrap();
    assert!(slow_response.starts_with("HTTP/1.1 408 Request Timeout\r\n"));

    let start = "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
    for _ in 0..=product_dev_host::MAX_OUTPUT_QUEUE_ITEMS {
        assert!(request(&origin, start).starts_with("HTTP/1.1 200 OK\r\n"));
    }
    let reset = request(&origin, "GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/event-stream\r\nLast-Event-ID: 0\r\nConnection: close\r\n\r\n");
    assert!(reset.contains("event: rusty-output-lag\n"));
    assert!(reset.contains("\"DEV_HOST_OUTPUT_LAG\""));
    host.shutdown().unwrap();
}

#[test]
fn exact_loopback_host_and_origin_are_required() {
    let host = start();
    let origin = host.origin();
    let hostile_host = request(
        &origin,
        "GET / HTTP/1.1\r\nHost: other.local\r\nConnection: keep-alive\r\n\r\n",
    );
    assert!(hostile_host.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    let hostile_origin = request(&origin, "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://other.local\r\nConnection: keep-alive\r\n\r\n");
    assert!(hostile_origin.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    let same_origin = request(&origin, &format!("GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\nConnection: keep-alive\r\n\r\n"));
    assert!(same_origin.starts_with("HTTP/1.1 200 OK\r\n"));
    host.shutdown().unwrap();
}

#[test]
fn live_debug_routes_are_opt_in_serialized_and_keep_semantic_failure_typed() {
    let disabled = start();
    let disabled_body = "fixture";
    let disabled_response = request(
        &disabled.origin(),
        &format!("POST /__rusty/product/runtime/debug/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{disabled_body}", disabled_body.len()),
    );
    assert!(disabled_response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    disabled.shutdown().unwrap();

    let host = start_debug();
    let origin = host.origin();
    let catalog = request(
        &origin,
        "GET /__rusty/product/runtime/debug/catalog HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(catalog.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(catalog.contains("\"fixture.echo\""));
    let failed_body = "fixture.fail";
    let failed = request(
        &origin,
        &format!("POST /__rusty/product/runtime/debug/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{failed_body}", failed_body.len()),
    );
    assert!(failed.starts_with("HTTP/1.1 422 Unprocessable Content\r\n"));
    assert!(failed.ends_with("fixture semantic failure"));
    let runtime_body = "fixture.runtime";
    let runtime = request(
        &origin,
        &format!("POST /__rusty/product/runtime/debug/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{runtime_body}", runtime_body.len()),
    );
    assert!(runtime.starts_with("HTTP/1.1 500 Internal Server Error\r\n"));
    assert!(runtime.contains("FIXTURE_DEBUG_RUNTIME"));
    let mut invalid_utf8_request = b"POST /__rusty/product/runtime/debug/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 1\r\n\r\n".to_vec();
    invalid_utf8_request.push(0xff);
    let invalid_utf8 = request_bytes(&origin, &invalid_utf8_request);
    assert!(invalid_utf8.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    host.shutdown().unwrap();
}
