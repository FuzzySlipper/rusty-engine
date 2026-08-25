# Downstream Engine dependency and Studio boundary

This path retains its old revision-oriented filename for links from historical
work, but the revision-pinning contract it once described is retired. Use the
[downstream renderer and Studio boundary](downstream-renderer-and-studio.md)
as the current authority.

## Current Rust dependency

Local downstream repositories sit beside the Engine checkout and declare the
complete Rust facade once:

```toml
[dependencies]
rusty-engine = { path = "../rusty-engine/rust/crates/rusty-engine" }
```

The facade preserves every public Engine library under its existing namespace,
such as `rusty_engine::entity_state` and
`rusty_engine::renderer_webview_host`. Downstream does not select individual
Engine crates or carry a second renderer/package closure.

The sibling checkout is consumed exactly as it stands. Downstream source,
scripts, and CI must not fetch, pull, reset, clean, checkout, pin, or otherwise
manage it. Another machine's operator may choose when to update that machine's
checkout, but that policy is outside downstream source and CI. Interface
breakage is expected to be loud and fixed forward.

Unrelated Cargo and npm lockfiles continue to manage their ordinary third-party
dependencies. They are not an Engine revision protocol.

## Renderer and Studio ownership

Native products submit canonical retained facts through the public facade and
the Engine-owned Rust webview host. Browser, Tauri, and Electron products that
need rich DOM use the one bundled
`@rusty-engine/application-host` entry point. Downstream does not install,
import, build, configure, or deep-import Engine Studio, renderer TypeScript,
Three, the private bridge, or the renderer package closure.

Engine-hosted Studio is the ordinary authoring product. A downstream Studio
project contributes only project data, a trusted root-local
`.rusty-studio.json`, and its project-owned Rust adapter. The persistent
`rusty-studio.service` is the recommended interactive entrypoint where
installed; concurrent automation uses isolated hosts, ports, settings roots,
and project copies as described by the current boundary document.

## Focused verification

When an Engine interface changes, run the narrowest affected downstream
compile, adapter, or browser proof. An Engine task does not automatically launch
every downstream repository's full suite. Engine-owned checks remain focused on
Engine unless a consumer proof is explicitly selected.

Exact source and consumer commits are useful reproducibility and review
evidence. Record them in Den task or review evidence rather than introducing a
committed dependency pin, synchronizer, or freshness ceremony.

```bash
./scripts/verify-rust-sdk-consumer.sh
```

The first command proves the complete local Rust facade in a clean temporary
consumer and does not change a sibling checkout.

## Character controller facade and selected consumer

The host-neutral FPS controller is consumed through the complete facade:
`rusty_engine::engine_spatial` exports `CharacterControllerService`, its
defaulted non-exhaustive config family, commands, receipts/facts, capsule query
types, and the separate `FirstPersonLookService`; `rusty_engine::entity_state`
exports the inert `CharacterMotionComponent` and its exact atomic publication
types. A controlled entity is authored with Transform plus character motion and
must not also carry the legacy kinematic or rigid-body component. Downstream
owns the fixed-step call schedule and semantic command construction.

`./scripts/verify-rust-sdk-consumer.sh` is the current Engine-owned clean facade
proof. The facade example is a directly runnable composition route:

```bash
cargo run -p rusty-engine --example character_controller --locked
```

For task 6847, CraftSurvive at `/home/dev/rusty-craftsurvive` remains the
explicit selected product consumer. The Engine-owned task-scoped command is:

```bash
./scripts/verify-character-controller-consumer.sh /home/dev/rusty-craftsurvive
```

That script checks that the selected checkout declares the adjacent complete
facade path, then runs its existing `./scripts/verify.sh`. It is an explicit
integration selection, not dependency freshness policy and not permission to
mutate the sibling checkout. Its eventual browser route and Luna/max usability
result must be recorded with exact Engine and consumer revisions. Neither
browser nor Luna evidence is implied by the Rust facade or focused controller
tests.

## Historical evidence

Older migration records may name provider or consumer commits, package
preparation details, and exact certification inputs. Those records explain what
a past artifact or review used. They are not instructions to restore revision
pinning, downstream Studio ownership, or automatic cross-repository
verification.
