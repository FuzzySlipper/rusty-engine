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
  expect(proof.snapshot).toContain('kind voxelObject');
  expect(proof.snapshot).toContain('frame 1');
  expect(proof.snapshot).toContain('kind sprite');
  expect(proof.snapshot).toContain('kind light/point');
  expect(proof.snapshot).toContain('layer viewmodel');
  expect(proof.snapshot).toContain('label "viewmodel-static-proof"');
  expect(proof.snapshot).toContain('label "viewmodel-animated-proof"');
  expect(proof.animationClip).toBe('run');
  expect(proof.viewmodelAnimationClip).toBe('idle');
  expect(proof.viewmodelNodeCount).toBe(3);
  expect(proof.viewmodelPickExcluded).toBe(true);
  expect(proof.autoStartRenderCount).toBe(1);
  expect(proof.autoFrameIntervalMs).toBeGreaterThan(0);
  expect(proof.backendSubmissionDurationMs).toBeGreaterThanOrEqual(0);
  expect(proof.batchedStaticPickHandle).toBe(1150);
  expect(proof.batchedStaticStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.batchedStaticStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 300,
  });
  expect(proof.batchedStaticStatistics.renderHandleCount).toEqual({
    scope: 'liveResident', status: 'available', value: 300,
  });
  expect(proof.batchedStaticResetStatistics).toEqual(proof.batchedStaticStatistics);
  expect(proof.batchedStaticRecreateStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.batchedStaticRecreateStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 2,
  });
  expect(proof.batchedStaticRecreateStatistics.renderHandleCount).toEqual({
    scope: 'liveResident', status: 'available', value: 2,
  });
  expect(proof.batchedStaticDisposed).toBe(true);
  expect(proof.explicitFrameIntervalMs).toBe(50);
  expect(proof.lightCount).toBe(1);
  expect(proof.pickHandle).toBe(101);
  expect(proof.projectionInsideViewport).toBe(true);
  expect(proof.hostSurfaceKind).toBe('rusty_renderer_surface.v1');
  expect(proof.inspectionSurfaceKind).toBe('rusty_renderer_inspection_surface.v1');
  expect(proof.inspectionGridLines).toBeGreaterThan(0);
  expect(proof.inspectionRendererStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 2,
  });
  expect(proof.inspectionRendererStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 12,
  });
  expect(proof.audioApplied).toBe(1);
  expect(proof.billboardText).toBe('Shared renderer host');
  expect(proof.particleElementCount).toBe(2);
  expect(proof.rendererStatistics.renderHandleCount).toEqual({
    scope: 'liveResident', status: 'available', value: 10,
  });
  expect(proof.rendererStatistics.animatedInstanceCount).toEqual({
    scope: 'liveResident', status: 'available', value: 2,
  });
  expect(proof.rendererStatistics.drawCallCount.status).toBe('available');
  expect(proof.rendererStatistics.drawCallCount.value).toBeGreaterThan(0);
  expect(proof.rendererStatistics.geometryResourceCount.status).toBe('available');
  expect(proof.rendererStatistics.materialResourceCount.status).toBe('available');
  expect(proof.rendererStatistics.textureResourceCount.status).toBe('available');
  expect(proof.rendererStatistics.triangleCount.status).toBe('available');
  expect(proof.resetRendererStatistics).toEqual(proof.rendererStatistics);
  expect(proof.replacementRenderSequence).toBe(1);
  expect(proof.replacementStatistics).toEqual({
    schemaVersion: 1,
    drawCallCount: { scope: 'perSubmission', status: 'available', value: 2 },
    renderHandleCount: { scope: 'liveResident', status: 'available', value: 2 },
    geometryResourceCount: { scope: 'liveResident', status: 'available', value: 2 },
    materialResourceCount: { scope: 'liveResident', status: 'available', value: 2 },
    textureResourceCount: { scope: 'liveResident', status: 'available', value: 0 },
    animatedInstanceCount: { scope: 'liveResident', status: 'available', value: 0 },
    triangleCount: { scope: 'perSubmission', status: 'available', value: 24 },
  });
  expect(proof.replacementDisposedWithHistoricalSample).toBe(true);
  expect(proof.replacementDisposedRenderRejected).toBe(true);
  expect(proof.staticMeshRecreateApplied).toBe(true);
  expect(proof.staticMeshRecreateSnapshot).not.toContain('static-lifetime-initial');
  expect(proof.staticMeshRecreateSnapshot).toContain('static-lifetime-recreated');
  expect(proof.staticMeshRecreateStatistics.renderHandleCount).toEqual({
    scope: 'liveResident', status: 'available', value: 1,
  });
  expect(proof.staticMeshRecreateStatistics.geometryResourceCount).toEqual({
    scope: 'liveResident', status: 'available', value: 1,
  });
  expect(proof.staticMeshRecreateStatistics.materialResourceCount).toEqual({
    scope: 'liveResident', status: 'available', value: 1,
  });
  expect(proof.staticMeshRecreateStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.staticMeshRecreateStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.staticMeshRecreateDisposed).toBe(true);
  expect(proof.telemetryText).toContain('Renderer proof');
  expect(proof.telemetryText).toContain('frameTimeMs:');
  expect(proof.telemetryText).toContain('backendSubmissionDurationMs:');
  expect(proof.telemetryText).toContain(
    `drawCallCount: ${String(proof.rendererStatistics.drawCallCount.value)} count`,
  );
  expect(proof.telemetryText).toContain('geometryResourceCount:');
  expect(proof.telemetryText).toContain('animatedInstanceCount: 2 count');
  expect(proof.presentationDiagnostics).toEqual([]);
  expect(proof.voxelFrameSwapApplied).toBe(true);
  expect(proof.voxelFrame).toBe(1);
  expect(consoleErrors).toEqual([]);

  const viewmodelBefore = await page.evaluate(() => window.__rustyRenderViewmodelState?.());
  await page.evaluate(() => window.__rustyRenderSetCameraPose?.([1, 2, 3]));
  await expect.poll(() => page.evaluate(() => window.__rustyRenderCameraPose?.())).toEqual([1, 2, 3]);
  expect(await page.evaluate(() => window.__rustyRenderViewmodelState?.())).toEqual(viewmodelBefore);
  const cameraBefore = await page.evaluate(() => window.__rustyRenderCameraPose?.());
  await page.click('#renderer');
  await page.keyboard.down('KeyW');
  await page.evaluate(() => window.__rustyRenderTick?.(166));
  await page.keyboard.up('KeyW');
  const cameraAfter = await page.evaluate(() => window.__rustyRenderCameraPose?.());
  expect(cameraBefore).toBeDefined();
  expect(cameraAfter).toBeDefined();
  expect(cameraAfter![2]).toBeLessThan(cameraBefore![2]);
  expect(await page.evaluate(() => window.__rustyRenderViewmodelState?.())).toEqual(viewmodelBefore);

  await page.setViewportSize({ width: 900, height: 600 });
  await page.evaluate(() => window.__rustyRenderTick?.(216));
  expect(await page.evaluate(() => window.__rustyRenderViewmodelState?.())).toEqual(viewmodelBefore);

  await page.evaluate(() => document.exitPointerLock());
  await page.click('#enable-audio');
  await expect.poll(() => page.evaluate(
    () => window.__rustyRenderProof?.audioResumeDiagnostics ?? null,
  )).toEqual([]);

  await page.evaluate(() => window.__rustyRenderDispose?.());
  expect(await page.evaluate(() => window.__rustyRenderBackendSnapshot?.())).toBe('(empty scene)\n');
  await expect(page.locator('[data-rusty-billboard-handle]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-particle-id]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-telemetry-handle]')).toHaveCount(0);
});
