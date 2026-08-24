use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    thread,
    time::Duration,
};

use product_dev_host::{
    CanonicalU64, ProductDevBundle, ProductDevBundleEntry, ProductDevHost, ProductDevHostConfig,
    ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeMode, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeState,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};

#[derive(Default)]
struct FixtureRuntime;

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
            ProductDevOperationResult::accepted(operation, Self::binding(), Self::readout())
                .unwrap(),
            vec![ProductDevRuntimeOutput::binding(Self::binding())],
        )
        .unwrap()
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
    ProductDevHost::start(FixtureRuntime, ProductDevHostConfig::new(0, bundle)).unwrap()
}

fn request(origin: &str, raw: &str) -> String {
    let address = origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let raw = raw.replace("Host: 127.0.0.1", &format!("Host: {address}"));
    stream.write_all(raw.as_bytes()).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
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
    assert!(start.contains("\"operation\":\"start\""));
    assert!(start.contains("\"instanceId\":\"7\""));

    let missing = request(
        &origin,
        "GET /not-admitted.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(missing.starts_with("HTTP/1.1 404 Not Found\r\n"));
    host.shutdown().unwrap();
}

#[test]
fn rejects_nonclosed_routes_headers_bodies_and_canonical_integers() {
    let host = start();
    let origin = host.origin();
    let generic = request(&origin, "POST /__rusty/product/runtime/call HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
    assert!(generic.starts_with("HTTP/1.1 404 Not Found\r\n"));
    let cookie = request(
        &origin,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: no\r\nConnection: close\r\n\r\n",
    );
    assert!(cookie.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    let bad_realtime = request(&origin, "POST /__rusty/product/runtime/advance-realtime HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 24\r\n\r\n{\"observedTimeNs\":\"01\"}");
    assert!(bad_realtime.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    let oversized = request(&origin, "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 999999999\r\n\r\n");
    assert!(oversized.starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
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
    thread::sleep(Duration::from_millis(60));
    let response = request(&origin, "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    let mut bytes = Vec::new();
    for _ in 0..4 {
        let mut output = [0_u8; 512];
        let count = stream.read(&mut output).unwrap();
        bytes.extend_from_slice(&output[..count]);
        if std::str::from_utf8(&bytes)
            .unwrap()
            .contains("data: {\"kind\":\"binding\"")
        {
            break;
        }
    }
    let output = std::str::from_utf8(&bytes).unwrap();
    assert!(output.contains("HTTP/1.1 200 OK\r\n"));
    assert!(output.contains("Content-Type: text/event-stream\r\n"));
    assert!(output.contains("data: {\"kind\":\"binding\""));
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
    let reset = request(&origin, "GET /__rusty/product/runtime/outputs HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/event-stream\r\nLast-Event-ID: 0\r\nConnection: close\r\n\r\n");
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
