import { expect, test } from '@playwright/test';

test('generated product browser host owns one canvas, cadence, input drain, and UI projection', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  await expect(page.locator('#projection')).toHaveText('projection: product.ui.v1');
  expect(await page.evaluate(() => window.__rustyProductBrowserUiContextShape)).toEqual({
    keys: ['intents', 'projection', 'ui'],
    projectionKeys: ['current', 'subscribe'],
    intentsKeys: ['claim'],
  });

  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').focus();
  await page.keyboard.press('KeyW');
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? []).some((batch) => batch.some((value) => (
    typeof value === 'object'
      && value !== null
      && 'fact' in value
      && (value as { readonly fact?: { readonly kind?: string; readonly code?: string } }).fact?.kind === 'key'
      && (value as { readonly fact?: { readonly kind?: string; readonly code?: string } }).fact?.code === 'key-w'
  ))))).toBe(true);
  await page.locator('#product-intent').click();
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? []).some((batch) => batch.some((value) => (
    typeof value === 'object'
      && value !== null
      && 'intent' in value
      && (value as { readonly intent?: string }).intent === 'product.jump'
  ))))).toBe(true);
  expect(await page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? [])
    .flat()
    .every((value) => BigInt(value.sequence) >= 1n))).toBe(true);
  expect(await page.evaluate(() => window.__rustyProductBrowserRealtimeTicks?.length ?? 0)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.__rustyProductBrowserRafCount ?? 0)).toBeGreaterThan(0);
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await expect(page.locator('#application')).toHaveAttribute('data-product-browser-state', 'ready');
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'ready');
  await expect.poll(async () => page.locator('body').getAttribute('data-rusty-product-runtime-progress')).not.toBe('0');
  await expect(page.locator('body')).not.toHaveAttribute('data-rusty-product-runtime-failure');
});

test('scheduled input receipts apply accepted and recovery cursors to later product claims', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');

  const emitScheduledResult = async (): Promise<void> => {
    await page.locator('#product-runtime-input-result').evaluate((element) => {
      (element as HTMLButtonElement).click();
    });
  };
  const claimProductIntent = async (): Promise<void> => {
    await page.locator('#product-intent').evaluate((element) => {
      (element as HTMLButtonElement).click();
    });
  };
  const expectClaimSequence = async (sequence: string): Promise<void> => {
    await expect.poll(() => page.evaluate((expected) => (window.__rustyProductBrowserInputBatches ?? [])
      .flat()
      .some((value) => typeof value === 'object'
        && value !== null
        && 'intent' in value
        && (value as { readonly sequence?: unknown }).sequence === expected), sequence)).toBe(true);
  };

  await emitScheduledResult();
  await claimProductIntent();
  await expectClaimSequence('5');

  await emitScheduledResult();
  await claimProductIntent();
  await expectClaimSequence('8');

  await emitScheduledResult();
  await claimProductIntent();
  await expectClaimSequence('9');
});

test('a transient local input request degrades once, drops its uncertain batch, and accepts fresh input', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html?transientInputFailure=1&rejectRecoveryDiagnostic=1');
  const canvas = page.locator('canvas[data-rusty-application-renderer="engine-owned"]');
  await canvas.focus();
  await page.keyboard.down('KeyW');
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserDiagnosticReports ?? []).some((report) =>
    report.hostState === 'degraded' && report.recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE',
  ))).toBe(true);
  await page.keyboard.up('KeyW');
  await expect.poll(() => page.evaluate(() => window.__rustyProductBrowserHost?.readout().state)).toBe('ready');
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserAcceptedDiagnosticReports ?? []).some((report) =>
    report.hostState === 'ready' && report.recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE',
  ))).toBe(true);
  const evidence = await page.evaluate(() => ({
    attempts: window.__rustyProductBrowserInputAttempts ?? [],
    accepted: window.__rustyProductBrowserInputBatches ?? [],
    diagnostics: window.__rustyProductBrowserDiagnosticReports ?? [],
    maximumActiveDiagnostics: window.__rustyProductBrowserMaximumActiveDiagnostics ?? 0,
    failure: document.body.dataset['rustyProductRuntimeFailure'] ?? null,
  }));
  expect(evidence.attempts).toHaveLength(2);
  expect(evidence.accepted).toHaveLength(1);
  expect(evidence.accepted[0]?.map((input) => input.sequence)).not.toEqual(
    evidence.attempts[0]?.map((input) => input.sequence),
  );
  expect(evidence.failure).toContain('transiently unavailable');
  expect(evidence.diagnostics.filter((report) =>
    report.recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE',
  )).toHaveLength(2);
  expect(evidence.maximumActiveDiagnostics).toBe(1);
});

test('a delayed degraded report flushes the later terminal transition with its first cause', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html?transientInputFailure=1&delayRecoveryDiagnostic=1&repeatRetryableFailure=1');
  const canvas = page.locator('canvas[data-rusty-application-renderer="engine-owned"]');
  await canvas.focus();
  await page.keyboard.down('KeyW');
  await expect.poll(() => page.evaluate(() => window.__rustyProductBrowserHost?.readout().state)).toBe('failed');
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserAcceptedDiagnosticReports ?? []).some((report) =>
    report.hostState === 'failed'
      && report.transportState === 'closed'
      && report.outputState === 'closed'
      && report.firstTerminal?.code === 'BROWSER_HOST_TRANSPORT_FAILED',
  ))).toBe(true);
  await expect(page.locator('#application')).toHaveAttribute('data-transport-disposed', 'true');
});

test('generated product browser host keeps UI controls out of gameplay pointer input and focuses canvas once', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await page.locator('#product-intent').click();
  await expect.poll(() => page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? []).some((batch) =>
    batch.some((value) => typeof value === 'object' && value !== null && 'intent' in value),
  ))).toBe(true);
  expect(await page.evaluate(() => (window.__rustyProductBrowserInputBatches ?? [])
    .flat()
    .filter((value) => typeof value === 'object' && value !== null && 'fact' in value)
    .map((value) => (value as { readonly fact: { readonly kind: string } }).fact.kind)))
    .not.toContain('pointer-button');

  await page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]');
    if (canvas === null) throw new Error('renderer canvas is unavailable');
    let requests = 0;
    const requestPointerLock = canvas.requestPointerLock.bind(canvas);
    canvas.requestPointerLock = () => {
      requests += 1;
      return requestPointerLock();
    };
    Object.defineProperty(canvas, '__rustyPointerLockRequests', { get: () => requests });
  });
  await page.locator('canvas[data-rusty-application-renderer="engine-owned"]').click({ position: { x: 400, y: 300 } });
  await expect.poll(() => page.evaluate(() => document.pointerLockElement?.tagName ?? null)).toBe('CANVAS');
  expect(await page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]');
    if (canvas === null) throw new Error('renderer canvas is unavailable');
    return (canvas as HTMLCanvasElement & { __rustyPointerLockRequests?: number }).__rustyPointerLockRequests;
  })).toBe(1);
});

test('generated product browser host disposes transport and application owners', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await page.locator('#product-dispose').click();
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(0);
  await expect(page.locator('#product-disposed-state')).toHaveText('state: disposed');
  await expect(page.locator('#application')).toHaveAttribute('data-product-browser-state', 'disposed');
});

test('named output lag closes the transport and leaves the host visibly failed', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await page.locator('#product-output-lag').click();
  await expect(page.locator('#product-state')).toHaveText('state: failed');
  await expect(page.locator('#application')).toHaveAttribute('data-transport-disposed', 'true');
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'failed');
  await expect(page.locator('body')).toHaveAttribute(
    'data-rusty-product-runtime-failure',
    'fixture output lag requires a fresh snapshot',
  );
  const rejected = await page.evaluate(async () => {
    try {
      await window.__rustyProductBrowserHost?.admitDemandStep();
      return null;
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    }
  });
  expect(rejected).toContain('has failed and its runtime transport is closed');
});

test('unbounded terminal diagnostics fail closed without exposing the payload', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await page.locator('#product-unbounded-terminal-failure').click();
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'failed');
  await expect(page.locator('body')).toHaveAttribute(
    'data-rusty-product-runtime-failure',
    'runtime terminal failure diagnostic exceeded host bounds',
  );
});

test('browser-owned products reject injected Rust-host progress evidence', async ({ page }) => {
  await page.goto('/browser/product-browser-host.html');
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await page.locator('#product-fake-rust-progress').click();
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'failed');
  await expect(page.locator('body')).toHaveAttribute(
    'data-rusty-product-runtime-failure',
    'Rust-host realtime progress is unavailable for this Product Browser Host mode',
  );
});
