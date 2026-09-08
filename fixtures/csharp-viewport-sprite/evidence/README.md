# Packaged C# runtime acceptance

Tested with SDK/runtime `0.1.0-dev.c62aeeff6897` and the adjacent fixture source
(including explicit playback start and publication after advancement).
Package restore and CoreCLR staging completed without warnings or errors.

At 640 × 360 CSS pixels, consecutive actual browser captures show portrait and
landscape atlas frames fitted into the same bottom-aligned viewport rectangle.
The measured colored bounds are 77 × 117 and 189 × 117 pixels respectively.
There were no page errors, Engine warnings/errors, or diagnostic drops.
Chromium reported ReadPixels performance warnings during software-rendered
screenshot capture; this is functional evidence, not a GPU performance result.

See the JSON readbacks and the two PNGs here. Multi-FOV, resize and DPR evidence
is in `../../render/viewport-sprite-evidence/`.
