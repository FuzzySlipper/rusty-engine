import { expect, test } from '@playwright/test';

test('Engine video host decodes WebM, advances, completes, skips, and cleans up', async ({ page }) => {
  await page.goto('/browser/video-host.html');
  await page.evaluate(() => window.__rustyVideoProof?.start());
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().width ?? 0)).toBe(32);
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().height ?? 0)).toBe(24);
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().time ?? 0)).toBeGreaterThan(0);
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().facts.length ?? 0)).toBe(1);
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().facts.at(-1))).toMatchObject({ kind: 'completed' });
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().active ?? true)).toBe(false);
  await page.evaluate(() => window.__rustyVideoProof?.start());
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().width ?? 0)).toBe(32);
  await page.evaluate(() => window.__rustyVideoProof?.skip());
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().facts.at(-1))).toMatchObject({ kind: 'skipped' });
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().active ?? true)).toBe(false);
  await page.evaluate(() => window.__rustyVideoProof?.start());
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().width ?? 0)).toBe(32);
  await page.evaluate(() => window.__rustyVideoProof?.stop());
  await expect.poll(() => page.evaluate(() => window.__rustyVideoProof?.read().active ?? true)).toBe(false);
});
