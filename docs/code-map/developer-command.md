# Developer commands

## Purpose

Route work involving host-neutral, typed developer-command envelopes and the
explicit in-process binding port that a downstream product invokes at its own
safe point.

## Owns

- `developer-command`: versioned request/reply envelopes, stable command,
  correlation, runtime, and profile identities; semantic lanes; bounded typed
  parameter/result/error descriptors; exact discovery; provenance and bounded
  command history.
- A synchronous, instance-owned `CommandBindings` port. A product explicitly
  declares compiled command descriptors, retains handlers only through the
  existing `Send + 'static` path, or exposes a descriptor for a caller-borrowed
  owner through `dispatch_borrowed`; the latter handler is never retained.
  Products refresh observed runtime facts and call either path from their own
  queue.
- Envelope preflight for protocol/runtime/revision/catalog guards, cancellation,
  timeout, profile, and correlation reuse. Rejected preflight never reserves a
  correlation, records history, or calls a handler.

## Does not own

- A game world, entity/component access, gameplay commands, complete owner
  receipts, scheduler, loop, queue, transaction, or persistence policy.
- A service locator, plugin registry, reflection/method-name bridge, generic
  component poke, transport, network endpoint, filesystem, shell, or UI.
- Lane authorization policy. Lanes are descriptor metadata; a profile and the
  explicit compiled/bound handler set decide availability.
- Studio authority. Studio classifies this crate as `non-studio`; it must not
  acquire a tooling queue, transport, authorization policy, or developer
  command vocabulary through this neutral provider contract.

## Primary paths

- [`developer-command/src/lib.rs`](../../rust/crates/developer-command/src/lib.rs)
- [`developer-command/src/identity.rs`](../../rust/crates/developer-command/src/identity.rs)
- [`developer-command/src/descriptor.rs`](../../rust/crates/developer-command/src/descriptor.rs)
- [`developer-command/src/dispatch.rs`](../../rust/crates/developer-command/src/dispatch.rs)
- [`developer-command/src/host_wire.rs`](../../rust/crates/developer-command/src/host_wire.rs)
- [`developer-command/src/wire.rs`](../../rust/crates/developer-command/src/wire.rs) and
  [`developer-command/src/bin/export-wire-contract.rs`](../../rust/crates/developer-command/src/bin/export-wire-contract.rs)
- [`developer-command/tests/contract.rs`](../../rust/crates/developer-command/tests/contract.rs)
- [Canonical design](../design.md)

## Public composition

The downstream product owns the command-family vocabulary and the existing
queue/safe point. It supplies an explicit handler for each exposed command;
the handler returns the owner's normal typed reply or error.

```rust
let mut commands = CommandBindings::new(profile, current_facts, 64)?;
commands.bind::<InspectEntity, _>(|context, request| {
    owner.inspect(context.facts(), request)
})?;

// The live owner can remain in the product's service aggregate. Engine borrows
// it only at the product-selected safe point and retains no closure.
commands.expose_borrowed::<AdminCommand>()?;
let reply = commands.dispatch_borrowed::<AdminCommand, _>(request, &mut owner);

// The product queue decides when this direct synchronous call is safe.
commands.set_facts(owner.current_command_facts());
let reply = commands.dispatch::<InspectEntity>(request);
```

`discover()` returns every explicitly declared descriptor in stable command-ID
order and marks the exact bound/exposed subset. `stored_bound` and
`borrowed_bound` distinguish retained handlers from caller-borrowed execution;
`bound` remains their compact union. An omitted descriptor is unknown; a
declared but unbound descriptor is unavailable. Requests carry no lane, so
callers cannot claim a lane different from the selected descriptor. The strict
`host_wire` module maps the generated v1 camelCase host envelope, including
decimal-string revision/catalog facts, typed product payloads, bounded receipt
references, and output response facts. Host discovery emits only executable
descriptors; internal response metadata retains provenance and whether an error
occurred before dispatch or after entering the owner without adding replay
fields to the generated wire contract. Wire decoding rejects unknown fields and
revalidates protocol, identity, profile, descriptor, text, collection, and node
bounds rather than bypassing their constructors. The binding port does not
catch a product handler panic; product panic policy remains outside this
contract. Only envelope failures are guaranteed to avoid owner mutation;
entered handlers own their ordinary mutation and receipt semantics.

## Acceptance gates

```bash
cargo test -p developer-command --locked
cargo clippy -p developer-command --all-targets --locked -- -D warnings
```

## Follow-up routing

- Product command modules and safe-point orchestration belong downstream.
- Client, shell, TypeScript, and transport adapters belong to their explicit
  host owner. The generated generic transport contract is intentionally limited
  to envelope, discovery, error, history, and sequence facts; descriptors stay
  help metadata and never become inferred owner DTO codecs.
- Read-only diagnostics remain in `engine-inspector`; it does not own this
  contract or runtime binding.
