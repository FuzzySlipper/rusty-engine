import { expect, test } from '@playwright/test';

const RUNTIME = { instanceId: '8', generation: '1', controlRevision: '1' };
const READOUT = {
  artifact: 'rusty.product.runtime-readout',
  runtime: RUNTIME,
  mode: 'realtime',
  state: 'running',
  admittedSimulationSteps: '0',
  admittedPresentations: '0',
  droppedRealtimeSteps: '0',
  clockRegressions: '0',
  scaledRemainder: 0,
  lastObservedTimeNs: null,
  fault: null,
};

test('generated-style browser composition reaches a local Rust-shaped HTTP/SSE transport', async ({ page }) => {
  const requests: string[] = [];
  await page.route('**/__rusty/product/runtime/**', async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    requests.push(`${request.method()} ${url.pathname}`);
    if (url.pathname.endsWith('/outputs/fresh')) {
      const outputs = [
        { kind: 'binding', runtime: RUNTIME, nextInputSequence: '0' },
        { kind: 'runtime-readout', readout: READOUT },
        {
          kind: 'ui-projection',
          envelope: {
            artifact: 'rusty.product.ui-projection',
            runtime: RUNTIME,
            sequence: '0',
            stream: 'product.local',
            contract: 'product.local.current',
            value: { status: 'ready' },
          },
        },
      ];
      const body = outputs
        .map((value, index) => `id: ${String(index + 1)}\ndata: ${JSON.stringify(value)}\n\n`)
        .join('')
        + `id: ${String(outputs.length + 1)}\nevent: rusty-output-baseline\ndata: ${JSON.stringify({
          accepted: true,
          operation: 'start',
          binding: RUNTIME,
          nextInputSequence: '0',
          readout: READOUT,
        })}\n\n`;
      await route.fulfill({
        status: 200,
        headers: { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' },
        body,
      });
      return;
    }
    if (request.method() !== 'POST') {
      await route.fulfill({ status: 405, body: JSON.stringify({ diagnostic: 'POST required' }) });
      return;
    }
    const operation = url.pathname.slice('/__rusty/product/runtime/'.length);
    if (operation === 'lifecycle/start') {
      await route.fulfill({
        status: 200,
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ accepted: true, operation: 'start', binding: RUNTIME, readout: READOUT }),
      });
      return;
    }
    if (operation === 'input') {
      const input = request.postDataJSON() as { readonly batch?: readonly unknown[] };
      await route.fulfill({
        status: 200,
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ accepted: true, count: input.batch?.length ?? 0, binding: RUNTIME, readout: READOUT }),
      });
      return;
    }
    if (operation === 'audio-feedback' || operation === 'animation-feedback') {
      await route.fulfill({
        status: 200,
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ accepted: true, runtime: RUNTIME }),
      });
      return;
    }
    if (operation === 'advance-realtime') {
      await route.fulfill({
        status: 200,
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          accepted: true,
          operation: 'advance-realtime',
          binding: RUNTIME,
          nextInputSequence: '0',
          readout: READOUT,
        }),
      });
      return;
    }
    await route.fulfill({ status: 404, body: JSON.stringify({ diagnostic: 'unknown fixed route' }) });
  });

  await page.goto('/browser/product-browser-local-transport.html');
  await expect(page.locator('#product-state')).toHaveText('state: ready');
  await expect(page.locator('#product-projection')).toHaveText('projection: product.local.current');
  await expect.poll(() => page.locator('canvas[data-rusty-application-renderer="engine-owned"]').count()).toBe(1);
  expect(requests).toContain('GET /__rusty/product/runtime/outputs/fresh');
  expect(requests).not.toContain('POST /__rusty/product/runtime/lifecycle/start');
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'ready');
  await expect(page.locator('body')).toHaveAttribute('data-rusty-product-runtime-mode', 'realtime');
  await expect.poll(
    () => ({
      advanced: requests.some((request) => request.startsWith('POST /__rusty/product/runtime/advance-realtime')),
      requests,
    }),
    { message: 'browser-owned realtime cadence should advance after attach' },
  ).toMatchObject({ advanced: true });
});
