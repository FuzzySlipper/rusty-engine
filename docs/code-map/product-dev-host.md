# Product development browser host

`rust/crates/product-dev-host` owns the Engine-facing local browser
development serving boundary for an already generated Product Assembly. It is
not a product network identity, an HTTP feature platform, a product persistence
owner, a browser renderer, or a general RPC bridge.

It binds only `127.0.0.1`, serves one immutable pre-admitted bundle map, and
accepts the closed same-origin operation routes consumed by the isolated
`@rusty-engine/product-browser-host` transport. The generated Product Assembly
owns the concrete `ProductDevRuntime`; calls are explicitly serialized and
each returns its exact bounded output batch. The host has no callback
registration API, dynamic operation name, filesystem reach-through, CORS,
cookies, or non-loopback binding option.

## Primary paths

- [`product-dev-host/src/lib.rs`](../../rust/crates/product-dev-host/src/lib.rs)
- [`product-dev-host/src/bundle.rs`](../../rust/crates/product-dev-host/src/bundle.rs)
- [`product-dev-host/src/model.rs`](../../rust/crates/product-dev-host/src/model.rs)
- [`product-dev-host/src/session.rs`](../../rust/crates/product-dev-host/src/session.rs)
- [`product-dev-host/src/host.rs`](../../rust/crates/product-dev-host/src/host.rs)
- [`product-dev-host/tests/loopback_host.rs`](../../rust/crates/product-dev-host/tests/loopback_host.rs)

## Fixed transport boundary

`ProductDevBundle` admits bounded normalized resource paths and exact bytes
before `ProductDevHost::start` binds a port. It must contain `index.html`.
The running host never opens a product directory, so a relocated generated
bundle has no runtime path dependency on its source tree. Its closed media
allowlist includes `audio/wav` and `application/octet-stream` for
Assembly-admitted renderer preload bytes; the server does not inspect media,
derive resource paths, or gain a general static-file route.

Only these runtime paths are admitted beneath
`/__rusty/product/runtime/`:

- `POST lifecycle/{start,pause,resume,restart,shutdown,report-fault}`;
- `POST input`, `advance-realtime`, `admit-demand-step`,
  `admit-external-step`, and `timeline-completion`; and
- `GET outputs` as a bounded SSE stream.

All JSON request DTOs deny unknown fields. JavaScript-visible u64 values use
canonical decimal strings. Browser input is decoded by `runtime-input`; timeline
completion is reconstructed as the existing bounded `runtime-timeline`
completion envelope. Render/presentation/UI outputs are only constructible from
their owning validated Rust frame/envelope types before their exact wire bytes
are emitted.

`ProductDevOperationOwner` is the transport-neutral in-process operation seam.
It serializes one generated `ProductDevRuntime` behind a mutex and exposes
only the fixed lifecycle, input, realtime, demand, external-step, and timeline
completion methods. Its JSON adapters use `CanonicalU64`,
`ProductDevInputBatch`, and `ProductDevTimelineCompletion` strict bounded
admission directly; they return the runtime owner's existing result-plus-output
receipts and do not create callbacks, subscriptions, dynamic dispatch, or
another runtime authority. A packaged host may compose these methods into its
own typed delivery adapter without depending on the loopback HTTP transport.

The deliberately bounded HTTP/1.1 implementation supports only the needed
request/response subset: fixed `Content-Length`, read/write timeouts,
`Connection: close` request semantics, and a dedicated SSE response. This
keeps the host-neutral provider free of a general server framework while making
the development-host trust boundary auditable in one owner.

## Focused verification

```bash
cargo test -p product-dev-host --locked
cargo clippy -p product-dev-host --all-targets --locked -- -D warnings
```

The integration test covers admitted bundle serving, fixed-route rejection,
canonical/bounded decoding, real loopback SSE receipt output, concurrent
port-zero relocation, timeout close behavior, and explicit shutdown.
