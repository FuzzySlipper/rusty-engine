# Depth-Splatted “FMV” Impostors

## Core idea

Use a conventional 3D actor as an invisible source, render it from a limited set of actor-relative views into a tiny pixel-art buffer, then reconstruct that image as a shallow 3D field of depth-aware splats rather than displaying it on one flat quad.

The result is neither a normal voxel character nor a conventional billboard. It is closer to a **directional sprite with just enough volume to survive small changes in viewpoint**. From its chosen source angle it reads as a coherent sprite; while the player moves within that angular neighborhood, its head, hands, weapon, and body gain restrained parallax. Crossing into a new view sector causes the entity to deliberately “select another depiction of itself.”

This should be used primarily for actors and interactable objects. The voxel or conventional 3D environment continues to own the full-screen geometry, perspective, depth, and optic flow, avoiding the discomfort that can come from applying coarse reprojection or perspective tricks to the entire first-person view.

## Visual and gameplay grammar

The discontinuity between representations is a feature rather than something to hide.

- **Voxel matter:** terrain, architecture, trees, debris, ambient animals, and noninteractive clutter. These are settled parts of the world.
- **Projected entities:** enemies, NPCs, loose loot, chests, important items, spell effects, and possibly the player’s hands and weapons. These participate in the game’s causal layer.
- **Voxel objects with projected organs:** doors, machines, shrines, harvestable plants, and other large environmental objects can remain voxel geometry while only their handle, lock, fruit, inscription, control surface, or other interactive component uses the projected material.

This creates a legible affordance language without outlines, glowing paint, or floating interaction icons: **things that can meaningfully act or be acted upon are made from a different visual substance.**

State changes can reinforce the rule:

- A living enemy is projected; an exhausted, non-lootable corpse settles into voxels.
- A chest is projected while unopened or containing loot, then becomes ordinary voxel clutter when emptied.
- A voxel statue becomes projected when possessed or awakened.
- A mundane object can acquire the projected layer after the player learns its significance.
- A mimic can deliberately violate the rule by appearing voxel-inert until it attacks.

## Base rendering pipeline

1. **Source actor**  
   Keep a normal rigged mesh for animation, collision, navigation, shadows, and authoring. It may never be directly visible.

2. **Quantized view and pose sampling**  
   Choose an actor-relative azimuth/elevation sector and an animation sample. Root motion and world movement can remain smooth even when the visible performance advances in held frames.

3. **Offscreen actor G-buffer**  
   Render at a deliberately small art resolution, such as 64×96 or 96×128. Useful outputs include:
   - palette-ready color or material index;
   - linear depth;
   - one or more deeper depth layers;
   - normal;
   - semantic body-part ID;
   - bone, triangle, or stable surface ID;
   - motion vector;
   - feature priority or silhouette importance.

4. **Pixel-art compilation pass**  
   Before reconstruction, convert the raw render into an intentional sprite-like image:
   - palette quantization and authored ramps;
   - removal of isolated one-pixel noise;
   - merging of small clusters;
   - minimum projected thickness for limbs, weapons, and other vital features;
   - simplified internal shading;
   - silhouette and major-form outlines;
   - semantic priority when details collide;
   - view-conditioned microglyphs for eyes, mouths, buckles, fingers, weapon edges, and similar sub-pixel details.

5. **Depth treatment**  
   Modify or quantize the captured depth according to the chosen representation. Physically correct depth is only one option and may not be the most attractive.

6. **Splat reconstruction**  
   Reconstruct every surviving art pixel or merged cluster into actor-local 3D and render it as a small quad, chip, surfel, or irregular patch.

7. **World compositing**  
   Use ordinary scene depth so voxel geometry correctly occludes the actor. Hidden proxy geometry can provide collision, contact shadows, and stable world lighting without dictating the visible representation.

## Depth-handling modes

| Mode | Description | Character |
|---|---|---|
| **Flat billboard** | All samples share one plane. | Maximum sprite coherence, no local parallax, familiar billboard behavior. |
| **Single-layer reconstruction** | Each visible source pixel is placed at its captured surface depth. | Cheap and spatial, but newly exposed surfaces are missing when the view changes. |
| **Layered depth image** | Store the nearest hit plus one to three deeper hits per source pixel. | Better small-angle movement and disocclusion; hidden arms, torso edges, or rear surfaces can emerge. |
| **Uniform depth compression** | Scale all depth offsets toward a central plane, perhaps to 20–50% of their real value. | Bas-relief or “FMV hologram” quality; preserves the source composition over a wider angular range. |
| **Semantic depth compression** | Compress different body parts by different amounts. | A nearly flat face and torso can coexist with deeper hands, weapons, horns, or other useful protrusions. |
| **Quantized depth bands** | Snap reconstructed depth into a few shallow layers rather than using continuous depth. | More visibly artificial and stable; resembles stacked animation cels or cut paper. |
| **Hybrid proxy relief** | Keep only selected features at real depth while the rest remain shallow or flat. | Strong silhouette readability with carefully rationed parallax. |

A useful continuous control is:

- `0.0`: ordinary cardboard sprite;
- `0.2–0.5`: compressed-depth FMV apparition;
- `1.0`: physically reconstructed surface.

The aesthetically interesting region is likely well below full physical depth.

## Splat and surface choices

Literal cubes are not required and may recreate the same angle-dependent noise as voxel characters.

Possible primitives include:

- **Source-camera-facing quads:** preserve the original pixel footprint well but look flatter from oblique views.
- **Surface-tangent splats:** reconstruct local surface orientation from depth and normals; more spatial but can become thin at grazing angles.
- **Viewer-biased surface splats:** mostly follow the source surface while cheating partway toward the current camera to avoid disappearing edge-on.
- **Cluster splats:** merge coherent pixel groups into larger irregular chips rather than rendering every source pixel independently.
- **Semantic splats:** hands, faces, weapon edges, cloth masses, and other important regions use different shapes or orientation rules.

A hybrid orientation is especially promising: each splat is attached to the actor’s surface but retains some loyalty to the source camera that originally composed the sprite.

## View-sector handling

The representation only needs to survive a small angular neighborhood. It does not need to be a perfect 360-degree object.

- Start with roughly **16–24 azimuth sectors** and a small number of elevation bands.
- Hold one source depiction while the camera moves perhaps **±8–12 degrees** around it.
- Use reconstructed depth for local parallax inside the sector.
- Add angular hysteresis so the system does not chatter at boundaries.
- Refresh when the view threshold is crossed, the pose changes, reprojection error becomes excessive, or important disocclusion occurs.
- Keep root motion smooth while pose, silhouette, and palette changes can advance at a deliberately stepped cadence.

When switching sectors, avoid normal alpha blending, which creates translucent half-pixels and muddy contours. Better transition methods include:

- ordered-dither replacement;
- silhouette-inward or body-part-by-body-part rematerialization;
- a rapid film-splice cut;
- splats briefly separating and reseating;
- alternating old and new depictions for one or two held frames.

Stable surface, bone, triangle, or semantic IDs can help correlate splats across adjacent views.

## Temporal and FMV-style treatment

The FMV quality should come from the representation itself, not from decorative VHS damage.

Useful properties include:

- each animation frame has its own resolved silhouette;
- internal highlights and facial marks change discretely instead of sliding continuously over a mesh;
- fine details appear or disappear according to the selected view;
- image-space organization is stronger than volumetric consistency;
- movement through the world is smooth while the visible performance is sampled or held;
- directional changes visibly rematerialize the actor;
- the actor may have a subtly different exposure or light response from voxel matter while still receiving world tint and shadow.

Distance-dependent animation and representation can reinforce this:

- distant actors: fewer frames, fewer depth layers, larger clusters, shallower relief;
- mid-distance actors: full gameplay representation;
- close actors or dialogue: more frames, more depth layers, denser feature glyphs, and richer facial depictions.

The actor can literally become more present as the player attends to it.

## Lighting, shadows, and integration

The projected entity should occupy the voxel world without becoming ordinary voxel matter.

- Use conventional scene depth and hidden proxy geometry for reliable occlusion.
- Allow world shadows, fog, ambient tint, and contact lighting to affect it.
- Preserve a bounded authored readability light or character-specific palette ramp so important features do not dissolve into the environment.
- Let proxy geometry cast contact shadows even when the visible samples are shallow or view-conditioned.
- Consider a small local shadow or grounding decal if the splat representation otherwise appears to float.
- Keep the environment’s low-frequency perspective completely conventional; reserve perceptual cheating for limited actor-sized screen regions.

## Salience and interaction intensity

Because the projected material is inherently noticeable, not every interactive object should constantly animate.

A dormant projected object can use:

- one held frame;
- very shallow depth;
- restrained contrast;
- no idle motion;
- sparse rematerialization;
- stronger temporal presence only when approached, aimed at, or activated.

Actors receive the full performance. Objects can share the ontology without becoming a field of visual notifications.

## Main failure modes and mitigations

### Disocclusion holes
Small camera movements reveal surfaces absent from the source image.

**Mitigations:** layered depth images, neighboring-view samples, hidden proxy fills, restricted angular windows, or deliberate stylized holes that rematerialize at sector changes.

### Grazing-angle splat collapse
Surface-aligned quads become thin or vanish.

**Mitigations:** viewer bias, minimum projected splat size, anisotropic shapes, cluster splats, or source-camera-oriented patches.

### Sector popping
A directional change visibly replaces the actor.

**Mitigations:** hysteresis, depth-supported local movement, correlated IDs, ordered-dither transitions, and treating the rematerialization as intentional visual grammar rather than an error.

### Excessive temporal noise
Frequent resampling produces shimmer or visual fatigue.

**Mitigations:** held frames, update thresholds, stable palette decisions, persistent cluster IDs, and avoiding continuous regeneration at display refresh rate.

### Actor/world mismatch feels accidental
The effect can read as an unfinished billboard if not consistently applied.

**Mitigations:** use the representation systematically for causal entities, establish it immediately through the player’s hands or first pickup, and give transitions and lighting a coherent authored language.

### Too much salience
Every collectible or switch competes for attention.

**Mitigations:** an intensity hierarchy, dormant held states, partial projected organs, distance LOD, and activation-dependent motion.

## Minimal prototype

Build the experiment on top of the existing directional-impostor pipeline before attempting any world-scale texel system.

1. Choose one broad humanoid, one thin skeleton-like actor, and one irregular creature or robed figure.
2. Render a 64×96 or 96×128 actor G-buffer with color and linear depth.
3. Reconstruct occupied pixels as small splats instead of one quad.
4. Add a live depth-compression slider from 0–100%.
5. Compare source-camera-facing, surface-tangent, and viewer-biased splats.
6. Add a second depth layer, then expand to three or four only if clearly useful.
7. Preserve the existing quantized directional views and allow approximately ±10 degrees of local camera movement around each one.
8. Add held animation sampling and angular hysteresis.
9. Test sector transitions using ordered dithering rather than alpha blending.
10. Composite against bright forest, snowy stone, visually busy terrain, and dark localized lighting.
11. Record normal-speed first-person circle strafing, approach/retreat, elevation changes, and abrupt mouse movement—not just flattering slow orbit shots.

Compare at least four variants:

- flat directional quad;
- physically correct single-layer depth splats;
- 20–50% compressed depth splats;
- compressed layered-depth splats with semantic depth rules.

The key question is not whether the reconstruction is geometrically accurate. It is whether it preserves a strong sprite composition while gaining enough spatial behavior to feel deliberately embedded in the world.

## Working thesis

> The landscape is stable voxel matter. Interactive things are recorded possibilities given shallow volume.

The goal is not to disguise a sprite as a conventional 3D model. It is to let an image behave as though it has just enough depth to haunt a three-dimensional world.
