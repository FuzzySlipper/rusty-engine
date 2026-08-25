import { expect, test } from '@playwright/test';

test('runtime startup failure disposes the application canvas and local transport', async ({ page }) => {
  await page.goto('/browser/product-browser-host-failure.html');
  await expect(page.locator('#product-browser-host-failure')).toContainText('deliberate runtime start failure');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(0);
  await expect(page.locator('#application')).toHaveAttribute('data-failure-observed', 'true');
  await expect(page.locator('#application')).toHaveAttribute('data-transport-disposed', 'true');
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'failed');
  await expect(page.locator('body')).toHaveAttribute(
    'data-rusty-product-runtime-failure',
    'deliberate runtime start failure',
  );
});

test('health diagnostic truncation preserves a strict UTF-8 byte bound', async ({ page }) => {
  await page.goto('/browser/product-browser-host-failure.html?unicodeFailure=1');
  const diagnostic = await page.locator('body').getAttribute('data-rusty-product-runtime-failure');
  expect(diagnostic).not.toBeNull();
  expect(new TextEncoder().encode(diagnostic!).byteLength).toBeLessThanOrEqual(512);
  expect(diagnostic).not.toContain('\uFFFD');
});
