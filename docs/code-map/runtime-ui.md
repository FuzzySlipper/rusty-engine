# Runtime UI projection

`rust/crates/runtime-ui` owns the small host-neutral transport boundary for a
downstream product's DOM-oriented UI projection. It is not a renderer, UI
store, scheduler, browser adapter, or gameplay authority. A downstream Rust
owner builds an owned DTO from a `product_kernel::ProductProjectionContext`;
this crate copies that DTO into a bounded JSON value and emits one immutable
transport envelope for a named stream.

## Primary paths

- [`runtime-ui/src/lib.rs`](../../rust/crates/runtime-ui/src/lib.rs)
- [`product-kernel/src/context.rs`](../../rust/crates/product-kernel/src/context.rs)
- [`runtime-lifecycle`](runtime-lifecycle.md)
- [`application-host` UI projection adapter](../../render/packages/application-host/src/ui-projection.ts)
- [`Rust UI projection fixture`](../../fixtures/runtime-ui/stealth.ui-projection.json)

## Rust-owned contract

The current strict wire shape is compact JSON with these exact fields:

```json
{"artifact":"rusty.product.ui-projection","runtime":{"instanceId":"33","generation":"1","controlRevision":"1"},"sequence":"0","stream":"stealth.hud","contract":"stealth.ui.snapshot.v1","value":{"alerts":2,"selected":"target-1"}}
```

Runtime identifiers and `sequence` are canonical decimal strings. `stream` and
`contract` use the same Product Model identity grammar as manifests and
capability targets. Unknown fields, a wrong artifact, malformed JSON, and
non-whitespace trailing bytes are rejected. Encoding is deterministic for the
same owned JSON value.

`ProductProjectionContext` is constructed only after validating a
`RuntimePhase::Projection` token against the live lifecycle. It exposes an
immutable product snapshot and the admitted simulation step, but no clock,
host, renderer, scheduler, callback, or mutation authority. `RuntimeUiProjection::project`
validates the lane/stream/contract/sequence boundary before invoking the typed
function; the returned DTO is serialized into an owned `serde_json::Value`
before publication. A stream's contract identity is fixed for its lifecycle
epoch and cannot change mid-epoch.

## Lifecycle and quotas

One non-cloneable lane is bound explicitly to a running lifecycle epoch. Every
emission revalidates the phase token and the lane's instance/generation/control
revision. Foreign, stale, wrong-phase, disposed, duplicate, and regressed
emissions fail closed before transport. Sequence is the simulation step, so at
most one envelope is accepted per stream per step; multiple streams may publish
on the same step. Explicit `rebind` clears all per-epoch stream sequence and
contract progress, an exact binding is a no-op, and a same-instance binding
must have a nondecreasing generation plus a strictly advanced control
revision. Older bindings are rejected, and `dispose` is terminal.

The current bounds are 256 streams per lane, 65,536 compact value JSON bytes,
2,048 JSON nodes, depth 16, 8,192-byte strings, 512 array entries, 256 object
keys, integer-valued numbers within JavaScript's exact safe range, and 262,144
encoded envelope bytes. The lane retains only per-stream
sequence/contract evidence; it never retains a source snapshot, callback, host
object, timer, or renderer resource.

## Host boundary

The isolated application host validates and deep-freezes the same envelope
shape before a mounted DOM UI observes it. That host-owned view/subscriber
store is a separate surface: Rust proves typed phase-gated projection and
strict transport, while the host proves message admission, detachment, and
presentation observation. Neither side becomes the other's gameplay or
renderer authority. The Rust-only `render-presentation` path remains separate.

## Focused verification

```bash
cargo test -p runtime-ui --locked
cargo clippy -p runtime-ui --all-targets --locked -- -D warnings
cargo test -p product-kernel --locked
```
