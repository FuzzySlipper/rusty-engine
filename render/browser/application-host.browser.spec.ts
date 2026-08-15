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
  expect(result.created.readout.entries[0]?.enhancement.captureCpuSubmissionMilliseconds)
    .not.toBeNull();
  expect(result.created.readout.entries[0]?.capture?.lighting?.mode).toBe('isolated');
  expect(result.created.readout.entries[0]?.enhancement.config.lightingMode).toBe('normal');
  expect(result.created.readout.entries[0]?.enhancement.config.outputGain).toBe(1.3);
  expect(result.created.readout.entries[0]?.enhancement.config.splatColumns).toBe(24);
  expect(result.created.readout.entries[0]?.enhancement.config.splatOpacity).toBe(0.4);
  expect(result.created.readout.entries[0]?.enhancement.geometrySampleCount).toBe((16 * 8) + (24 * 12));
  expect(result.created.readout.entries[0]?.enhancement.composition)
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
    .find((entry) => entry.id === 'runtime-proof')?.enhancement.mode).toBe('sprite-splat');
  expect(result.recaptured.applied).toBe(true);
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')?.capture?.resolution)
    .toBe(32);
  expect(result.finalReadout.entries.find((entry) => entry.id === 'runtime-proof')?.capture?.lighting?.mode)
    .toBe('scene');

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
