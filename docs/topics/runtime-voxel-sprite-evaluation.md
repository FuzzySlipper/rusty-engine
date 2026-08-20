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

The checked active frame had four RGBA8 96 by 96 textures, or 147,456 decoded
texture bytes (144 KiB). Sixteen directions would be 2,359,296 decoded bytes
(2.25 MiB) per actor before mipmaps, driver alignment, or compression. Three
such actors would be 6.75 MiB of decoded frame textures. CraftSurvive's checked
fixture is 5,333,416 encoded bytes across 195 resources, including its three
source GLBs; encoded file bytes are not VRAM.

That run used one 48 by 48 grid for both the base plane and splats. The
iteration instrument now separates the base/depth-parallax grid (up to 128 by
128) from the instanced-splat grid (up to 512 by 512). Depth quantization steps
divide the captured subject's projected front-to-back relief; they do not
change either grid's spatial detail. Captured surface depth is centered on the
card plane before amplitude is applied, so increasing amplitude expands
thickness instead of translating the whole layer through the capture camera
clip range. A bounded contrast control expands the often narrow visible-surface
range before optional quantization; continuous relief is the baseline.
Sprite-backed splats still add one draw to the base presentation, while
full splats remain one instanced draw. These counts describe one selected
representation, not a production crowd budget.

Runtime capture accepts an explicit experimental ceiling of 4,096 by 4,096.
That size retains four RGBA8 outputs totaling 256 MiB and also needs a temporary
32-bit hardware depth texture of 64 MiB while capture is in flight, before
driver alignment and other backend overhead. High resolutions are therefore
manual stress/quality probes, not defaults or evidence for routine capture.

Credible scope from this campaign is therefore one or a few explicitly captured
actors at 96 by 96, with capture triggered by product policy. It does not
establish a direction count, animation-frame count, crowd count, GPU frame-time,
or persistent storage budget.

## Mechanism and tuning boundary

The useful mechanisms are the disposable attachment lifecycle, explicit source
selection, triggered recapture, fail-atomic replacement, bounded texture
validation, and readout separation between capture and steady submission. The
orientation experiment now also supports a camera-facing baseline, a held
admitted capture basis, and a bounded held-to-camera blend. Held cards may keep
captured elevation or retain capture azimuth while remaining world-upright.
Readout reports the admitted basis and unsigned current angular offset without
turning camera observation into content revision. These mechanisms remain
behind the explicitly experimental application-host port.

The connected-card mode now adds bounded parallax-occlusion lookup: UV travel
and a fixed 4-to-32-step budget are explicit, and zero steps restores the prior
vertex-displacement fallback. The representation transition seam also accepts
one bounded weight per retained capture using opaque selection, complementary
screen-space dither intervals, or alpha blending. CraftSurvive uses that seam
to retain left, center, and right runtime views and align them onto the center
held card. This is a software lenticular comparison, not an attempt to recover
occluded geometry.

Mode selection, base sample rows/columns, splat rows/columns, splat opacity and
blend mode, depth amplitude, quantization steps, POM travel/steps, neighboring
view separation and transition, base sprite contribution, normal influence,
splat overlap, capture elevation, resolution, and direction
are laboratory controls. Alpha-blended splats disable depth writes; additive
splats also disable depth writes and use additive blending. Instances remain
unsorted within their single draw, so alpha compositing is approximate. None is
a production default or a renderer-neutral gameplay/content property.

The next campaign mechanism is a separate retained-only `ghost-plate` mode.
It captures and displays one isolated frozen clone of the exact retained pose,
then compresses that mesh along source-camera rays while preserving its
source-view projection. Captured color remains dominant through plate-locked
or source-projective mapping while the warped mesh retains ordinary world
depth, fog, and occlusion. This is a style probe, not recovered geometry:
prepared frames are rejected and one complete retained hierarchy is required.
The directional follow-on captures 1, 4, 8, or 16 actor-relative azimuth
sectors from one exact frozen pose and fixed lens/framing/lighting setup.
Selection uses configurable angular hysteresis. Changes may hard-cut or use a
brief complementary 4x4 ordered partition in plate texel coordinates; only the
previous and current depiction render during that handoff and both suppress
depth writes until the selected plate settles. Held-animation cadence,
regional depth, stylization, elevation sectors, and offline baking remain
outside this slice. Technical acceptance does not change the campaign's visual verdict;
CraftSurvive must still establish whether the haunted-plate look is useful.

## Failure ownership

- **Source/capture:** dark rendered color, material/lighting dependence, and the
  prepared world-space versus runtime view-space normal mismatch.
- **Reconstruction:** weak visual separation among the checked modes and loss of
  sprite composition in full replacement.
- **Three/WebGL experiment:** RGBA8 depth, approximate splat orientation,
  unsorted transparent splats, and absent GPU timer evidence.
- **Host presentation:** comparison labels and compact controls can obscure the
  scene; CraftSurvive improved side-specific labels after the playtest.
- **Representation:** a bounded azimuth bank does not prove elevation
  continuity, keyed animation, occluded geometry, or body-part-specific behavior.

## Reopening criteria

Continue through visual judgment rather than adding reconstruction modes by
default. The active comparison holds one capture sector while moving about
8–12 degrees around a color-readable runtime subject: first compare connected
POM against the BLUE camera-facing card, then compare discrete, complementary
dither, and alpha transitions across the three RED angular captures. Add
viewer-biased splat axes, custom-material fog/tint, or depth-banded splats only
if that live result identifies a concrete need. Keyed animation, offline
caching, GPU timing, or another public contract remain later questions.

The deeper follow-on ideas—depth layers, motion vectors, dithered
rematerialization, and runtime G-buffer capture—remain backlog concepts. They do
not become dependencies or implied next steps from this result.
