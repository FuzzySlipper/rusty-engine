import { expect, test, type Page } from '@playwright/test';

declare global {
  interface Window {
    __rustyApplicationAdmissionGate?: {
      readonly arm: () => void;
      readonly pending: () => boolean;
      readonly release: () => void;
    };
    __rustyApplicationCanvasMutationCount?: number;
    __rustyApplicationCanvasObserver?: MutationObserver;
    __rustyApplicationPendingIncremental?: unknown;
    __rustyApplicationPendingReplacement?: Promise<unknown>;
  }
}

async function installResourceAdmissionGate(page: Page): Promise<void> {
  await page.evaluate(() => {
    // Delay the exact private application-host resolver boundary without adding
    // a production test hook or exposing renderer resource implementation.
    const originalResolve = Promise.resolve.bind(Promise);
    let armed = false;
    let pending = false;
    let release: (() => void) | null = null;
    const gatedResolve = ((value?: unknown) => {
      if (armed && value instanceof ArrayBuffer && value.byteLength === 72) {
        armed = false;
        pending = true;
        return new Promise((resolve) => {
          release = () => {
            pending = false;
            resolve(value);
          };
        });
      }
      return originalResolve(value);
    }) as typeof Promise.resolve;
    Object.defineProperty(Promise, 'resolve', {
      configurable: true,
      value: gatedResolve,
      writable: true,
    });
    window.__rustyApplicationAdmissionGate = {
      arm: () => {
        if (pending) throw new Error('resource admission gate is already pending');
        armed = true;
      },
      pending: () => pending,
      release: () => {
        if (release === null) throw new Error('resource admission gate was not reached');
        const complete = release;
        release = null;
        complete();
      },
    };
  });
}

async function observePublishedCanvasMutations(page: Page): Promise<void> {
  await page.evaluate(() => {
    window.__rustyApplicationCanvasObserver?.disconnect();
    window.__rustyApplicationCanvasMutationCount = 0;
    const host = document.querySelector('[data-rusty-application-host]');
    if (host === null) throw new Error('application host is unavailable');
    const observer = new MutationObserver((records) => {
      for (const record of records) {
        const changedCanvas = [...record.addedNodes, ...record.removedNodes]
          .some((node) => node instanceof HTMLCanvasElement);
        if (changedCanvas) {
          window.__rustyApplicationCanvasMutationCount =
            (window.__rustyApplicationCanvasMutationCount ?? 0) + 1;
        }
      }
    });
    observer.observe(host, { childList: true });
    window.__rustyApplicationCanvasObserver = observer;
  });
}

async function publishedSurfaceSnapshot(page: Page): Promise<unknown> {
  return page.evaluate(() => {
    const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>(
      'canvas[data-rusty-application-renderer="engine-owned"]',
    ));
    const canvas = canvases[0];
    let centerPixel: readonly number[] = [];
    if (canvas !== undefined) {
      const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
      if (context !== null) {
        const pixel = new Uint8Array(4);
        context.readPixels(
          Math.floor(context.drawingBufferWidth / 2),
          Math.floor(context.drawingBufferHeight / 2),
          1,
          1,
          context.RGBA,
          context.UNSIGNED_BYTE,
          pixel,
        );
        centerPixel = Array.from(pixel);
      }
    }
    return {
      canvasCount: canvases.length,
      canvasIds: canvases.map((item) => item.id),
      centerPixel,
      readout: window.__rustyApplicationHost?.readout(),
    };
  });
}

async function assertPublishedSurfaceRemains(
  page: Page,
  expected: unknown,
): Promise<void> {
  for (let sample = 0; sample < 8; sample += 1) {
    await page.evaluate(() => new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve());
    }));
    expect(await publishedSurfaceSnapshot(page)).toEqual(expected);
  }
}

test('application host owns composition, input arbitration, and disposal', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await expect(page.locator('[data-rusty-application-ui="downstream"] #gameplay-zone')).toBeVisible();
  expect(await page.evaluate(() => window.__rustyApplicationHost?.readout())).toMatchObject({
    contentRevision: 1,
    resourceBytes: 124,
    resourceCount: 2,
  });
  await page.evaluate(() => {
    window.__rustyApplicationHost?.renderer.setCameraPose({
      position: [0, 0, 3], pitchDegrees: 0, yawDegrees: 0,
    });
    window.__rustyApplicationHost?.renderer.renderOnce();
  });
  const visibleResourcePixels = await page.locator('canvas').evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    if (context === null) return 0;
    const pixels = new Uint8Array(4 * 9);
    let populated = 0;
    for (let y = 1; y <= 3; y += 1) {
      for (let x = 1; x <= 3; x += 1) {
        context.readPixels(
          Math.floor(context.drawingBufferWidth * x / 4),
          Math.floor(context.drawingBufferHeight * y / 4),
          1,
          1,
          context.RGBA,
          context.UNSIGNED_BYTE,
          pixels.subarray(populated * 4, populated * 4 + 4),
        );
        populated += 1;
      }
    }
    return Array.from(pixels).filter((value) => value > 8).length;
  });
  expect(visibleResourcePixels).toBeGreaterThan(0);

  await page.locator('#audio-button').click();
  await expect.poll(() => page.evaluate(() => ({
    presentation: window.__rustyApplicationAudioReceipt,
    resume: window.__rustyApplicationAudioResume,
  }))).toEqual({
    presentation: { applied: 1, diagnostics: [] },
    resume: { resumed: true, diagnostics: [] },
  });

  await page.locator('canvas').evaluate((canvas) => { canvas.id = 'resource-backed-renderer'; });
  await installResourceAdmissionGate(page);
  await observePublishedCanvasMutations(page);
  const publishedBeforeReplacement = await publishedSurfaceSnapshot(page);
  await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    const content = window.__rustyApplicationResourceContent?.(true);
    if (host === undefined || content === undefined) {
      throw new Error('application host or resource content helper is unavailable');
    }
    window.__rustyApplicationAdmissionGate?.arm();
    window.__rustyApplicationPendingReplacement =
      host.renderer.replaceContent(content);
  });
  await expect.poll(() => page.evaluate(() =>
    window.__rustyApplicationAdmissionGate?.pending() ?? false,
  )).toBe(true);
  await assertPublishedSurfaceRemains(page, publishedBeforeReplacement);
  expect(await page.evaluate(() => window.__rustyApplicationCanvasMutationCount)).toBe(0);
  const corruptContent = await page.evaluate(async () => {
    window.__rustyApplicationAdmissionGate?.release();
    return window.__rustyApplicationPendingReplacement;
  });
  expect(corruptContent).toMatchObject({
    applied: false,
    diagnostics: [{ code: 'resource_admission_failed' }],
  });
  expect(await page.evaluate(() => window.__rustyApplicationCanvasMutationCount)).toBe(0);
  await expect(page.locator('canvas#resource-backed-renderer')).toHaveCount(1);
  expect(await page.evaluate(() => window.__rustyApplicationHost?.readout().contentRevision)).toBe(1);

  const missingResource = await page.evaluate(() => {
    const content = window.__rustyApplicationResourceContent?.();
    if (content === undefined) throw new Error('resource content helper is unavailable');
    return window.__rustyApplicationHost?.renderer.replaceContent({
      frame: content.frame,
      resources: [],
    });
  });
  expect(missingResource).toMatchObject({
    applied: false,
    diagnostics: [{ code: 'resource_admission_failed' }],
  });
  await expect(page.locator('canvas#resource-backed-renderer')).toHaveCount(1);

  await observePublishedCanvasMutations(page);
  await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    const content = window.__rustyApplicationResourceContent?.();
    if (host === undefined || content === undefined) {
      throw new Error('application host or resource content helper is unavailable');
    }
    window.__rustyApplicationAdmissionGate?.arm();
    window.__rustyApplicationPendingReplacement =
      host.renderer.replaceContent(content);
    window.__rustyApplicationPendingIncremental =
      host.renderer.applyFrame({
      schemaVersion: 1,
      ops: [],
    });
    content.resources?.[0]?.bytes.fill(0);
  });
  await expect.poll(() => page.evaluate(() =>
    window.__rustyApplicationAdmissionGate?.pending() ?? false,
  )).toBe(true);
  await assertPublishedSurfaceRemains(page, publishedBeforeReplacement);
  expect(await page.evaluate(() => window.__rustyApplicationCanvasMutationCount)).toBe(0);
  const restoredContent = await page.evaluate(async () => {
    window.__rustyApplicationAdmissionGate?.release();
    return {
      incremental: window.__rustyApplicationPendingIncremental,
      replacement: await window.__rustyApplicationPendingReplacement,
    };
  });
  expect(restoredContent).toEqual({
    incremental: {
      applied: false,
      diagnostics: [{
        code: 'content_replacement_in_progress',
        message: 'incremental frames are rejected while complete content replacement is pending',
      }],
    },
    replacement: { applied: true, diagnostics: [] },
  });
  await expect.poll(() => page.evaluate(() =>
    window.__rustyApplicationCanvasMutationCount,
  )).toBe(1);
  await expect(page.locator('canvas#resource-backed-renderer')).toHaveCount(0);
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  expect(await page.evaluate(() => window.__rustyApplicationHost?.readout())).toMatchObject({
    contentRevision: 2,
    resourceBytes: 124,
    resourceCount: 2,
  });

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

test('initial resource failure never publishes a surface or mounts downstream UI', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const message = await page.evaluate(() =>
    window.__rustyApplicationInitialResourceFailureProbe?.(),
  );
  expect(message).toContain('expected sha256:');
  await expect(page.locator('canvas')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-host]')).toHaveCount(0);
  await expect(page.locator('[data-rusty-application-failure]')).toContainText(
    'expected sha256:',
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

test('queued complete content replacements publish in call order', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const receipts = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    const content = window.__rustyApplicationResourceContent?.();
    if (host === undefined || content === undefined) throw new Error('application host unavailable');
    return Promise.all([
      host.renderer.replaceContent({ frame: { schemaVersion: 1, ops: [] }, resources: [] }),
      host.renderer.replaceContent(content),
    ]);
  });
  expect(receipts).toEqual([
    { applied: true, diagnostics: [] },
    { applied: true, diagnostics: [] },
  ]);
  expect(await page.evaluate(() => window.__rustyApplicationHost?.readout())).toMatchObject({
    contentRevision: 3,
    resourceCount: 2,
    resourceBytes: 124,
  });
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
});
