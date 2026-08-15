import assert from 'node:assert/strict';
import { test } from '@playwright/test';

test('triggered runtime capture produces and replaces nonempty GPU texture frames', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/browser/voxel-sprite-capture.html');
  await page.waitForFunction(() => window.voxelSpriteCaptureProof?.ready === true);

  const snapshot = await page.evaluate(() => window.voxelSpriteCaptureProof!.snapshot());
  assert.equal(snapshot.captureCount, 2);
  assert.equal(snapshot.rejectedCaptureCount, 0);
  assert.equal(snapshot.currentFrameBytes, 64 * 64 * 16);
  assert.equal(snapshot.colorChanged, true);
  assert.equal(snapshot.normalChanged, true);
  for (const source of [snapshot.first, snapshot.second]) {
    for (const sample of Object.values(source)) {
      assert.ok(sample.checksum > 0);
      assert.ok(sample.nonzeroPixels > 0);
    }
  }
  assert.notEqual(snapshot.first.depth.checksum, snapshot.second.depth.checksum);
  assert.notEqual(snapshot.first.coverage.checksum, snapshot.second.coverage.checksum);
  assert.deepEqual(errors, []);

  await testInfo.attach('runtime-voxel-sprite-capture.png', {
    body: await page.locator('main').screenshot(),
    contentType: 'image/png',
  });
  const disposed = await page.evaluate(() => window.voxelSpriteCaptureProof!.dispose());
  assert.deepEqual(disposed, { disposed: true });
});
