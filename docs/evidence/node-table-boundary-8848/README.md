# render-wgpu's node table is a twin, not a second world (#8848)

**Decision: world-matrix propagation stays in `render-wgpu`.**

## Who reads what

| Reader | Needs | Source |
|---|---|---|
| `render-wgpu` drawing: batches, shadows, sprites, view lists | Full world matrix, visibility and layer of every drawable, every frame | `NodeRow` (dirty subtrees propagate in `frame.rs`) |
| `render-wgpu` joint attachments (`animated.rs`) | The posed joint matrix of the parent's skeleton | `NodeRow`, plus poses that only `render-wgpu` evaluates from the GLB keyframes (#8847) |
| `render-wgpu` picking, ghost captures | Posed world geometry | `NodeRow` and parts |
| Particle and label anchors (runtime `frame_output.rs`) | One entity position on demand | `PresentationWorld::entity_world_position` |
| Audio emitter positions (runtime `audio_output.rs`, #8813) | One entity position on demand | `PresentationWorld::entity_world_position` |

**Why not move propagation into `render-presentation`.**
- Every drawable's matrix every frame is needed only by the renderer. The
  other consumers ask for a handful of positions.
- Joint attachments depend on animated poses. Moving propagation would make
  `render-presentation` evaluate glTF animation, which reverses #8847's
  ownership rule.
- So the two structures have different roles:
  - `PresentationWorld` is the committed retained graph, the authority;
  - `NodeRow` is the renderer's derived row, rebuilt from its deltas.

## Guards (this change)

- **`NodeRow`'s doc comment** (`tables.rs`) says what it may hold: hierarchy
  links, local and world matrices, visibility and layer, the realized kind,
  and parts. Gameplay-facing facts, entity or product identities, and
  anything another crate reads back are refused at review. Metadata that
  picking reports stays in the `metadata` table.
- **`tables` is now a private module** (`lib.rs`: `pub mod tables` became
  `mod tables`).
  - It never exported a usable type: every row is `pub(crate)`, and no crate
    outside `render-wgpu` imported from it.
  - The crate's public surface (`lib.rs` re-exports) has no hierarchy type.
  - `render-stream` and `csharp-product-runtime` still build.
- **`docs/architecture.md`.** The "wgpu realization" row now states the rule.

## Follow-up filed

**#8856.** `entity_world_position` finds the entity's node with a linear scan
of every retained node. It is called every frame for anchors and per voice for
audio, so each lookup is O(retained nodes). The task adds a
`source_entity → handle` index, keeping today's semantics.

## Checks

`cargo test -p render-wgpu` passes all 70 tests, and clippy `-D warnings` is
clean. Propagation did not move, so the screenshot suite and the attachment
fixture are unchanged.
