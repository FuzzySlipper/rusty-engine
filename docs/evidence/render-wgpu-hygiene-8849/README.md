# render-wgpu hygiene (#8849)

## 1. CPU geometry copies: measured, kept

`Renderer::mesh_memory()` (new) totals the CPU geometry copies (`GpuMesh.cpu`,
each shared copy counted once) beside the GPU vertex and index bytes of the
same meshes. The tables covered are static meshes, payload meshes, voxel
objects, animated rigid primitives and skinned instance buffers.
`examples/render_capture` prints it.

| Capture (live, fresh attachment) | Meshes | CPU geometry | GPU vertex + index |
|---|---|---|---|
| Doom room study (idle at spawn) | 125 | 5.95 MB | 16.49 MB |
| Dagger (spawn) | 66 | 0.34 MB | 1.02 MB |

The copies cost about 35% of GPU mesh memory, not double: GPU vertices carry
normals, UVs and tangents that the copy leaves out.

**Decision: keep the current shape.**
- Picking can target any mesh. Layer and metadata filters apply at query
  time, and live debug and interaction picking choose what to ask for.
- Limiting the copy to "pick targets" would need a pickability declaration
  that the Engine does not have.
- The measured cost is small for both products.

## 2. String-keyed tables: per-frame check

**Fixed: sprites** (per sprite, per view pass).

Before, each sprite in each pass did the following, and draw hashed the key
again:
- hashed its atlas id (`atlases.get(&sprite.asset)`);
- cloned the atlas texture id and the lighting detail texture id into
  `String`s;
- hashed that pair of strings for the bind-group check.

Now:
- `Tables.names` interns atlas and texture names to dense ids at apply time.
- Atlases are a `Vec` indexed by name id, and each atlas stores its texture's
  name id.
- A sprite node (`SpriteRow`) stores its atlas id and its authored
  normal/depth texture id, resolved at `CreateSprite`. `UpdateSprite` changes
  neither.
- Sprite bind groups are keyed by `(Option<u32>, Option<u32>)`.

The frame path now indexes and hashes integers only. A name reaches a string
map only on a bind-group cache miss, and `forget_texture` works by id. Names
stay bound to their ids; the table grows with the distinct atlas and texture
names seen.

**Checked, left as is:**
- **Composition targets and camera motions:** one lookup per view per frame.
  They scale with the composition's views (a handful), not with the scene.
- **Animated assets, clips and joints:** looked up per posed instance per
  frame (asset in pose, skin and part writes; clip per active action; joint
  per attached child), about 6–8 lookups.
  - The same functions CPU-skin every vertex. The fixture body has 1,029
    vertices on 45 joints, so each pose does thousands of matrix
    blend-and-transform operations. The lookups are well under 1% of that
    work.
  - Held and sampled poses skip the whole path (#8850).
  - Interning them becomes worthwhile if skinning moves to the GPU.
- **Label fonts and icons:** resolved when a label is laid out, not per
  frame.
- **Sky textures:** only when the sky is rebuilt.
- **Particles:** the per-particle content-hash lookup was already removed
  (#8851).

## 3. Direct dependency pins

`glam`, `png`, `bytemuck` and `pollster` are now workspace dependencies in
the root `Cargo.toml`. glam carries a comment that it is private to
`render-wgpu` (#8846). `render-wgpu` uses `.workspace = true`.
- `Cargo.lock` is unchanged, so the pair build keeps one entry each.
- The boundary check passes.

## Checks

- `cargo test -p render-wgpu` passes all 70 tests, including the `sprites`,
  `sprites-lit` and blended-order screenshots.
- Clippy `-D warnings` is clean.
- `render-stream` and `csharp-product-runtime` build.

## Review fix: no name lookups on frame paths

**Finding.** The review asked for the rest of item 2 to be done: the
animated asset, clip and joint lookups and the composition lookups, resolved
at apply time instead of argued away. Rechecking the frame paths also turned
up a miss in the first version of this README. `frame.rs::draw_batches`
looked up every draw's mesh (`MeshRef::Static(String)`) and material
(`MaterialRef::Retained(String)`) by name, in every pass. That was the
hottest string hashing in the crate, and the first check did not catch it
because each lookup was split across lines.

**Change: every name a frame path uses is resolved when it changes.**

| Frame path | Before | Now | Resolved at |
|---|---|---|---|
| Draw batches (`draw_batches`), shadows, ghosts, picking | `static_meshes.get(name)`, `voxel_objects.get(name)`, `animated_assets.get(name)`, `materials.get(id)` | `MeshRef::{Static, Voxel, AnimatedRigid}` and `MaterialRef::Retained` carry `Tables.names` ids; the tables are `Slots` indexed by them | Part build (`rebuild_parts`), which runs on definition, binding and release |
| Animated pose, skin, part writes, bounds | `animated_assets.get(&instance.asset)` | `animated_assets.get(instance.slot)` | Instance creation (a name keeps its slot across redefinition) |
| Direct playback (`actions_of`, `evaluate_pose`) | `clips.get(name)`, plus a cloned `String` per action per frame | `Playback.timeline_clip` / `pose_clips` indices into `AnimatedAssetRow.clips` | `set_animated_playback`, and `reset_animated_instance` (create, redefinition, release) |
| Controller-driven poses | a name-keyed temporary playback, then `clips.get(name)` | the controller's clip table (`ControllerRow.clips`) matches the few clips its state samples | Controller create/update, and the target's reset |
| Joint attachments (`frame.rs::update_subtree`) | `joint_pose(parent, name)`: two lookups per attached child | `NodeRow.parent_joint_node`, `joint_pose_at(parent, index)` | `SetParentJoint`, and the parent's reset |
| Composition (`render_composition`) | `motions.get_mut(&camera.id)`, a `&str`-keyed pose map, `targets.get(id)`, and an id-string sort of the passes | a `Plan` of camera, target, view and presentation indices in (order, id) order; targets and motions aligned with the composition | `set_view_composition` |

- **Redefinition and release.** A name keeps its slot across redefinition,
  and a release empties the slot, so resource lifetime still follows Engine
  release. Parts and instances re-resolve at the points the table names.
- **Remaining name work.** Controller sampling itself is name-based inside
  `render-presentation`: `frozen_pose` builds a name-keyed map per call. The
  renderer resolves the controller's clip set once and compares the handful
  of names it returns. It does not hash them.
- **Not on frame paths.** Label fonts and icons (resolved when a label is
  created), GLB export (an output job) and asset admission still use names.

**Evidence.**
- `tests/animated.rs` `clip_and_joint_indices_are_resolved_again_when_the_asset_changes`:
  a repeating clip with a weapon on the hand is redefined three times. Clip
  index order is not stable across definitions, and the image is identical
  each time.
- `a_held_attachment_follows_its_moved_parent_by_the_resolved_joint`: a held
  pose whose parent moves renders identically to the same scene built at the
  moved position. This is the `update_subtree` joint path the review named.
- The composition half landed with #8841 (`4ad84170d`), with
  `tests/views.rs` `drawn_cameras_report_the_sampled_and_observer_poses_each_view_drew_from`.
- `cargo test -p render-wgpu` passes all 82 tests, including every screenshot
  suite and the controller blend test. Clippy `-D warnings` is clean.
- **Test harness.** `tests/support` now shares one device per binary, as
  `tests/common` already did. The animated fixtures crashed the Vulkan
  loader (SIGSEGV on RADV) about one run in four when test threads created
  devices at once. After the change, five parallel runs of the four
  support-harness binaries were clean.

