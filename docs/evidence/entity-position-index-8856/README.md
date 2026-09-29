# Entity positions from an index, not a scan (#8856)

`PresentationWorld::entity_world_position(entity)` found the entity's node by
scanning every retained node for a matching `RenderMetadata::source_entity`,
then composed the parent chain. Each frame calls it for entity-anchored labels
and particle anchors (`csharp-product-runtime/src/frame_output.rs`), and the
device audio path calls it per positioned voice. So each lookup cost O(retained
nodes).

## Change

- `render-presentation/src/world.rs`: `RetainedGraphics` keeps
  `entity_nodes: source_entity → handles`, maintained where the nodes change:
  - node creation (every `Create*` goes through `insert`);
  - `Update` with metadata, which reindexes only when the source entity
    changes;
  - `Destroy`, which unindexes the whole removed subtree.

  No other operation writes nodes or their metadata.
- The lookup takes the lowest handle for the entity, which is the node the
  handle-ordered scan found first. It then composes the parent chain as before.
  A joint attachment still uses its parent node's transform.
- There is no cache of world positions and no second hierarchy.

## Evidence

- **Index test.** `the_entity_index_follows_creation_metadata_updates_and_subtree_removal`:
  - after creation, a metadata move from entity 7 to 9, and a parent's
    destruction that takes a child with it, the index equals a recomputation
    from the nodes;
  - two nodes for one entity answer with the lower handle.
- **Scaling test.** `entity_lookup_work_does_not_grow_with_unrelated_nodes`
  runs 20,000 lookups of an entity whose node sorts last, with 10 and with
  20,000 unrelated nodes (debug build):

  | | 10 unrelated nodes | 20,000 unrelated nodes |
  |---|---|---|
  | Before (scan) | 10.7 ms | 14.3 s |
  | After (index) | 4.2 ms | 10.4 ms |

  The test asserts the large scene stays under 20 times the small one, where
  the scan was about 1,340 times.
- **Unchanged tests.** The existing
  `entity_world_position_composes_parent_transforms_and_follows_updates` test,
  and the `render-wgpu` label and particle anchor tests. `render-presentation`,
  `csharp-engine-services`, `render-wgpu` and `csharp-product-runtime` tests
  all pass, as do clippy (1.95 and stable) and fmt.
