# C# capability map

This is the present-tense inventory of Rusty Engine's downstream C# surface.
It is a discovery aid, not a promise that every public Rust function is
available across the generated boundary.

> The product decides. The Engine guarantees.

The product owns application state, gameplay meaning, policy, orchestration,
and its ordinary entity/component model. Engine services provide reusable
host, rendering, spatial, content, persistence, and platform mechanisms. The
boundary is trusted first-party interop; its ceremony is limited to real ABI,
memory, lifetime, and Engine-invariant concerns.

## Generated Engine services

[`IEngineContext`](../csharp/Rusty.Engine) currently exposes 21 generated
service families. The declarations originate in
[`csharp-engine-abi`](../rust/crates/csharp-engine-abi), their implementations
live in [`csharp-engine-services`](../rust/crates/csharp-engine-services), and
the ignored `obj/Generated` output is produced by
[`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh).

| Family | Product-facing purpose |
| --- | --- |
| `Look` | Integrate, reset, rebase, and diagnose view rotation facts. |
| `Dynamics` | Own native dynamics worlds, bodies, contacts, stepping, and collision binding. |
| `Motion` | Resolve reusable motion requests. |
| `Kinematic` | Integrate kinematic movement and run bounded motion operations. |
| `Spatial` | Own collision, navigation, character movement proposals, voxel picking, spatial queries, and session-owned triggers. It can atomically admit collision plus planar navigation from an immutable Engine `ContentReference`; navigation also includes distinct planar and volumetric traversal overlays plus bounded weighted queries. `EvaluateNavigationStep` returns bounded one-step navigation facts without changing retained session state, while `ProposeNavigationStep` retains its existing path mutation. Registered triggers support revision-guarded activation/retirement and fact-free restore rebasing through generated APIs. |
| `Perception` | Query reusable visibility facts; the product retains AI and awareness policy. |
| `WorldOrigin` | Prepare, inspect, and commit world-origin rebases. |
| `Voxel` | Read and mutate Engine-owned voxel state. |
| `VoxelContent` | Admit and inspect reusable voxel content resources. |
| `VoxelScenePresentation` | Project Engine voxel scenes into retained renderer resources. |
| `Content` | Read product content admitted by the host. |
| `AuthoredContent` | Admit and resolve authored catalogs, scenes, prefabs, and related resources. |
| `Appearance` | Create and update renderer-owned materials, meshes, sprites, lights, and appearance state. |
| `Presentation` | Publish presentation effects and diagnostic facts without creating another renderer. |
| `Animation` | Own animation resources, graphs, controllers, parameters, and playback realization. |
| `Audio` | Own audio clips, voices, control, and presentation feedback. |
| `CameraView` | Select and update the active Engine camera view. |
| `Random` | Provide Engine-owned deterministic random streams and keyed draws. |
| `Persistence` | Read and write bounded Engine persistence blobs and stores. |
| `ContentStore` | Plan, publish, and inspect durable content-store generations. |
| `Ui` | Publish bounded product UI projections through the Engine host. |

The generated contracts are authoritative when this table and source ever
disagree. Add a missing coherent family at the ABI and generator edge; do not
handwrite a parallel C# declaration or generic dispatch protocol.

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

The default runtime dependency remains one `Rusty.Engine` assembly. These
namespaces are ordinary safe C# compiled into that assembly, not additional
native services or mandatory framework layers:

| Namespace | Current role |
| --- | --- |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | Optional update-pipeline and admitted-step scheduling helpers. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | Product-owned entity/component storage, revisions, batches, snapshots, and selected Engine adapters. |
| [`Rusty.Engine.Mechanics`](../csharp/Rusty.Engine/Mechanics) | Reusable managed values, stats, tracks, sources, effects, inventory, and equipment mechanisms. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Product codecs, stores, restore plans, and entity-world persistence composition. |
| [`Rusty.Engine.Resolution`](../csharp/Rusty.Engine/Resolution) | Ordinary managed structural coordination and transaction helpers. |
| [`Rusty.Engine.StateMachine`](../csharp/Rusty.Engine/StateMachine) | Product-owned managed state-machine definitions and instances. |

Using one of these namespaces is optional. A product may organize its own
ordinary C# architecture, and a `using` declaration is enough to ignore a
helper that is irrelevant. The separate BindingGenerator and ProductGenerator
projects are build-time tools, not runtime assembly partitions.

Damage, healing, combat meaning, AI policy, rules, state transitions, content
meaning, and gameplay orchestration remain downstream application concepts
even when they are implemented using reusable Engine mechanisms.

## Retained native runtime and host mechanisms

The following Rust owners remain upstream because they hold reusable native or
host state rather than gameplay meaning:

- [`runtime-lifecycle`](../rust/crates/runtime-lifecycle) admits lifecycle and
  update steps without owning a product scheduler or clock.
- [`runtime-input`](../rust/crates/runtime-input) normalizes physical/direct
  input, held state, ordering, and lifecycle fences.
- [`runtime-timeline`](../rust/crates/runtime-timeline) owns bounded timeline
  operation and completion-ticket queues; C# decides ticket meaning.
- [`runtime-ui`](../rust/crates/runtime-ui) transports bounded copied UI
  projections and owns no DOM or gameplay state.
- [`product-dev-host`](../rust/crates/product-dev-host) supplies the local
  browser development transport around a concrete NativeAOT product.
- [`csharp-product-runtime`](../rust/crates/csharp-product-runtime) loads the
  product library, binds generated tables, and integrates lifecycle with the
  host.

Renderer, spatial, asset, content, voxel, persistence, and diagnostic crates
remain named Engine owners behind the generated service families. Their Rust
source APIs are not automatically C# APIs; expose product-useful operations as
coherent generated services rather than mirroring crate internals one method
at a time.

## TypeScript boundary

TypeScript remains appropriate for DOM UI, accessibility, and explicit Engine
host/backend implementation. It does not own product state, gameplay logic, or
non-UI rendering. Engine renderer resources, canvas/backend lifecycle, and
frame realization stay in the Engine path.

## Retired lanes

The supported downstream path does not include compiled TypeScript gameplay,
JSON-authored gameplay packages, a downstream Rust SDK facade, or native
Rules, Mechanics, Resolution, and State Machine service families. Git history
retains their implementation and migration rationale when historical evidence
is needed; current products should not carry compatibility adapters for them.

## Missing capabilities

When a product cannot express a needed Engine mechanism through the generated
surface:

1. name the missing mechanism and the lifecycle point where it is needed;
2. record the concrete product facts, request, and result shape;
3. file or link the narrow upstream task when authorized; and
4. stop the downstream substitution.

An upstream gap is a valid task result. Do not bypass it with handwritten
interop, a browser renderer, TypeScript gameplay, a JSON command bus, or a
parallel native implementation in the product repository.

## Keeping this map current

After changing the ABI, regenerate bindings and compare the generated
`IEngineContext` family list with this page. Verify only the changed boundary:
focused Rust compilation, managed compilation, and a representative NativeAOT
publish are normally sufficient. Generated files stay ignored and must never
be edited or committed.
