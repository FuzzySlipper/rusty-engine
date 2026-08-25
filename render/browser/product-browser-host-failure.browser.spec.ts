import { expect, test } from '@playwright/test';

test('runtime startup failure disposes the application canvas and local transport', async ({ page }) => {
  await page.goto('/browser/product-browser-host-failure.html');
  await expect(page.locator('#product-browser-host-failure')).toContainText('deliberate runtime start failure');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(0);
  await expect(page.locator('#application')).toHaveAttribute('data-failure-observed', 'true');
  await expect(page.locator('#application')).toHaveAttribute('data-transport-disposed', 'true');
});
