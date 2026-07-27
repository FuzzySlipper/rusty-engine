# Inspection and diagnostics

## Purpose

Route read-only structured inspection of Engine-owned facts and the
`rusty-inspect` command-line surface.

## Owns

- `engine-inspector`: structured diagnostics over entities, catalogs, imports,
  persistence, scenes, and voxels.
- Stable severity/code/context reporting for operator and agent consumption.
- Read-only CLI projection of those reports.

## Does not own

- Runtime authority, repair, mutation, orchestration, or policy.
- A dependency that ordinary runtime/library crates may import.
- Browser UI or renderer telemetry authority.

## Primary paths

- [`engine-inspector/src/lib.rs`](../../rust/crates/engine-inspector/src/lib.rs)
- [`engine-inspector/src/diagnostic.rs`](../../rust/crates/engine-inspector/src/diagnostic.rs)
- [`engine-inspector/src/main.rs`](../../rust/crates/engine-inspector/src/main.rs)
- [Inspection and diagnostics](../inspection-and-diagnostics.md)
- [`audit-standalone.sh`](../../scripts/audit-standalone.sh)

## Public downstream surfaces

- The library accepts owner facts and returns structured read-only reports.
- The `rusty-inspect` binary is a tool surface for humans and agents.
- Consumers may render diagnostics, but the diagnostics do not redefine the
  facts they observe.

## Private or forbidden paths

- Runtime and library crates must not depend on `engine-inspector`; it remains
  a leaf.
- Do not auto-repair state as a side effect of inspection.
- Do not collapse typed diagnostic identity into generic strings when callers
  can act on the distinction.

## Acceptance gates and fixtures

```bash
cargo test -p engine-inspector --locked
cargo clippy -p engine-inspector --all-targets --locked -- -D warnings
./scripts/audit-standalone.sh
./scripts/verify.sh
```

## Common agent mistakes

- Adding mutation because the inspector can see an inconsistency.
- Making runtime success depend on an optional observability path.
- Importing the inspector from a crate it inspects.
- Treating renderer telemetry as canonical state.

## Follow-up routing

- Fix a diagnosed invariant in the owning map, not in the inspector.
- Presentation telemetry:
  [Rust render model and projection](rust-render-model-and-projection.md).
- Product/operator UI belongs in the relevant downstream host.
