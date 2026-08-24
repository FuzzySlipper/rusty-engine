import { expect, test } from '@playwright/test';

test('realtime cadence coalesces slow runtime advancement to one in-flight plus the latest time', async ({ page }) => {
  await page.goto('/browser/product-browser-host-cadence.html');
  await expect(page.locator('#product-cadence-state')).toHaveText('state: ready');
  await expect.poll(async () => page.evaluate(() => window.__rustyProductCadenceRafCount ?? 0), {
    timeout: 15_000,
  }).toBeGreaterThan(1_000);
  await expect.poll(async () => page.evaluate(() => window.__rustyProductCadenceActiveRequests ?? 1)).toBe(0);
  const evidence = await page.evaluate(() => ({
    rafCount: window.__rustyProductCadenceRafCount ?? 0,
    advanceCalls: window.__rustyProductCadenceAdvanceCalls ?? 0,
    maximumActiveRequests: window.__rustyProductCadenceMaximumActiveRequests ?? 0,
    latestFrameMs: window.__rustyProductCadenceLatestFrameMs ?? 0,
    observedTimes: window.__rustyProductCadenceObservedTimes ?? [],
  }));
  expect(evidence.rafCount).toBeGreaterThan(1_000);
  expect(evidence.advanceCalls).toBeLessThan(100);
  expect(evidence.maximumActiveRequests).toBe(1);
  expect(evidence.observedTimes.length).toBeGreaterThan(0);
  expect(BigInt(evidence.observedTimes.at(-1)!)).toBe(
    BigInt(Math.round(evidence.latestFrameMs * 1_000_000)),
  );
});
