# Gameplay resolution

## Ownership

`rust/crates/gameplay-resolution` owns a headless, host-neutral lifecycle for
one bounded downstream gameplay attempt. It owns structural traversal,
deterministic phase and interceptor order, correlation/causation, quotas,
staged transaction control, and generic receipts/traces.

It does not own gameplay vocabulary, authoritative game state, scheduling,
randomness policy, persistence, presentation, or authored TypeScript. Concrete
intents, facts, predicates, selectors, operations, effects, events, rejections,
faults, suspensions, and trace details remain downstream types.

## Source routes

| Path | Owner |
|---|---|
| `src/program.rs` | Structural `Sequence` / `When` / opaque-operation grammar |
| `src/policy.rs` | Downstream policy border and planned effects/events/children |
| `src/resolver.rs` | Standard phase owner, traversal, child planning, quotas, and one transaction commit |
| `src/transaction.rs` | Fail-atomic downstream staging/commit contract |
| `src/receipt.rs` | Attempt tree, commit outcome, effects, events, evidence, and trace readout |
| `src/trace.rs` | Structural phases and downstream detail sink |
| `src/identity.rs` | Explicit resolution/correlation/causation identities |
| `src/limits.rs` | Bounded traversal and output limits |
| `tests/contract.rs` | Headless lifecycle, preview/apply, hook, child, suspension, and failure proof |

The complete downstream facade re-exports the crate as
`rusty_engine::gameplay_resolution` without wrappers.

## Forbidden dependencies and shortcuts

- no `gameplay-rules`, `gameplay-mechanics`, `entity-state`, renderer, browser,
  Node, TypeScript, or downstream-game dependency;
- no attack, target, stat, damage, spell, item, condition, weapon, pickup, turn,
  or other game vocabulary;
- no ambient resolver recursion, global registry, callback persistence,
  scheduler, event bus, replay certification, or complete save format; and
- no authoritative mutation during `stage`: the downstream transaction must
  validate and publish once in `commit`, or leave authority unchanged.

## Focused verification

```bash
cargo test -p gameplay-resolution --locked
cargo clippy -p gameplay-resolution --all-targets --locked -- -D warnings
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```

Full provider verification remains `./scripts/verify.sh`.
