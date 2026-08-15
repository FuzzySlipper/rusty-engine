import assert from 'node:assert/strict';
import { test } from '@playwright/test';

test('one runtime capture drives five live voxel-sprite enhancement modes', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/browser/voxel-sprite-enhancement.html');
  await page.waitForFunction(() => window.voxelSpriteEnhancementProof?.ready === true);

  const snapshot = await page.evaluate(() => window.voxelSpriteEnhancementProof!.snapshot());
  assert.deepEqual(errors, []);
  assert.equal(snapshot.captureCount, 1);
  assert.equal(snapshot.controlsChangedWithoutRecapture, true);
  assert.equal(Object.keys(snapshot.modes).length, 5);
  assert.ok(snapshot.distinctInitialModes >= 3);
  assert.equal(snapshot.totalExpectedDrawCalls, 6);
  assert.equal(snapshot.totalGeometrySamples, 32 * 44 * 6);
  assert.equal(snapshot.modes.sprite.expectedDrawCalls, 1);
  assert.equal(snapshot.modes['sprite-splat'].expectedDrawCalls, 2);
  assert.equal(snapshot.modes['full-splat'].baseSpriteVisible, false);
  assert.equal(snapshot.modes['depth-parallax'].config.depthQuantizationSteps, 4);
  assert.ok(snapshot.modes.sprite.captureCpuSubmissionMilliseconds !== null);
  assert.ok(snapshot.modes.sprite.steadyStateCpuSubmissionMilliseconds !== null);
  assert.ok(snapshot.initialChecksums.some((value, index) => value !== snapshot.finalChecksums[index]));

  await testInfo.attach('runtime-voxel-sprite-enhancement.png', {
    body: await page.locator('main').screenshot(),
    contentType: 'image/png',
  });
  const disposed = await page.evaluate(() => window.voxelSpriteEnhancementProof!.dispose());
  assert.deepEqual(disposed, { disposed: true });
});
