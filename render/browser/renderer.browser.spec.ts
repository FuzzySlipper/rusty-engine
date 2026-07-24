import { expect, test } from '@playwright/test';

test('shared renderer realizes the full retained family in a real WebGL context', async ({ page }) => {
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
  expect(proof.lightCount).toBe(1);
  expect(proof.pickHandle).toBe(101);
  expect(proof.projectionInsideViewport).toBe(true);
  expect(consoleErrors).toEqual([]);

  await page.evaluate(() => window.__rustyRenderDispose?.());
});
