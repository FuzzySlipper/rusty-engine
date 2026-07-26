import { expect, test } from '@playwright/test';

test('shared host realizes retained, presentation, and inspection families in a real browser', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text());
  });
  await page.goto('/browser/');
  await expect.poll(() => page.evaluate(() => window.__rustyRenderFailure ?? null)).toBeNull();
  await page.waitForFunction(() => window.__rustyRenderProof?.ready === true);

  const proof = await page.evaluate(() => window.__rustyRenderProof!);
  expect(['webgl', 'webgl2']).toContain(proof.context);
  expect(proof.snapshot).toContain('shape group');
  expect(proof.snapshot).toContain('shape cube');
  expect(proof.snapshot).toContain('kind staticMesh');
  expect(proof.snapshot).toContain('kind animatedMesh');
  expect(proof.snapshot).toContain('kind sprite');
  expect(proof.snapshot).toContain('kind light/point');
  expect(proof.animationClip).toBe('run');
  expect(proof.autoStartRenderCount).toBe(1);
  expect(proof.autoFrameIntervalMs).toBeGreaterThan(0);
  expect(proof.backendSubmissionDurationMs).toBeGreaterThanOrEqual(0);
  expect(proof.explicitFrameIntervalMs).toBe(50);
  expect(proof.lightCount).toBe(1);
  expect(proof.pickHandle).toBe(101);
  expect(proof.projectionInsideViewport).toBe(true);
  expect(proof.hostSurfaceKind).toBe('rusty_renderer_surface.v1');
  expect(proof.inspectionSurfaceKind).toBe('rusty_renderer_inspection_surface.v1');
  expect(proof.inspectionGridLines).toBeGreaterThan(0);
  expect(proof.audioApplied).toBe(1);
  expect(proof.billboardText).toBe('Shared renderer host');
  expect(proof.particleElementCount).toBe(2);
  expect(proof.telemetryText).toContain('Renderer proof');
  expect(proof.telemetryText).toContain('frameTimeMs:');
  expect(proof.telemetryText).toContain('backendSubmissionDurationMs:');
  expect(proof.telemetryText).toContain('drawCallCount: 7 count');
  expect(proof.presentationDiagnostics).toEqual([]);
  expect(consoleErrors).toEqual([]);

  await page.evaluate(() => window.__rustyRenderSetCameraPose?.([1, 2, 3]));
  await expect.poll(() => page.evaluate(() => window.__rustyRenderCameraPose?.())).toEqual([1, 2, 3]);
  const cameraBefore = await page.evaluate(() => window.__rustyRenderCameraPose?.());
  await page.click('#renderer');
  await page.keyboard.down('KeyW');
  await page.evaluate(() => window.__rustyRenderTick?.(166));
  await page.keyboard.up('KeyW');
  const cameraAfter = await page.evaluate(() => window.__rustyRenderCameraPose?.());
  expect(cameraBefore).toBeDefined();
  expect(cameraAfter).toBeDefined();
  expect(cameraAfter![2]).toBeLessThan(cameraBefore![2]);

  await page.evaluate(() => document.exitPointerLock());
  await page.click('#enable-audio');
  await expect.poll(() => page.evaluate(
    () => window.__rustyRenderProof?.audioResumeDiagnostics ?? null,
  )).toEqual([]);

  await page.evaluate(() => window.__rustyRenderDispose?.());
  await expect(page.locator('[data-rusty-billboard-handle]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-particle-id]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-telemetry-handle]')).toHaveCount(0);
});
