# Standard developer commands

## Purpose

Route work involving the optional standard-gameplay command modules. This is a
tooling composition leaf, not a runtime or a replacement gameplay owner.

## Owns

- Stable inspect, preview, play, and admin descriptors for the Engine-owned
  standard gameplay route.
- Exact typed command markers for `engine-inspector` entity/mechanics reads
  and the named mechanics stat/track/effect services.
- Direct helper calls to the existing inspection, mechanics, standard-plan,
  and `StandardResolver` surfaces.
- Mode selection only: preview selects `ResolutionMode::Preview`; play selects
  `ResolutionMode::Apply` on the same product-supplied resolver, policy, and
  transaction.

## Does not own

- `developer-command` envelopes, dispatch, profiles, transport, or discovery
  policy. The generic substrate has no gameplay dependency.
- Any entity state, mechanics catalog, resolver policy, transaction, queue,
  safe point, persistence, product consequence, product payload schema, or
  authorization policy.
- A console UI, client generation implementation, service locator, registry,
  scheduler, world facade, or product command namespace.

## Descriptor boundary

The current `TypeDescriptor` values are bounded discovery and help summaries,
not a complete Rust-to-host DTO codec. The named-owner marker types and helper
signatures are the exact request/receipt/error contract. In particular, an
interactive host must supply an explicit adapter/schema before it offers an
admin mutation form; it must not decode a partial descriptor record as an
`EffectApplyRequest`, `TrackSetRequest`, or another owner value. Product-generic
commands explicitly advertise a product schema placeholder rather than claiming
Engine ownership of their payload/result/error shape.

## Primary paths

- [`developer-command-standard/src/lib.rs`](../../rust/crates/developer-command-standard/src/lib.rs)
- [`developer-command-standard/src/commands.rs`](../../rust/crates/developer-command-standard/src/commands.rs)
- [`developer-command-standard/src/inspect.rs`](../../rust/crates/developer-command-standard/src/inspect.rs)
- [`developer-command-standard/src/resolution.rs`](../../rust/crates/developer-command-standard/src/resolution.rs)
- [`developer-command-standard/src/admin.rs`](../../rust/crates/developer-command-standard/src/admin.rs)
- [Developer commands](developer-command.md)
- [Gameplay standard](gameplay-standard.md)

## Public composition

```rust
let mut bindings = CommandBindings::new(profile, facts, 64)?;
developer_command_standard::declare_standard_commands(&mut bindings)?;

bindings.bind::<AdminSetTrack, _>(move |_context, request| {
    developer_command_standard::admin_set_track(&mut state, &catalog, request)
})?;
```

The product chooses which descriptors to bind, refreshes command facts at its
own safe point, and performs any client/transport decoding outside this crate.
Read helpers never mutate. Admin helpers return the owner receipts/errors
unchanged. Preview/play helpers do not construct a policy or transaction.

## Acceptance gates

```bash
cargo test -p developer-command-standard --locked
cargo clippy -p developer-command-standard --all-targets --locked -- -D warnings
```
