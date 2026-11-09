# Parts rig render fixtures

`../parts_rigs.rs` draws rig-c's gallery `squash` and the clipping feature
rig, whose model documents are `animation-parts-rig`'s fixtures
`squash.model.json` and `clipping.model.json`.

| File | What it is |
| --- | --- |
| `squash.png` | The page `rigc build --pack` wrote beside that model document (`@rig-c/core` 3.6.0, commit `1c085852856ea07a111d07b64a45ad4f800dae5e`). |
| `squash-bounce-f0005.rigc.png` | Frame 5 of `rigc render --candidate <squash build> --poser core --animation bounce --max 256`: the oracle the Engine's front-on orthographic view is compared with. |
| `clipping.png` | The page `rigc build --pack` wrote beside `clipping.model.json`. |
| `clipping-a-f0004.rigc.png` | Frame 4 of `rigc render --candidate <clipping build> --poser core --animation a --max 512`. |

rig-c's code, gallery rigs and art are MIT-licensed, Copyright (c) 2026
firejune (Joon Kyoung); the licence is
[`../../../animation-parts-rig/LICENSE-rig-c`](../../../animation-parts-rig/LICENSE-rig-c).
`socket.png` in the clipping page is the Engine's own (`animation-parts-rig/tests/oracle/socket.py`).

To make the frame again, build the rig as `animation-parts-rig`'s fixture
README says, then run `rigc render` under Bun as above and take
`bounce/f0005.png` (or `a/f0004.png`) from its output.
