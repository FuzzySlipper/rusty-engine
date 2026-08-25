import { expect, test } from '@playwright/test';

test('generated ProductDevHost serves the mounted UI and one Engine canvas', async ({ page, baseURL }) => {
  expect(baseURL).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/u);
  const outputStream = page.waitForRequest((request) => {
    const pathname = new URL(request.url()).pathname;
    return request.method() === 'GET' && pathname === '/__rusty/product/runtime/outputs';
  });
  const lifecycleStart = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.request().method() === 'POST'
      && url.pathname === '/__rusty/product/runtime/lifecycle/start';
  });
  const realtime = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.request().method() === 'POST'
      && url.pathname === '/__rusty/product/runtime/advance-realtime';
  });

  await page.goto('/index.html');
  await outputStream;
  const startResponse = await lifecycleStart;
  expect(startResponse.ok()).toBe(true);
  await expect(page.locator('#e2e-ui')).toHaveText('0');
  await expect(page.locator('[data-rusty-application-host]')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
  expect(await startResponse.json()).toMatchObject({
    accepted: true,
    operation: 'start',
    readout: { state: 'running' },
  });

  const realtimeResponse = await realtime;
  expect(realtimeResponse.ok()).toBe(true);
  // The generated browser host consumes this response itself. Its later
  // input-driven projection is the stronger proof that realtime admission
  // remained usable; Playwright may release a passively observed body after
  // the page's fetch consumer has read it.

  // The lifecycle response publishes the runtime binding onto the real SSE
  // stream. A claim/input request can only be admitted after that binding is
  // consumed by the browser host; there is no direct test-only input route.
  const inputResponse = page.waitForResponse((response) => {
    const url = new URL(response.url());
    if (response.request().method() !== 'POST'
      || url.pathname !== '/__rusty/product/runtime/input') return false;
    const body = response.request().postDataJSON() as {
      readonly batch?: ReadonlyArray<{ readonly fact?: { readonly kind?: string; readonly code?: string; readonly edge?: string } }>;
    };
    return body.batch?.some((event) => event.fact?.kind === 'key'
      && event.fact.code === 'key-w'
      && event.fact.edge === 'pressed') ?? false;
  });
  const canvas = page.locator('canvas[data-rusty-application-renderer="engine-owned"]');
  await canvas.focus();
  await page.keyboard.down('KeyW');
  await page.keyboard.up('KeyW');
  const input = await inputResponse;
  expect(input.ok()).toBe(true);
  const requestBody = input.request().postDataJSON() as {
    readonly batch?: ReadonlyArray<{ readonly fact?: { readonly kind?: string; readonly code?: string; readonly edge?: string } }>;
  };
  expect(requestBody.batch).toEqual(expect.arrayContaining([
    expect.objectContaining({
      fact: expect.objectContaining({ kind: 'key', code: 'key-w', edge: 'pressed' }),
    }),
  ]));
  const inputBody = await input.json() as { readonly accepted?: boolean; readonly count?: number };
  expect(inputBody.accepted).toBe(true);
  expect(inputBody.count).toBeGreaterThan(0);
  await expect(page.locator('#e2e-ui')).toHaveText('1');
});
