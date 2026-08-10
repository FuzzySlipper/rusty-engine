# Downstream renderer and Studio boundary

This document is the central authority for how an ordinary downstream game uses
Rusty Engine rendering and Studio. It describes the supported current path, not
historical package-consumer or exact-revision migration evidence.

## One Rust dependency

Local downstream repositories are expected to sit beside the Engine checkout
and depend on the complete Rust facade once:

```toml
[dependencies]
rusty-engine = { path = "../rusty-engine/rust/crates/rusty-engine" }
```

The facade re-exports every public Engine library under its existing namespace.
Downstream code uses names such as `rusty_engine::entity_state`,
`rusty_engine::render_projection`, and
`rusty_engine::renderer_webview_host`; it does not select a smaller set of
Engine crates.

The sibling checkout is consumed as it stands. Downstream tooling must not
fetch, pull, reset, clean, checkout, pin, or otherwise manage it. An operator on
another machine may decide when to update that machine's Engine checkout, but
that policy is outside downstream source and CI. Interface breakage should be
loud and fixed forward. Exact commits remain useful review evidence; they are
not a source dependency protocol.

## The green renderer path

The supported native path keeps the current backend entirely behind an
Engine-owned Rust boundary:

```text
downstream Rust facts and game meaning
        |
        v
rusty_engine facade
        |
        +--> render_model / render_projection / render_presentation
        |         renderer-neutral retained facts
        |
        v
renderer_webview_host::RendererWebviewAdapter
        |         named Rust operations and typed observations
        v
Engine-private compiled renderer and backend
```

The return path is equally important:

```text
physical input, picks, lifecycle, and renderer observations
        |
        v
render_host_contracts typed Rust readouts
        |
        v
downstream Rust assigns semantic meaning and changes authoritative state
```

Downstream agents do not need to know that the current implementation uses
TypeScript, Three, WebGL, a private bridge, object URLs, or a child HTML
document. They must not import, configure, duplicate, or test those internals.
Engine owns strict decoding, resource realization and disposal, viewport and
backend lifecycle, physical input capture, picking, and renderer observations
at that sensitive boundary.

Downstream still owns the game: authoritative facts, gameplay semantics,
orchestration and scheduling, content meaning, storage policy, resource
admission, the outer window and event loop, semantic input mapping, and
user-facing product acceptance. A pick or key readout is an observation until
downstream Rust deliberately applies game meaning. Renderer and UI state never
become a second gameplay authority.

Product-specific presentation should use typed Rust frame and host contracts.
Do not create a second downstream canvas, renderer package graph, JavaScript
bridge, or renderer control transport. If a needed presentation capability is
absent, request a game-neutral Engine mechanism instead of reaching through the
boundary.

## Studio is an Engine-hosted product

Rusty Studio runs from Rusty Engine. A downstream repository does not install,
import, build, bundle, configure, or copy Studio, renderer TypeScript, or Three
packages. Its complete ordinary Studio integration is:

- project content and storage policy owned by that repository;
- one trusted root-local `.rusty-studio.json` bootstrap; and
- one project-owned Rust adapter implementing the closed Studio protocol.

The bootstrap tells the generic Engine host how to start the adapter. The
adapter understands the downstream schema, validates and publishes mutations,
and projects canonical rereads. Studio owns the editor application and current
renderer implementation without acquiring project or gameplay meaning.

On a machine with the persistent service installed, use that service as the
normal interactive entrypoint and open the downstream root there. On machines
where Studio is used regularly, establishing a local persistent service is the
recommended setup. See [Persistent generic Studio service](../studio-service.md)
for installation, health, update, binding, and rollback operations.

Service lifecycle is an operator concern. Restart/prepare may package the
current committed Engine checkout without fetching it. Agents must not run a
service update, pull, or checkout as an incidental part of downstream work;
another machine may establish its own periodic update policy.

## Current concurrency limit

The persistent service is not currently a multi-user session boundary. One
host process owns one selected adapter and one active project for all connected
browsers. Opening another root replaces that process-wide selection, so two
agents can redirect or stale each other's work. Project and settings hashes
reject many stale writes, but rejection is conflict detection, not isolation.

Use the persistent service for one active interactive authoring session at a
time. Concurrent agents, browser automation, and acceptance tests should start
separate Studio host processes on unique loopback ports with separate
host-user-settings roots. Agents mutating the same project also need separate
project copies/worktrees or explicit coordination; separate ports do not make
shared project files safe. Do not claim shared-service concurrency until a
dedicated session-isolation implementation and probe prove it.

## Agent checklist

Before adding a downstream rendering or Studio dependency, verify:

- Is this already reachable through the complete `rusty-engine` Rust facade?
- Does authoritative meaning stay in downstream Rust?
- Does renderer interaction use named Rust operations and typed readouts only?
- Does Studio integration stop at `.rusty-studio.json` plus the Rust adapter?
- Is an automated or concurrent Studio run isolated from the persistent
  single-session service?

If any answer is no, stop at the boundary and fix or extend the owning Engine
surface rather than creating downstream renderer or Studio authority.
