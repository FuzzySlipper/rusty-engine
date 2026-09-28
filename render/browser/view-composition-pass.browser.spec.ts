import { expect, test } from '@playwright/test';

const CLEAR: readonly number[] = [18, 52, 86, 255];
const red = ([r = 0, g = 0, b = 0]: readonly number[]) => r > 200 && g < 100 && b < 100;
const green = ([r = 0, g = 0, b = 0]: readonly number[]) => g > 200 && r < 100 && b < 100;

test('a composed primary view replaces the fallback world pass', async ({ page }) => {
  await page.goto('/browser/view-composition-pass.html');
  await expect.poll(() => page.evaluate(() => window.__rustyViewPassProof !== undefined)).toBe(true);
  const { fallback, full, inset } = await page.evaluate(() => {
    const proof = window.__rustyViewPassProof!;
    return {
      fallback: proof.fallback(),
      full: proof.fullPrimaryView(),
      inset: proof.insetPrimaryView(),
    };
  });

  // Fallback-only rendering: world cube plus camera-relative viewmodel cube.
  expect(red(fallback.center)).toBe(true);
  expect(green(fallback.viewmodel)).toBe(true);

  // A full-canvas primary view draws the same content without an extra world
  // pass that it would immediately clear and redraw.
  expect(full.drawCallCount).toBe(fallback.drawCallCount);
  expect(red(full.center)).toBe(true);
  expect(green(full.viewmodel)).toBe(true);

  // The composition owns the primary canvas: area no primary view covers keeps
  // the clear color instead of a stale fallback camera's world.
  expect(inset.center).toEqual(CLEAR);
  expect(inset.viewmodel).toEqual(CLEAR);
});
