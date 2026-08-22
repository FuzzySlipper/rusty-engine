# Runtime developer console

The optional runtime developer console is also called the **developer
console**, **debug console**, or **developer commands**. These names describe
one diagnostic/tooling path: a downstream Rust product exposes selected typed
operations, and an optional host presents or drives them. The console is not
player input, a persistence format, a replay runtime, or a second gameplay
authority.

For the Rust contracts, see [Developer commands](../../code-map/developer-command.md)
and [Standard developer commands](../../code-map/developer-command-standard.md).

## Ownership and end-to-end flow

The flow is deliberately product-owned at both ends:

```text
product Rust owner and named services
  -> product bounded command queue and product-selected safe point
  -> developer-command::CommandBindings
       + optional developer-command-standard adapters
  -> HostCommandDiscovery / HostCommandRequest / HostCommandResponse
  -> product-owned transport (if a process or host boundary is needed)
  -> DOM-free @rusty-engine/developer-command-client
  -> optional @rusty-engine/application-host pull-down shell
```

1. The product decides whether to admit a request to its bounded queue and
   when that queue reaches a safe point. Engine owns neither the queue nor its
   scheduler. A queue receipt means only that admission succeeded; completion
   is reported after the selected owner has actually been dispatched.
2. At that safe point, the product refreshes `DispatchFacts`, calls
   `CommandBindings::set_facts`, and dispatches a typed request. The binding
   port performs protocol, runtime/profile, revision, catalog, cancellation,
   timeout, and correlation preflight before entering an owner.
3. For a host boundary, the product maps strict host DTOs with
   `HostCommandRequest::into_command_parts`, calls `dispatch` or
   `dispatch_borrowed` at the same safe point, and maps the result with
   `map_command_response`. `HostCommandDiscovery::from_bindings` (or
   `from_snapshot`) supplies the executable discovery view. Payload codecs,
   response details, receipts, transport, and authorization remain product
   decisions.
4. A product-owned adapter implements the client's `discover` and `execute`
   functions. It can use a browser transport, typed Tauri IPC, or another
   explicit boundary; the Engine contract does not require HTTP, WebSocket,
   Tauri, a sidecar, or any other transport.
5. A headless Rust tool can stop after step 2. A browser or Tauri product may
   create a `RustyDeveloperCommandClient` from the DOM-free package and, only
   if it wants a UI, mount the optional application-host shell. The shell is a
   presentation convenience over the injected client, not a requirement for
   Rust-only tooling.

## What the feature provides

| Capability | Current owner and boundary |
|---|---|
| Discovery, profiles, and lanes | `CommandProfile`, `CommandBindings::discover`, and `HostCommandDiscovery` expose bounded identities, summaries, permitted lanes, and executable descriptors. A lane is descriptor metadata; the caller does not choose authority by putting a lane in a request, and a profile plus explicit binding decides availability. |
| Inspect | `developer-command-standard` supplies `standard.inspect.entity`, `standard.inspect.mechanics`, and the product-typed `standard.inspect.gameplay` descriptor. `HostEntityRequest` strictly maps a canonical decimal entity string into the exact owner `EntityId`; the read helpers call named inspection/projection owners and do not grant mutable component access. |
| Preview and play | `standard.preview.attempt` and `standard.play.attempt` are typed markers and adapters around the product-supplied policy, resolver, and transaction. Preview selects preview mode; play selects apply mode. Product code owns consequences and transaction timing. |
| Admin | `standard.admin.stat.set-base`, `standard.admin.track.set`, `standard.admin.effect.apply`, and `standard.admin.effect.remove` are the current standard admin descriptors. Their named adapters retain mechanics request/receipt/error types and reacquire live component revisions immediately before the owner service. `HostTrackSetReceipt::from_owner` projects the authoritative non-Serde receipt's decision, bounds, source/provenance, revisions, and bounded source-cost evidence without changing owner receipt authority. |
| Product commands | A product implements `DeveloperCommand` with its own request, reply, and error types, then uses `declare`/`bind` for a retained `Send + 'static` handler or `expose_borrowed`/`dispatch_borrowed` for an owner borrowed only during the safe-point call. Engine does not invent the product command namespace. |
| Facts guards | `CommandRequest::with_expected`, `cancelled`, and `timed_out`, together with `DispatchFacts`, reject stale or cancelled work before owner entry. The client also rejects stale discovery/response context. Rejected preflight does not reserve a correlation or record completed history. |
| Correlation, history, and cancellation | Rust bindings retain bounded provenance/history and reject duplicate correlations. The client accepts `AbortSignal`, records bounded local failures/history, supports `history()` and `exportSequence()`, and disposes late work safely. An exported sequence is portable command intent/history, explicitly not deterministic replay. Timers and cancellation policy remain with the product/adapter. |
| Schemas and forms | Products provide exact `RustyDeveloperCommandWireSchema` values and may validate with `validateRustyDeveloperCommandWireValue`. Generated standard host schemas are available through `RUSTY_STANDARD_HOST_WIRE_SCHEMAS` (`RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS` remains a compatibility alias). A descriptor's `TypeDescriptor` is help/discovery metadata, not a wire codec; the shell offers a form only when an exact schema is supplied. |
| Disposal and input arbitration | `RustyDeveloperCommandClient.dispose()` and the shell's `dispose()` stop late work and remove listeners/DOM. The shell calls the injected `enterInterface` seam while open and restores it when closed; the application host owns the actual interaction mode and input policy. |

Extensions are namespaced schema attachments created through
`RustyDeveloperCommandClientOptions.extensions`. Each attachment names an
already-discovered command, expected lane and profile, and exact codec. The
client rejects duplicate schema ownership, unavailable commands, and
lane/profile drift on every discovery refresh. An extension cannot append an
executable descriptor, alias, summary, or availability: discovery remains the
only authority for those facts.

## Minimal adoption recipes

### In-process or Rust-only owner

1. Define a product command marker implementing `DeveloperCommand` and keep
   its request/reply/error types in the product.
2. At the product composition root, construct
   `CommandBindings::new(profile, facts, history_capacity)`. Call
   `declare_standard_commands` for the optional standard family, or declare
   the product marker directly through `declare`/`declare_descriptor`.
3. Bind a retained owner with `bind`, or expose a live owner with
   `expose_borrowed`. Do not store an owner or closure in a component or a
   global registry.
4. In the product queue's selected safe point, call `set_facts`, construct a
   `CommandRequest::new(...).with_expected(...)` as needed, and call
   `dispatch` or `dispatch_borrowed`. Return the owner's typed result/receipt
   and preserve its normal mutation/error semantics.

This path needs no browser, DOM, Node, application-host shell, or transport.

### Browser or Tauri adapter

1. Keep the Rust steps above as the authority. Add a product-owned adapter
   whose `discover` path returns the strict `HostCommandDiscovery` shape and
   whose `execute` path maps a `HostCommandRequest`, dispatches at the product
   safe point, and returns the mapped `HostCommandResponse`.
2. Supply product payload codecs as schema-only `extensions` bindings for
   already-discovered product commands. For standard inspect/admin forms, use
   the generated standard host schemas and call `HostEntityRequest::into_entity`
   or each `Host*Request::map_live` immediately
   before the named mechanics service; never serialize the opaque component
   guard as a substitute for reacquiring it.
3. Import `@rusty-engine/developer-command-client` from its package root and
   call `createRustyDeveloperCommandClient({ adapter, schemas, extensions })`.
   Use `client.discover()`, `client.execute(...)`, `client.history()`,
   `client.exportSequence()`, and `client.dispose()` through the product's
   lifecycle.
4. If a rich DOM is useful, import `@rusty-engine/application-host` from its
   package root and call `mountRustyDeveloperCommandShell(root, { client,
   enterInterface })`. The application host supplies input arbitration; the
   shell does not acquire gameplay, transport, or persistence authority.

## Public entry points and source

- Rust substrate: [`developer-command/src/lib.rs`](../../../rust/crates/developer-command/src/lib.rs), especially `CommandBindings`, `CommandRequest`, `DispatchFacts`, `ExpectedFacts`, `DeveloperCommand`, `CommandHandler`, `BorrowedCommandHandler`, `HostCommandDiscovery`, `HostCommandRequest`, `map_command_response`, and `developer_command_wire_contract_json`.
- Standard adapters: [`developer-command-standard/src/lib.rs`](../../../rust/crates/developer-command-standard/src/lib.rs), including `declare_standard_commands`, the nine current marker families named above, the `inspect_*`, `preview_standard_attempt`, `execute_standard_attempt`, and `admin_*` helpers, `standard_host_wire_schemas_json`, `HostEntityRequest::into_entity`, `HostTrackSetReceipt::from_owner`, and the `Host*Request::map_live` methods.
- DOM-free client: [`@rusty-engine/developer-command-client`](../../../render/packages/developer-command-client/src/index.ts), with `createRustyDeveloperCommandClient`, `validateRustyDeveloperCommandWireValue`, `RustyDeveloperCommandAdapter`, `RustyDeveloperCommandWireSchema`, `RustyDeveloperCommandExtension`, and the client lifecycle methods.
- Optional shell: [`@rusty-engine/application-host`](../../../render/packages/application-host/src/developer-command-shell.ts), with `mountRustyDeveloperCommandShell`, `RustyDeveloperCommandShellOptions`, `enterInterface`, and `dispose`.

The generated contract is Rust-owned. `render/contracts/developer-command-contract.json`
and the generated TypeScript constants are regenerated through the existing
developer-command contract tooling; a host must not hand-edit a second wire
contract.

## Focused checks

For Rust changes, run the two owning crate checks:

```bash
cargo test -p developer-command --locked
cargo clippy -p developer-command --all-targets --locked -- -D warnings
cargo test -p developer-command-standard --locked
cargo clippy -p developer-command-standard --all-targets --locked -- -D warnings
```

For DOM-free client or shell changes, the package tests are the narrow loop:

```bash
pnpm --dir render --filter @rusty-engine/developer-command-client test
pnpm --dir render --filter @rusty-engine/application-host test
```

The owning isolated render/host gate is `./scripts/verify-render.sh` when the
browser or packaged artifact boundary is part of the change. Documentation
navigation uses `./scripts/check-doc-links.sh`; it does not prove runtime
behavior.

## Anti-patterns

- Do not turn the console into a generic component poke, reflection surface,
  service locator, or method-name bridge. Bind named commands to named owners.
- Do not make TypeScript evaluate gameplay, schedule ticks, mutate live state,
  own persistence, or become the player-facing gameplay authority.
- Do not treat `TypeDescriptor` as a Rust-to-host wire codec. Supply an exact
  product schema/adapter; help-only descriptors must remain unavailable to a
  form or execution path.
- Do not duplicate extension discovery or merge a product descriptor into the
  client. Extensions attach schemas only to an already-discovered command.
- Do not report bounded queue admission as command completion. The response is
  complete only after safe-point dispatch returns an owner result or a bounded
  pre-dispatch rejection.
- Do not mutate the owner outside its product-selected safe point, and do not
  let the browser shell bypass the product queue, facts guards, or owner
  transaction.
- Do not require the optional browser shell for Rust-only diagnostics, or treat
  local history/export as persistence or deterministic replay.
