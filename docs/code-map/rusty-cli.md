# Rusty Product workflow CLI

## Ownership

`rust/crates/rusty-cli` owns the binary-only `rusty` command boundary for
Product Layout discovery, safe minimum initialization, read-only conformance,
bounded inspection, generated Product Assembly development/test/build, and
exact pre-wrapper package publication.

The CLI composes existing Product Model owners. It does not evaluate authored
logic, register live callbacks, acquire Product Kernel meaning, or implement a
desktop wrapper. `rusty package` seals the selected wrapper policy and exact
runtime closure; wrapper realization and headed proof remain a separate owner.

## Primary paths

- [`rusty-cli/src/main.rs`](../../rust/crates/rusty-cli/src/main.rs)
- [`rusty-cli/src/args.rs`](../../rust/crates/rusty-cli/src/args.rs)
- [`rusty-cli/src/init.rs`](../../rust/crates/rusty-cli/src/init.rs)
- [`rusty-cli/src/check.rs`](../../rust/crates/rusty-cli/src/check.rs)
- [`rusty-cli/src/commands.rs`](../../rust/crates/rusty-cli/src/commands.rs)
- [`rusty-cli/src/workflow.rs`](../../rust/crates/rusty-cli/src/workflow.rs)
- [`rusty-cli/src/kernel_probe.rs`](../../rust/crates/rusty-cli/src/kernel_probe.rs)
- [`rusty-cli/src/inspect.rs`](../../rust/crates/rusty-cli/src/inspect.rs)
- [`rusty-cli/src/package.rs`](../../rust/crates/rusty-cli/src/package.rs)
- [`rusty-cli/src/report.rs`](../../rust/crates/rusty-cli/src/report.rs)
- [`rusty-cli/src/tests.rs`](../../rust/crates/rusty-cli/src/tests.rs)
- [`render/scripts/rusty-cli-browser-test.mjs`](../../render/scripts/rusty-cli-browser-test.mjs)

## Source routes

| Path | Owner |
|---|---|
| `src/main.rs` | Thin process boundary, command dispatch, stable exits |
| `src/args.rs` | Current command and output-format parsing |
| `src/init.rs` | Complete-write preflight, staging, publication, rollback, exact repeated invocation |
| `src/check.rs` | Root discovery, bounded reads, realpath containment, layout and prohibited-host checks, doctor posture |
| `src/commands.rs` | Thin command contracts, layered test disposition, generated-host process boundary |
| `src/workflow.rs` | Read-only admission, materializer/assembly orchestration, isolated generated Cargo build |
| `src/kernel_probe.rs` | Bounded compiled `RustyProductRuntime` capability discovery; never Rust text parsing |
| `src/inspect.rs` | Bounded Product Model/linkage/receipt facts and explicit unavailable live readouts |
| `src/package.rs` | Exact staged pre-wrapper runtime package, deterministic receipt, relocation readback |
| `src/report.rs` | Ordered, field-bounded and aggregate-bounded human/JSON diagnostics |
| `src/tests.rs` | Discovery, initialization, non-mutation, symlink, host-shape, output-kind, and reporting proof |
| `render/scripts/rusty-cli-browser-test.mjs` | Real Chromium loopback-root and stable one-canvas proof for `rusty test`; optional conformance hooks add fixture-only semantics |

The package is deliberately binary-only. It does not add a library namespace
to the complete `rusty-engine` facade.

## Forbidden dependencies and shortcuts

- no shell-owned validation semantics or Rust source-text capability parsing;
- no live runtime registry, callback admission, product authority, or wrapper
  implementation;
- no browser claim from HTTP/headless Rust evidence: `rusty test` invokes the
  explicit real-Chromium proof owner;
- no overwrite of conflicting product source or repair during `check` and
  `doctor`; and
- no claim that the pre-wrapper package is a Tauri/Electron application.

## Focused verification

```bash
cargo test -p rusty-cli --locked
cargo clippy -p rusty-cli --all-targets --locked -- -D warnings
python3 scripts/dependency_boundary_check.py
```

Full provider verification remains `./scripts/verify.sh`.
