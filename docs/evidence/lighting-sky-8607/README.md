# Lighting and sky fixture evidence

Original Crew browser captures from the package-consuming Engine fixture,
2026-09-25. `index.json` retains the observer's commands, values, original paths
and capture IDs; `indexed_path` names the copied original image in this folder.
A mistyped original torch-restoration path was corrected from its capture-ID
metadata before copying. No image editing was performed.

- Torch on/off/restored: warm wall/floor gradients, black room with the source
  disabled, and restored illumination. CPU luminance 24.3412 / 0 / 24.3412.
- A sample beyond a closed voxel wall reads zero; source light plus scene
  persistence round-trip passes.
- Sky 0 / 0.5 / 1: pale blue, blended blue-gray, and dark night with stars.
- `lighting.panorama` returns to the single authored day panorama. The observer
  recorded uncertainty because the generic receipt does not label that state.
  It does not change the camera: remaining outside the room is expected;
  `lighting.room` independently restores the interior view. Source and renderer
  tests cover the original single-panorama selection separately.

These are explicit fixture debug selections, not physical-input or downstream
product certification. The local package/runtime are development builds before
final immutable publication. Final exact-pair proof belongs in the Den review
handoff rather than changing the source SHA after release.

The observer reported no page errors and known WebGL ReadPixels screenshot
stall warnings. The separate report-only capture in `warnings.json` completed
both browser and Engine reads with no lag/drops and no findings; no compatible
baseline was provided, so no clean warning-delta claim is made.
