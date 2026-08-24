import { expect, test } from '@playwright/test';

test('generated product browser host owns one canvas, cadence, input drain, and UI projection', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await expect(page.locator('#projection')).toHaveText('projection: product.ui.v1');
  expect(await page.evaluate(() => window.__rustyProductBrowserUiContextShape)).toEqual({
    keys: ['intents', 'projection', 'ui'],
    projectionKeys: ['current', 'subscribe'],
    intentsKeys: ['claim'],
  });

  await page.locator('#product-intent').click();
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? []).some((batch) => batch.some((value) => (
    typeof value === 'object'
      && value !== null
      && 'intent' in value
      && (value as { readonly intent?: string }).intent === 'product.jump'
  ))))).toBe(true);
  expect(await page.evaluate(() => window.__rustyProductBrowserRealtimeTicks?.length ?? 0)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.__rustyProductBrowserRafCount ?? 0)).toBeGreaterThan(0);
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await expect(page.locator('#application')).toHaveAttribute('data-product-browser-state', 'ready');
});

test('generated product browser host disposes transport and application owners', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await page.locator('#product-dispose').click();
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(0);
  await expect(page.locator('#product-disposed-state')).toHaveText('state: disposed');
  await expect(page.locator('#application')).toHaveAttribute('data-product-browser-state', 'disposed');
});

test('named output lag closes the transport and leaves the host visibly failed', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await page.locator('#product-output-lag').click();
  await expect(page.locator('#product-state')).toHaveText('state: failed');
  await expect(page.locator('#application')).toHaveAttribute('data-transport-disposed', 'true');
});
