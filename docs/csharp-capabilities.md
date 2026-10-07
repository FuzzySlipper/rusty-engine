# C# capability map

This is the inventory of Rusty Engine's downstream C# surface.
It is a discovery aid, not a promise that every public Rust function is
available across the generated boundary.

> The product decides. The Engine guarantees.

The product owns application state, gameplay meaning, policy, orchestration,
and its ordinary entity/component model. Engine services provide reusable
host, rendering, spatial, content, persistence, and platform mechanisms. The
boundary is trusted first-party interop; it handles only real ABI, memory
and lifetime concerns.

`Look.Integrate`, `Look.Reset`, `Look.Rebase`, and `Look.Diagnose` are
ordinary managed helpers. Their radian-based state/configuration and copied
receipts require no native context service. `Motion` is native collision
resolution over a call-local `MotionSpatialEntity` view; it does not construct
a second product entity world.

## Generated Engine services

[`IEngineContext`](../csharp/Rusty.Engine) exposes generated named
service families. The declarations originate in
[`csharp-engine-abi`](../rust/crates/csharp-engine-abi), their implementations
live in [`csharp-engine-services`](../rust/crates/csharp-engine-services), and
the ignored `obj/Generated` output is produced by
[`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh).
The script fingerprints the ABI crate, the binding generator, itself (it pins
cbindgen), the clang and dotnet versions, and its previous outputs. It skips
the pipeline when none of them changed; delete `obj/Generated/.generation-stamp`
to force a rerun.

| Family | Product-facing purpose |
| --- | --- |
| `Dynamics` | Own native dynamics worlds, bodies, contacts, stepping, and collision binding. |
| `Motion` | Resolve reusable motion requests. |
| `Kinematic` | Integrate kinematic movement and run bounded motion operations. |
| `Spatial` | Own collision, navigation, character movement proposals, voxel picking, spatial queries, and session-owned triggers. It can atomically admit collision plus planar navigation from an immutable Engine `ContentReference`, or derive a bounded planar projection from the session's retained voxel/static-mesh collision scene under a `CollisionNavigationConfig` and explain why a column or edge is refused; navigation also includes distinct planar and volumetric traversal overlays plus bounded weighted queries. Planar path queries run A* over the published cells for the fewest steps, or the least cost when weighted; between equally good cells they prefer the one nearer the goal, then the canonical neighbour order, so a query's path is deterministic. `MaxVisited` bounds the cells a query expands. `EvaluateNavigationStep` returns bounded one-step navigation facts and its path without changing session state; path requests return their cells directly. Registered triggers support activation/retirement and fact-free restore rebasing through generated APIs. |
| `Perception` | Query reusable visibility facts; the product retains AI and awareness policy. |
| `WorldOrigin` | Prepare, inspect, and commit world-origin rebases. |
| `Voxel` | Read and mutate Engine-owned voxel state: edits, residency, densities and brush edits, implicit field stamps, collision and surface modes per material ([smooth voxel surfaces](smooth-voxel-surfaces.md)). |
| `VoxelContent` | Admit and inspect reusable voxel content resources, including bounded MagicaVoxel objects and retained object presentations. |
| `VoxelScenePresentation` | Project Engine voxel scenes into retained renderer resources, including GreedyCubes face-directed material selection, distance level of detail for reconstructed chunks, and scatter: Engine-placed instanced grass and clutter on the ground around the camera ([smooth-voxel-surfaces.md](smooth-voxel-surfaces.md#scatter)). |
| `Content` | Read product content admitted by the host, open content bundles and [portable assets](portable-assets.md), and admit product-owned content snapshots. |
| `AuthoredContent` | Admit and resolve authored catalogs, scenes, prefabs, and related resources. |
| `Graphics` | Create and update renderer-owned materials, meshes, atlas sprites, synchronized sprite playback, lights, and retained appearance state. |
| `Presentation` | Publish presentation effects and diagnostic facts without creating another renderer, including retained ghost-plate captures. |
| `RenderOutput` | Capture offline images and export GLB from the retained appearance snapshot ([offline images](csharp-offline-images.md)). |
| `ImplicitSurfaces` | Build scalar fields and generate retained meshes from them ([implicit surfaces](csharp-implicit-surfaces.md)). |
| `Animation` | Own animation resources, graphs, controllers, parameters, and playback realization. |
| `Audio` | Own audio clips, voices, control, and presentation feedback. |
| `Video` | Own one content-backed full-viewport WebM presentation and terminal realization facts. |
| `CameraView` | Retain cameras, offscreen targets, and ordered primary/offscreen view compositions; select one active camera as a convenience. |
| `RendererSettings` | Read the renderer's settings in effect and select its pipeline features and quality at runtime: shadows and their budget, ambient occlusion mode, strength and radius, antialiasing, vsync, clustered lighting and GPU culling. The product manifest supplies the initial values ([renderer settings](lighting-and-sky.md#renderer-settings)). |
| `Random` | Provide Engine-owned deterministic streams, keyed draws, and explicit-state compatibility draws. `DrawLcg15` advances a caller-held wrapping 32-bit LCG state, exposes its 15-bit sample, and reduces it with modulo arithmetic; it is intentionally compatibility behavior, so it has modulo bias and is not a general uniform random API. |
| `Persistence` | Read and write bounded Engine persistence blobs and stores. |
| `Http` | Fetch small HTTPS bodies into memory and download large ones into a product library directory, with progress, cancellation and library listing ([HTTP downloads](http-downloads.md)). |
| `Session` | Host or join a small multiplayer session by invitation, with member identities, ordered messages, fresh views for joiners and chat ([multiplayer sessions](multiplayer-sessions.md)). |
| `Ui` | Publish bounded product UI projections through the Engine host. |
| `Input` | Replace the product's physical input mappings at runtime. |
| `GameplayTime` | Hold simulation, run it slower than realtime, or advance it a bounded amount and hold, in a realtime product ([gameplay time](csharp-lifecycle.md#gameplay-time)). |
| `Diagnostics` | Publish product diagnostics and read renderer statistics. |

The generated contracts are authoritative when this table and source
disagree. Add a missing coherent family at the ABI and generator edge; do not
handwrite a parallel C# declaration or generic dispatch protocol.

### Dynamic body configuration

`DynamicsBodyConfig(Transform, HalfExtents, Properties)` accepts the same
`DynamicsBodyProperties` as shape-specific creation. Both `CreateBody` and
`ReplaceBody` apply initial velocities, mass policy, axis locks, damping, gravity,
material, collision filters, enabled/sleeping state, and continuous collision
selection. The six-argument constructor retains the standard defaults: zero
velocities/damping/restitution, friction 0.5, all collision groups/masks, enabled,
awake, and discrete collision.

Products select `ContinuousCollision` in these properties; Engine owns the CCD
solver. There is no per-step motion limit: a fast discrete body can pass through
thin geometry, and CCD is the remedy. `UpdateBody` keeps the body's handle,
pose and attached ropes.


### Sprite viewport placement

`Graphics.SetSpriteViewport` optionally places an existing sprite in CSS
viewport units through a lower-left rectangle, alignment, and `Stretch` or
`Contain` fitting. `Contain` uses the selected atlas frame's aspect, so frame
and playback changes refit without product-side geometry work. A placement
owns the sprite's final screen geometry; authored transforms cannot offset it.
The normal viewport clips overscan and offscreen rectangles, without a custom
target-rectangle clipping surface. CSS dimensions determine layout while DPR
only changes the backing buffer. Depth policy, render order, layer, and
playback are unaffected.

`SpriteSizeMode.Pixel` uses CSS-pixel size at the sprite's authored
position/depth. Its authored orientation and scale still apply, and the
renderer recomputes projection for each camera. A sprite exactly on the camera
plane has no finite projected size and contributes no pixels for that pass.
Viewport placement overrides this sizing and authored transform, fitting the
selected atlas frame instead.

### Ghost-plate presentation

The generated `Presentation` service exposes `CreateGhostPlate`,
`UpdateGhostPlate`, `RecaptureGhostPlate`, `ReadGhostPlate`, and disposal for a
retained ghost plate. C# selects the live `Appearance` object by its stable
Engine object ID and owns placement, tuning, and recapture policy. Capture
settings cover resolution, framing, near/far range, field of view, and scene
or isolated lighting. Plate mapping, depth retention, source-shell mode and
epsilon, plus directional sector counts of 1, 4, 8, or 16 and hysteresis, are
explicit configuration.

The Engine owns the copied capture, renderer resources, directional bank, and
cleanup. Direction changes are hard snaps selected by the Engine; there is no
downstream transition renderer. `GhostPlatePresentationReadout` is copied
back to C# and reports source presence, renderer observation/source match,
fallback and limitation facts, selected sector/offset, capture/configuration,
and retained resource counts and timing where available. A plate is a frozen
capture of a currently retained Appearance hierarchy: it is not a live
animated source, a general mesh renderer, or an arbitrary browser surface.

### Direct microvoxel object path

For a small standalone voxel object, use
`VoxelContent.AdmitMagicaVoxelObject`, inspect its copied palette/object facts,
then `ProjectObject` with one ordinary `Appearance` material binding per
admitted palette slot. `VoxelObjectPresentation` retains the Engine-owned
object projection; update its frame/transform/visibility through the generated
service and dispose it when finished. The MagicaVoxel path admits
bounded v150 model data and its default greedy surface emits axis-aligned face
normals. A roughness-1 ordinary material is sufficient for a matte microvoxel
presentation; no voxel-specific shader or downstream renderer is involved.

This is an object path, not a general scene importer. Source format, size,
voxel, frame, mesh, and material limits still apply, and every palette slot
must be bound. If a product needs a presentation or resource mechanism that
the generated services cannot express, file the narrow Engine request and
stop at that boundary rather than inventing C#, TypeScript, or browser
rendering to fill it.

Volumetric navigation is keyed by voxel coordinates and the resident voxel
source, not by planar `NavProjection` membership. Its overlay admits bounded
allowed/cost records; omitted cells remain allowed at unit cost, while the
volumetric query still owns occupancy, agent-volume, neighbors, budget, and
deterministic ordering. Replacement and weighted-query receipts expose both
the overlay hash and volumetric source hash so callers can identify the facts
used without treating a surface projection hash as authoritative.

Animation rig admission keeps structural roots, explicitly designated motion
roots, and authored pose-translation channels as separate typed facts. The
importer selects a motion root only when the source is unambiguous; multi-root
translation remains valid pose data instead of being guessed from joint names
or rejected. Primary meshes and clip packs must agree on the structural rig
and motion policy, while clip-specific pose channels may differ.

## Managed helpers in the default assembly

The runtime dependency is one `Rusty.Engine` assembly. These
namespaces are ordinary safe C# compiled into that assembly, not additional
native services or mandatory framework layers:

| Namespace | Role |
| --- | --- |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | Optional update-pipeline and admitted-step scheduling helpers. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | `EntityStore` class/value component storage, `EntityTypeId` kind metadata, the optional `Actor` facade, `EntityBatch` grouped writes, the seven responsibility adapters, and `EntityStoreDebugModule` live inspection. See [entity stores](csharp-helpers.md#entity-stores-mechanics-stores-and-engine-adapters), [component attachment](csharp-helpers.md#ordinary-component-attachment), [metadata and Actor](csharp-helpers.md#entity-metadata-and-the-optional-actor-facade), and [explicit edits](csharp-helpers.md#explicit-edits-and-persistence) sections. |
| [`Rusty.Engine.Mechanics`](../csharp/Rusty.Engine/Mechanics) | Double-backed `Stat`, referenced-maximum `Track`, `StatsComponent`/`EffectsComponent` mechanics owners, and `InventoryStore` with optional `InventoryEdit` plus live `InventoryComponent`/`EquipmentComponent` facades. See [stats](csharp-helpers.md#ordinary-numeric-stats), [tracks](csharp-helpers.md#resource-tracks), [stats collections](csharp-implicit-surfaces.md#entity-stats-collections), [effects/inventory](csharp-implicit-surfaces.md#effects-and-owner-scoped-inventory-components), and [capture/restore/inspection](csharp-implicit-surfaces.md#explicit-capture-restore-and-live-inspection) sections. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Explicit product codecs (`JsonProductStateCodec`, custom binary) and stores for the current shape (`ProductStateStore` save/load, `StatsComponentCapture` rebuild). See [explicit capture, restore and live inspection](csharp-implicit-surfaces.md#explicit-capture-restore-and-live-inspection). |
| [`Rusty.Engine.StateMachine`](../csharp/Rusty.Engine/StateMachine) | Product-owned managed state-machine definitions and instances. |
| [`Rusty.Engine.Input`](../csharp/Rusty.Engine/Input) | `PhysicalInputState`, analog deadzones and the `FpsInput` baseline ([FPS input](controller-interaction.md#fps-input-baseline)). |
| [`Rusty.Engine.Implicit`](../csharp/Rusty.Engine/Implicit) | Managed room, wall and architectural recipes over `ImplicitSurfaces`. |

Using one of these namespaces is optional. A product may organize its own
ordinary C# architecture, and a `using` declaration is enough to ignore a
helper that is irrelevant. The separate BindingGenerator and ProductGenerator
projects are build-time tools, not runtime assembly partitions.

Mechanics and state machines are managed helpers, not native services.
Damage, healing, combat meaning, AI policy, rules, state transitions, content
meaning, and gameplay orchestration are downstream application concepts even
when they are implemented using reusable Engine mechanisms.

## Shared world interaction and aim assistance

The packaged managed `Rusty.Engine.Interaction` surface owns `InteractionFocus`
(sticky acquisition/cycling/inspection), `WorldInteraction` (fresh checks and a
shared product action), `InteractionVisibilityQuery` (full Spatial ray composition)
and `AimAssist` (bounded stick tracking/slowdown and shot-direction correction).
`InteractionDebugModule` exposes `interaction.help`, `interaction.inspect` and
explicit assisted `interaction.use <id> <revision>` in the ordinary debug catalog.
See [the implementation and agent green path](controller-interaction.md).
These helpers use the input, look and spatial services; they add no native state
or alternate gameplay authority. Products supply targets, eligibility and actions.

## Native runtime and host mechanisms

These Rust owners hold reusable native or host state rather than gameplay
meaning:

- [`runtime-lifecycle`](../rust/crates/runtime-lifecycle) admits lifecycle and
  update steps without owning a product scheduler or clock.
- [`runtime-input`](../rust/crates/runtime-input) normalizes physical/direct
  input, held state, ordering, and lifecycle fences.
- [`runtime-ui`](../rust/crates/runtime-ui) transports bounded copied UI
  projections and owns no DOM or gameplay state.
- [`product-host`](../rust/crates/product-host) is the runtime's HTTP
  host for both outputs: it serves the product's UI page, input, lifecycle,
  the output stream and, in stream output, the frame stream, with the
  live-debug routes behind `--live-debug`, around a staged CoreCLR or
  NativeAOT product. Its `timeline` module carries completion binding, ticket
  and outcome data to product callbacks; C# owns scheduling and ticket
  meaning.
- [`desktop-shell`](../rust/crates/desktop-shell) owns the native window when
  the runtime presents to it ([desktop shell](desktop-shell.md)).
- [`csharp-product-runtime`](../rust/crates/csharp-product-runtime) loads the
  product library, binds generated tables, and integrates lifecycle with the
  host.

Renderer, spatial, asset, content, voxel, persistence, and diagnostic crates
are named Engine owners behind the generated service families. Their Rust
source APIs are not automatically C# APIs; expose product-useful operations as
coherent generated services rather than mirroring crate internals one method
at a time.

## TypeScript boundary

Downstream TypeScript owns DOM UI and accessibility, never gameplay state or
non-UI rendering. Engine TypeScript is the browser shell: it shows the frames
the runtime renders, carries input, and mounts the product UI. Rendering
(`render-wgpu`), audio (`render-audio`) and video are Rust mechanisms in the
runtime process; presentation intent is Rust-owned, and gameplay decisions
are C#.

## Missing capabilities

When a product cannot express a needed Engine mechanism through the generated
surface, follow the [missing capability workflow](csharp-sdk.md#missing-capability-workflow).
An upstream gap is a valid task result.

## Keeping this map current

After changing the ABI, regenerate bindings and compare the generated
`IEngineContext` family list with this page. Verify only the changed boundary:
focused Rust compilation, managed compilation, CoreCLR staging, and a
representative NativeAOT publish are normally sufficient. Generated files stay
ignored and must never be edited or committed.

The [viewport sprite package-consumer fixture](../fixtures/csharp-viewport-sprite/README.md) demonstrates safe C# atlas playback and viewport placement, with retained runtime evidence.
