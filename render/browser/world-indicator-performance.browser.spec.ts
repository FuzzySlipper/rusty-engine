import { expect, test } from '@playwright/test';

test('characterizes bounded structured world indicator layout and updates', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/browser/world-indicator-performance.html');
  const result = await page.evaluate(() => window.__rustyWorldIndicatorPerformance);
  expect(result?.visibleAfter100).toBe(100);
  expect(result?.visibleAfter500).toBeLessThanOrEqual(256);
  expect(result?.visibleAfter500).toBeGreaterThan(0);
  console.log(`WORLD_INDICATOR_PERF ${JSON.stringify(result)}`);
});

test('real Chromium exercises indicator layers, occlusion, overlap, edges, and camera motion', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/browser/world-indicator-performance.html');
  await page.evaluate(() => window.__rustyWorldIndicatorPerformance);
  const edge = page.locator('[data-rusty-billboard-handle="1"]');
  const depth = page.locator('[data-rusty-billboard-handle="2"]');
  const occluded = page.locator('[data-rusty-billboard-handle="3"]');
  const suppressed = page.locator('[data-rusty-billboard-handle="4"]');
  await expect(edge).toBeVisible();
  await expect(edge).toHaveAttribute('data-rusty-billboard-layer', 'alwaysOnTop');
  await expect(depth).toBeVisible();
  await expect(occluded).toBeVisible();
  await expect(suppressed).toBeHidden();
  expect(Number.parseFloat(await edge.evaluate((element) => element.style.left))).toBeGreaterThan(0);

  const leftBefore = await depth.evaluate((element) => element.style.left);
  await page.evaluate(() => window.__rustyWorldIndicatorLayout?.setCameraOffset(37));
  await expect.poll(() => depth.evaluate((element) => element.style.left)).not.toBe(leftBefore);
  await page.evaluate(() => window.__rustyWorldIndicatorLayout?.setOccluded(true));
  await expect(occluded).toBeHidden();
  await expect(depth).toBeVisible();
  await expect(edge).toBeVisible();
});
