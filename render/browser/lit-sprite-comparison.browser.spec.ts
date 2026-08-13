import assert from 'node:assert/strict';
import { test } from '@playwright/test';

test('lit sprite comparison shares fixtures across moving light camera and flipbook routes', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/browser/lit-sprite-comparison.html');
  await page.waitForFunction(() => window.litSpriteComparison?.ready === true);

  const initial = await page.evaluate(() => window.litSpriteComparison!.snapshot());
  assert.equal(initial.fixtureCount, 5);
  assert.deepEqual(initial.modes, [
    'unlit', 'authoredNormal', 'authoredDepth', 'derivedGradient', 'synthetic',
  ]);
  assert.equal(initial.meshCount, 30, 'five fixtures x five modes plus five soft-overlap sprites');
  assert.equal(initial.materialCount, 25);
  assert.equal(initial.textureCount, 75, 'three explicit texture roles per fixture/mode');
  assert.ok(initial.shaderProgramCount >= 5 && initial.shaderProgramCount <= 20);
  assert.ok(initial.drawCalls >= 26 && initial.drawCalls <= 60);

  const first = await page.evaluate(() => ({
    capture: window.litSpriteComparison!.capture(),
    sample: window.litSpriteComparison!.sample(),
  }));
  const routed = await page.evaluate(() => {
    const api = window.litSpriteComparison!;
    const quarter = api.step(0.25);
    const half = api.step(0.5);
    const threeQuarter = api.step(0.75);
    return { quarter, half, threeQuarter, capture: api.capture(), sample: api.sample() };
  });
  assert.equal(routed.quarter.flipbookFrame, 1);
  assert.equal(routed.half.flipbookFrame, 2);
  assert.equal(routed.threeQuarter.flipbookFrame, 3);
  assert.notEqual(routed.capture, first.capture, 'moving camera and lights materially change the rendered image');
  assert.ok(meanAbsoluteDifference(routed.sample, first.sample) > 3);

  const restored = await page.evaluate(() => {
    window.litSpriteComparison!.step(0);
    return window.litSpriteComparison!.sample();
  });
  const restorationDifference = meanAbsoluteDifference(restored, first.sample);
  assert.ok(
    restorationDifference < 1,
    `returning to the same route phase is visually stable within one luminance level; observed ${String(restorationDifference)}`,
  );

  const measured = await page.evaluate(() => window.litSpriteComparison!.measure());
  assert.ok(measured.averageRouteRenderMs !== null && measured.averageRouteRenderMs > 0);
  assert.equal(measured.flipbookFrame, 0);
  console.log('LIT_SPRITE_COMPARISON', JSON.stringify(measured));
  await testInfo.attach('lit-sprite-comparison.png', {
    body: await page.locator('#stage').screenshot(),
    contentType: 'image/png',
  });

  const disposed = await page.evaluate(() => window.litSpriteComparison!.dispose());
  assert.equal(disposed.disposed, true);
  assert.equal(disposed.meshCount, 0);
  assert.equal(disposed.materialCount, 0);
  assert.equal(disposed.textureCount, 0);
  assert.deepEqual(errors, []);
});

function meanAbsoluteDifference(left: readonly number[], right: readonly number[]): number {
  assert.equal(left.length, right.length);
  return left.reduce((total, value, index) => total + Math.abs(value - right[index]!), 0) / left.length;
}
