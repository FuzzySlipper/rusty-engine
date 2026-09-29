# Sprite discovery follows the sprites (#8852)

**Finding (confirmed on current source).** `effects.rs::prepare_effects`
walked every entry of `tables.nodes` to find sprites. It did this once per
view pass (world and viewmodel, in every view), including in scenes with no
sprites at all.

**Change.**
- `Tables.sprites` is a `BTreeSet<RenderHandle>` that holds exactly the
  sprite nodes. It is added to the layout table in `tables.rs`.
- Membership is kept at the node lifecycle points in `apply.rs`:
  - `insert_node` adds a sprite (and clears a reused handle that is not one);
  - `destroy_node` removes every handle in the destroyed subtree;
  - `UpdateLight` removes a handle whose kind becomes a light.
- `prepare_effects` iterates the set and looks each node up by handle.
- `FrameStats.sprite_candidates` counts the nodes sprite preparation examined
  across the frame's passes. It is the probe for this task.
- The set is local to the backend and keyed by existing handles. There is no
  query layer or second scene authority.

**Measured** (`tests/effects.rs`
`sprite_discovery_follows_the_sprites_not_the_scene`, one `render_offscreen`
per step, i.e. a world and a viewmodel pass):

| Scene | Candidates before | Candidates after |
|---|---|---|
| floor only, no sprites | 2 | 0 |
| 2 sprites (one under a parent) | 8 | 4 |
| plus 500 unrelated static nodes | 1,008 | 4 |
| parent destroyed, 1 sprite left | 1,004 | 2 |

The image with the 500 extra nodes is identical to the one without them.
"Before" is the same test with the loop switched back to walking
`tables.nodes`. The test's assertions fail on that loop: "no sprites, nothing
examined" gives `left: 2, right: 0`.

**Remaining per-view work, and why.**
- Each pass still computes every visible sprite's quad and row. A billboard's
  orientation and pixel-sized sprites depend on the view's camera and
  viewport, so those rows differ per view and per camera move.
- Each pass also sorts the pass's sprites for the shared transparent order.
- Skipping unchanged rows would only help a still camera. That was not
  measured as a cost, so it is not added.

**Checks.** `cargo test -p render-wgpu` passes all 70 tests. That includes
the sprite screenshots, the lit sprites, and blended sprites sharing the
transparent order with meshes. Clippy `-D warnings` is clean.
