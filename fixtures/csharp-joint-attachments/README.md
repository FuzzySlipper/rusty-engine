# Portable mesh-to-joint package consumer

Use the same explicit SDK/feed and matching runtime procedure as
`../csharp-portable-assets`, with `CsharpJointAttachments.csproj`.
The descriptor selects a body GLB and separately authored cuboid weapon GLB.
The body is the existing Kenney character fixture; its attribution is retained
under `fixtures/render/assets/kenney-retro-character`. The weapon is an Engine
fixture cuboid, not a product asset.

Debug commands:

- `attachment.pose 0` and `attachment.pose 0.5`: hold two run poses.
- `attachment.missing`: reject `MissingHand8647`, retaining the valid scene.
- `attachment.reload`: remove/dispose/reload both meshes and their binding.
- `attachment.inspect`: report pose, joint, missing-reference error and reloads.

The inherited rig has a scale of 100. The descriptor's explicit local child
scale of 0.01 makes the separate meter-authored weapon match the model. This is
ordinary inherited glTF scale, not an Engine retargeting convention.
