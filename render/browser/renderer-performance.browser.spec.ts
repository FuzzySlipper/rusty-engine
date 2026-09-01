import { expect, test } from '@playwright/test';

test('reports bounded renderer submission performance and environment facts', async ({ page }) => {
  await page.goto('/browser/renderer-performance.html');
  const result = await page.evaluate(() => window.__rustyRendererPerformance);
  expect(result?.iterations).toBe(200);
  expect(result?.minimum).toBeGreaterThanOrEqual(0);
  expect(result?.p95).toBeGreaterThanOrEqual(result?.median ?? Number.NaN);
  expect(result?.canvas.cssWidth).toBe(640);
  expect(result?.canvas.cssHeight).toBe(360);
  expect(result?.canvas.backingWidth).toBeGreaterThan(0);
  expect(result?.canvas.backingHeight).toBeGreaterThan(0);
  expect(result?.submission.statistics.drawCallCount.status).toBe('available');
  console.log(`RUSTY_PERF ${JSON.stringify(result)}`);
});
