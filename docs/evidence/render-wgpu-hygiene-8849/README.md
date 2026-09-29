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
