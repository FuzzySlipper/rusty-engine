# Parts rig render fixtures

`../parts_rigs.rs` draws rig-c's gallery `squash`, whose model document is
`animation-parts-rig`'s fixture `squash.model.json`.

| File | What it is |
| --- | --- |
| `squash.png` | The page `rigc build --pack` wrote beside that model document (`@rig-c/core` 3.6.0, commit `1c085852856ea07a111d07b64a45ad4f800dae5e`). |
| `squash-bounce-f0005.rigc.png` | Frame 5 of `rigc render --candidate <squash build> --poser core --animation bounce --max 256`: the oracle the Engine's front-on orthographic view is compared with. |

rig-c's code, gallery rigs and art are MIT-licensed, Copyright (c) 2026
firejune (Joon Kyoung); the licence is
[`../../../animation-parts-rig/LICENSE-rig-c`](../../../animation-parts-rig/LICENSE-rig-c).

To make the frame again, build the rig as `animation-parts-rig`'s fixture
README says, then run `rigc render` under Bun as above and take
`bounce/f0005.png` from its output.
