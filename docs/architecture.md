# Engine architecture

Rusty Engine hosts an ordinary C# product. The product decides game
meaning; the Engine guarantees reusable mechanisms and integration.

## Campaign #8723: authority and reading this document

The September architecture reset authorizes removal of the validation, replay,
transaction, and recovery framework within its child tasks. The owner's current
direction and task scope take precedence over preservation language in this
document and linked guides. See [AGENTS.md](../AGENTS.md#architecture-reset--campaign-8723)
and Den document `rusty-engine/architecture-reset-2026-09`.

Descriptions below identify current source owners and behavior.
Exact-revision preconditions, replay restrictions, receipts, and fresh-baseline
recovery are all subject to removal.
They are not acceptance requirements for their replacements. The same applies
to existing validators, caps, leases, copies, and generation tools.

Start with a runnable removal experiment where the task is exploratory. Add
back only the smallest mechanism justified by an observed failure or concrete
required behavior. Old tests and contracts alone do not justify preservation;
change them with the behavior. Product/Engine ownership and actual ABI/layout/
lifetime correctness still apply. Keep this document truthful about what has
landed, and distinguish experimental results from production behavior.

## Ownership flow

```text
C# product application and state
  │  product rules, orchestration, content meaning, policy
  ▼
Rusty.Engine safe services and optional managed helpers
  │  generated contracts/values plus product-side composition helpers
  ▼
Rusty.Engine native bridge and C ABI function table
  │  compiled interop and service implementations, copied values, explicit leases;
  │  a small generated product export selects the product and its debug catalog
  ▼
Rust Engine services and runtime host
  │  lifecycle, input, renderer, spatial mechanisms, content, persistence
  ▼
browser/host implementation and DOM UI
```

The arrows describe a cooperation boundary, not a hierarchy of game authority.
C# does not need to imitate Rust. Rust mechanisms remain upstream so a product
does not grow its own renderer, platform host, resource loader, or native ABI.

## Source owners

| Layer | Current owner | Source of truth |
| --- | --- | --- |
| ABI declarations | Rust | [`csharp-engine-abi`](../rust/crates/csharp-engine-abi) defines the C ABI and named function tables. |
| Concrete Engine bridges | Rust | [`csharp-engine-services`](../rust/crates/csharp-engine-services) implements ABI-backed named capabilities. |
| Dynamics bodies and ropes | Rust | `svc-collision::DynamicsSolver` keeps one live Rapier world per Dynamics world (bodies, static colliders, rope joints) and changes it in place. The Dynamics bridge maps generated handles onto it, owns chains, and binds Spatial collision scenes. See [rope physics](rope-physics.md). |
| Retained graphics intent | Rust | `render-presentation::PresentationWorld` owns the committed graphics graph, snapshots, and publication revision. Existing appearance and voxel projectors feed typed changes into it. |
| wgpu realization | Rust | [`render-wgpu`](../rust/crates/render-wgpu) applies `PresentationWorld` deltas to typed GPU tables and renders offscreen (with readback) or to a window surface. It is the only crate that may depend on wgpu. The runtime does not drive it yet: the streaming mode (#8786) and desktop shell (#8790) wire it in, and Three stays the browser realizer until #8792. |
| Session serialization and recovery facts | Rust | `runtime-session` currently owns the runtime guard, receipts, prepared replacement, and recovery vocabulary; `product-dev-host` adapts them to transport. Campaign #8723 may collapse or remove these layers. |
| Runtime publications | Rust | `runtime-publication` carries typed graphics, presentation, UI, cues, and baseline facts. Runtime operations return these before the host converts them to browser DTOs and applies delivery byte limits. Input acknowledgements and the runtime readout remain host observations. |
| Runtime diagnostics | Rust | `runtime-diagnostics` owns bounded events, cursors, coalescing, and raw update attribution. The development host attaches its file/stderr writer to the shared sink. |
| Binding generation | Engine tooling | [`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh) runs cbindgen, ClangSharp, and the binding generator. |
| Safe C# contracts and native bridge | Generated and handwritten C# | [`Rusty.Engine`](../csharp/Rusty.Engine) compiles the generated contracts, values, internal interop and service implementations from ignored `obj/Generated` output, plus the handwritten [`ProductBridge`](../csharp/Rusty.Engine/NativeProduct/ProductBridge.cs) that implements the product ABI table and lifetime. |
| Product bootstrap | Generated C# | [`Rusty.Engine.ProductGenerator`](../csharp/Rusty.Engine.ProductGenerator) emits only what depends on the product: a safe `rusty_product_bind_v1` export naming the product constructor, and the product's debug command catalog. It serves CoreCLR and NativeAOT alike. |
| Product lifecycle and host | Rust | [`csharp-product-runtime`](../rust/crates/csharp-product-runtime) loads the product and drives its lifecycle. |
| Product logic | Downstream C# | The product implements the generated `IEngineProduct` contract and owns its own state and code organization. |
| UI and host/backend implementation | TypeScript/host | DOM UI and explicit Engine host/backend work only; not downstream gameplay ownership or game-rendering substitution. |

The generated ABI surface is deliberately a capability surface, not a claim of
full source-level Rust API coverage. Its current shape is defined by the Rust
ABI crate and binding generator; use current Den coverage guidance for
planning, rather than copying a volatile service table into this document.

Named capabilities should expose composable Engine mechanisms to trusted C#
callers. A convenient helper or authored-content route must not be the only
way to select facts that an underlying mechanism already supports. Keep
resource ownership, retained state, and realization in the Engine while
providing direct typed operations for those facts. Defaults should guide
ordinary use without silently overriding an explicit caller selection.
Missing bindings and restrictive adapter policy are upstream gaps, distinct
from genuinely new Engine mechanisms; exposing every implementation detail
is not the objective.

Apply the same distinction to failures. A failing Engine operation returns its
status and diagnostic to C#, and that is its only consequence: a product that
catches `EngineCallException` continues normally.

A product call is not a transaction. Engine services change their state
directly while the product runs, and there is no rollback. When a C# exception
escapes a callback (or the Engine fails while finishing the call's renderer
work), the runtime keeps everything the call did, publishes it, and logs the
full exception with its stack trace. It then moves the lifecycle to `Faulted`:
simulation stops, the product stays loaded for inspection, and renderers get a
fresh baseline. `Resume` continues the same product and `Restart` resets it.
A runtime error never ends the host process; the supervisor restarts the
runtime only when the process itself dies (see *Packaging and development*).

## Trusted runtime resource delivery

Engine-owned resource delivery trusts the supplied identity and bytes. Browser
loading and renderer admission do not rehash bodies or repeat format checks;
the consuming decoder handles the format. Resource buffers are immutable by
contract and may be borrowed. Copy only where an API needs its own storage
(for example, audio decoding that detaches its input) or an exact buffer range.
Build/import content identity and cache invalidation remain separate concerns.
Add runtime verification only for a concrete identified requirement.

## Lifecycle and data movement

1. The packaged Rust runtime loads the staged Product through CoreCLR during
   ordinary development, or through its NativeAOT module during an explicit
   fidelity/release check, and calls the same generated versioned bind.
2. That bind verifies the exact SDK/runtime ABI identity before product
   construction. The generated bootstrap then constructs an `IEngineContext`
   from named safe Engine services, copies product content/input configuration,
   and creates the product with `ProductCreateContext`.
3. Rust sends lifecycle calls and update facts. The generated bootstrap copies
   input events into C# values and forwards `ProductUpdate` to the product.
4. The product uses named services to read or publish facts. Explicit leases
   make retained native resources disposable on the C# side; borrowed data must
   not be stored past its documented call/lease boundary. A borrowed result
   (a `Native*Result` with pointer/`_len` collections, such as Dynamics
   `StepAndRead` and `ReadWorld`) points into bridge storage that stays valid
   until the next call on the same service context. The generated wrapper
   copies it before returning, with no handle or destroy call. Other transient
   results still use a lease and destroy call until they move to this shape.
5. The Engine turns admitted product presentation facts into its renderer
   state. DOM UI observes Engine-supported UI/projection paths and emits
   semantic input; it does not become a second game implementation.

Ghost plates and standalone microvoxel objects illustrate this boundary. C#
selects a retained `Graphics` appearance source, placement, capture/configuration, and
ordinary material bindings. The Engine performs the retained capture or voxel
mesh projection, owns renderer resources and cleanup, and returns copied
observation/readout facts. Ghost direction uses an Engine-selected hard snap
among 1/4/8/16 captured sectors with optional hysteresis; a plate is a frozen
source pose. The MagicaVoxel object path uses bounded v150 model admission,
ordinary matte-capable materials, and the generated greedy surface's
axis-aligned face normals. Neither path asks a downstream product to supply a
voxel shader, browser renderer, or TypeScript game implementation.

The Engine fixtures exercise provider generation, ABI, lifecycle, and both
loaders. They are not downstream product architecture or launch templates.

## Reconstructible presentation

This section describes the current implementation. Its candidates, transport
ordering, revision checks, and recovery paths are starting points for campaign
experiments, not requirements to reproduce in a simpler design.

C# selects presentation facts through named services. Rust commits the resulting
graphics intent into `PresentationWorld`; TypeScript realizes that intent in
browser/GPU objects. Fresh browser attachment reads committed Rust snapshots
without calling the product's `Attach` callback. Graphics snapshots preserve
active handles and resource dependencies, with a `presentation-world`
continuation revision. The runtime session guard covers snapshot capture and
the output cursor handover, so subsequent deltas follow that snapshot. The
presentation revision and transport cursor remain separate facts.
The runtime moves owned typed publications through operation settlement into
ProductDev wire DTOs. Publication and host adapters do not clone and readmit
already admitted graphics, presentation, UI, or view payloads. The serving
adapter adds input receipt observations and publishes the runtime readout
only when it changes. Mailbox draining and publication callbacks belong to that host
scheduler; neutral session scopes retain the single runtime lock.

The renderer realizes admitted changes once, without constructing disposable
resources or sampling phantom animated instances as a frame preflight. Backend
realization failure currently makes that surface terminal; recovery rebuilds
from committed intent. Campaign #8723 includes exploring in-place recovery and
resource reuse. Retained texture descriptors are immutable shared values.
Shadows initialize on new world objects without a scene-wide frame sweep or an
Engine shadow-light quota; products choose their lighting workload.
Generated C# bridges convert borrowed spans directly to
native arrays while retaining the required pin and release lifetimes.

Auxiliary publication frontier checks read only the stream revision; they do
not fork retained graphics maps. Commit rechecks after asynchronous realization.
SSE subscribers share one immutable encoding of each output batch. Product artifact
resolution checks paths/metadata without reading bodies that it would discard;
actual loaders and UI staging consume those bodies when needed.

Catalog admission canonicalizes owned data once before encoding it.

Each operation's outputs become ordered output batches, each sent as one SSE
event of any size; there is no default byte or count cap and no fragmenting. A
binding opens a baseline that must complete within the same operation.

There is no output history and no resume. Every SSE connection starts from a
fresh complete baseline: a reload, a dropped connection, and a replaced runtime
all reconnect the same way. Each subscriber gets its own live queue from the
moment its baseline is captured. A subscriber that falls
`MAX_SUBSCRIBER_QUEUE_EVENTS` (256) events behind, or stops reading for the
750 ms write timeout, is closed, and the browser reconnects for a fresh
baseline. One-shot transients published while a browser is disconnected are
not replayed. SSE ids remain only as an output sequence, so a caller can wait
until an operation's outputs have been observed.

Renderer resources are served from the runtime's current retained set only;
content-addressed responses are immutable and cached by the browser. A body the
runtime releases before the browser fetches it, whether in the same callback
or a later one, answers 404. The browser treats that 404 (or a 503 while no
runtime serves) as a stale projection, not a failure: it discards the queued
output and requests a fresh baseline, which references only retained bodies.
Browser callers may choose an explicit per-batch byte budget. Immutable host bundles and C# content
have no default file/count/aggregate byte quotas. Resource-format and browser
loader restrictions remain separate.

The TS `render-projection` model has no Three or DOM dependency and remains an
explicit tool/snapshot consumer. Mounted surfaces realize admitted operations
directly in Three and retain only publication frontiers alongside the backend;
there is no neutral scene mirror or whole-frame rollback staging. A partial
realization failure currently stops and disposes the surface.
`product-browser-host` uses its existing attachment epochs and fresh committed
baseline recovery, without replaying the product callback. On-demand inspection
reads actual backend nodes.

Resource inventories reconcile on inventory or resource-owner changes, not on
transform-only frames. Local static-instance edits update affected membership
and batch groups; camera culling and picking retain their existing behavior.
Packed mesh decoding retains byte-range, encoding, and copy-out lifetime checks,
without rescanning Engine-admitted indices, UVs, colors, or light semantics.

A product call owns each service's state for its duration and hands it back
when it finishes, so a call's first write never copies the graphics or
`PresentationWorld` state. Idle settlement reuses retained Appearance effect
frames; audio/video cursors update separately. Render-output resource catalogs
are copied only for a requested capture. Explicit scene captures remain
independent snapshots.

`PresentationWorld` also commits the retained audio/effect baseline and stamps
auxiliary presentation deltas with the same revision as graphics. Named Rust
mechanisms admit their state; the ABI adapter supplies their copied snapshots.
The shared TS continuation advances when configured hosts apply their operations.
Explicitly absent optional hosts do not strand unrelated graphics. A configured
host's partial or rejected published delta currently triggers a fresh baseline;
its diagnostics remain visible.

Playback cursors advance from admitted Engine update facts. Audio baselines
resume loops and preserve paused or completed voices; direct sounds and emitter
creation bursts are not replayed. Audio is realized either by the browser or,
with `RUSTY_AUDIO_OUTPUT=device`, by `render-audio` on the runtime's output
device ([recorded audio](recorded-audio.md#device-realization)). Continuous emitters restart their cosmetic
simulation from their retained descriptor. Animation baselines carry playback
cursors and per-clip controller phases, suppressing historical cues and
completion callbacks. Controller phase anchors initialize fresh realization;
attached mixers and cue cursors advance together on browser display time without
seeking on ordinary weight updates. A ghost plate retains its capture-time graphics subtree,
resource definitions, lights, and sampled animation pose. The backend rebuilds
its capture bank from that immutable input, including after a reconnect; only
explicit recapture replaces the source pose.

The public C# service is `Graphics`; `Appearance` remains a resource/fact name.
Facts can form a hierarchy, so equipment and layered visuals compose with
ordinary resources rather than feature-specific ABI calls.
`RuntimeAppearanceProjector` (`render-projection`) retains each object's last
fact, a parent/child index and the objects using each appearance. A batch of
changes validates and emits operations for the named objects, plus any whose
appearance or mesh definition changed. A complete snapshot is an adapter over
the same path. Typed runtime mesh
admission copies C# triangle streams into retained Engine resources. Mesh
appearances reuse the existing material and static-mesh projection; explicit
resource release removes its canonical definition and browser/GPU realization.

## Packaging and development

The Engine publishes two matched artifacts: an immutable `Rusty.Engine` SDK
package and a runtime pack containing `rusty`, `rusty-product-host`, and the
Engine-owned browser shell. The package makes the product project build its
own bind export and stages a loose Product directory from that build. `rusty dev` asks the package to stage that
directory, launches CoreCLR, and watches only the declared Product inputs.

Packaged CoreCLR launches (`rusty dev` and a direct `rusty-product-host
--product … --loader coreclr`) and any `--supervised` or `--headless` launch run
two processes. The runtime uses the loader the launch selected, so a
supervised or headless NativeAOT launch runs its native module:

- A small **supervisor** binds the product listener and keeps terminal
  signals. It owns `rusty dev` replacement, one automatic restart after a
  runtime crash, the failure pause, and headless browser launch. It never
  relays product traffic.
- One **runtime** process (the selected loader, the Engine, the product, HTTP
  and SSE)
  runs in its own process group and serves that listener directly. Closing
  its stdin is the clean stop: the product is disposed before exit.

`rusty dev` sends the supervisor one of two commands on its stdin. After a
full restage it sends `replace-runtime`. After a UI-only or content-bundle-only
restage it sends `reload-assets`, which the supervisor forwards to the running
runtime's stdin; the runtime re-reads its staged UI and bundle inventory in
place, with no product restart and no reconnect.

Replacement stops the old runtime first, so persistence is never shared
between two incarnations, then starts the next one. While no runtime is
serving, the supervisor answers requests with 503: JSON for runtime routes,
and a page that refreshes itself for navigations. A browser treats the new
runtime as a new incarnation: its output stream reconnects fresh, retrying
through 503s, and receives a complete baseline.

A direct launch stops with a named nonzero exit if its runtime crashes. Under
`rusty dev`, a second crash pauses until the next source restage. NativeAOT,
`--exercise` and `--performance-probe` run in one process.

Neither artifact contains product meaning. A Product repository does not carry
Engine JavaScript, generated bindings, a checked native bootstrap, or an Engine
Rust host. Selecting `--engine-source` is an explicit Engine-contributor
override that still uses the packaged contract; normal downstream development
never searches for an adjacent checkout.

## What stays out of downstream code

- Handwritten P/Invoke declarations, `UnmanagedCallersOnly` exports, raw
  function-table use, pointer ownership, and browser/native host adaptation.
- A custom renderer, retained frame format, browser canvas owner, TypeScript
  game renderer, or downstream TypeScript gameplay path.
- JSON invocation protocols, generic command buses, reflection/discovery
  frameworks, or policy/security layers at the trusted product boundary.

When a product needs a mechanism that current generated services cannot
express, name that mechanism precisely and create or link the upstream request.
Stopping there is preferable to a local substitute.

## Character tether ownership

The existing `CharacterControllerService` owns optional tether projection,
collision-swept correction and canonical motion continuation. Dynamics observes
body-local anchors on the live solver body, including mass, inertia and locked
axes. The character returns bounded equal-and-opposite reaction proposals; the
product applies them through `Dynamics.StepWithReactions` at its chosen update
order. A reaction is an ordinary impulse at the observed point, with no revision
check. Attachment selection, reel controls, consequences and presentation remain
product policy.
See [character tether use](csharp-lifecycle.md#character-tethers) and the
[bounded rope contract](rope-physics.md#kinematic-character-coupling).
