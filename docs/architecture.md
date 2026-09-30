# Engine architecture

Rusty Engine hosts an ordinary C# product. The product decides game
meaning; the Engine guarantees reusable mechanisms and integration.

## Ownership flow

```text
C# product application and state
  │  product rules, orchestration, content meaning, policy
  ▼
Rusty.Engine safe services and optional managed helpers
  │  generated contracts/values plus product-side composition helpers
  ▼
Rusty.Engine native bridge and C ABI function table
  │  compiled interop and service implementations, copied values, borrowed results;
  │  a small generated product export selects the product and its debug catalog
  ▼
Rust Engine services and runtime host
  │  lifecycle, input, renderer, spatial mechanisms, content, persistence
  ▼
render-wgpu frames: a native desktop window, or streamed to a browser page,
with the product's TypeScript DOM UI over them
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
| wgpu realization | Rust | [`render-wgpu`](../rust/crates/render-wgpu) applies `PresentationWorld` deltas to typed GPU tables and renders offscreen (with readback) or to a window surface. It is the only crate that may depend on wgpu. Its node table is a derived GPU-side twin, not a second retained world. It holds only what encoding a view and picking need, it is crate-private, and no other crate reads it. It propagates world matrices itself because joint attachments follow poses only it evaluates; other consumers take positions from `PresentationWorld::entity_world_position` (#8848). The runtime drives it for the streamed frames and the desktop window (below); it is the only renderer (#8792). |
| Animated mesh glTF | Rust | The retained model keeps admitted GLB bytes (`animated-mesh-resource/…`, `clip-pack-resource/…`). [`asset-import`](../rust/crates/asset-import), run by the services at open, owns admission and every Engine-visible fact: clip ids, names and declared durations; the rig signature (joint identity is a skin joint's unique node name) and clip-pack compatibility; material slots; bounds. [`render-wgpu`](../rust/crates/render-wgpu) `glb.rs` reads the admitted bytes only to realize them: streams, skins, keyframes, materials, textures. Playback timing and completion come from its decoded keyframes, and nothing else reads the declared durations. It adds no admission rule, and it binds clips and clip-pack channels by the identities `asset-import` defined (#8847). |
| Math vocabulary | Rust | Engine value types are plain arrays and the small f32 [`core-math`](../rust/crates/core-math) types. World-space spatial work is f64 and uses nalgebra through parry and rapier inside `svc-collision` and `svc-implicit`. glam is private to `render-wgpu`, which needs f32 column-major matrices for GPU rows, and `scripts/dependency_boundary_check.py` refuses it anywhere else. Conversions between Engine arrays and glam go only through `render-wgpu/src/convert.rs` (#8846). |
| Desktop window | Rust | [`desktop-shell`](../rust/crates/desktop-shell) opens the native window, presents `render-wgpu` to its surface, and composites the product UI rendered by Chromium (CEF) over it. See [desktop shell](desktop-shell.md). |
| Streamed frames | Rust | [`render-stream`](../rust/crates/render-stream) owns the runtime's `render-wgpu` renderer: it applies each committed call's publications, draws offscreen on its own thread, and JPEG-encodes frames (the only crate that may depend on the encoder). `product-dev-host` serves them at `/__rusty/product/runtime/frames`. |
| Runtime serialization | Rust | `runtime-session` is a mutex around one runtime instance. `product-dev-host` holds it so a runtime call and its output handover happen in one ordered scope. |
| Runtime publications | Rust | `runtime-publication` carries typed graphics, presentation, UI, and baseline facts. The runtime's renderer and audio output apply them in process; the host sends the browser shell only its binding, baseline markers and UI projections. Input acknowledgements and the runtime readout remain host observations. |
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
4. The product uses named services to read or publish facts. Disposable C#
   handles own retained native resources. Every transient result is borrowed:
   a `Native*Result` with pointer/`_len` collections, and the diagnostics of a
   refused call's `NativeOperationErrorReceipt`, point into bridge storage that
   stays valid until the next call on the same service context. The generated
   wrapper copies them before returning or throwing; there is no result handle
   or destroy call.
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

C# selects presentation facts through named services. Rust commits the
resulting graphics intent into `PresentationWorld`, and `render-wgpu` realizes
it in the runtime process. Graphics snapshots preserve active handles and
resource dependencies, with a `presentation-world` continuation revision. The
runtime moves owned typed publications through operation settlement to its
renderer without cloning or readmitting already admitted payloads; the
renderer reads resource bodies from the committed services while it holds
them. A call whose renderer work was lost, or a world replacement, rebuilds
the renderer from the committed snapshot without calling the product.

A product call owns each service's state for its duration and hands it back
when it finishes, so a call's first write never copies the graphics or
`PresentationWorld` state. Idle settlement reuses retained Appearance effect
frames; audio/video cursors update separately. Render-output resource catalogs
are copied only for a requested capture. Explicit scene captures remain
independent snapshots.

`PresentationWorld` also commits the retained audio/effect baseline and stamps
auxiliary presentation deltas with the same revision as graphics. Named Rust
mechanisms admit their state; the ABI adapter supplies their copied snapshots.

Playback cursors advance from admitted Engine update facts. Audio baselines
resume loops and preserve paused or completed voices; direct sounds and emitter
creation bursts are not replayed. `render-audio` plays audio on the runtime's
output device ([recorded audio](recorded-audio.md#device-realization)). A
machine with no output device runs silent after one warning, with no
completions reported; `RUSTY_AUDIO_OUTPUT=device` requires the device instead.
Continuous emitters restart their cosmetic simulation from their retained
descriptor. Animation baselines carry playback cursors and per-clip controller
phases, suppressing historical completion callbacks. A ghost plate
retains its capture-time graphics subtree, resource definitions, lights, and
sampled animation pose; the renderer rebuilds its capture bank from that
immutable input, and only explicit recapture replaces the source pose.

### Runtime outputs to the browser shell

The browser shell realizes no world. Each operation's outputs for the page
(its binding, the product UI projections, the runtime readout and scheduled
input results) become ordered output batches, each sent as one SSE event of
any size; there is no default byte or count cap and no fragmenting. A binding
opens a baseline that must complete within the same operation.

There is no output history and no resume. Every SSE connection starts from a
fresh complete baseline: a reload, a dropped connection, and a replaced runtime
all reconnect the same way. Each subscriber gets its own live queue from the
moment its baseline is captured. A subscriber that falls
`MAX_SUBSCRIBER_QUEUE_EVENTS` (256) events behind, or stops reading for the
750 ms write timeout, is closed, and the browser reconnects for a fresh
baseline; the page ignores incremental outputs until that baseline arrives.
SSE ids remain only as an output sequence, so a caller can wait until an
operation's outputs have been observed. Immutable host bundles and C# content
have no default file/count/aggregate byte quotas.

Every shape that crosses to the browser shell is declared once, in Rust:
- the outputs, operation results and requests;
- the product bootstrap and the input and UI projection wires;
- the diagnostics and telemetry;
- the Engine's own live-debug answers;
- the `RSF1` frame header.

ts-rs emits them into `render/packages/*/src/generated/contracts.ts`. The
TypeScript reads them as typed values and does not check their shape again: the
host is first-party, and the Rust decoders reject bad requests.
`product-dev-host`'s `typescript_contracts_are_current` test fails while a
checked-in file differs from the Rust types, and
`scripts/generate-typescript-contracts.sh` rewrites them.

### Runtime-rendered output

`RUSTY_RENDER_OUTPUT` selects where the runtime's renderer draws: `stream`
(the default) streams frames to the browser shell, and `window` presents to
the [desktop shell](desktop-shell.md)'s native window
(`csharp-product-runtime/src/frame_output.rs`, `render-stream`).

- **Runtime.** Each finished product call's frame, presentation and view
  composition publications are applied to the renderer as they are committed,
  under one lock with the simulation step and held state the call left, so a
  frame never shows a call's changes under the previous step. Animation,
  video and ghost plate facts reach the Engine through the ordinary
  realization feedback. In stream output the renderer draws when a change is
  applied and a viewer watches: every step while the simulation runs, once
  per change while it is paused or inspection time is held. With no viewer it
  still advances animation and video on Engine time without drawing, so
  completions do not wait for a page (#8871). It takes the product manifest's default light rigs.
  Once a second it refreshes the renderer statistics C# reads with
  `Diagnostics.ReadRenderer`; `engine.renderer.status` shows the same.
- **Transport.** A viewer pulls frames one at a time:
  `GET /__rusty/product/runtime/frames?after=N&width=W&height=H&cssWidth=C`
  answers with the latest frame newer than `N`, or `204` after a second. Each
  frame is a 40-byte `RSF1` header (sequence, simulation step, size, format,
  held and video flags) and a JPEG (quality 80) payload;
  `product-dev-host/src/frames.rs` is the format's source. The renderer draws
  at the most recent viewer's size, and at its pixel ratio (`W / C`): labels,
  pixel-sized sprites and particle points are CSS pixels
  (`Renderer::set_pixel_ratio`). The desktop window uses its scale factor.
  `RUSTY_RENDER_STREAM_FORMAT=rgba` sends raw frames, for measurement only.
- **Browser.** `product-bootstrap.json` carries `renderer.output`. The
  runtime-pack shell (`product-browser-host` over `application-host`) owns the
  Engine canvas: with `stream` it paints the frames under the unchanged
  product UI and marks the canvas with `data-rusty-frame-sequence`, `-step`,
  `-held` and `-video`; with `window` it leaves the page transparent over the
  native window. The canvas stays the focus, pointer-lock and input target.
- **Video.** The runtime renderer plays video clips into the frames and the
  window. A streamed frame a clip covers carries the video flag, and the page
  shows it above the product UI (z-index 1000) until a frame without it
  arrives. The clip's sound plays with the runtime's device audio.
- **Inspection.** The runtime renderer answers playtest inspection through
  Engine debug commands: `engine.renderer.camera` (read; set an observer pose
  that replaces every primary view's camera; `none` restores),
  `engine.renderer.drawing` (`continuous` or `on-demand`, which draws only on
  request) and `engine.renderer.frame` (draw one frame now). A change draws a
  frame and the answer names its sequence and step; the page's
  `__rustyPlaytest` hook waits until the canvas shows that frame.
  `engine.renderer.presentation` describes the last drawn frame, with
  `frameSequence`, `simulationStep`, `held` and `observer`, and
  `captureCorrelation: "frame-sequence"`. Its `views.cameras` are the poses the
  frame drew from (motion sampled, or the observer's where it replaced a
  primary view's camera, marked `observer`, with `offscreenPose` for that
  camera's offscreen views); `views.sourceCameras` are the product's
  descriptors. The observer and drawing mode belong to the runtime, so every
  attached page sees them. A tool captures lossless frames at its own size,
  in either output and without resizing any viewer, through
  `GET frames/capture` ([presentation capture](presentation-capture.md)).
  Held simulation time is the runtime's own. Nothing asks for a renderer
  pick.

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
resource release removes its canonical definition and GPU realization.

## Packaging and development

The Engine publishes two matched artifacts: an immutable `Rusty.Engine` SDK
package and a runtime pack containing `rusty`, `rusty-product-host`, and the
Engine-owned browser shell. A desktop runtime pack with the same ABI adds the
window host and Chromium's runtime; `rusty` fetches it on first use of window
output ([distribution](csharp-distribution.md)). The package makes the product project build its
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
place, with no product restart. It then sends each attached page a
`rusty-ui-reloaded` SSE event, and the page calls `location.reload()`: a page
keeps running the UI module it loaded, and the `--headless` page has no one to
refresh it. The reloaded page attaches with a fresh baseline; the product keeps
running.

Replacement stops the old runtime first, so persistence is never shared
between two incarnations, then starts the next one. While no runtime is
serving, the supervisor answers requests with 503: JSON for runtime routes,
and a page that refreshes itself for navigations. An open page's output
stream reconnects fresh, retrying through 503s. When the baseline comes from a
new runtime incarnation, the page calls `location.reload()` instead of
attaching: the new runtime may serve a different UI. A reconnect to the same
incarnation attaches the fresh baseline in place.

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
