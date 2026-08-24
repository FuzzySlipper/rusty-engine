# Rusty CLI foundation

## Ownership

`rust/crates/rusty-cli` owns the binary-only `rusty` command boundary for
Product Layout discovery, safe minimum initialization, filesystem conformance
checks, and foundational doctor diagnostics.

This campaign foundation does not compile Runtime Composition, admit runtime
content, run a product, publish generated assemblies, serve a browser host, or
build desktop wrappers. `doctor` reports that incomplete posture explicitly.

## Primary paths

- [`rusty-cli/src/main.rs`](../../rust/crates/rusty-cli/src/main.rs)
- [`rusty-cli/src/args.rs`](../../rust/crates/rusty-cli/src/args.rs)
- [`rusty-cli/src/init.rs`](../../rust/crates/rusty-cli/src/init.rs)
- [`rusty-cli/src/check.rs`](../../rust/crates/rusty-cli/src/check.rs)
- [`rusty-cli/src/report.rs`](../../rust/crates/rusty-cli/src/report.rs)
- [`rusty-cli/src/tests.rs`](../../rust/crates/rusty-cli/src/tests.rs)

## Source routes

| Path | Owner |
|---|---|
| `src/main.rs` | Thin process boundary, command dispatch, stable exits |
| `src/args.rs` | Current command and output-format parsing |
| `src/init.rs` | Complete-write preflight, staging, publication, rollback, exact repeated invocation |
| `src/check.rs` | Root discovery, bounded reads, realpath containment, layout and prohibited-host checks, doctor posture |
| `src/report.rs` | Ordered, field-bounded and aggregate-bounded human/JSON diagnostics |
| `src/tests.rs` | Discovery, initialization, non-mutation, symlink, host-shape, output-kind, and reporting proof |

The package is deliberately binary-only. It does not add a library namespace
to the complete `rusty-engine` facade.

## Forbidden dependencies and shortcuts

- no shell-owned validation semantics;
- no TypeScript compiler, Node, browser, renderer, Studio, application host,
  downstream product, or wrapper generator dependency;
- no overwrite of conflicting product source or repair during `check` and
  `doctor`; and
- no claim that the foundation commands certify the unfinished campaign
  workflow.

## Focused verification

```bash
cargo test -p rusty-cli --locked
cargo clippy -p rusty-cli --all-targets --locked -- -D warnings
python3 scripts/dependency_boundary_check.py
```

Full provider verification remains `./scripts/verify.sh`.
