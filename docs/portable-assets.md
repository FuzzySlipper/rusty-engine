# Portable assets

A portable descriptor is readable JSON over ordinary files. Load it through
`Content.LoadPortableAsset` (or the `PortableAssetContent` SDK helper) from an
ordinary `ContentReference`, including one opened by `ProductContentBundle`.
Rust parses and resolves the selected dependency closure; C# receives typed,
copied facts and independently retained file references. Existing directory
bundles are sufficient: there is no new archive or virtual filesystem.

## Version 1

The root has `schemaVersion: 1`, `assets: [...]` and optional `provenance`
(authoring-only JSON, never required). Unsupported versions report the supplied
version and the supported version. New optional fields may be added within a
version; changed field meaning requires a new version. Parsing permits drafts;
loading validates only the selected asset and its dependencies. IDs are local,
case-sensitive stable strings, independent of file names. Referenced IDs must
resolve uniquely. No hash or production-history fields are required.

Each asset has `id` and one `kind`:

- `texture`: `path` to the raw texture.
- `model`: `path` to glTF/GLB; optional `clips` maps local clip names to authored
  glTF animation names; optional `materials` maps glTF material slot names to
  local material IDs. No geometry, hierarchy, tracks or bind poses are copied.
- `material`: optional `textures` maps named material roles to local texture IDs.
  These are relationships, not replacements for the existing material API.
- `sprite`: `frames`, named `animations`, optional `directions`, as below.

Paths use forward slashes relative to the descriptor's directory. Absolute,
empty, dot-segment, backslash and colon paths reject at load. Every referenced
file must exist in that descriptor's admitted context; opening other bundles
cannot change resolution. Resource decoders remain responsible for their own
formats and glTF's embedded dependencies.

## Sprites

Each frame has stable `id`, a local `texture` ID, and a region:

- `region: "cell"`, zero-based `cell: [column,row]`, `extent: [width,height]`.
- `region: "rect"`, `origin: [x,y]`, `extent: [width,height]`.
- `region: "image"`, `extent: [width,height]`, for a separate image.

All pixels use image top-left coordinates. `canvas` is the original untrimmed
size; optional `trim` (default zero) is the trimmed rectangle's offset within
it. `pivot` and optional named `anchors` are pixel positions in that original
canvas. Keep these facts when applying product-selected placement; they are not
normalized renderer pivots. Texture IDs may differ between frames.

Each named animation has ordered `frames` (repetition allowed), `looping`, and
`timing`: either `{"kind":"fps","fps":12}` or
`{"kind":"durations","seconds":[0.1,0.2]}` with one positive finite duration
per frame. Typed readout always supplies seconds, preserving order.

`directions.convention` is explicitly
`right-handed-y-up-yaw-zero-positive-z-toward-positive-x`. `sectors` are named
`{id,yawDegrees}` values in [0,360); `actions` maps action IDs to direction IDs
to animation names. An omitted mapping means missing, not an implicit mirrored
or nearest fallback. Products choose actions and any missing-direction policy.

## C# consumption

```csharp
using var source = engine.Content.OpenReference(new("hero/asset.json"));
using var hero = new PortableAssetContent(engine.Content, source, "hero");
using var image = hero.OpenMember("sheet");
var texture = engine.Graphics.OpenResourceFromContent(
    new(image, TextureFilter.Nearest, TextureWrap.Clamp));
var atlas = hero.CreateAtlas(engine.Graphics, texture.Handle, "sheet");
// Create an ordinary sprite appearance from this atlas, then:
var playback = hero.CreatePlayback(engine.Graphics, appearance, atlas,
    hero.FindAnimation("walk", "front")!);
```

`CreateAtlas` uses Engine-decoded texture dimensions and the existing atlas API.
Its lower-level `AtlasFrames` helper accepts explicit dimensions;
it checks rectangle containment against the supplied decoded image extent.
Separate-image facts retain distinct texture identities for ordinary sprite
resource composition. `CreatePlayback` uses Engine's existing playback owner
and clock. Dispose playback, appearance, atlas and texture through their normal
owners. Opened member references survive descriptor or bundle disposal.

For models, open the selected model member and pass it to
`Animation.OpenAnimatedMeshFromContent` for GLB (including zero-clip static GLB),
or `Graphics.CreateStaticMeshFromContentReference` for Engine mesh JSON. Resolve named clip relationships
against `Animation.ReadClips` and use ordinary playback/scrub APIs. Descriptors do
not replace glTF hierarchy or grant product code a second animation loop.

The packaged fixture `fixtures/csharp-portable-assets` contains equivalent loose
and bundle sprite/model/material documents, ordered timing, explicit missing
directions, and repeated release/reload. `portable.inspect` reports its facts.

## Mesh-to-joint attachments

An asset with `kind: "attachment"` names `target` and `child` local model IDs,
`joint` (the exact named skin joint), `convention:
"gltf-right-handed-y-up-meters"`, and `translation`, `rotation` (quaternion
x/y/z/w), and `scale` arrays. TRS is local to the joint and inherits the glTF
hierarchy's transforms, including authored scale. It does not replace an
attachment already expressed by that hierarchy. Use it for a separate mesh.

Loading this selection retains both file dependencies and exposes a typed
`PortableMeshAttachment` in `Facts.Attachments`. Load the target through
`Animation.OpenAnimatedMeshFromContent`, and the child through the existing
static or animated mesh admission. Publish ordinary body and child
`AppearanceFact` values with the child's `parentObjectId` naming the body and
its transform taken from the descriptor. Then use:

```csharp
engine.Graphics.PublishAttachedSnapshot(new(facts,
    new MeshJointAttachment[] { new(childObjectId, attachment.Joint) }));
```

This is a complete snapshot, like `PublishSnapshot`. Omitted bindings return
children to their retained parent root. The Engine checks the actual admitted
rig, reports missing/ambiguous joints with the requested name and target, and
preserves the prior staged snapshot on rejection. Joint names are case-sensitive;
unnamed or duplicate source joints are diagnosed by normal glTF admission.
Arbitrary non-skin node/socket lookup is not implied by the joint API.

The retained presentation baseline includes the binding. The renderer attaches
the child to its instance's animated bone once; ordinary playback and exact
pose sampling drive it. There is no product bone-transform polling or follow
loop. Remove the appearance facts before disposing their appearances/resources;
normal hierarchy teardown also removes the child when its parent is removed.

`fixtures/csharp-joint-attachments` exercises body/weapon descriptor loading,
`attachment.pose 0` / `attachment.pose 0.5`, recoverable `attachment.missing`,
and repeated `attachment.reload` through the packaged SDK with either loader.
