# Runtime voxel-sprite campaign evaluation

## Decision

Keep the complete capability experimental. Stabilize no additional backend,
application-host, Rust, or renderer-neutral contract from this campaign, and do
not create an offline-cache format or Asset Pipeline task yet.

The campaign proved that one Engine-owned frame representation can be produced
by explicit retained-model capture or supplied as admitted prepared textures,
then consumed by the same reconstruction and lifecycle code. It did not prove
that any enhancement mode improves the desired sprite-plus-voxel aesthetic.
The public application-host attachment is therefore an iteration instrument,
not a recommended character renderer.

## Evidence boundary

Engine tasks 7003 through 7005 established capture, enhancement, and interactive
lab mechanisms. Task 7008 exposed the same bounded experiment through the
application-host boundary at exact Engine revision
`2dcb3b922b80fa5a29430f1a4e69994dd07ab5fa`; its managed review passed.

CraftSurvive task 7006 supplied the product comparison. The Luna/max run
`rusty-craftsurvive-playtest-20260815T072611.660414615Z-3924910` exercised four
representative modes, both producers on two subjects, manual direction,
recapture, fail-atomic fallback, and resumed first-person movement. Its indexed
screenshots are the visual evidence. It ran before a final clarity-only change
to labels and the prepared-normal warning; reconstruction and capture behavior
did not change. Exact revision
`637ee527648119f9edce784af663482c8fe66c69` then passed managed review, the
GitHub `verify` gate, and exact-head smoke. Deterministic smoke is mechanism
evidence, not a visual verdict.

## Visual verdict by mode

No mode earned promotion:

| Mode | Observation | Decision |
|---|---|---|
| Plain sprite | Best reference for the authored silhouette and composition, but the checked captures were already very dark. | Keep only as baseline/fallback. |
| Normal relight | Changed shading without making form or material separation reliably clearer. | Laboratory-only. |
| Quantized depth/parallax | Produced a bounded depth response, but the sampled stills did not show a material aesthetic gain over the sprite. | Laboratory-only. |
| Sprite-backed splats | Preserved the sprite while adding samples, but the extra layer did not read as a clearly better voxel character. | Laboratory-only. |
| Full splat replacement | Demonstrated complete replacement and failure-safe lifecycle, but lost the compositional safety of the underlying sprite without a compensating quality gain. | Laboratory-only. |

The negative visual result is not a finding against one shader alone. The
comparison inputs were not controlled well enough to isolate reconstruction:
runtime capture stores rendered source color rather than canonical albedo, the
prepared normals are a remapped Blender-world bake while the runtime contract is
view-space, and the checked subjects remained dark at both close and medium
range. Prepared and runtime producers are structurally interchangeable, but
they are not visually or semantically equivalent in this corpus.

## Capture cadence and caching

Explicit capture is useful for static or dirty-on-change representations and
for changing a deliberately selected pose. Keyed-animation capture was not
exercised, and per-frame capture is not supported by the evidence.

Observed 96 by 96 captures ranged from about 0.5 ms for a warm recapture to
66.7 ms for the rigged wizard and 116.4 ms for an initial sampled capture. These
are CPU submission observations with large source/cache sensitivity, not a
stable budget and not GPU completion time. Steady-state submission observations
were approximately 0.7 to 1.4 ms in the checked interactions, again without GPU
timing. That is sufficient to keep explicit/dirty capture available, not to
schedule capture every frame.

Offline serialization would preserve the same frame representation, but this
campaign does not justify it. Capture frequency, visual quality, producer
equivalence, and storage pressure are all unresolved. Creating a cache format
now would freeze authoring assumptions before the runtime experiment has found
a successful look.

## Bounded accounting

The checked active frame has four RGBA8 96 by 96 textures, or 147,456 decoded
texture bytes (144 KiB). Sixteen directions would be 2,359,296 decoded bytes
(2.25 MiB) per actor before mipmaps, driver alignment, or compression. Three
such actors would be 6.75 MiB of decoded frame textures. CraftSurvive's checked
fixture is 5,333,416 encoded bytes across 195 resources, including its three
source GLBs; encoded file bytes are not VRAM.

At the configured 48 by 48 sample grid, relight, depth-parallax, and full-splat
reported one enhancement draw and 2,304 samples. Sprite-backed splats reported
two enhancement draws and 4,608 samples. The separately visible baseline adds
its own presentation draw. These counts describe one selected representation,
not a production crowd budget.

Credible scope from this campaign is therefore one or a few explicitly captured
actors at 96 by 96, with capture triggered by product policy. It does not
establish a direction count, animation-frame count, crowd count, GPU frame-time,
or persistent storage budget.

## Mechanism and tuning boundary

The useful mechanisms are the disposable attachment lifecycle, explicit source
selection, triggered recapture, fail-atomic replacement, camera-facing
preparation, bounded texture validation, and readout separation between capture
and steady submission. They remain behind the explicitly experimental
application-host port.

Mode selection, sample rows/columns, depth amplitude, quantization steps, base
sprite contribution, normal influence, splat overlap, capture elevation,
resolution, and direction are laboratory controls. None is a production default
or a renderer-neutral gameplay/content property.

## Failure ownership

- **Source/capture:** dark rendered color, material/lighting dependence, and the
  prepared world-space versus runtime view-space normal mismatch.
- **Reconstruction:** weak visual separation among the checked modes and loss of
  sprite composition in full replacement.
- **Three/WebGL experiment:** RGBA8 depth, approximate splat orientation,
  unsorted transparent splats, and absent GPU timer evidence.
- **Host presentation:** comparison labels and compact controls can obscure the
  scene; CraftSurvive improved side-specific labels after the playtest.
- **Representation:** one view cannot prove multi-view continuity, keyed
  animation, occluded geometry, or body-part-specific behavior.

## Reopening criteria

Do not continue by adding more reconstruction modes. A future campaign should
start only when it has a controlled, color-readable subject set and equivalent
prepared/runtime inputs: matching view-space normals, depth encoding, capture
basis, color semantics, resolution, pose, and lighting. It should compare close
and medium camera routes before considering keyed animation, offline caching,
GPU timing, or another public contract.

The deeper follow-on ideas—depth layers, motion vectors, dithered
rematerialization, and runtime G-buffer capture—remain backlog concepts. They do
not become dependencies or implied next steps from this result.
