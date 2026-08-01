import { expect, test } from '@playwright/test';

test('shared host realizes retained, presentation, and inspection families in a real browser', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text());
  });
  await page.goto('/browser/');
  await expect.poll(() => page.evaluate(() => ({
    failure: window.__rustyRenderFailure ?? null,
    ready: window.__rustyRenderProof?.ready ?? false,
  }))).toEqual({ failure: null, ready: true });

  const proof = await page.evaluate(() => window.__rustyRenderProof!);
  expect(proof.animatedCapture).toEqual({
    asset: 'mesh-animation/kenney-retro-character-medium',
    contactSheetPng: true,
    contentHash: 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674',
    diagnostics: [
      ['sampled_bounds_implausible'],
      ['sampled_bounds_implausible'],
      ['sampled_bounds_implausible'],
    ],
    imageCount: 3,
    individualPngs: true,
    normalizedTimes: [0, 0.5, 1],
    providerRevision: '1111111111111111111111111111111111111111',
    statisticsAvailable: [true, true, true],
    worldBoundsPresent: [true, true, true],
  });
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
  expect(proof.autoStartRenderCount).toBe(4);
  expect(proof.autoFrameIntervalMs).toBeGreaterThan(0);
  expect(proof.backendSubmissionDurationMs).toBeGreaterThanOrEqual(0);
  expect(['completionOnly', 'timerFailed', 'timerQuery'])
    .toContain(proof.automaticSubmissionPacing.mode);
  expect(proof.automaticSubmissionPacing.state).toBe('measuring');
  expect(['accelerated', 'software', 'unknown'])
    .toContain(proof.automaticSubmissionPacing.rendererClass);
  expect(proof.automaticSubmissionPacing.completionAgeMs).toBeGreaterThanOrEqual(0);
  expect(proof.automaticSubmissionPacing.completionAllowanceMs).toBeGreaterThanOrEqual(0);
  expect(proof.automaticSubmissionPacing.effectiveDurationMs).toBeGreaterThanOrEqual(0);
  expect(proof.automaticSubmissionPacing.targetDutyFraction).toBeGreaterThanOrEqual(0.2);
  expect(proof.automaticSubmissionPacing.targetDutyFraction).toBeLessThanOrEqual(0.5);
  expect(proof.automaticSubmissionPacing.admittedAtMs).toBeGreaterThanOrEqual(0);
  expect(proof.automaticSubmissionPacing.observedAtMs).toBeGreaterThanOrEqual(0);
  expect(proof.automaticSubmissionPacing.automaticSubmissionCapacity)
    .toBeGreaterThanOrEqual(1);
  expect(proof.automaticSubmissionPacing.automaticSubmissionLimit)
    .toBeGreaterThanOrEqual(1);
  expect(proof.automaticSubmissionPacing.automaticSubmissionLimit)
    .toBeLessThanOrEqual(proof.automaticSubmissionPacing.automaticSubmissionCapacity);
  expect(['active', 'disabled', 'unsupported'])
    .toContain(proof.automaticSubmissionPacing.completionFenceMode);
  expect(proof.automaticSubmissionPacing.pendingSubmissionCount)
    .toBeLessThanOrEqual(proof.automaticSubmissionPacing.maximumPendingSubmissions);
  expect(proof.automaticSubmissionPacing.pendingMeasurementCount)
    .toBeLessThanOrEqual(proof.automaticSubmissionPacing.maximumPendingMeasurements);
  const hostAdmission = proof.automaticSubmissionPacing.hostAdmission;
  expect(hostAdmission.attemptCount).toBeGreaterThanOrEqual(1);
  expect(
    hostAdmission.admittedCount
      + hostAdmission.backendBlockedCount
      + hostAdmission.noDemandCount,
  ).toBe(hostAdmission.attemptCount);
  expect(hostAdmission.recentAttempts.length).toBeGreaterThanOrEqual(1);
  expect(hostAdmission.recentAttempts.length).toBeLessThanOrEqual(64);
  for (const [index, attempt] of hostAdmission.recentAttempts.entries()) {
    expect(['admitted', 'backendBlocked', 'noDemand']).toContain(attempt.outcome);
    expect(attempt.sourceTimeMs).toBeGreaterThanOrEqual(0);
    expect(attempt.backend.pendingSubmissionCount)
      .toBeLessThanOrEqual(attempt.backend.maximumPendingSubmissions);
    expect(attempt.backend.pendingMeasurementCount)
      .toBeLessThanOrEqual(attempt.backend.automaticSubmissionLimit);
    const previous = hostAdmission.recentAttempts[index - 1];
    if (previous !== undefined) {
      expect(attempt.sequence).toBe(previous.sequence + 1);
      expect(attempt.sourceTimeMs).toBeGreaterThanOrEqual(previous.sourceTimeMs);
    }
  }
  if (proof.automaticSubmissionPacing.mode === 'timerQuery') {
    expect(proof.automaticSubmissionPacing.timerDurationMs).toBeGreaterThanOrEqual(0);
    expect(proof.automaticSubmissionPacing.automaticSubmissionLimit)
      .toBe(proof.automaticSubmissionPacing.automaticSubmissionCapacity);
  } else {
    expect(proof.automaticSubmissionPacing.timerDurationMs).toBeNull();
    expect(proof.automaticSubmissionPacing.automaticSubmissionLimit).toBe(1);
  }
  expect(proof.automaticSubmissionPacingSamples).toHaveLength(4);
  expect(proof.automaticSubmissionIntervalsMs).toHaveLength(4);
  expect(proof.automaticSubmissionSourceTimesMs).toHaveLength(4);
  for (const [index, sample] of proof.automaticSubmissionPacingSamples.entries()) {
    expect(sample.state).toBe('measuring');
    expect(sample.rendererClass).toBe(proof.automaticSubmissionPacing.rendererClass);
    expect(sample.automaticSubmissionCapacity)
      .toBe(proof.automaticSubmissionPacing.automaticSubmissionCapacity);
    expect(sample.pendingSubmissionCount)
      .toBeLessThanOrEqual(sample.maximumPendingSubmissions);
    expect(sample.pendingMeasurementCount)
      .toBeLessThanOrEqual(sample.maximumPendingMeasurements);
    expect(sample.admissionObservedAtMs ?? 0)
      .toBeGreaterThanOrEqual(sample.admittedAtMs ?? 0);
    expect(proof.automaticSubmissionSourceTimesMs[index]).toBeGreaterThanOrEqual(0);
  }
  if (proof.automaticSubmissionPacing.rendererClass === 'software') {
    expect(proof.automaticSubmissionPacing.completionAllowanceMs).toBe(0);
    expect(proof.rendererBufferPixelRatio[0]).toBeCloseTo(0.25);
    expect(proof.rendererBufferPixelRatio[1]).toBeCloseTo(0.25);
    for (const interval of proof.automaticSubmissionIntervalsMs.slice(1)) {
      expect(interval).toBeGreaterThanOrEqual(50);
    }
  } else {
    expect(proof.automaticSubmissionPacing.completionAllowanceMs).toBe(17);
    expect(proof.rendererBufferPixelRatio[0]).toBeCloseTo(1);
    expect(proof.rendererBufferPixelRatio[1]).toBeCloseTo(1);
  }
  expect(proof.batchedStaticPickHandle).toBe(1150);
  expect(proof.batchedStaticStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.batchedStaticStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 200,
  });
  expect(proof.batchedStaticFarStatistics.drawCallCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 1,
  });
  expect(proof.batchedStaticFarStatistics.triangleCount).toEqual({
    scope: 'perSubmission', status: 'available', value: 100,
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
  expect(proof.rendererStatistics.textureResourceCount.value).toBeGreaterThanOrEqual(1);
  expect(proof.rendererStatistics.triangleCount.status).toBe('available');
  expect(proof.resetRendererStatistics).toEqual(proof.rendererStatistics);
  expect(proof.staticDemandApplied).toBe(true);
  expect(proof.staticDemandIdleRenderCount).toBe(1);
  expect(proof.staticDemandRejectedApplied).toBe(false);
  expect(proof.staticDemandRejectedRenderCount).toBe(0);
  expect(proof.staticDemandDirtyRenderCount).toBe(1);
  expect(proof.staticDemandCameraPosition).toEqual([3, 1.62, 8]);
  expect(proof.staticDemandCameraRenderCount).toBe(1);
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
  expect(proof.voxelSurfaceAtlasPixel[0]).toBeGreaterThan(120);
  expect(proof.voxelSurfaceAtlasPixel[1]).toBeLessThan(24);
  expect(proof.voxelSurfaceAtlasPixel[2]).toBeLessThan(24);
  expect(proof.voxelSurfaceAtlasPixel[3]).toBe(255);
  expect(proof.voxelSurfaceSpecializations).toEqual([{
    material: 'material/voxel-atlas-proof',
    texture: 'texture/voxel-atlas-proof',
    mapping: 'atlas',
    tileScaleCells: [1, 1],
    tileOriginCells: [-8, 4],
    sampleUvMin: [0.375, 0.5],
    sampleUvMax: [0.375, 0.5],
    alphaMode: 'opaque',
    alphaCutoff: null,
  }]);
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
