# Animated-mesh GLB ownership (#8847)

**Question.** Should animated-mesh GLBs be decoded once at import into a
retained skinned-mesh form, or keep being read by `render-wgpu` from the
admitted bytes?

**Decision: keep the bytes in the retained model** (option 2), with an
explicit ownership rule. The one drift point found in clip-pack channel
binding is fixed.

## What the consumers are

- **Opening a mesh already runs the importer.**
  `csharp-engine-services/src/appearance.rs` calls
  `asset_import::import_animated_glb_asset` when a product opens an animated
  mesh. So "import" happens at open, in the runtime process, and each GLB is
  parsed twice there: once by the importer for the Engine-visible facts, once
  by `render-wgpu/src/glb.rs` for realization. Both parses happen once per
  open, never per frame.
- **The Three browser renderer reads the same bytes.** It is the default
  browser renderer until #8792 and loads them through `GLTFLoader` from the
  resource route. Replacing the bytes with a decoded form now would mean
  keeping both forms, or breaking Three before its parity gate.
- **Nothing outside the renderer needs the decoded hierarchy:**
  - the services use only the importer's rig signature (joint identities, for
    clip-pack compatibility) and clip descriptors;
  - audio positions (#8813) come from retained entity positions, not joints;
  - picking and joint attachments (`SetParentJoint`) are realized inside
    `render-wgpu`.
- **Sizes.**
  - Doom's attachment body is 217 KB and its example astronaut 8 MB.
  - CraftSurvive's animation GLBs are 3.9–7.6 MB.
  - Dagger ships no animated GLB; its weapon swing is a viewmodel sprite, as
    #8788 recorded.

  A decoded form holds the same vertex, keyframe and texture data uncompressed
  and would be no smaller.

Decoding at import would add a new retained asset shape, an importer change and
a dual form until #8792, to save one parse per open. The drift risk it
addresses is better handled by stating who owns which rule and aligning the one
place where the two sides disagreed.

## Ownership rule (`docs/architecture.md`, Source owners, "Animated mesh glTF")

- **`asset-import`** owns admission and every Engine-visible fact:
  - clip ids, names and declared durations;
  - the rig signature, where joint identity is a skin joint's unique node name;
  - clip-pack compatibility;
  - material slots and bounds.
- **`render-wgpu`** reads the admitted bytes only to realize them: streams,
  skins, keyframes, materials, textures.
  - Playback timing and completion come from its decoded keyframes, and
    nothing else reads the declared durations.
  - It adds no admission rule, and it binds by the identities `asset-import`
    defined.

## Drift fixed: clip-pack channel binding

**The disagreement.**
- The importer approves a clip pack when its rig signature equals the
  primary's.
- Joint identity there is the skin joint's node name, which must be unique
  among skin joints and is never synthesized.
- The renderer then bound each pack channel through a map of every named node
  in the primary, built with "last one wins".
- A mesh or helper node that reused a joint's name, and came after the joint,
  took the joint's channels.

**The fix.** `channel_targets` in `render-wgpu/src/animated.rs` lets skin
joints win their names over other nodes. Other named nodes still bind by name,
as before.

`clip_pack_channels_bind_to_the_skin_joint_that_shares_a_name` builds that
case: a joint `RightHand` first, then a mesh node named `RightHand`. It fails
under the old binding (`left: 1, right: 0`) and passes now.

## Evidence

- **`cargo test -p render-wgpu`: 66 pass.** This includes `tests/animated.rs`,
  which admits the `csharp-joint-attachments` GLBs through `asset-import`
  exactly as the runtime does and compares with the #8788 reference PNGs:
  - the skinned character sampling its clip and carrying the weapon on its
    hand;
  - repeating playback;
  - completion and bounds;
  - output capture;
  - picking;
  - controller blending;
  - redefinition.
- Clippy `-D warnings` is clean for `render-wgpu`.
- Dagger's weapon swing is a sprite, so no animated GLB is involved (#8788).

## Revisit when

Revisit this if a consumer outside the renderer needs the decoded hierarchy,
or if after #8792 the per-open parse cost shows up in a measurement. Neither
is true today.
