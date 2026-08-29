# C# SDK guide

This guide describes the C# surface that exists today. It distinguishes that
surface from recommended product organization so an agent does not mistake a
proposal for an API.

## Build a product through the generated surface

`Rusty.Engine` is the public C# dependency. Its project invokes
[`generate-csharp-native-bindings.sh`](../scripts/generate-csharp-native-bindings.sh)
and compiles generated safe contracts and values from `obj/Generated`.
`Rusty.Engine.ProductGenerator` is an analyzer/source-generator dependency of a
NativeAOT composition project. It emits the internal native bootstrap and safe
service implementations.

An ordinary product project should:

1. reference [`Rusty.Engine`](../csharp/Rusty.Engine),
2. implement generated `IEngineProduct`,
3. accept `ProductCreateContext` in its product constructor,
4. keep an `IEngineContext` or only the named services it needs,
5. select exactly one product with `[assembly: EngineProduct(typeof(...))]` in
   the composition project, and
6. let the generator supply exports and interop code.

The fixture's [`Product.cs`](../fixtures/csharp-nativeaot-trial/Product.cs) and
[`NativeProduct.cs`](../fixtures/csharp-nativeaot-trial/NativeProduct.cs) show
the smallest current reference. The fixture's broad capability exercise is
intentional proof scaffolding; a normal product should use only the service
families it needs.

## Current lifecycle

The generated `IEngineProduct` contract has lifecycle callbacks for `Start`,
`Update`, `Pause`, `Resume`, `Restart`, `Shutdown`, and `Dispose`. Its optional
`CompleteTimeline` callback receives copied, host-admitted completion data and
lets product code accept or reject the product-owned ticket meaning.

- The constructor receives `ProductCreateContext`, including `IEngineContext`,
  admitted product content, and input configuration.
- `Update(ProductUpdate)` receives Engine-owned update facts and copied input
  events, then returns `ProductUpdateResult` when it needs a supported host
  action.
- The runtime, not the product, drives lifecycle transitions and owns host
  integration. Do not create another central game loop or advance Engine time
  yourself.
- Timeline completion is binding-fenced by the Rust runtime before C# receives
  it. Correlation, outcome, and provenance values are copied into safe managed
  data; a product that does not own timeline tickets may leave the default
  rejecting implementation in place.

Current `IEngineContext` properties are named service families generated from
the ABI, such as input-adjacent look/motion services, spatial, camera,
appearance/presentation, content, persistence, random, mechanics, rules,
resolution, and UI. The exact available family/method set is defined by the
current generated `Rusty.Engine` output and Rust ABI source; do not assume a
Rust API is callable from C# simply because its Rust crate is public.

`VoxelScenePresentation` projects the canonical `Spatial` session voxel scene
through the Engine renderer. Bind every currently used scene material slot to
a live `Appearance` material, retain the disposable projection, and call
`RefreshScene` after voxel edits, residency changes, or origin changes. The
Engine keeps incremental renderer identity and owns all generated mesh/frame
work; C# receives only a small readout. `Clear` or disposal stages the matching
renderer destroys. Select `VoxelSurfaceMode` in `SpatialSessionConfig` when
creating the session; it chooses only the Engine-derived mesh posture and is
retained through subsequent voxel changes. Changing the mode of an existing
session is not currently a C# API.

`CharacterStepRequest.Obstacles` is a borrowed, call-local list of active
product-authored colliders. Give each obstacle its stable entity identity,
current transform, local bounds, collision participation, and motion facts;
the Engine uses them for the one controller proposal and returns ordinary
`CharacterMotion`, `CharacterSupport`, and platform facts. Resubmit the
current support transform and obstacle list on later steps so Engine-owned
support/carry continuation can apply; the session never retains product
entities or collider records. Collision uses the existing translation-offset
AABB posture with unit scale; obstacle rotation participates in platform carry
but does not rotate the collider volume.

## Values, leases, and native lifetime

The public C# layer turns direct service calls into typed requests, receipts,
values, and disposable handles. Follow the type's ownership model:

- Use returned value records directly or copy their data when you need to keep
  it.
- Dispose values that represent an Engine lease, session, snapshot, resource,
  or handle when their scope ends. `using` is the usual product-side shape.
- Do not retain borrowed spans, native pointers, or callback-backed data beyond
  their stated call/lease lifetime.
- Do not add unsafe code, handwritten P/Invoke, ABI structs, or
  `UnmanagedCallersOnly` exports to normal product code. The generator owns
  those details.

The product boundary is trusted, but these memory and lifetime rules are real
correctness requirements rather than policy ceremony.

## Optional managed helper packages

These packages are current reusable helpers, not required product framework
pieces:

| Package | Current role |
| --- | --- |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | An optional Engine-context update pipeline and deterministic scheduler helper, compiled into `Rusty.Engine`. Its `SimulationScheduler` can resume on the next admitted step, wait fixed admitted steps, or wait for a caller-owned completion condition without creating a second clock. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | Product component keys, managed entity worlds, snapshots, batches, and managed adapters around Engine mechanisms, compiled into `Rusty.Engine`. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Product-state codecs, stores, restoration plans, and entity-world persistence helpers, compiled into `Rusty.Engine`. |
| [`Rusty.Engine.Resolution`](../csharp/Rusty.Engine.Resolution) | Structural resolution sessions and typed product transaction coordination. |

Use a helper when it fits the product's real domain. A product may compose its
own ordinary C# architecture instead. None of these packages implies a hidden
`ProductApplication`, `ProductBuilder`, `IProductModule`, analyzer suite,
typed-content framework, or projection framework: those names are not current
SDK APIs.

## Recommended product architecture, not a framework contract

[C# product style](csharp-product-style.md) recommends organizing product code
by domain modules, keeping state ownership explicit, and using thin
Read/Decide/Apply/Publish coordinators. These are conventions a product can
adopt directly; they do not require registration APIs or runtime discovery.

For mechanics that need Engine infrastructure, prefer named generated service
calls and managed adapters over downstream reimplementation. For product-only
rules or state transitions, ordinary C# types are the appropriate owner.

## Missing capability workflow

If the needed behavior cannot be expressed through the generated API:

1. identify the missing Engine mechanism and the lifecycle point it needs;
2. record the concrete product call shape or fact the mechanism should admit;
3. file or link the narrow Engine task when authorized; and
4. stop the downstream substitution work.

Do not bypass the gap with a browser renderer, TypeScript gameplay path,
handwritten interop, JSON bridge, or a parallel Rust implementation in the
product repository. The inability to proceed is useful evidence for the
upstream capability work.
