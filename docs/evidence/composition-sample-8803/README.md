# Composition colour check in `renderer.browser.spec.ts` (#8803, #8774)

## Cause

This was a stale fixture expectation, not a rendering regression.

`readCompositionPixels` in `render/browser/main.ts` samples four points. The
first two, at (0.69, 0.71) and (0.8, 0.71), land on the red and green cubes in
the overview presentation, and they passed. The last two, at (0.35, 0.45) and
(0.55, 0.45), are outside every view. They expected the fallback-camera world
pass to fill the canvas centre.

#8728 (`66e8b3ee`) removed that pass on purpose. A composition with a primary
view owns the primary canvas, and area that no primary view covers stays
cleared. #8728 proved this in `view-composition-pass.browser.spec.ts` but did
not update this fixture. The two points now read the clear colour
`[2, 4, 8, 255]` (`0x020408`), which fails `green > red * 2` as `4 > 4`.

## Change

The last two points move to (0.15, 0.23) and (0.225, 0.23), inside the red and
green cubes of the primary front-inset view. The assertion is unchanged: a
point that reads clear colour, or the wrong cube, still fails.

A full-buffer class dump chose the points. SwiftShader is a software renderer,
so its pixel ratio is capped and the 320×200 canvas has an 80×50 backing
buffer. After narrowing, the backing buffer is 40×25. At 80×50 the inset cubes
cover x 10–14 (red) and 16–20 (green), y 9–14. At 40×25 they cover x 5–7 and
8–10, y 4–6. The new points fall inside each cube at both sizes.

## Evidence

- `renderer.browser.spec.ts` passed 3 of 3 runs with system Chromium and
  SwiftShader (`PLAYWRIGHT_CHROMIUM_EXECUTABLE=$(command -v chromium)`).
- `pnpm run test:browser`: 57 passed.
- `pnpm run typecheck` passes.
