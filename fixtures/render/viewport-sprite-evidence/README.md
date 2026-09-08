# Viewport sprite browser evidence — Engine #7895

Captured by `render/browser/renderer.browser.spec.ts` in real Chromium WebGL
(software/SwiftShader). The same retained parentless Viewmodel sprite uses a
full-viewport `Contain` rectangle with bottom-center alignment, while its
nonidentity authored transform would put ordinary geometry out of view.

`viewport-bounds.json` records actual framebuffer bounds normalized back to CSS
units. The test compares them with the independently calculated fitted rectangle,
allowing one backing-buffer pixel for rasterization. Nine cases cover FOV 35/100/60,
640×360 → 300×480 resize, and atlas frame changes from 320:200 to 200:320.
The PNGs preserve representative wide, portrait, and changed-frame captures.

This backend caps requested DPR 1 and 2 to 0.25. A requested ratio of 0.125
exercises a second actual backing scale. These are layout and rendering checks,
not accelerated-GPU or performance evidence. Re-running the test retains all
nine images and the bounds under `render/test-results/`.
