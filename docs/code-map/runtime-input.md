# Runtime input

## Ownership

`rust/crates/runtime-input` owns a host-neutral, instance-owned input lane
after Product Model has admitted and linked the current composition. It
compiles descriptor references and typed mappings once, accepts ordered
normalized physical facts and direct Product UI intent claims, and forms one
immutable `InputFrame` plus ordered typed intent envelopes for a
lifecycle-validated `InputSnapshot` phase token.

It owns neither DOM events, pointer lock, browser focus, Gamepad polling,
controller selection cadence, UI state, a callback/bus, capability invocation,
movement, collision, gameplay consequences, storage, or a scheduler. The
application host normalizes browser facts before this crate; the downstream
runtime supplies lifecycle tokens and deliberately dispatches the descriptor
readout itself.

## Primary paths

- [`runtime-input/src/lib.rs`](../../rust/crates/runtime-input/src/lib.rs)
- [`runtime-input/src/compile.rs`](../../rust/crates/runtime-input/src/compile.rs)
- [`runtime-input/src/model.rs`](../../rust/crates/runtime-input/src/model.rs)
- [`runtime-input/src/lane.rs`](../../rust/crates/runtime-input/src/lane.rs)
- [`runtime-input/src/wire.rs`](../../rust/crates/runtime-input/src/wire.rs)
- [`runtime-input/tests/runtime_input.rs`](../../rust/crates/runtime-input/tests/runtime_input.rs)
- [`host-neutral input fixture`](../../fixtures/runtime-input/host-neutral-input-envelope.json)
- [`rusty-engine facade`](../../rust/crates/rusty-engine/src/lib.rs)

The complete downstream facade re-exports the crate as
`rusty_engine::runtime_input`.

## Data flow and ordering

```text
admitted + linked intent descriptors / mappings
        |
        v
CompiledInputMappings
        |
host physical ingress -----------+----> RuntimeInputLane ----> InputFrame
direct product UI claim ---------+              |
                                               `--> RuntimeIntentEnvelope[]
                                                     descriptor + value + provenance
```

Each ingress/claim carries a `RuntimeInputBinding` (instance id, generation,
control revision), Product context, and canonical u64 sequence. The lane
requires gap-free sequence numbers, does not wrap at u64 max, and clears state
on rejection. A restart or control-revision rebind must begin with the matching
clear reason at sequence zero; a context transition is only a same-epoch
`interaction-mode-loss` clear carrying the *new* context. Context-only and
rebind clears are thus part of the same order as all other facts.

Physical mapping envelopes retain their source sequence; mappings caused by
one fact use authored mapping order. Direct UI envelopes retain their own
source sequence. Held readouts are snapshot-synthetic and follow all real
envelopes with the same final sequence, preserving authored held-map order.
Keyboard chords fire a press when the last required member arrives and a
release when any member leaves. Pointer/wheel deltas clear after each snapshot;
controller axes persist until a newer observation or clear, and an explicit
zero controller fact emits the neutral axis intent.

## Wire boundary

`decode_runtime_input_wire_event_json` and
`decode_runtime_input_wire_events_json` strictly decode the structural host
union. The only wire integers are canonical decimal u64 strings; all object
variants deny unknown fields, reject null cross-variant fields, bound bytes and
batch length, and validate finite bounded axes. Direct UI axis claims are
limited to `[-1, 1]`; physical accumulation remains separately bounded. A
direct-UI-only `product-payload` claim carries one stable contract identity and
strict plain JSON `data`. Rust bounds it to 65,536 encoded bytes, depth 32,
4,096 nodes, 1,024 array/object entries, 16,384-byte strings, and safe integer
numbers; its contract must equal the linked descriptor's `payloadContract`
before its immutable envelope reaches the Product Runtime Adapter. Physical
input mappings cannot target this kind. Browser ingress independently rejects
functions, DOM values, custom prototypes, accessors, symbols, holes, and
non-finite or unsafe numeric data before Rust repeats the data check.
`AxisValue` normalizes negative zero to positive zero for deterministic
readout. The shared fixture is consumed by Rust and the application-host
TypeScript tests.

## Focused verification

```bash
cargo test -p runtime-input --locked
cargo clippy -p runtime-input --all-targets --locked -- -D warnings
cargo test -p product-model --locked
pnpm --dir rules run generate:check
pnpm --dir rules run verify
./scripts/verify-rust-sdk-consumer.sh
./scripts/verify.sh
```

The runtime suite covers direct/physical chronology, W press/hold/release,
repeated keydown, chord edges, pointer/wheel transient accumulation,
controller persistence/neutral zero, context/rebind/disposal clears, lifecycle
state/phase validation, queue/sequence exhaustion, canonical frame order,
descriptor readout, and shared Rust/TypeScript wire parity.
