# Downstream Engine revision contract

## Default: one rolling Rust dependency

An ordinary downstream game depends on the complete `rusty-engine` Rust facade
from the canonical public `main` branch:

```toml
[dependencies]
rusty-engine = { git = "https://github.com/FuzzySlipper/rusty-engine", branch = "main" }
```

The facade preserves every public library as an exact namespace such as
`rusty_engine::entity_state` or `rusty_engine::render_model`. It has no feature
flags and downstream must not select individual Engine crates. The complete
namespace index is [the Rust SDK capability index](../../rust-sdk-capabilities.md).

The renderer implementation is included behind that Rust surface. A game does
not add `render/package.json`, pnpm dependencies, TypeScript imports, JavaScript
commands, or deep package paths. It submits Rust-owned render and presentation
frames through `rusty_engine::renderer_webview_host` and receives typed Rust
observations from `rusty_engine::render_host_contracts`. Engine owns the
compiled TypeScript/Three artifact and its private bridge.

This is intentionally rolling-current during development. `Cargo.lock` records
the exact Engine commit used by one build, but it is not a request to remain on
that commit. Engine changes may break downstream compilation. That loud failure
is preferred to an agent silently retaining an old revision and reimplementing
a mechanism that has since moved upstream.

## Required freshness check

Downstream CI and the start of ordinary agent work update or check the Engine
lock before feature work:

```bash
cargo update -p rusty-engine
python3 ./scripts/check_downstream_engine_freshness.py --manifest ./Cargo.toml ./Cargo.lock
```

Consumers may copy the checked Engine helpers
`scripts/check_downstream_engine_freshness.py` and
`scripts/sync-downstream-engine.sh` into their repository. The checker requires:

- exactly one locked `rusty-engine` package;
- exactly one direct dependency from the canonical Engine repository, and it is
  the complete `rusty-engine` facade rather than an individual crate;
- the canonical Git repository and `main` branch source;
- an exact 40-character resolved commit in the lock; and
- equality with current public `main`, resolved with `git ls-remote`.

It fails with both the locked and current revisions when stale. It does not
silently rewrite source, switch repositories, inspect a sibling checkout, or
hide a compile/protocol failure. A temporarily unavailable public repository is
also loud because freshness could not be established.

The sync helper accepts an explicit downstream `Cargo.toml`, runs `cargo update
-p rusty-engine`, and then runs the same freshness check. Downstream owns when
that mutation is appropriate; Engine never reaches into another checkout.

## Exact revisions are exceptional

Use an exact public revision only for a bounded review packet, release
reproduction, rollback, or reverse-certification fixture:

```toml
[dependencies]
rusty-engine = { git = "https://github.com/FuzzySlipper/rusty-engine", rev = "<40-character-public-sha>" }
```

The reason and exit condition must accompany the pin. Return to the rolling
branch after the review, release, or rollback investigation. Do not normalize a
temporary pin into the development workflow and do not split Engine into
separate crate or language revision lanes.

Engine-owned reverse-certification fixtures may name exact downstream and
provider commits. They are immutable evidence, not dependency instructions for
ordinary downstream work. Historical provenance hashes likewise remain
historical facts and are never rewritten by the freshness helper.

## Consumer proof

Engine verifies the supported shape with:

```bash
./scripts/verify-rust-sdk-consumer.sh
./scripts/verify-rust-sdk-consumer.sh <40-character-public-sha>
```

The proof creates a clean Rust binary with exactly one dependency, exercises
representative entity, retained-frame, presentation, host-contract, and webview
adapter namespaces, and rejects any package-manager carrier. The optional exact
revision form is the post-push/review proof.

## Ownership and non-goals

Downstream still owns authoritative gameplay state, substantial game logic,
orchestration, storage policy, window/event-loop policy, semantic input
mapping, content meaning, and product acceptance. The Rust adapter owns one
renderer instance and its private webview boundary; renderer picks, physical
input, readouts, and diagnostics remain observations that downstream
revalidates or interprets.

This contract does not introduce a registry, release train, universal runtime,
provider-owned consumer list, cross-repository mutator, generic JavaScript
escape hatch, or promise that Engine development changes are compatible.
