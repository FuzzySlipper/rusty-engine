# Cloak comparison

Ordinary packaged C# product comparing two ways to move a character's
cloak, on the owner-supplied mage (`asset-pipeline/tests/mage-128-female`,
Tripo retopology of the hi3d model):

- **bones**: 5 columns of 4 cape bones hang from the chest as a
  [ragdoll](../../docs/ragdolls.md)'s simulated bones, and the GLB skins the
  cloak to them;
- **soft**: a [soft body](../../docs/soft-bodies.md) sheet of 13 × 16
  particles under the cloak, its collar row driven to the joints the GLB's
  collar follows, and the cloak drawn as a skin riding it.

Both draw the same cloak triangles with the same material, collide with the
same shapes (torso boxes and limb capsules: ragdoll bones that follow the
pose), feel the same wind and still-air drag, and step in one world at
60 Hz. One mode runs at a time, so its costs are its own.

`content/` holds what both modes need, derived from the supplied mesh by
`asset-pipeline/tests/mage-128-female/cloak-comparison/derive.py`:
`mage.glb` (the body, the cloak below the collar removed), `mage-cape.glb`
(the body and the cloak skinned to the cape bones), `mage.clips.glb` (CC0
Quaternius UAL1 clips retargeted to the SkinTokens rig), `cloak.png` (the
cloak's base colour at half size, for the cloak material both modes draw
with) and `cloak.json` (the drawn cloak, its simulation sheet, the
collar joints and the cape bones, at rest).

The run (27 s): settle (the rest pose the cloak was attached in eases into
idle), idle, walk, run, an abrupt turn, run back, an abrupt stop, a jump, a
gust of side wind, a walk past a pillar that brushes the cloak, and rest.
Clips are sampled at the run's time and cross-fade at every change, so a
moment of the run is the same pose and place in every mode and every run;
the cloaks' contacts vary a little from run to run.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-cloak-comparison/CsharpCloakComparison.csproj --live-debug
```

Live debug commands:

- `cloak.mode bones|soft` restarts the run in that mode (bones by default).
- `cloak.count <n>` restarts with `n` mages side by side; the first is
  measured.
- `cloak.tune <setting> <value>` restarts with one setting changed: the
  cape bones' `plate` share of their column, cone `swing` and `twist`,
  `twistDamping` and `angularDamping`; the soft cloak's contact `thickness`,
  `extraSubsteps`, `edge` and `bend` stiffness (Hz), the world's soft
  `substeps`; and the `drag` both feel. `cloak.tune reset 0` restores the defaults.
- `cloak.probe on|off` restarts measuring the first mage's cloak, or only
  the step's time (its readouts cost time a cost run leaves out).
- `cloak.freeze <seconds>|off` holds the run at that time, for matched
  stills; `cloak.restart` runs it again.
- `cloak.camera side|back|front|quarter` chooses where the camera follows
  the first mage from.
- `cloak.inspect`: mode, mages, the run's time and phase, and the cloak's
  particles or bones.
- `cloak.report`: per phase, the mean and worst Dynamics step (µs, every
  mage), and for the first mage: the collar's gap from where the drawn pose
  holds it, the deepest a cloak point went into the collision shapes, the
  soft cloak's largest stretch or the bone cloak's widest link gap, the
  hem's jitter while standing (RMS acceleration, m/s²), its fastest
  speed and its speed as the phase ends (over about a quarter second), and
  soft contacts.
