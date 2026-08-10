import { expect, test } from '@playwright/test';

test('application host owns composition, input arbitration, and disposal', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await expect(page.locator('[data-rusty-application-ui="downstream"] #gameplay-zone')).toBeVisible();

  await page.locator('#interface-button').click();
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(0);
  await expect.poll(() => page.evaluate(() =>
    window.__rustyApplicationHost?.ui.interactionMode(),
  )).toBe('interface');
  await page.evaluate(() => window.__rustyApplicationHost?.ui.setInteractionMode('gameplay'));
  await page.locator('#gameplay-zone').click();
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(1);

  const initialBackingSize = await page.locator('canvas').evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    return [canvas.width, canvas.height] as const;
  });
  await page.setViewportSize({ width: 900, height: 640 });
  await expect.poll(() => page.locator('canvas').evaluate((element, before) => {
    const canvas = element as HTMLCanvasElement;
    const bounds = canvas.getBoundingClientRect();
    return bounds.height === 640
      && bounds.width === 900
      && canvas.width > 0
      && canvas.height > 0
      && (canvas.width !== before[0] || canvas.height !== before[1]);
  }, initialBackingSize)).toBe(true);

  const replacement = await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.replaceFrame({ schemaVersion: 1, ops: [] }),
  );
  expect(replacement).toEqual({ applied: true, diagnostics: [] });
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await page.locator('canvas').evaluate((canvas) => { canvas.id = 'accepted-renderer'; });
  const rejectedReplacement = await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.replaceFrame({ schemaVersion: 1, ops: null }),
  );
  expect(rejectedReplacement?.applied).toBe(false);
  await expect(page.locator('canvas#accepted-renderer')).toHaveCount(1);
  await expect(page.locator('canvas')).toHaveCount(1);

  await page.locator('#gameplay-zone').click();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null))
    .toBe('CANVAS');

  await page.keyboard.down('KeyW');
  await page.evaluate(() => window.__rustyApplicationHost?.ui.setInteractionMode('modal'));
  await expect.poll(() => page.evaluate(() => document.pointerLockElement)).toBeNull();
  expect(await page.evaluate(() => window.__rustyApplicationHost?.readout())).toMatchObject({
    interactionMode: 'modal',
    pointerLocked: false,
    state: 'ready',
  });

  await page.locator('#text-entry').focus();
  await page.keyboard.type('trusted downstream UI');
  await expect(page.locator('#text-entry')).toHaveValue('trusted downstream UI');

  await page.evaluate(() => {
    window.__rustyApplicationHost?.ui.setInteractionMode('gameplay');
    window.__rustyApplicationHost?.ui.focusGameplay();
  });
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null))
    .toBe('CANVAS');

  await page.evaluate(() => window.__rustyApplicationHost?.dispose());
  await expect(page.locator('canvas')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-ui]')).toHaveCount(0);
  expect(await page.evaluate(() => window.__rustyApplicationUiDisposed)).toBe(true);

  await page.evaluate(async () => {
    const remounted = await window.__rustyApplicationMount?.();
    if (remounted === undefined) throw new Error('application remount helper is unavailable');
    window.__rustyApplicationHost = remounted;
  });
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await page.evaluate(() => window.__rustyApplicationHost?.dispose());
  await expect(page.locator('canvas')).toHaveCount(0);
});

test('late trusted UI failure cleans the renderer transactionally and leaves bounded failure UI', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const message = await page.evaluate(() => window.__rustyApplicationFailureProbe?.());
  expect(message).toContain('trusted UI mount rejected');
  await expect(page.locator('canvas')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-host]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-failure]')).toContainText(
    'trusted UI mount rejected',
  );
});

test('disposal waits for an admitted replacement and still releases every owner', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host did not mount');
    const replacement = host.renderer.replaceFrame({ schemaVersion: 1, ops: [] });
    const disposal = host.dispose();
    return {
      replacement: await replacement,
      disposal: await disposal.then(() => 'complete'),
    };
  });
  expect(result).toEqual({
    replacement: { applied: true, diagnostics: [] },
    disposal: 'complete',
  });
  await expect(page.locator('canvas')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-ui]')).toHaveCount(0);
  expect(await page.evaluate(() => window.__rustyApplicationUiDisposed)).toBe(true);
});
