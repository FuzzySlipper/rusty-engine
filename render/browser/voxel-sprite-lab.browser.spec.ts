import assert from 'node:assert/strict';
import { expect, test } from '@playwright/test';

test('public voxel-sprite lab controls switch producers, recapture, tune live modes, and dispose', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/browser/voxel-sprite-lab.html');
  await expect(page.locator('#lab-status')).toHaveText('Ready');
  await expect(page.locator('#source-provenance')).toContainText('Runtime capture #1');
  await expect(page.locator('#capture-revision')).toHaveText('1');
  await expect(page.locator('#draw-count')).toHaveText('6');
  await expect(page.locator('#sample-count')).toHaveText(String(24 * 32 * 6));
  await expect(page.locator('[data-mode]:visible')).toHaveCount(5);

  await page.selectOption('#source-kind', 'prepared');
  await expect(page.locator('#source-provenance')).toContainText('Prepared frame');
  await expect(page.locator('#capture-ms')).toHaveText('prepared / n/a');
  await expect(page.locator('#capture-revision')).toHaveText('1');
  const preparedScreenshot = await page.locator('main').screenshot();
  assert.ok(preparedScreenshot.byteLength > 1_000);
  await testInfo.attach('prepared-voxel-sprite-source.png', {
    body: preparedScreenshot,
    contentType: 'image/png',
  });

  await page.selectOption('#capture-resolution', '128');
  await page.locator('#capture-azimuth').fill('35');
  await page.locator('#capture-elevation').fill('12');
  await page.selectOption('#capture-color-mode', 'faceted');
  await page.locator('#capture-near').fill('0.2');
  await page.locator('#capture-far').fill('12');
  await page.locator('#recapture').click();
  await expect(page.locator('#capture-revision')).toHaveText('2');
  await expect(page.locator('#source-kind')).toHaveValue('runtime');
  await expect(page.locator('#source-provenance')).toContainText('Runtime capture #2');
  await expect(page.locator('#source-provenance')).toContainText('128²');
  await page.locator('#recapture').click();
  await expect(page.locator('#capture-revision')).toHaveText('3');

  const liveBefore = Number(await page.locator('#live-revision').textContent());
  await page.locator('#depth-amplitude').fill('0.9');
  await page.locator('#depth-clamp').fill('0.62');
  await page.locator('#depth-quantization').fill('5');
  await page.locator('#splat-footprint').fill('1.6');
  await page.locator('#orientation-blend').fill('0.8');
  await page.locator('#normal-influence').fill('0.95');
  await expect(page.locator('#lab-status')).toHaveText('Reconstruction updated without recapture');
  const liveAfter = Number(await page.locator('#live-revision').textContent());
  assert.ok(liveAfter > liveBefore);
  await expect(page.locator('#capture-revision')).toHaveText('3');

  await page.selectOption('#display-mode', 'full-splat');
  await expect(page.locator('#lab-status')).toHaveText('Display mode updated immediately');
  await expect(page.locator('[data-mode]:visible')).toHaveCount(1);
  await expect(page.locator('[data-mode="full-splat"]')).toBeVisible();
  await expect(page.locator('#draw-count')).toHaveText('1');
  await expect(page.locator('#sample-count')).toHaveText(String(24 * 32));
  await expect(page.locator('#steady-ms')).not.toHaveText('—');
  assert.deepEqual(errors, []);

  await testInfo.attach('interactive-runtime-voxel-sprite-lab.png', {
    body: await page.locator('main').screenshot(),
    contentType: 'image/png',
  });
  await page.locator('#dispose-lab').click();
  await expect(page.locator('#lab-status')).toHaveText('Disposed');
  await expect(page.locator('#draw-count')).toHaveText('0');
  await expect(page.locator('#recapture')).toBeDisabled();
  assert.deepEqual(errors, []);
});
