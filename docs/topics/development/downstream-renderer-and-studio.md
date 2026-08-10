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

## Rich DOM presentation without renderer ownership

Browser-delivered products and webview wrappers use one additional public
artifact from the adjacent Engine checkout:

```json
{
  "dependencies": {
    "@rusty-engine/application-host": "file:../rusty-engine/render/artifacts/application-host"
  }
}
```

That artifact bundles its complete renderer closure. Its manifest and the
downstream lock expose no `render-contracts`, `render-projection`,
`renderer-host`, `renderer-three`, Three, or Studio dependency. Downstream code
does not build, select, configure, or deep-import those packages.

`mountRustyApplication` creates one Engine-owned composition root containing
the canvas and one bounded downstream UI root. It owns renderer startup,
strict frame admission, whole-frame replacement, the sole render cadence,
resize/DPR behavior, pointer/focus arbitration, startup/failure presentation,
and transactional cleanup. The downstream UI mount receives only:

- a renderer port for Rust-projected frames and authoritative camera poses;
- an interaction port with `gameplay`, `interface`, and `modal` modes plus
  gameplay focus recovery; and
- its own DOM root, where Angular, another framework, or direct typed DOM code
  can build the product HUD, menus, forms, and accessibility tree.

It never receives the canvas, Three/WebGL objects, renderer package topology,
the private bridge, or a generic renderer command/eval path. Incremental frame
diffs are admitted atomically. Complete frame replacement prepares a fresh
surface before publishing it and retires the prior surface only after the new
frame succeeds, so reconnects and static-resource revisions do not replay
`create` operations into partial retained state.

The same bootstrap runs in an ordinary web application, Tauri, or Electron.
Only the downstream-owned typed Rust transport adapter varies: HTTP/WebSocket,
Tauri command/event wiring, or another host-neutral carrier can deliver the
same Rust frame and semantic intents without changing DOM composition.

The interaction mode controls the Engine-owned renderer/input surface, while
`context.ui.allowsGameplayInput(event)` synchronously classifies an original
DOM event for a downstream global adapter. Every `window`, `document`, or outer
host listener must pass its event through that operation before assigning
gameplay meaning. It rejects buttons, links, text entry, dialogs, and explicitly
interactive UI even if their later click handler has not changed the coarse
mode yet. Pointer-transparent and background gameplay regions continue to pass
while the mode is `gameplay`. Do not defer suppression until a click handler
opens a panel. Switching to `interface` or `modal` also releases Engine capture;
returning to `gameplay` uses the public focus-recovery operation.

Keep the application host at application scope when product routes change.
Disposing a game route releases its session/input owners and calls the public
renderer `clear()` operation, which installs Engine's empty frame while
preserving the one Engine-owned canvas. Full `replaceFrame(...)` is for initial
publication, reconnect, or a changed immutable static-resource/frame identity;
ordinary camera and dynamic retained-frame updates use their focused ports and
must not rebuild the renderer surface on every observation.

Downstream UI modules, templates, styles, assets, and local project files are
trusted application source, just like downstream Rust. This boundary enforces
lifecycle, decoding, and authority; it is not a hostile plugin boundary. Do
not add an HTML sanitizer, executable-module sandbox, CSP framework,
capability-security layer, or security ceremony to protect Engine from its own
application. Loading third-party or user-authored executable UI would require
a separate explicit threat model and task.

### Fixed Rust-only DOM presentation

The native adapter's fixed private document owns both its canvas and one DOM
presentation region. Downstream Rust can place UI-like presentation in that
region without owning DOM or renderer code by submitting typed
`render_presentation` operations through `RendererWebviewAdapter`:

- billboards provide bounded localized text, values, or icons anchored to a
  world position or entity;
- particle billboard cues provide disposable visual feedback; and
- telemetry overlays provide the fixed diagnostic presentation family.

Engine privately realizes those descriptors as DOM where the current host uses
DOM. Downstream supplies only typed Rust facts and observes typed receipts. It
does not receive an element, selector, callback, template slot, or raw event.

This remains the Rust-only path for products that do not have a product DOM.
Do not inject markup into the fixed `RendererWebviewAdapter` document or use
private bridge calls. A product that needs arbitrary rich DOM uses the public
application host instead; both paths retain the same Rust authority posture.

## Minimal product bootstrap

For the native/webview adapter path there is no downstream web bootstrap.
Downstream Rust creates its outer window/event loop, mounts one
`RendererWebviewAdapter`, submits frames and presentation, and handles typed
observations. Engine generates and owns the child HTML document and private
renderer startup.

A browser/Tauri/Electron product owns one deliberately small composition root:

```text
index.html -> main.ts -> mountRustyApplication -> mount downstream UI root
```

`index.html` supplies one empty root. `main.ts` calls the public mount function,
mounts the framework root through its callback, and supplies the returned
bounded context through ordinary dependency injection. Engine reports bounded
startup failure. Feature components and Rust transport owners live outside the
bootstrap. It does not own gameplay state, renderer/backend construction,
frame decoding, resource lifecycle, a second render loop, or another renderer
control transport.

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
- For a web UI, is `@rusty-engine/application-host` the only Engine package?
- Does authoritative meaning stay in downstream Rust?
- Does renderer interaction use named Rust operations and typed readouts only?
- Does Studio integration stop at `.rusty-studio.json` plus the Rust adapter?
- Is an automated or concurrent Studio run isolated from the persistent
  single-session service?

If any answer is no, stop at the boundary and fix or extend the owning Engine
surface rather than creating downstream renderer or Studio authority.
