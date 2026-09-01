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
    __rustyIndicatorMeterNode?: Element | null;
    __rustyApplicationRiggedFixtureUrl?: string;
    __rustyApplicationUiContextShape?: {
      readonly keys: readonly string[];
      readonly projectionKeys: readonly string[] | null;
      readonly intentsKeys: readonly string[] | null;
    };
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
    resourceBytes: 203,
    resourceCount: 3,
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
    const pixels = new Uint8Array(
      context.drawingBufferWidth * context.drawingBufferHeight * 4,
    );
    context.readPixels(
      0,
      0,
      context.drawingBufferWidth,
      context.drawingBufferHeight,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixels,
    );
    return Array.from(pixels).filter((value) => value > 8).length;
  });
  expect(visibleResourcePixels).toBeGreaterThan(0);
  const foggedPixels = await page.locator('canvas').evaluate((element) => {
    window.__rustyApplicationHost?.renderer.renderOnce();
    const canvas = element as HTMLCanvasElement;
    const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    if (context === null) return 0;
    const pixels = new Uint8Array(
      context.drawingBufferWidth * context.drawingBufferHeight * 4,
    );
    context.readPixels(
      0,
      0,
      context.drawingBufferWidth,
      context.drawingBufferHeight,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixels,
    );
    let count = 0;
    for (let index = 0; index < pixels.length; index += 4) {
      if (pixels[index]! > 220 && pixels[index + 1]! < 35 && pixels[index + 2]! > 220) count += 1;
    }
    return count;
  });
  expect(foggedPixels).toBeGreaterThan(100);

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
    resourceBytes: 203,
    resourceCount: 3,
  });

  await page.locator('#interface-button').click();
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(0);
  await expect.poll(() => page.evaluate(() =>
    window.__rustyApplicationHost?.ui.interactionMode(),
  )).toBe('interface');
  await page.evaluate(() => window.__rustyApplicationHost?.ui.setInteractionMode('gameplay'));
  await page.locator('#gameplay-zone').click();
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(1);
  await page.evaluate(() => {
    const nativeModal = document.querySelector<HTMLElement>('#native-modal');
    const ariaModal = document.querySelector<HTMLElement>('#aria-modal-section');
    if (nativeModal === null || ariaModal === null) throw new Error('modal fixtures are unavailable');
    nativeModal.hidden = false;
    nativeModal.setAttribute('open', '');
    ariaModal.hidden = false;
  });
  await page.evaluate(() => {
    for (const id of ['native-modal', 'aria-modal-section']) {
      const modal = document.querySelector<HTMLElement>(`#${id}`);
      if (modal === null) throw new Error(`${id} is unavailable`);
      modal.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    }
  });
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

test('application-host public frames hold an admitted skinned mesh at exact normalized samples', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  const admitted = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host is unavailable');
    const fixtureUrl = window.__rustyApplicationRiggedFixtureUrl;
    if (fixtureUrl === undefined) throw new Error('animated fixture URL is unavailable');
    const response = await fetch(fixtureUrl);
    if (!response.ok) throw new Error(`animated fixture fetch failed: ${String(response.status)}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    const contentHash = 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674';
    return host.renderer.replaceContent({
      frame: {
        schemaVersion: 1,
        ops: [
          {
            op: 'defineAnimatedMesh',
            asset: {
              asset: 'mesh-animation/application-host-sample-proof', runtimeFormat: 'glb', contentHash,
              clips: [{ id: 'run', name: 'run', durationSeconds: 0.666666686534882 }],
              defaultClip: 'run', materialSlots: [],
              bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
            },
          },
          {
            op: 'createAnimatedMeshInstance', handle: 701, parent: null,
            instance: {
              asset: 'mesh-animation/application-host-sample-proof',
              transform: { translation: [0, 0, -2], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
              visible: true, materialOverrides: [],
              playback: { kind: 'sample', clip: 'run', normalizedTime: 0 },
              metadata: { sourceEntity: null, sourceSceneNode: null, tags: ['browser-proof'], label: 'held-sample' },
            },
          },
        ],
      },
      resources: [{
        identity: `mesh-resource/${contentHash.slice('sha256:'.length)}`,
        contentHash, mediaType: 'application/octet-stream', bytes,
      }],
    } as never);
  });
  expect(admitted).toEqual({ applied: true, diagnostics: [] });

  const samples = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host is unavailable');
    host.renderer.setCameraPose({ position: [0, 0.9, 3], pitchDegrees: 0, yawDegrees: 0 });
    const signature = (): number => {
      const canvas = document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]');
      const context = canvas?.getContext('webgl2') ?? canvas?.getContext('webgl');
      if (context === null || context === undefined) throw new Error('WebGL surface is unavailable');
      const pixels = new Uint8Array(context.drawingBufferWidth * context.drawingBufferHeight * 4);
      context.readPixels(0, 0, context.drawingBufferWidth, context.drawingBufferHeight, context.RGBA, context.UNSIGNED_BYTE, pixels);
      let hash = 2_166_136_261;
      for (let index = 0; index < pixels.length; index += 17) hash = Math.imul(hash ^ pixels[index]!, 16_777_619);
      return hash >>> 0;
    };
    const sample = (normalizedTime: number): number => {
      const receipt = host.renderer.applyFrame({
        schemaVersion: 1,
        ops: [{ op: 'setAnimatedMeshPlayback', handle: 701, playback: { kind: 'sample', clip: 'run', normalizedTime } }],
      });
      if (!receipt.applied) throw new Error(`sample rejected: ${JSON.stringify(receipt.diagnostics)}`);
      host.renderer.renderOnce(100 + normalizedTime * 1000);
      return signature();
    };
    const atStart = sample(0);
    const atMiddle = sample(0.5);
    const atEnd = sample(1);
    await new Promise<void>((resolve) => window.setTimeout(resolve, 80));
    host.renderer.renderOnce(2_000);
    return { atStart, atMiddle, atEnd, heldAfterAdvance: signature() };
  });
  expect(samples.atStart).not.toBe(samples.atMiddle);
  expect(samples.atMiddle).not.toBe(samples.atEnd);
  expect(samples.heldAfterAdvance).toBe(samples.atEnd);
});

test('public application host realizes and refreshes structured world indicators', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await expect.poll(() => page.evaluate(() => window.__rustyApplicationIndicatorReceipt))
    .toEqual({ applied: 1, diagnostics: [] });

  const indicator = page.locator('[data-rusty-billboard-handle="41"]');
  await expect(indicator).toBeVisible();
  await expect(indicator).toHaveAttribute('aria-label', 'Ranger status; Open');
  await expect(indicator).toContainText('Ranger');
  await expect(indicator).toContainText('Open');
  await expect(indicator.locator('img')).toHaveCount(1);
  const statusCue = indicator.locator('[data-rusty-indicator-kind="status"]');
  await expect(statusCue).toHaveCSS('background-image', /blob:/u);
  const meter = indicator.getByRole('progressbar', { name: 'Health' });
  await expect(meter).toHaveAttribute('aria-valuenow', '72');
  await expect(meter).toHaveAttribute('aria-valuemin', '0');
  await expect(meter).toHaveAttribute('aria-valuemax', '100');
  await expect(indicator.getByRole('progressbar', { name: 'Stamina' }))
    .toHaveAttribute('aria-valuenow', '44');
  expect(await indicator.evaluate((element) => getComputedStyle(element).pointerEvents)).toBe('none');

  const transformBefore = await indicator.evaluate((element) => element.getAttribute('style'));
  await page.evaluate(() => {
    window.__rustyIndicatorMeterNode =
      document.querySelector('[data-rusty-billboard-handle="41"] [role="progressbar"]');
    window.__rustyApplicationHost?.renderer.setCameraPose({
      position: [0.5, 0, 3],
      pitchDegrees: 0,
      yawDegrees: 0,
    });
  });
  await expect.poll(() => indicator.evaluate((element) => element.getAttribute('style')))
    .not.toBe(transformBefore);

  const update = await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{
        domain: 'billboard',
        meta: { sequence: 0 },
        op: {
          op: 'update',
          handle: 41,
          patch: {
            anchor: null,
            content: {
              kind: 'structured',
              indicator: {
                label: { localizationKey: 'actor.ranger.name', fallbackText: 'Ranger' },
                icon: null,
                accessibleLabel: {
                  localizationKey: 'actor.ranger.indicator',
                  fallbackText: 'Ranger status',
                },
                meters: [{
                  id: 'health',
                  accessibleLabel: {
                    localizationKey: 'resource.health',
                    fallbackText: 'Health',
                  },
                  current: 58,
                  min: 0,
                  max: 100,
                  preview: 52,
                  fillDirection: 'leftToRight',
                  segments: 10,
                  fill: [0.16, 0.72, 0.28, 1],
                  previewFill: [0.95, 0.72, 0.12, 1],
                  back: [0.04, 0.04, 0.04, 0.9],
                  border: [0, 0, 0, 1],
                }],
                statusCues: [{
                  id: 'interact',
                  label: { localizationKey: 'prompt.open', fallbackText: 'Open' },
                  icon: null,
                }],
                widthPixels: 192,
                spacingPixels: 6,
                alignment: 'center',
                style: {
                  opacity: 0.96,
                  backing: [0, 0, 0, 0.58],
                  border: [0.2, 0.2, 0.2, 1],
                  radiusPixels: 6,
                },
              },
            },
            font: null,
            heightPixels: null,
            color: null,
            background: null,
            maxDistance: null,
            layer: null,
            visible: null,
          },
        },
      }],
    }),
  );
  expect(update).toEqual({ applied: 1, diagnostics: [] });
  await expect(meter).toHaveAttribute('aria-valuenow', '58');
  expect(await page.evaluate(() =>
    window.__rustyIndicatorMeterNode ===
      document.querySelector('[data-rusty-billboard-handle="41"] [role="progressbar"]'),
  )).toBe(true);
  await expect(statusCue).toHaveCSS('background-image', 'none');

  await page.setViewportSize({ width: 720, height: 540 });
  await expect(indicator).toBeVisible();
  expect(await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{
        domain: 'billboard',
        meta: { sequence: 0 },
        op: {
          op: 'update',
          handle: 41,
          patch: {
            anchor: null,
            content: {
              kind: 'text',
              localizationKey: 'indicator.legacy',
              fallbackText: 'Legacy indicator',
              arguments: [],
            },
            font: null,
            heightPixels: null,
            color: null,
            background: null,
            maxDistance: null,
            layer: null,
            visible: null,
          },
        },
      }],
    }),
  )).toEqual({ applied: 1, diagnostics: [] });
  await expect(indicator).toHaveAttribute('role', 'status');
  await expect(indicator).not.toHaveAttribute('aria-label');
  await expect(indicator).toHaveText('Legacy indicator');
  expect(await indicator.evaluate((element) => (element as HTMLElement).style.width)).toBe('');
  expect(await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{
        domain: 'billboard',
        meta: { sequence: 0 },
        op: { op: 'destroy', handle: 41 },
      }],
    }),
  )).toEqual({ applied: 1, diagnostics: [] });
  await expect(indicator).toHaveCount(0);
});

test('public application host realizes and advances Three particle bursts', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const receipt = await page.evaluate(() =>
    window.__rustyApplicationHost?.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{
        domain: 'particle',
        meta: { sequence: 0 },
        op: {
          op: 'emit',
          signalId: 'application-host-particle-proof',
          descriptor: {
            anchor: { kind: 'world', position: [0, 0, 1.5] },
            visual: { kind: 'cube' },
            ratePerSecond: 0,
            burstCount: 1,
            lifetimeSeconds: [2, 2],
            velocityMin: [0, 0, 0],
            velocityMax: [0, 0, 0],
            acceleration: [0, 0, 0],
            sizeCurve: [{ age: 0, value: 1.5 }, { age: 1, value: 1.5 }],
            colorCurve: [
              { age: 0, color: [1, 0, 1, 1] },
              { age: 1, color: [1, 0, 1, 1] },
            ],
            flipbookFramesPerSecond: 0,
            seed: 7,
            maxParticles: 1,
            visible: true,
          },
        },
      }],
    }),
  );
  expect(receipt).toEqual({ applied: 1, diagnostics: [] });

  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
  const magentaPixels = await page.locator('canvas').evaluate((element) => {
    window.__rustyApplicationHost?.renderer.renderOnce();
    const canvas = element as HTMLCanvasElement;
    const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    if (context === null) return 0;
    const pixels = new Uint8Array(
      context.drawingBufferWidth * context.drawingBufferHeight * 4,
    );
    context.readPixels(
      0,
      0,
      context.drawingBufferWidth,
      context.drawingBufferHeight,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixels,
    );
    let count = 0;
    for (let offset = 0; offset < pixels.length; offset += 4) {
      if (pixels[offset]! > 120 && pixels[offset + 1]! < 90 && pixels[offset + 2]! > 120) {
        count += 1;
      }
    }
    return count;
  });
  expect(magentaPixels).toBeGreaterThan(100);
});

test('public application host exposes a fail-atomic voxel-sprite experiment port', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host is unavailable');
    const experiment = host.renderer.createVoxelSpriteExperiment();
    window.__rustyApplicationVoxelSpriteExperiment = experiment;
    const created = experiment.create({
      id: 'runtime-proof',
      source: {
        kind: 'retained',
        handle: 1,
        capture: {
          resolution: 64,
          azimuthDegrees: 0,
          elevationDegrees: 0,
          near: 0.1,
          far: 20,
          lighting: {
            mode: 'isolated',
            ambientIntensity: 1.2,
            keyIntensity: 2.5,
            fillIntensity: 0.9,
          },
        },
      },
      transform: { position: [-1.1, 0, 0], width: 1.8, height: 1 },
      mode: 'sprite-splat',
      config: {
        sampleColumns: 16,
        sampleRows: 8,
        splatColumns: 24,
        splatRows: 12,
        splatOpacity: 0.4,
        splatBlendMode: 'alpha-blend',
        depthAmplitude: 0.2,
        orientationPolicy: 'capture-camera-blend',
        orientationBlend: 0.25,
        orientationElevationPolicy: 'world-upright',
        lightingMode: 'normal',
        ambientLight: 0.5,
        diffuseLight: 1.2,
        outputGain: 1.3,
      },
    });
    const canvasChecksum = (): number => {
      host.renderer.renderOnce();
      const canvas = document.querySelector('canvas');
      const context = canvas?.getContext('webgl2') ?? canvas?.getContext('webgl');
      if (context === null || context === undefined) return 0;
      const pixels = new Uint8Array(context.drawingBufferWidth * context.drawingBufferHeight * 4);
      context.readPixels(
        0,
        0,
        context.drawingBufferWidth,
        context.drawingBufferHeight,
        context.RGBA,
        context.UNSIGNED_BYTE,
        pixels,
      );
      let checksum = 0;
      for (let index = 0; index < pixels.length; index += 1) {
        checksum = (checksum + pixels[index]! * ((index % 251) + 1)) % 2_147_483_647;
      }
      return checksum;
    };
    const studioDefaultChecksum = canvasChecksum();
    const lightingRecapture = experiment.recapture('runtime-proof', {
      resolution: 64,
      azimuthDegrees: 0,
      elevationDegrees: 0,
      near: 0.1,
      far: 20,
      lighting: {
        mode: 'isolated',
        ambientIntensity: 0.15,
        keyColor: [0.2, 0.4, 1],
        keyIntensity: 0.35,
        fillIntensity: 0,
      },
    });
    const studioAlternateChecksum = canvasChecksum();
    const prepared = experiment.create({
      id: 'prepared-proof',
      source: {
        kind: 'prepared',
        frame: {
          width: 8,
          height: 8,
          textures: {
            color: 'texture/application-host-voxel-color',
            depth: 'texture/application-host-voxel-depth',
            normal: 'texture/application-host-voxel-normal',
            coverage: 'texture/application-host-voxel-coverage',
          },
          depth: { near: 0.1, far: 20 },
          capture: {
            projection: 'perspective',
            position: [0, 0, 3],
            right: [1, 0, 0],
            up: [0, 1, 0],
            forward: [0, 0, -1],
            boundsMinimum: [-1, -0.5, -0.5],
            boundsMaximum: [1, 0.5, 0.5],
          },
        },
      },
      transform: { position: [1.1, 0, 0], width: 1.8, height: 1 },
      mode: 'depth-parallax',
      config: { sampleColumns: 16, sampleRows: 8, depthAmplitude: 0.2 },
    });
    const failedReplacement = experiment.replace({
      id: 'runtime-proof',
      source: {
        kind: 'retained',
        handle: 999,
        capture: {
          resolution: 64,
          azimuthDegrees: 0,
          elevationDegrees: 0,
          near: 0.1,
          far: 20,
        },
      },
      transform: { position: [0, 0, 0], width: 2, height: 1 },
      mode: 'full-splat',
    });
    const recaptured = experiment.recapture('runtime-proof', {
      resolution: 32,
      azimuthDegrees: 20,
      elevationDegrees: 5,
      near: 0.1,
      far: 20,
      lighting: { mode: 'scene' },
    });
    host.renderer.renderOnce();
    return {
      created,
      lightingRecapture,
      studioDefaultChecksum,
      studioAlternateChecksum,
      prepared,
      failedReplacement,
      recaptured,
      finalReadout: experiment.readout(),
    };
  });
  expect(result.created.applied).toBe(true);
  expect(result.created.readout.entries[0]?.source).toBe('retained');
  expect(result.created.readout.entries[0]?.enhancement?.captureCpuSubmissionMilliseconds)
    .not.toBeNull();
  expect(result.created.readout.entries[0]?.capture?.lighting?.mode).toBe('isolated');
  expect(result.created.readout.entries[0]?.enhancement?.config.lightingMode).toBe('normal');
  expect(result.created.readout.entries[0]?.enhancement?.config.outputGain).toBe(1.3);
  expect(result.created.readout.entries[0]?.enhancement?.config.splatColumns).toBe(24);
  expect(result.created.readout.entries[0]?.enhancement?.config.splatOpacity).toBe(0.4);
  expect(result.created.readout.entries[0]?.enhancement?.config.orientationPolicy)
    .toBe('capture-camera-blend');
  expect(result.created.readout.entries[0]?.enhancement?.geometrySampleCount).toBe((16 * 8) + (24 * 12));
  expect(result.created.readout.entries[0]?.enhancement?.composition)
    .toBe('base-blend-then-alpha-blended-splats');
  expect(result.lightingRecapture.applied).toBe(true);
  expect(result.studioDefaultChecksum).toBeGreaterThan(0);
  expect(result.studioAlternateChecksum).toBeGreaterThan(0);
  expect(result.studioAlternateChecksum).not.toBe(result.studioDefaultChecksum);
  expect(result.prepared.applied).toBe(true);
  expect(result.prepared.readout.entries.some((entry) => entry.source === 'prepared')).toBe(true);
  expect(result.failedReplacement.applied).toBe(false);
  expect(result.failedReplacement.diagnostics[0]?.code).toBe('missing_source');
  expect(result.failedReplacement.readout.entries
    .find((entry) => entry.id === 'runtime-proof')?.enhancement?.mode).toBe('sprite-splat');
  expect(result.recaptured.applied).toBe(true);
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')?.capture?.resolution)
    .toBe(32);
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')?.capture?.lighting?.mode)
    .toBe('scene');
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')
    ?.enhancement?.angularOffsetDegrees).not.toBeNull();
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')
    ?.enhancement?.captureBasis.forward).toHaveLength(3);

  const visiblePixels = await page.locator('canvas').evaluate((element) => {
    window.__rustyApplicationHost?.renderer.renderOnce();
    const canvas = element as HTMLCanvasElement;
    const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    if (context === null) return 0;
    const pixels = new Uint8Array(context.drawingBufferWidth * context.drawingBufferHeight * 4);
    context.readPixels(
      0,
      0,
      context.drawingBufferWidth,
      context.drawingBufferHeight,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixels,
    );
    let count = 0;
    for (let offset = 0; offset < pixels.length; offset += 4) {
      if (pixels[offset]! > 100 && pixels[offset + 1]! < 100) count += 1;
    }
    return count;
  });
  expect(visiblePixels).toBeGreaterThan(10);

  const replacement = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    const content = window.__rustyApplicationResourceContent?.();
    if (host === undefined || content === undefined) throw new Error('fixture is unavailable');
    const receipt = await host.renderer.replaceFrame(content.frame);
    let staleCode = '';
    try {
      window.__rustyApplicationVoxelSpriteExperiment?.readout();
    } catch (cause) {
      staleCode = cause instanceof Error && 'code' in cause ? String(cause.code) : String(cause);
    }
    return { receipt, staleCode };
  });
  expect(replacement.receipt).toEqual({ applied: true, diagnostics: [] });
  expect(replacement.staleCode).toBe('stale_renderer_port');
});

test('retained ghost-plate compiles in real WebGL and changes coherently off the source view', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host is unavailable');
    const experiment = host.renderer.createVoxelSpriteExperiment();
    const ghostDefinition = {
      id: 'ghost-webgl',
      source: {
        kind: 'retained',
        handle: 1,
        capture: {
          resolution: 64,
          fieldOfViewDegrees: 55,
          azimuthDegrees: 0,
          elevationDegrees: 0,
          near: 0.1,
          far: 20,
          lighting: { mode: 'isolated' },
        },
      },
      transform: { position: [0, 0, 0], width: 1.8, height: 1.8 },
      mode: 'ghost-plate',
      config: {
        ghostDepthRetention: 0.12,
        ghostAnchorPolicy: 'bounds-center',
        ghostAnchorValue: 0.5,
        ghostPlateMapping: 'plate-locked',
      },
    } as const;
    const created = experiment.create(ghostDefinition);
    const sample = (position: readonly [number, number, number]) => {
      host.renderer.setCameraPose({ position, pitchDegrees: 0, yawDegrees: 0 });
      host.renderer.renderOnce();
      const canvas = document.querySelector('canvas');
      const context = canvas?.getContext('webgl2') ?? canvas?.getContext('webgl');
      if (context === null || context === undefined) throw new Error('WebGL is unavailable');
      const pixels = new Uint8Array(context.drawingBufferWidth * context.drawingBufferHeight * 4);
      context.readPixels(
        0,
        0,
        context.drawingBufferWidth,
        context.drawingBufferHeight,
        context.RGBA,
        context.UNSIGNED_BYTE,
        pixels,
      );
      let checksum = 0;
      let covered = 0;
      const mask = new Uint8Array(pixels.length / 4);
      for (let offset = 0; offset < pixels.length; offset += 4) {
        checksum = (checksum + pixels[offset]! * (((offset / 4) % 251) + 1)) % 2_147_483_647;
        if (pixels[offset]! > 100 && pixels[offset + 2]! > 100) {
          covered += 1;
          mask[offset / 4] = 1;
        }
      }
      return {
        checksum,
        covered,
        error: context.getError(),
        mask,
        width: context.drawingBufferWidth,
        height: context.drawingBufferHeight,
      };
    };
    const silhouetteMismatch = (
      first: ReturnType<typeof sample>,
      second: ReturnType<typeof sample>,
    ) => {
      if (first.width !== second.width || first.height !== second.height) return 1;
      const hasNeighbor = (mask: Uint8Array, index: number) => {
        const x = index % first.width;
        const y = Math.floor(index / first.width);
        for (let dy = -1; dy <= 1; dy += 1) {
          for (let dx = -1; dx <= 1; dx += 1) {
            const nextX = x + dx;
            const nextY = y + dy;
            if (
              nextX >= 0 && nextX < first.width && nextY >= 0 && nextY < first.height
              && mask[nextY * first.width + nextX] === 1
            ) return true;
          }
        }
        return false;
      };
      let unmatched = 0;
      for (let index = 0; index < first.mask.length; index += 1) {
        if (first.mask[index] === 1 && !hasNeighbor(second.mask, index)) unmatched += 1;
        if (second.mask[index] === 1 && !hasNeighbor(first.mask, index)) unmatched += 1;
      }
      return unmatched / Math.max(1, first.covered + second.covered);
    };
    const sourceViewPosition = created.readout.entries[0]?.ghostPlate?.sourceViewBasis.position;
    if (sourceViewPosition === undefined) throw new Error('ghost source-view pose is unavailable');
    experiment.destroy('ghost-webgl');
    const canonicalSource = sample(sourceViewPosition);
    const recreated = experiment.create(ghostDefinition);
    const sourceView = sample(sourceViewPosition);
    const exactSourceMismatch = silhouetteMismatch(canonicalSource, sourceView);
    const sourceViewEndpoint = experiment.configure('ghost-webgl', {
      ghostDepthRetention: 1,
    });
    const sourceViewAtFullDepth = sample(sourceViewPosition);
    const configured = experiment.configure('ghost-webgl', {
      ghostDepthRetention: 0.3,
      ghostAnchorPolicy: 'bounds-normalized',
      ghostAnchorValue: 0.25,
      ghostPlateMapping: 'projective-surface',
    });
    const offAxis = sample([
      sourceViewPosition[0] + 0.5,
      sourceViewPosition[1] + 0.15,
      sourceViewPosition[2],
    ]);
    const strictConfigured = experiment.configure('ghost-webgl', {
      ghostShellMode: 'strict-source',
      ghostShellDepthEpsilon: 0.2,
    });
    const strictOffAxis = sample([
      sourceViewPosition[0] + 0.5,
      sourceViewPosition[1] + 0.15,
      sourceViewPosition[2],
    ]);
    const repairedConfigured = experiment.configure('ghost-webgl', {
      ghostShellMode: 'repaired-source',
    });
    const repairedOffAxis = sample([
      sourceViewPosition[0] + 0.5,
      sourceViewPosition[1] + 0.15,
      sourceViewPosition[2],
    ]);
    const preparedRejected = experiment.create({
      id: 'prepared-ghost-webgl',
      source: {
        kind: 'prepared',
        frame: {
          width: 8,
          height: 8,
          textures: {
            color: 'texture/application-host-voxel-color',
            depth: 'texture/application-host-voxel-depth',
            normal: 'texture/application-host-voxel-normal',
            coverage: 'texture/application-host-voxel-coverage',
          },
          depth: { near: 0.1, far: 20 },
          capture: {
            projection: 'perspective',
            position: [0, 0, 3],
            right: [1, 0, 0],
            up: [0, 1, 0],
            forward: [0, 0, -1],
            boundsMinimum: [-1, -1, -1],
            boundsMaximum: [1, 1, 1],
          },
        },
      },
      transform: { position: [2, 0, 0], width: 1, height: 1 },
      mode: 'ghost-plate',
    });
    const ghostReadout = experiment.readout();
    const ordinaryReplacement = experiment.replace({
      ...ghostDefinition,
      mode: 'sprite',
    });
    const ordinaryAfterGhost = sample(sourceViewPosition);
    return {
      created,
      recreated,
      configured,
      sourceViewEndpoint,
      canonicalSource: { ...canonicalSource, mask: undefined },
      sourceView: { ...sourceView, mask: undefined },
      sourceViewAtFullDepth: { ...sourceViewAtFullDepth, mask: undefined },
      offAxis: { ...offAxis, mask: undefined },
      strictConfigured,
      strictOffAxis: { ...strictOffAxis, mask: undefined },
      repairedConfigured,
      repairedOffAxis: { ...repairedOffAxis, mask: undefined },
      exactSourceMismatch,
      preparedRejected,
      ordinaryReplacement,
      ordinaryAfterGhost: { ...ordinaryAfterGhost, mask: undefined },
      readout: ghostReadout,
    };
  });
  expect(result.created.applied).toBe(true);
  expect(result.created.readout.entries[0]?.presentation).toBe('ghost-plate');
  expect(result.created.readout.entries[0]?.ghostPlate?.matchedPose).toBe(true);
  expect(result.recreated.applied).toBe(true);
  expect(result.canonicalSource.covered).toBeGreaterThan(10);
  expect(result.sourceView.error).toBe(0);
  expect(result.sourceView.covered).toBeGreaterThan(10);
  expect(result.exactSourceMismatch).toBeLessThan(0.08);
  expect(result.sourceViewEndpoint.applied).toBe(true);
  expect(result.sourceViewAtFullDepth.error).toBe(0);
  expect(result.sourceViewAtFullDepth.covered).toBe(result.sourceView.covered);
  expect(result.configured.applied).toBe(true);
  expect(result.offAxis.error).toBe(0);
  expect(result.offAxis.covered).toBeGreaterThan(10);
  expect(result.offAxis.checksum).not.toBe(result.sourceView.checksum);
  expect(result.strictConfigured.applied).toBe(true);
  expect(result.strictOffAxis.error).toBe(0);
  expect(result.strictOffAxis.covered).toBeGreaterThan(0);
  expect(result.strictOffAxis.covered).toBeLessThanOrEqual(result.offAxis.covered);
  expect(result.repairedConfigured.applied).toBe(true);
  expect(result.repairedOffAxis.error).toBe(0);
  expect(result.repairedOffAxis.covered).toBeGreaterThanOrEqual(result.strictOffAxis.covered);
  expect(result.readout.entries[0]?.ghostPlate?.angularOffsetDegrees).not.toBeNull();
  expect(result.readout.entries[0]?.ghostPlate?.depthRetention).toBe(0.3);
  expect(result.readout.entries[0]?.ghostPlate?.plateMapping).toBe('projective-surface');
  expect(result.readout.entries[0]?.ghostPlate?.shellMode).toBe('repaired-source');
  expect(result.readout.entries[0]?.ghostPlate?.shellDepthQuantizationStep).toBeGreaterThan(0);
  expect(result.readout.entries[0]?.ghostPlate?.rejectedFragmentRatio.status).toBe('unavailable');
  expect(result.preparedRejected.applied).toBe(false);
  expect(result.preparedRejected.diagnostics[0]?.code).toBe('invalid_definition');
  expect(result.ordinaryReplacement.applied).toBe(true);
  expect(result.ordinaryReplacement.readout.entries[0]?.presentation).toBe('enhancement');
  expect(result.ordinaryAfterGhost.error).toBe(0);
});

test('typed ghost-plate presentation operations reach Three and hard-snap sectors', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host === undefined) throw new Error('application host is unavailable');
    const descriptor = {
      source: 2,
      placement: {
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        width: 1.8,
        height: 1.8,
      },
      capture: {
        resolution: 64,
        azimuthDegrees: 0,
        elevationDegrees: 0,
        near: 0.1,
        far: 20,
        fieldOfViewDegrees: 55,
        lighting: {
          mode: 'isolated', ambientColor: [1, 1, 1], ambientIntensity: 1.1,
          keyDirection: [0.55, 0.8, 1], keyColor: [1, 0.95, 0.85], keyIntensity: 2.4,
          fillDirection: [-0.7, 0.25, 0.65], fillColor: [0.55, 0.7, 1], fillIntensity: 1,
        },
      },
      config: {
        depthRetention: 0.15, anchorPolicy: 'bounds-center', anchorValue: 0.5,
        plateMapping: 'plate-locked', shellMode: 'whole-mesh', shellDepthEpsilon: 0.12,
        sectorCount: 8, sectorHysteresisDegrees: 3,
      },
    } as const;
    const sourceFrame = host.renderer.applyFrame({
      schemaVersion: 1,
      ops: [{
        op: 'create', handle: 2, parent: null,
        node: {
          geometry: { kind: 'cube' }, material: { color: [0.9, 0.5, 0.2, 1], wireframe: false },
          transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
          visible: true, layer: 'scene', metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'typed-ghost-source' },
        },
      }],
    });
    if (!sourceFrame.applied) throw new Error(`ghost source frame failed: ${JSON.stringify(sourceFrame.diagnostics)}`);
    const created = await host.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{ domain: 'ghostPlate', meta: { sequence: 0 }, op: { op: 'create', handle: 41, descriptor } }],
    });
    if (created.applied !== 1) throw new Error(`ghost create failed: ${JSON.stringify(created.diagnostics)}`);
    host.renderer.setCameraPose({ position: [0, 0, 6], pitchDegrees: 0, yawDegrees: 0 });
    host.renderer.renderOnce();
    const first = host.renderer.ghostPlateReadout?.()?.plates[0];
    host.renderer.setCameraPose({ position: [6, 0, 0], pitchDegrees: 0, yawDegrees: -90 });
    host.renderer.renderOnce();
    const second = host.renderer.ghostPlateReadout?.()?.plates[0];
    const updated = await host.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{ domain: 'ghostPlate', meta: { sequence: 0 }, op: {
        op: 'update', handle: 41, patch: { config: { ...descriptor.config, plateMapping: 'projective-surface', shellMode: 'repaired-source', sectorCount: 16 } },
      } }],
    });
    const afterUpdate = host.renderer.ghostPlateReadout?.()?.plates[0];
    const destroyed = await host.renderer.applyPresentation({
      schemaVersion: 1,
      ops: [{ domain: 'ghostPlate', meta: { sequence: 0 }, op: { op: 'destroy', handle: 41 } }],
    });
    return { created, updated, destroyed, first, second, afterUpdate, remaining: host.renderer.ghostPlateReadout?.()?.activePlates };
  });
  expect(result.created.applied).toBe(1);
  expect(result.first?.sourceMatch).toBe(true);
  expect(result.first?.captureCpuSubmissionMilliseconds).not.toBeNull();
  expect(result.first?.currentSector).not.toBe(result.second?.currentSector);
  expect(result.updated.applied).toBe(1);
  expect(result.afterUpdate?.config.sectorCount).toBe(16);
  expect(result.afterUpdate?.config.plateMapping).toBe('projective-surface');
  expect(result.destroyed.applied).toBe(1);
  expect(result.remaining).toBe(0);
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
    resourceCount: 3,
    resourceBytes: 203,
  });
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
});

test('public application-host input ingress observes bounded physical facts and ordered UI claims', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.locator('#gameplay-zone').click({ position: { x: 40, y: 40 } });
  await expect.poll(() => page.evaluate(() => document.pointerLockElement instanceof HTMLCanvasElement)).toBe(true);
  await page.mouse.move(120, 120);
  await page.mouse.move(156, 108);
  await page.keyboard.down('w');
  await page.keyboard.down('w');
  await page.keyboard.up('w');
  await page.evaluate(() => document.dispatchEvent(new WheelEvent('wheel', { deltaY: 180 })));
  const physical = await page.evaluate(() => window.__rustyApplicationHost?.input?.drain());
  expect(physical).toBeDefined();
  const facts = (physical ?? []).map((entry) => 'fact' in entry ? entry.fact : null);
  expect(facts).toContainEqual({ kind: 'key', code: 'key-w', edge: 'pressed' });
  expect(facts).toContainEqual({ kind: 'key', code: 'key-w', edge: 'released' });
  expect(facts).toContainEqual({ kind: 'wheel', x: 0, y: 64 });
  expect(facts).toContainEqual({ kind: 'pointer-delta', x: 32, y: 32 });
  expect(physical?.every((entry, index) => entry.sequence === String(index))).toBe(true);
  expect(physical?.every((entry) => entry.runtime.instanceId === '7'
    && entry.runtime.generation === '3' && entry.runtime.controlRevision === '11')).toBe(true);

  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => page.evaluate(() => document.pointerLockElement === null)).toBe(true);
  await page.locator('#input-claim-button').click();
  const afterUiClaim = await page.evaluate(() => window.__rustyApplicationHost?.input?.drain());
  expect(afterUiClaim).toHaveLength(2);
  expect(afterUiClaim?.[0]).toMatchObject({
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'interaction-mode-loss' },
  });
  expect(afterUiClaim?.[1]).toMatchObject({
    context: 'gameplay.default',
    intent: 'ui.confirm',
    value: { kind: 'digital', active: true },
  });

  await page.evaluate(() => window.__rustyApplicationHost?.ui.setInteractionMode('interface'));
  expect(await page.evaluate(() => window.__rustyApplicationHost?.input?.drain())).toMatchObject([{
    fact: { kind: 'clear', reason: 'interaction-mode-loss' },
  }]);
  const disposalInput = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    await host?.dispose();
    return host?.input?.drain();
  });
  expect(disposalInput).toMatchObject([{ fact: { kind: 'clear', reason: 'dispose' } }]);
});

test('public application-host UI projection is read-only in the mounted DOM lane and rebinds cleanly', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    const projection = host?.uiProjection;
    if (host === undefined || projection === undefined) {
      throw new Error('UI projection ingress is unavailable');
    }
    const events: Array<string | null> = [];
    const unsubscribe = projection.subscribe((value) => events.push(value?.sequence ?? null));
    const source = {
      artifact: 'rusty.product.ui-projection' as const,
      runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
      sequence: '0',
      stream: 'product.hud',
      contract: 'product.hud.v1',
      value: { alerts: 2, selected: 'target-1' },
    };
    const accepted = projection.ingest(source);
    source.value.alerts = 99;
    const current = projection.current();
    let mutationError: string | null = null;
    try {
      if (current === null) throw new Error('projection snapshot is missing');
      Object.defineProperty(current.value, 'alerts', { value: 100 });
    } catch (error) {
      mutationError = error instanceof Error ? error.name : String(error);
    }
    const rebound = projection.bindRuntime({
      instanceId: '7', generation: '4', controlRevision: '12',
    });
    const afterRebind = projection.readout();
    unsubscribe();
    return {
      accepted,
      afterRebind,
      context: window.__rustyApplicationUiContextShape,
      current,
      events,
      mutationError,
      rebound,
    };
  });
  expect(result.accepted).toBe(true);
  expect(result.current?.value).toEqual({ alerts: 2, selected: 'target-1' });
  expect(result.mutationError).toBe('TypeError');
  expect(result.events).toEqual([null, '0', null]);
  expect(result.rebound).toBe(true);
  expect(result.afterRebind).toMatchObject({ hasCurrent: false, sequence: null, subscriberCount: 1 });
  expect(result.context).toEqual({
    keys: ['intents', 'projection', 'ui'],
    projectionKeys: ['current', 'subscribe'],
    intentsKeys: ['claim'],
  });
});

test('application host makes noninteractive UI transparent while preserving explicit UI controls', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const hitTargets = await page.evaluate(() => {
    const canvas = document.querySelector('canvas[data-rusty-application-renderer="engine-owned"]');
    const gameplay = document.querySelector<HTMLElement>('#gameplay-zone');
    const button = document.querySelector<HTMLElement>('#interface-button');
    const input = document.querySelector<HTMLElement>('#text-entry');
    if (canvas === null || gameplay === null || button === null || input === null) {
      throw new Error('hit-testing fixtures are unavailable');
    }
    const targetAtCenter = (element: HTMLElement): Element | null => {
      const bounds = element.getBoundingClientRect();
      return document.elementFromPoint(bounds.left + (bounds.width / 2), bounds.top + (bounds.height / 2));
    };
    return {
      button: targetAtCenter(button)?.id ?? null,
      input: targetAtCenter(input)?.id ?? null,
      noninteractive: targetAtCenter(gameplay) === canvas,
      uiPointerEvents: getComputedStyle(gameplay.parentElement!).pointerEvents,
    };
  });
  expect(hitTargets).toEqual({
    button: 'interface-button',
    input: 'text-entry',
    noninteractive: true,
    uiPointerEvents: 'none',
  });

  await page.evaluate(() => window.__rustyApplicationHost?.input?.drain());
  await page.locator('#interface-button').click();
  const buttonEntries = await page.evaluate(() => window.__rustyApplicationHost?.input?.drain() ?? []);
  expect(buttonEntries.filter((entry) => 'fact' in entry && entry.fact.kind === 'pointer-button')).toEqual([]);

  await page.evaluate(() => window.__rustyApplicationHost?.ui.setInteractionMode('gameplay'));
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').click();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null))
    .toBe('CANVAS');
  await page.keyboard.down('KeyW');
  await page.locator('#text-entry').focus();
  const textEntries = await page.evaluate(() => window.__rustyApplicationHost?.input?.drain() ?? []);
  expect(textEntries.some((entry) => 'fact' in entry
    && entry.fact.kind === 'clear'
    && (entry.fact.reason === 'focus-loss' || entry.fact.reason === 'pointer-lock-loss'))).toBe(true);
  await page.keyboard.up('KeyW');
});

test('application host without runtime ingress owns canvas focus through replacement and remount', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.evaluate(async () => {
    await window.__rustyApplicationHost?.dispose();
    const host = await window.__rustyApplicationMount?.(undefined, false);
    if (host === undefined) throw new Error('application host remount helper is unavailable');
    window.__rustyApplicationHost = host;
  });
  const requestCounter = async (): Promise<number> => page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]');
    if (canvas === null) throw new Error('renderer canvas is unavailable');
    let requests = 0;
    const requestPointerLock = canvas.requestPointerLock.bind(canvas);
    canvas.requestPointerLock = () => {
      requests += 1;
      return requestPointerLock();
    };
    canvas.dataset['pointerLockRequestCounter'] = 'installed';
    Object.defineProperty(canvas, '__rustyPointerLockRequests', { get: () => requests });
    return requests;
  });
  const requests = (): Promise<number> => page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]');
    if (canvas === null) throw new Error('renderer canvas is unavailable');
    return (canvas as HTMLCanvasElement & { __rustyPointerLockRequests?: number }).__rustyPointerLockRequests ?? -1;
  });

  expect(await page.evaluate(() => window.__rustyApplicationHost?.input)).toBeUndefined();
  await requestCounter();
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').click();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null)).toBe('CANVAS');
  expect(await requests()).toBe(1);

  await page.evaluate(() => document.exitPointerLock());
  await page.evaluate(() => window.__rustyApplicationHost?.renderer.replaceFrame({ schemaVersion: 1, ops: [] }));
  await requestCounter();
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').click();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null)).toBe('CANVAS');
  expect(await requests()).toBe(1);

  await page.evaluate(async () => {
    await window.__rustyApplicationHost?.dispose();
    const host = await window.__rustyApplicationMount?.(undefined, false);
    if (host === undefined) throw new Error('application host remount helper is unavailable');
    window.__rustyApplicationHost = host;
  });
  await requestCounter();
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').click();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null)).toBe('CANVAS');
  expect(await requests()).toBe(1);
});

test('application-host input ingress treats pointer cancellation as a fail-closed loss', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const entries = await page.evaluate(() => {
    const gameplay = document.querySelector<HTMLElement>('#gameplay-zone');
    if (gameplay === null) throw new Error('gameplay zone is unavailable');
    gameplay.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
    document.dispatchEvent(new PointerEvent('pointercancel'));
    return window.__rustyApplicationHost?.input?.drain();
  });
  expect(entries).toEqual([{
    runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'interaction-mode-loss' },
  }]);
});

test('application-host input ingress clears held gameplay keys when a text entry receives focus', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').evaluate((canvas) => {
    (canvas as HTMLCanvasElement).focus();
  });
  await page.keyboard.down('w');
  await page.locator('#text-entry').focus();
  expect(await page.evaluate(() => window.__rustyApplicationHost?.input?.drain())).toEqual([{
    runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'focus-loss' },
  }]);
  await page.keyboard.up('w');
});

test('public application-host samples only the selected controller on caller demand', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.locator('#gameplay-zone').click({ position: { x: 40, y: 40 } });
  await expect.poll(() => page.evaluate(() => document.pointerLockElement instanceof HTMLCanvasElement)).toBe(true);
  const controller = await page.evaluate(() => {
    window.__rustyApplicationHost?.input?.drain();
    Object.defineProperty(navigator, 'getGamepads', {
      configurable: true,
      value: () => [{
        connected: true,
        axes: [0.5, -0.25, 2, 0],
        buttons: Array.from({ length: 16 }, (_, index) => ({ pressed: index === 0 || index === 15 })),
      }],
    });
    const sampled = window.__rustyApplicationHost?.input?.sampleController();
    return { sampled, entries: window.__rustyApplicationHost?.input?.drain() };
  });
  expect(controller.sampled).toBe(5);
  expect(controller.entries).toMatchObject([
    { fact: { kind: 'controller-axis', axis: 'axis-0', value: 0.5 } },
    { fact: { kind: 'controller-axis', axis: 'axis-1', value: -0.25 } },
    { fact: { kind: 'controller-axis', axis: 'axis-2', value: 1 } },
    { fact: { kind: 'controller-button', button: 'button-0', edge: 'pressed' } },
    { fact: { kind: 'controller-button', button: 'button-15', edge: 'pressed' } },
  ]);
});

test('application-host input ingress clears against the replacement canvas without changing renderer controls', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host?.input === undefined) throw new Error('runtime input ingress is unavailable');
    const replacement = await host.renderer.replaceFrame({ schemaVersion: 1, ops: [] });
    return { replacement, entries: host.input.drain() };
  });
  expect(result.replacement).toEqual({ applied: true, diagnostics: [] });
  expect(result.entries).toEqual([{
    runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'pointer-lock-loss' },
  }]);
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
});

test('application-host input ingress detaches browser listeners after the owning host disposes', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const result = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    if (host?.input === undefined) throw new Error('runtime input ingress is unavailable');
    await host.dispose();
    const disposal = host.input.drain();
    document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW' }));
    document.dispatchEvent(new PointerEvent('pointercancel'));
    window.dispatchEvent(new Event('blur'));
    return { disposal, afterEvents: host.input.drain() };
  });
  expect(result.disposal).toMatchObject([{ fact: { kind: 'clear', reason: 'dispose' } }]);
  expect(result.afterEvents).toEqual([]);
});

test('application-host input ingress clears a pressed pointer when its release lands in a presentation gutter', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.evaluate(async () => {
    await window.__rustyApplicationHost?.dispose();
    const root = document.querySelector<HTMLElement>('#application');
    if (root === null) throw new Error('application root is unavailable');
    root.style.cssText = 'height:400px;min-height:0;overflow:hidden;width:600px;';
    const host = await window.__rustyApplicationMount?.({ minimum: 1, maximum: 1 });
    if (host === undefined) throw new Error('bounded application host did not mount');
    window.__rustyApplicationHost = host;
  });
  await page.mouse.move(300, 200);
  await page.mouse.down();
  await expect.poll(() => page.evaluate(() => document.pointerLockElement instanceof HTMLCanvasElement)).toBe(true);
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => page.evaluate(() => document.pointerLockElement === null)).toBe(true);
  await page.mouse.move(20, 20);
  await page.mouse.up();
  expect(await page.evaluate(() => window.__rustyApplicationHost?.input?.drain())).toEqual([{
    runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'interaction-mode-loss' },
  }]);
});

test('bounded presentation frame contains every layer, survives replacement and transient zero size, and clips oversized UI', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const snapshots = await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    const mount = window.__rustyApplicationMount;
    const root = document.querySelector<HTMLElement>('#application');
    if (host === undefined || mount === undefined || root === null) {
      throw new Error('application host fixture is unavailable');
    }
    await host.dispose();
    root.style.cssText = 'height:600px;min-height:0;overflow:hidden;width:600px;';
    const bounded = await mount({ minimum: 4 / 3, maximum: 16 / 9 });
    window.__rustyApplicationHost = bounded;
    const snapshot = () => {
      const rect = (selector: string) => {
        const element = document.querySelector<HTMLElement>(selector);
        if (element === null) throw new Error(`missing ${selector}`);
        const bounds = element.getBoundingClientRect();
        return { height: bounds.height, left: bounds.left, top: bounds.top, width: bounds.width };
      };
      const canvas = document.querySelector<HTMLCanvasElement>(
        'canvas[data-rusty-application-renderer="engine-owned"]',
      );
      if (canvas === null) throw new Error('renderer canvas is unavailable');
      bounded.renderer.renderOnce();
      return {
        canvas: rect('canvas[data-rusty-application-renderer="engine-owned"]'),
        canvasCount: document.querySelectorAll('canvas[data-rusty-application-renderer="engine-owned"]').length,
        frame: rect('[data-rusty-application-presentation-frame="bounded"]'),
        indicators: rect('[data-rusty-application-indicators="engine-owned"]'),
        ui: rect('[data-rusty-application-ui="downstream"]'),
        viewport: { bufferHeight: canvas.height, bufferWidth: canvas.width },
      };
    };
    const waitForLayout = async (): Promise<void> => {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    };
    const narrow = snapshot();
    const content = window.__rustyApplicationResourceContent?.();
    if (content === undefined) throw new Error('application content fixture is unavailable');
    const replacement = await bounded.renderer.replaceContent(content);
    const afterReplacement = snapshot();
    root.style.width = '0px';
    root.style.height = '0px';
    await waitForLayout();
    const zeroSized = snapshot();
    root.style.width = '800px';
    root.style.height = '500px';
    await waitForLayout();
    const inside = snapshot();
    root.style.width = '900px';
    root.style.height = '400px';
    await waitForLayout();
    const wide = snapshot();
    const scrollBeforeOversizedUi = {
      height: document.documentElement.scrollHeight,
      width: document.documentElement.scrollWidth,
    };
    const oversized = document.createElement('div');
    oversized.style.cssText = 'height:1600px;width:2400px;';
    document.querySelector('[data-rusty-application-ui="downstream"]')?.append(oversized);
    await waitForLayout();
    const afterOversizedUi = snapshot();
    return {
      afterOversizedUi,
      afterReplacement,
      inside,
      narrow,
      replacement,
      scrollAfterOversizedUi: {
        height: document.documentElement.scrollHeight,
        width: document.documentElement.scrollWidth,
      },
      scrollBeforeOversizedUi,
      wide,
      zeroSized,
    };
  });

  expect(snapshots.replacement).toEqual({ applied: true, diagnostics: [] });
  for (const state of [
    snapshots.narrow,
    snapshots.afterReplacement,
    snapshots.inside,
    snapshots.wide,
    snapshots.afterOversizedUi,
  ]) {
    expect(state.canvas).toEqual(state.frame);
    expect(state.indicators).toEqual(state.frame);
    expect(state.ui).toEqual(state.frame);
    expect(state.viewport.bufferWidth).toBeGreaterThan(0);
    expect(state.viewport.bufferHeight).toBeGreaterThan(0);
    expect(state.canvasCount).toBe(1);
  }
  expect(snapshots.zeroSized.frame).toEqual({ height: 0, left: 0, top: 0, width: 0 });
  expect(snapshots.zeroSized.canvas).toEqual(snapshots.zeroSized.frame);
  expect(snapshots.narrow.frame.width).toBeCloseTo(600);
  expect(snapshots.narrow.frame.height).toBeCloseTo(450);
  expect(snapshots.narrow.frame.top).toBeCloseTo(75);
  expect(snapshots.inside.frame).toEqual({ height: 500, left: 0, top: 0, width: 800 });
  expect(snapshots.wide.frame.width).toBeCloseTo(400 * (16 / 9));
  expect(snapshots.wide.frame.height).toBeCloseTo(400);
  expect(Math.abs(snapshots.wide.frame.left - (900 - 400 * (16 / 9)) / 2)).toBeLessThan(0.02);
  expect(snapshots.afterOversizedUi.frame).toEqual(snapshots.wide.frame);
  expect(snapshots.scrollAfterOversizedUi).toEqual(snapshots.scrollBeforeOversizedUi);
});

test('bounded gameplay input rejects gutters and accepts frame-local non-interactive input after resize', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  await page.evaluate(async () => {
    const host = window.__rustyApplicationHost;
    const mount = window.__rustyApplicationMount;
    const root = document.querySelector<HTMLElement>('#application');
    if (host === undefined || mount === undefined || root === null) {
      throw new Error('application host fixture is unavailable');
    }
    await host.dispose();
    root.style.cssText = 'height:600px;min-height:0;overflow:hidden;width:600px;';
    window.__rustyApplicationHost = await mount({ minimum: 4 / 3, maximum: 16 / 9 });
  });
  const coordinateContract = await page.evaluate(() => {
    const host = window.__rustyApplicationHost;
    const frame = document.querySelector<HTMLElement>('[data-rusty-application-presentation-frame="bounded"]');
    if (host === undefined || frame === null) throw new Error('bounded frame is unavailable');
    const bounds = frame.getBoundingClientRect();
    return {
      bottom: host.ui.allowsGameplayInput(new MouseEvent('mousedown', {
        clientX: bounds.left + 1,
        clientY: bounds.bottom,
      })),
      keyboard: host.ui.allowsGameplayInput(new KeyboardEvent('keydown', { code: 'KeyW' })),
      right: host.ui.allowsGameplayInput(new MouseEvent('mousedown', {
        clientX: bounds.right,
        clientY: bounds.top + 1,
      })),
    };
  });
  expect(coordinateContract).toEqual({ bottom: false, keyboard: true, right: false });
  await page.mouse.click(300, 10);
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(0);

  await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>('#application');
    if (root === null) throw new Error('application root is unavailable');
    root.style.width = '900px';
    root.style.height = '400px';
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    root.style.width = '600px';
    root.style.height = '600px';
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  });
  await page.locator('#gameplay-zone').click();
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(1);
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null))
    .toBe('CANVAS');

  await page.evaluate(() => {
    const gameplay = document.querySelector<HTMLElement>('#gameplay-zone');
    const button = document.querySelector<HTMLButtonElement>('#interface-button');
    if (gameplay === null || button === null) throw new Error('input fixture is unavailable');
    const buttonBounds = button.getBoundingClientRect();
    button.dispatchEvent(new MouseEvent('mousedown', {
      bubbles: true,
      clientX: buttonBounds.left + 1,
      clientY: buttonBounds.top + 1,
    }));
    const malformed = new Event('mousedown', { bubbles: true });
    Object.defineProperties(malformed, {
      clientX: { value: Number.NaN },
      clientY: { value: Number.NaN },
    });
    gameplay.dispatchEvent(malformed);
    button.click();
  });
  expect(await page.evaluate(() => window.__rustyApplicationGameplayInputCount)).toBe(1);
  await expect.poll(() => page.evaluate(() => window.__rustyApplicationHost?.ui.interactionMode()))
    .toBe('interface');
});

test('invalid bounds do not publish partial DOM and bounded mount failure retains the clipped frame', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const invalid = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>('#application');
    const mount = window.__rustyApplicationMount;
    if (root === null || mount === undefined) throw new Error('application host fixture is unavailable');
    const before = root.innerHTML;
    let code: string | null = null;
    try {
      await mount({ minimum: 0, maximum: 16 / 9 });
    } catch (error) {
      code = typeof error === 'object' && error !== null && 'code' in error
        ? String(error.code)
        : null;
    }
    return { code, unchanged: root.innerHTML === before };
  });
  expect(invalid).toEqual({ code: 'invalid_presentation_aspect_bounds', unchanged: true });

  const failure = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>('#application');
    if (root === null) throw new Error('application root is unavailable');
    root.style.cssText = 'height:600px;min-height:0;overflow:hidden;width:600px;';
    const message = await window.__rustyApplicationBoundedFailureProbe?.();
    const frame = document.querySelector<HTMLElement>('[data-rusty-application-presentation-frame="bounded"]');
    const failureElement = document.querySelector<HTMLElement>('[data-rusty-application-failure]');
    if (frame === null || failureElement === null) throw new Error('bounded failure frame is unavailable');
    const frameBounds = frame.getBoundingClientRect();
    const failureBounds = failureElement.getBoundingClientRect();
    const initialCanvasCount = document.querySelectorAll('canvas').length;
    const initialUiCount = document.querySelectorAll('[data-rusty-application-ui]').length;
    root.style.width = '900px';
    root.style.height = '400px';
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const resizedFrame = document.querySelector<HTMLElement>('[data-rusty-application-presentation-frame="bounded"]');
    if (resizedFrame === null) throw new Error('resized bounded failure frame is unavailable');
    const resizedBounds = resizedFrame.getBoundingClientRect();
    const remounted = await window.__rustyApplicationMount?.({ minimum: 1, maximum: 1 });
    if (remounted === undefined) throw new Error('application remount is unavailable');
    window.__rustyApplicationHost = remounted;
    root.style.width = '600px';
    root.style.height = '400px';
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const remountedFrame = document.querySelector<HTMLElement>('[data-rusty-application-presentation-frame="bounded"]');
    if (remountedFrame === null) throw new Error('remounted frame is unavailable');
    const remountedBounds = remountedFrame.getBoundingClientRect();
    return {
      canvasCount: initialCanvasCount,
      failure: { height: failureBounds.height, left: failureBounds.left, top: failureBounds.top, width: failureBounds.width },
      frame: { height: frameBounds.height, left: frameBounds.left, top: frameBounds.top, width: frameBounds.width },
      message,
      remounted: {
        canvasCount: document.querySelectorAll('canvas[data-rusty-application-renderer="engine-owned"]').length,
        failureCount: document.querySelectorAll('[data-rusty-application-failure]').length,
        frameCount: document.querySelectorAll('[data-rusty-application-presentation-frame="bounded"]').length,
        height: remountedBounds.height,
        left: remountedBounds.left,
        top: remountedBounds.top,
        width: remountedBounds.width,
      },
      resizedFrame: { height: resizedBounds.height, left: resizedBounds.left, top: resizedBounds.top, width: resizedBounds.width },
      uiCount: initialUiCount,
    };
  });
  expect(failure.message).toContain('bounded trusted UI mount rejected');
  expect(failure.canvasCount).toBe(0);
  expect(failure.uiCount).toBe(0);
  expect(failure.failure).toEqual(failure.frame);
  expect(failure.frame).toEqual({ height: 450, left: 0, top: 75, width: 600 });
  expect(failure.resizedFrame.height).toBeCloseTo(400);
  expect(failure.resizedFrame.width).toBeCloseTo(400 * (16 / 9));
  expect(Math.abs(failure.resizedFrame.left - (900 - 400 * (16 / 9)) / 2)).toBeLessThan(0.02);
  expect(failure.remounted).toEqual({
    canvasCount: 1,
    failureCount: 0,
    frameCount: 1,
    height: 400,
    left: 100,
    top: 0,
    width: 400,
  });
});

test('bounded loading presentation shares the live frame while unbounded mounting keeps its legacy direct layout', async ({ page }) => {
  await page.goto('/browser/application-host.html');
  const unbounded = await page.evaluate(() => ({
    frameCount: document.querySelectorAll('[data-rusty-application-presentation-frame]').length,
    hostChildren: document.querySelector('[data-rusty-application-host]')?.children.length,
  }));
  expect(unbounded).toEqual({ frameCount: 0, hostChildren: 3 });

  await page.evaluate(async () => {
    await window.__rustyApplicationHost?.dispose();
    const root = document.querySelector<HTMLElement>('#application');
    const gate = window.__rustyApplicationLoadingGate;
    if (root === null || gate === undefined) throw new Error('bounded loading fixture is unavailable');
    root.style.cssText = 'height:600px;min-height:0;overflow:hidden;width:600px;';
    void gate.mount().then((host) => { window.__rustyApplicationHost = host; });
  });
  await expect(page.locator('[data-rusty-application-loading]')).toBeVisible();
  const loading = await page.evaluate(() => {
    const rect = (selector: string) => {
      const element = document.querySelector<HTMLElement>(selector);
      if (element === null) throw new Error(`missing ${selector}`);
      const bounds = element.getBoundingClientRect();
      return { height: bounds.height, left: bounds.left, top: bounds.top, width: bounds.width };
    };
    return {
      canvas: rect('canvas[data-rusty-application-renderer="engine-owned"]'),
      frame: rect('[data-rusty-application-presentation-frame="bounded"]'),
      indicators: rect('[data-rusty-application-indicators="engine-owned"]'),
      loading: rect('[data-rusty-application-loading]'),
      ui: rect('[data-rusty-application-ui="downstream"]'),
    };
  });
  expect(loading.frame).toEqual({ height: 450, left: 0, top: 75, width: 600 });
  expect(loading.canvas).toEqual(loading.frame);
  expect(loading.indicators).toEqual(loading.frame);
  expect(loading.ui).toEqual(loading.frame);
  expect(loading.loading).toEqual(loading.frame);
  await page.evaluate(() => window.__rustyApplicationLoadingGate?.release());
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('[data-rusty-application-loading]')).toHaveCount(0);
});
