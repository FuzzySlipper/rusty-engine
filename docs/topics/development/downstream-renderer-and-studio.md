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

The ordinary `/home/dev/rusty-engine` sibling is a stable `main` integration
checkout, not an Engine development scratch tree. Substantial Engine work uses
a task branch in a separate persistent worktree. Once that work is coherent,
compile-clean, and passes its owning local checks, it is fast-forwarded into the
stable checkout; every adjacent consumer then sees the candidate automatically
without changing its manifest or running an updater. Main-branch CI and review
therefore evaluate the same exact revision consumed downstream. Review findings
are corrected forward through the task worktree and another checked
fast-forward. This isolates unfinished and intentionally non-compiling
intermediate edits while retaining loud, fixed-forward adoption of completed
API and behavior changes.

Downstream product checks normally run after promotion. A task may explicitly
require pre-promotion consumer evidence; in that exceptional case the test
orchestrator creates a disposable adjacency layout for the candidate Engine and
selected consumer rather than changing the consumer's normal dependency path
or destabilizing the shared integration checkout.

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

Camera poses use one renderer-neutral Engine convention: yaw zero faces `-Z`,
positive yaw turns toward `+X`, and positive pitch turns toward `+Y`. A pose's
forward vector is
`[sin(yaw) * cos(pitch), sin(pitch), -cos(yaw) * cos(pitch)]`. Downstream Rust
may use that convention for authoritative movement and ray queries; the Engine
backend alone converts it into the current renderer's private camera rules.

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

Ordinary retained sprites may select Engine's bounded lit-sprite material
modes through Rust render facts. Downstream does not import Three materials or
provide shader source. Color textures are sRGB; authored tangent-space normal
and depth textures are admitted as linear resources. Asset Pipeline owns those
source maps, while Engine owns strict admission, camera-facing realization,
alpha/shadow behavior, and disposal. See
[lit sprite shader comparison](../lit-sprite-shaders.md).

## Product playtesting uses the same public path

A downstream final-product playtest starts the real product and drives its
visible output with ordinary public input. It does not deep-import renderer
packages, add a second canvas or bridge, expose Engine-private handles, or move
gameplay meaning into browser state for testing. Browser diagnostics can help
reproduce a defect, but they remain labelled evidence rather than a production
readback contract or gameplay authority.

Model-driven sessions are completion, review, nightly, or release evidence and
do not belong in ordinary every-commit CI. Browser and native adapters provide
separate host evidence even when they share scenario intent. See
[Product playtesting and evidence authority](product-playtesting.md) for the
Engine fixture, evidence layers, and minimal downstream manifest/scenario.

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
strict frame/resource admission, whole-content replacement, the sole render cadence,
resize/DPR behavior, pointer/focus arbitration, startup/failure presentation,
and transactional cleanup. The downstream UI mount receives only:

- a renderer port for Rust-projected retained frames, typed presentation
  frames, authoritative camera poses, and user-gesture audio resume;
- an interaction port with `gameplay`, `interface`, and `modal` modes plus
  gameplay focus recovery; and
- its own DOM root, where Angular, another framework, or direct typed DOM code
  can build the product HUD, menus, forms, and accessibility tree.

The renderer port also exposes `createVoxelSpriteExperiment()` as a deliberately
experimental, disposable presentation attachment. It can capture an admitted
retained handle on explicit request or borrow four admitted prepared textures,
then compare the bounded sprite/relight/depth/splat modes without exposing
Three or adding those modes to Rust gameplay contracts. The attachment belongs
to the current renderer surface: complete-content replacement disposes it, and
subsequent use of the old port fails with `stale_renderer_port`. See the
[runtime voxel-sprite limitation](../../known-limitations.md#runtime-voxel-sprite-enhancement-experiment)
before using it in a downstream experiment.

It never receives the canvas, Three/WebGL objects, renderer package topology,
the private bridge, or a generic renderer command/eval path. The public
`RustyApplicationContent` aggregate carries one complete Rust-projected frame
plus its exact immutable packed-mesh, texture, animated-GLB, and WAV resource
bytes.
Engine snapshots
the caller's bytes before asynchronous work, validates bounded counts and byte
sizes, requires each identity to match its `sha256` content hash, and admits
only the media type owned by that resource family. The application host
privately derives renderer manifests and resolvers; downstream never does.

Audio remains presentation, not downstream behavior. A product publishes
content-addressed `audio/wav` bytes, sends Rust-authored audio operations through
`applyPresentation(...)`, and invokes `resumeAudio()` directly from a physical
user-gesture handler when browser policy requires it. The application host
owns the `AudioContext`, typed audio host, hash-checked resolver, replacement,
and disposal; downstream code never constructs Web Audio nodes or interprets
gameplay outcomes into sound choices.

World indicators follow the same public presentation route. A product submits
Rust-authored billboard operations for labels, exact-hash icons, ranged meters,
prompts, and status cues through `applyPresentation(...)`. The application host
owns one pointer-transparent overlay, camera-to-screen projection,
deterministic overlap/edge/suppression layout, semantic DOM realization, and
disposal. A configured entity-position resolver may expose current
gameplay-owned anchor positions; it does not return DOM or backend handles.
Downstream code must not create parallel health-bar DOM or import renderer
packages. See [structured world indicators](../world-indicators.md).

`initialContent` mounts the first aggregate. `replaceContent(...)` prepares a
fresh surface, resource catalog, listeners, and complete frame before one
atomic publication; failure disposes the candidate and leaves the prior
surface and content revision authoritative. `replaceFrame(...)` is the
focused complete-frame operation when the immutable resource catalog is
unchanged. These operations prevent reconnects and static-resource revisions
from replaying `create` operations into partial retained state.

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
Disposing a game route releases its session/input owners and may call the
public renderer `clear()` operation, which atomically installs Engine's empty
frame and empty resource catalog in the same sole surface slot. Complete
`replaceContent(...)` is for initial publication, reconnect, or a changed
immutable static-resource/frame identity; ordinary camera and dynamic
retained-frame updates use their focused ports and must not rebuild the
renderer surface on every observation.

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
- billboard or primitive-cube particle cues provide disposable visual feedback;
  optional collision is limited to captured emitter-local planes/AABBs; and
- telemetry overlays provide the fixed diagnostic presentation family.

Engine privately realizes particle descriptors in the Three scene and keeps the
DOM particle sink only as a compatibility/performance reference. Other fixed UI
descriptors continue to use Engine-owned DOM where appropriate. Downstream
supplies only typed Rust facts and observes typed receipts. It
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
loads one Rust-authored content aggregate through a purpose-named transport
owner, mounts the framework root through its callback, and supplies the
returned bounded context through ordinary dependency injection. Engine reports
bounded startup failure. Feature components and Rust transport owners live
outside the bootstrap. It does not own gameplay state, renderer/backend
construction, renderer manifest/resolver construction, resource lifecycle, a
second render loop, or another renderer control transport.

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
