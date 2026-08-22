# Continuous cadence and residual experiment

Status: bounded evidence for task #7188 and the continuous-mechanics follow-up.

This is not a continuous stat, track, save format, scheduler, product clock, or
unit system. It measures what a future owner would need to make an explicit
choice about, using the accepted `ContinuousValue` and named quantization APIs.

## Probe shape

The host-neutral probe evaluates one `ContinuousExpr::Input` from an explicit
`ContinuousInputBundle`: the authored rate is binary64 `7.0` per unnamed
declared span. It compares the one-interval reference with 35, 60, and 120
partitions over one span and 10,000 spans. The order in each vector is
`[1, 35, 60, 120]`.

| Approach | Caller-held state | One span | 10,000 spans |
| --- | --- | --- |
| Direct binary64 accumulation | Current binary64 value | bits `[401c000000000000, 401c000000000004, 401bfffffffffff5, 401c000000000006]` | bits `[40f1170000000000, 40f116ffffff8527, 40f11700000069fa, 40f117000000d13d]` |
| `TowardZero` residual carry | Carry bits, residual schema/evaluator/policy/mode, source, previous interval/cadence | positive totals `[7, 6, 7, 7]`; negative `[-7, -6, -7, -7]` | positive `[70000, 69999, 70000, 70000]`; negative `[-70000, -69999, -70000, -70000]` |
| Exact tick deadline | Exact rate, canonical interval, elapsed steps, emitted total | `[7, 7, 7, 7]`, and negative `[-7, -7, -7, -7]` | `[70000, 70000, 70000, 70000]`, and negative `[-70000, -70000, -70000, -70000]` |

Direct binary64 and residual carry therefore have observable cadence drift even
when they begin with the same evaluator-produced authored rate. Carry reduces
per-frame loss, but is not a cadence-invariance proof. Exact deadlines agree
only because this particular authored rate is an exact whole mechanics-unit
rate per declared span; it is not a fallback conversion for arbitrary binary64
rates.

The test also covers a positive/negative zero crossing, a caller-side
`MechanicsScalar` cap, positive and negative nearest-even half ties, and the
existing mechanics plus/minus 1e12 boundary (including rejection outside both
ends). A cap remains downstream policy: the probe returns an exact delta and
does not store a resource.

## Boundary receipts and continuation

`gameplay_standard::quantize_rate_with_caller_residual` produces a
`CadencedQuantizationReceipt` with the quantizer's normalized source bits,
mode, policy version, exact result, and normalized remainder, plus the
canonical declared interval, its binary64 readout bits, and cadence identity.
The caller receives the next `ResidualCarry`; no global or component-owned
accumulator exists. `attempt_quantize_rate_with_caller_residual` retains the
same source/interval/cadence/policy context for an out-of-range rejection.

Carries bind to a residual schema version distinct from the quantization policy,
continuous evaluator version, mode, full typed quantization source, prior
canonical interval, and cadence. A same-local-ID `Fact` cannot resume a
`Parameter` carry. A changed cadence is permitted only through explicit
`CadenceTransition`; the next receipt records it. A hidden, mismatched, stale,
or corrupt carry is rejected. Rational interval identity is canonical
(`1/60 == 2/120`) while the receipt retains its normalized binary64 interval
readout.

`ResidualCarrySnapshot` and `Binary64AccumulatorSnapshot` are deliberately
small untrusted record shapes. Reopening checks finite bits, the mode-specific
remainder interval, residual schema/evaluator/policy/mode, and source context.
`ExactDeadlineAccumulatorSnapshot` validates its recovered emitted total. The
focused continuation probe saves after half of a 60 Hz run and proves reopen
equals uninterrupted continuation for direct binary64, residual carry, and
exact deadlines under identical declared inputs.

Typed failures distinguish invalid interval or cadence identity, non-finite
step result, quantizer out-of-range candidate, stale schema/evaluator/policy/
mode/source, hidden or mismatched cadence transition, invalid restored carry/
accumulator, and exact-deadline overflow. Deadline advancement computes every
checked value before it changes elapsed or emitted state. These are evidence
identities, not product error presentation.

## Recommendation for continuous mechanics

Do not put a generic residual field, cadence loop, or persistence schema into
Engine merely because named quantization exposes a remainder. Task 7189 may add
persisted continuous values, stats, or tracks, but must not embed generic rate
residual, cadence, or cap-order semantics in them from this probe. A future
continuous-mechanics owner must either:

1. define a concrete persisted component/service whose value, residual, policy
   version, interval interpretation, cadence identity, cap behavior, and
   migration semantics are all one coherent product-neutral mechanism; or
2. leave rate integration and carry entirely with the downstream caller, which
   must persist the full carry record and declare every changed cadence.

The current evidence favors option 2 until a concrete continuous resource has
earned the first mechanism. Exact tick deadlines are a separate exact-authoring
route, not a conversion fallback for arbitrary continuous rates. The exact
vitality column is unaffected. A quantized `MechanicsScalar` delta does not
prove that applying it to caller state is safe: the caller must still check
state overflow and decide cap-before/after-delta ordering. The cap exercise in
the probe is explicitly a downstream clamp example, not Engine authority.

## Focused evidence

```text
cargo test -p gameplay-standard --locked
cargo clippy -p gameplay-standard --all-targets --locked -- -D warnings
```

The cadence tests are in
[`cadence.rs`](../../../rust/crates/gameplay-standard/src/cadence.rs). They do
not prove a downstream product's clock, storage, or gameplay policy.
