# Parts-rig fixtures

`tests/rigc_core.rs` poses each `<rig>.model.json` here and compares the
result with `<rig>.expected.json`, the poses rig-c's own core
(`@rig-c/core` 3.6.0, commit `1c085852856ea07a111d07b64a45ad4f800dae5e`)
gave for the same steps.

| Rig | Source |
| --- | --- |
| `squash`, `flex`, `portrait`, `nod` | rig-c's gallery rigs, `rigc build --pack` |
| `turned` | `squash` with trimmed and turned page regions (`../oracle/turn.py`), for page UVs |
| `features` | `../oracle/features.rig.json` and `features.motion.json` over the `squash` gallery art: shear, every inherit mode under a reflected parent, two-colour tint, separable colour timelines, blend modes, a linked mesh, a weighted mesh, a skin-required bone, a sequence, draw order and events |
| `walk` | rig-c's gallery `walk`: two two-bone iks keyed by ik timelines |
| `constraints` | `../oracle/constraints.rig.json` and `constraints.motion.json` over the same art as `features`: one-bone iks under each inherit mode with compress, stretch and both `scaleY` modes; uniform and non-uniform two-bone iks with softness, stretch and both bends; world, local, additive and clamped transforms with offsets and cross mappings onto bones of each inherit mode; a skin-listed transform; ik and transform timelines; a `turn` bone |
| `townsperson` | rusty-dagger's prototype `local/evidence/10670-townsperson/walk/front-build`: 26 additive transforms from its `turn` bone |

The model documents are rig-c's output with whitespace removed. rig-c's code,
gallery rigs and art are MIT-licensed, Copyright (c) 2026 firejune (Joon
Kyoung); the licence is [`../../LICENSE-rig-c`](../../LICENSE-rig-c).
`townsperson` is rusty-dagger's own art. A rig with a `turn` bone also has its
first clip posed with `turn`'s x driven to 24 and -24.

## Regenerating

Install rigc and Bun outside the repository, for example:

```bash
npm install --prefix <tools> bun@1
<tools>/node_modules/.bin/bun add --cwd <rigc> @rig-c/core@3.6.0
```

Build the rigs with `rigc build --rig <rig.json> --motion <motion.json> --out <dir> --pack`:
the gallery rigs from a rig-c checkout at the same commit, and `features`
from a directory holding the two `features.*.json` specs as `rig.json` and
`motion.json` beside a `parts/` directory: the gallery's
`squash/parts`, plus `blink_01.png`, `blink_02.png` and `blink_03.png` copied
from its `eyes.png`, `eyes_shut.png` and `scarf_knot.png`; `constraints` the
same way, over the same `parts/`. Make `turned` with `turn.py` from the packed
`squash`. Then write the fixtures:

```bash
python3 -I ../oracle/expect.py <bun> <rigc>/node_modules/@rig-c/core/src/core/entry.ts \
  ../oracle/oracle.ts <out> 0.1371 4 squash=<squash>/skeleton.model.json turned=… flex=… portrait=… nod=… features=… townsperson=…
python3 -I ../oracle/expect.py <bun> <entry.ts> ../oracle/oracle.ts <out> 0.1371 12 walk=… constraints=…
```
