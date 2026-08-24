# High-density voxel and pixel-art runtime decision (#6911)

Status: campaign closeout, 2026-08-24

This campaign tested how Rusty Engine should consume detailed voxel-authored
and pixel-art-like 3D content without becoming an authoring application. The
result is a family of explicit routes, not one universal voxel renderer.

## Runtime recommendation

Choose the representation from the authority the product needs:

| Product need | Canonical route | Why |
| --- | --- | --- |
| Editable or queryable block world whose cells drive collision and navigation | Admit canonical `voxel-volume` or `voxel-object` data and use `engine-spatial` plus the existing voxel mesh projection | Occupancy, material IDs, collision, navigation, edits, and derived meshes stay synchronized under one owner. |
| Static high-density microvoxel art | Compile offline to a checked zero-clip GLB and admit it through the ordinary mesh-GLB lifecycle | The runtime needs exact bytes, bounds, materials, transforms, lifecycle, and rendering, not a second copy of the authoring grid. |
| High-density art that must retain stable palette colour regardless of lights | Use GLB-owned `KHR_materials_unlit` deliberately | Unlit is material intent on that asset, not a global visual-voxel mode. |
| High-density microvoxel art that belongs to scene lighting | Compile VOX occupancy to matte PBR vertex colour with outward axis face normals | The owner selected this as the best baseline in the #6923 visual gate. It preserved the microvoxel form without the adjacency-normal edge bands. |
| Coarse-grid block art | Start with axis normals; reconsider occupancy-adjacency normals only with a concrete coarse-grid comparison | The adjacency treatment was valid but visually harsher on the dense corpus from this campaign. |
| Lit camera-facing pixel art | Use the closed lit-sprite material modes and supply authored tangent-space normal or linear depth resources when their extra shape evidence is worthwhile | A plane, a block mesh, and an arbitrary synthetic volume do not share one truthful normal rule. |
| Disposable voxel-like debris | Use bounded instanced-cube particles | Particle collision and positions remain presentation-only; consequential debris must be an ordinary downstream entity. |
| Smooth volumes, SDFs, or neural sparse fields | No accepted runtime route yet | The campaign had no checked smooth-volume artifact and no consumer proof for OpenVDB, NanoVDB, SVDAG, Transvoxel, or a learned representation. Engine must reject or convert these offline rather than infer a durable format. |

An ordinary GLB mesh does not acquire voxel collision or cell queries merely
because it was authored in a voxel editor. A product that needs those facts
must supply a separate admitted collision representation or use the canonical
voxel route. Presentation cannot be read back as gameplay authority.

## Import and authoring boundary

Engine directly owns only bounded neutral artifacts that it can validate:

- canonical voxel-volume and voxel-object formats for durable cell meaning;
- GLB and normalized glTF import for ordinary static or animated mesh meaning;
- retained render descriptions, checked resource identity, and explicit
  create/replace/dispose lifecycle.

MagicaVoxel VOX, Vengi scenes, Blockbench projects, editor JSON, palette tools,
and learned reconstruction outputs remain authoring inputs. Asset Pipeline may
parse or convert them offline, retain source hashes and conversion receipts,
and emit an Engine-owned artifact. Rusty Engine does not embed those authoring
applications or open their evolving project formats at runtime.

Task #6920 made the one required importer extension: a zero-clip source is now
reported as `staticGlb` while using the same bounded GLB bytes, catalog identity,
descriptor, and lifecycle as animated GLB. Existing animated callers and wire
shapes remain compatible. No voxel profile, sidecar, implicit axis correction,
or material rewrite was added.

## Measured evidence

The #6925 checked high-density vignette loaded four palette-unlit GLBs totaling
34,559,992 bytes, 491,422 triangles, 768 primitives, and 769 materials through
the public Application Host. The shrine alone contained 203,606 triangles; the
tree contained 190,502. A real Chromium run admitted one canvas, displayed the
upright assets, and retained movement and pointer-look. The public host did not
expose GPU memory or frame time, so this is a lifecycle and visible-frame proof,
not a target-device performance budget. The full inventory and provenance are
in [Voxel vignette visual gate](voxel-vignette-visual-gate.md).

The #6923 follow-up held the shrine, tree, door, transforms, viewport, camera,
and adjustable retained lights constant across six material/normal variants.
Terrain was deliberately excluded. The owner selected the direct-VOX axis
normal/matte-PBR result. Dark, directional-only, and camera-point-light states
produced distinct real canvas frames after Application Host gained a bounded
forwarder for the renderer host's existing default-light and shadow policy.
Omitting that option preserves compatibility. See
[Visual-voxel shading comparison](voxel-shading-comparison-6923.md).

The related presentation experiments remain separate mechanisms:

- [Three scene particles](../three-scene-particles.md) measured 64, 512, and
  4,096 particles. Three billboard and instanced-cube paths retained one draw
  call per 256-slot batch and were materially cheaper than DOM particles in the
  observed headless browser run.
- [Lit sprite shader comparison](../lit-sprite-shaders.md) retained five closed
  lighting modes and measured the common fixture under moving camera and light.
  It did not turn arbitrary shader text into content.

These numbers characterize the observed browser and corpus. They are not
portable performance thresholds, nor evidence for several-hundred-thousand
editable cells, smooth volumes, destruction, LOD, or cluster residency.

## Deliberate non-promotions and future work

The campaign cancelled speculative static-mesh LOD, destructible-volume,
quantized-dissolve, camera-postprocess, and static-cluster tasks because the
accepted artifacts did not establish those contracts. Existing generic
mechanisms remain available where already owned; this campaign did not create
voxel-branded alternatives.

Create a follow-up only when a concrete consumer supplies the missing evidence:

1. compare axis and adjacency normals on an accepted coarse-grid asset before
   promoting another offline normal policy;
2. admit a checked smooth-volume/SDF artifact and specify collision, query,
   update, and persistence meaning before adding a volume format;
3. supply real LODs, collision proxies, destruction states, or cluster
   manifests before extending retained mesh contracts;
4. measure the target product on representative hardware before setting
   budgets beyond the existing bounded resource limits.

This preserves migration compatibility: existing voxel assets, animated GLBs,
retained frames, and downstream gameplay authority are unchanged. New formats
must be additive, versioned, bounded, and selected explicitly by their owning
product or offline conversion request.
