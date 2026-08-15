import assert from 'node:assert/strict';
import { test } from '@playwright/test';

test('offline depth-splat variants remain batched retained meshes across depth fog visibility pick and disposal routes', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/browser/depth-splat-comparison.html');
  await page.waitForFunction(() => window.depthSplatComparison?.ready === true);

  const initial = await page.evaluate(() => window.depthSplatComparison!.snapshot());
  assert.equal(initial.source.project, 'asset-pipeline');
  assert.equal(initial.source.task, 6977);
  assert.equal(initial.source.subject, 'spatial-wizard');
  assert.equal(initial.metrics.variants.length, 5);
  assert.deepEqual(initial.metrics.variants.map((variant) => variant.id), [
    'quad', 'flat', 'physical', 'compressed', 'tangent',
  ]);
  assert.equal(initial.metrics.variants.reduce((sum, variant) => sum + variant.triangles, 0), 12_130);
  assert.equal(initial.metrics.uploadedMeshBytes, 1_120_168);
  assert.equal(initial.metrics.packedMeshBytes, 1_120_200);
  assert.equal(initial.metrics.encodedTextureBytes, 8_377);
  assert.equal(initial.metrics.decodedTextureBytes, 36_864);
  assert.equal(initial.mechanisms.depictionCount, 5);
  assert.equal(initial.mechanisms.retainedInstancesPerDepiction, 1);
  assert.equal(initial.mechanisms.textureFilter, 'nearest');
  assert.deepEqual(initial.mechanisms.alphaModes, ['mask', 'mask']);
  assert.equal(initial.mechanisms.allDoubleSided, true);
  availableEquals(initial.submission.statistics.renderHandleCount, 7);
  availableEquals(initial.submission.statistics.geometryResourceCount, 7);
  availableEquals(initial.submission.statistics.textureResourceCount, 1);
  availableEquals(initial.submission.statistics.drawCallCount, 7);
  availableEquals(initial.submission.statistics.triangleCount, 12_154);
  assert.equal(initial.pick.hint?.handle, 201, 'front cube owns the center pick while it occludes the physical splat');

  const first = await page.evaluate(() => ({
    capture: window.depthSplatComparison!.capture(),
    sample: window.depthSplatComparison!.sample(),
  }));
  const nearer = await page.evaluate(() => {
    const readout = window.depthSplatComparison!.step(0.5);
    return { readout, capture: window.depthSplatComparison!.capture(), sample: window.depthSplatComparison!.sample() };
  });
  assert.notEqual(nearer.capture, first.capture);
  assert.ok(meanAbsoluteDifference(nearer.sample, first.sample) > 1.5, 'camera distance changes visible fog and depth submission');

  const unoccluded = await page.evaluate(() => window.depthSplatComparison!.setOccluder(false));
  assert.equal(unoccluded.occluderVisible, false);
  assert.equal(unoccluded.pick.hint?.handle, 102, 'center pick crosses only the renderer hint boundary to the physical depiction');
  assert.equal(unoccluded.pick.hint?.sourceTrace?.entity, 697_802);

  const isolated = await page.evaluate(() => window.depthSplatComparison!.setVisibleVariant('tangent'));
  const staticVisibility = isolated.visibility.world.handles.filter((handle) => handle.handle >= 100 && handle.handle <= 104);
  assert.deepEqual(staticVisibility.map((handle) => [handle.handle, handle.state]), [
    [100, 'hidden'], [101, 'hidden'], [102, 'hidden'], [103, 'hidden'], [104, 'frustumVisible'],
  ]);
  availableEquals(isolated.submission.statistics.geometryResourceCount, 7);
  availableEquals(isolated.submission.statistics.triangleCount, 3_044);

  const transformed = await page.evaluate(() => {
    const before = window.depthSplatComparison!.sample();
    const readout = window.depthSplatComparison!.transformVariant('tangent', 0, 2.1);
    return { before, after: window.depthSplatComparison!.sample(), readout };
  });
  assert.ok(meanAbsoluteDifference(transformed.before, transformed.after) > 2);
  assert.equal(
    transformed.readout.visibility.world.handles.find((handle) => handle.handle === 104)?.state,
    'frustumVisible',
  );

  const measured = await page.evaluate(() => {
    window.depthSplatComparison!.setVisibleVariant(null);
    window.depthSplatComparison!.setOccluder(true);
    return window.depthSplatComparison!.measure();
  });
  assert.ok(measured.averageCameraRouteMs !== null && measured.averageCameraRouteMs > 0);
  assert.ok(measured.averageCameraRouteMs < 100, `camera route averaged ${String(measured.averageCameraRouteMs)}ms`);
  console.log('DEPTH_SPLAT_COMPARISON', JSON.stringify({
    averageCameraRouteMs: measured.averageCameraRouteMs,
    metrics: measured.metrics,
    statistics: measured.submission.statistics,
  }));
  await testInfo.attach('depth-splat-comparison.png', {
    body: await page.locator('#stage').screenshot(),
    contentType: 'image/png',
  });

  const disposed = await page.evaluate(() => window.depthSplatComparison!.dispose());
  assert.deepEqual(disposed, { disposed: true });
  assert.deepEqual(errors, []);
});

function availableEquals(
  statistic: { readonly status: string; readonly value: number | null },
  expected: number,
): void {
  assert.equal(statistic.status, 'available');
  assert.equal(statistic.value, expected);
}

function meanAbsoluteDifference(left: readonly number[], right: readonly number[]): number {
  assert.equal(left.length, right.length);
  return left.reduce((sum, value, index) => sum + Math.abs(value - right[index]!), 0) / left.length;
}
