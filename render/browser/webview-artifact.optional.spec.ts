import { expect, test } from '@playwright/test';

interface WebviewMessage {
  readonly kind: string;
  readonly message?: string;
  readonly operation?: string;
  readonly requestId?: number;
  readonly value?: unknown;
}

interface PrivateRendererApi {
  readState(requestId: number): void;
  readInput(requestId: number): void;
  renderOnce(requestId: number, timeMs: number): void;
  resize(requestId: number, width: number, height: number, pixelRatio: number): void;
  start(requestId: number): void;
  stop(requestId: number): void;
  dispose(requestId: number): void;
}

interface ArtifactWindow extends Window {
  readonly __rustyEnginePrivateRenderer: PrivateRendererApi;
  readonly __rustyWebviewMessages: WebviewMessage[];
  readonly __rustyTrackedListenerCounts: () => Readonly<Record<string, number>>;
}

test('Engine-private artifact realizes one renderer through the fixed host contract', async ({ page }) => {
  await page.goto('/browser/webview-artifact.html');
  await expect.poll(
    () => page.evaluate(() => (window as unknown as ArtifactWindow)
      .__rustyWebviewMessages.some((message) => message.kind === 'ready')),
  ).toBe(true);

  await page.keyboard.down('KeyW');
  await page.mouse.move(24, 32);
  await page.evaluate(() => {
    const renderer = (window as unknown as ArtifactWindow).__rustyEnginePrivateRenderer;
    renderer.readInput(1);
    renderer.renderOnce(2, 10);
    renderer.resize(3, 400, 300, 1);
    renderer.start(4);
    renderer.stop(5);
    renderer.dispose(6);
  });
  await page.keyboard.up('KeyW');

  await expect.poll(
    () => page.evaluate(() => (window as unknown as ArtifactWindow).__rustyWebviewMessages.length),
  ).toBeGreaterThanOrEqual(7);
  const messages = await page.evaluate(
    () => (window as unknown as ArtifactWindow).__rustyWebviewMessages,
  );
  expect(messages.map((message) => message.kind)).toEqual([
    'ready',
    'operationSucceeded',
    'operationSucceeded',
    'operationSucceeded',
    'operationSucceeded',
    'operationSucceeded',
    'operationSucceeded',
  ]);
  expect(messages.slice(1).map((message) => message.operation)).toEqual([
    'readInput',
    'renderOnce',
    'resize',
    'start',
    'stop',
    'dispose',
  ]);
  expect(messages[1]?.value).toMatchObject({
    pressedCodes: ['KeyW'],
    pointer: { xPixels: 24, yPixels: 32 },
  });
});

test('late mount failure cleans partial owners and permanently rejects operations', async ({ page }) => {
  await page.goto('/browser/webview-artifact.html?mountFailure=audio');
  await expect.poll(
    () => page.evaluate(() => (window as unknown as ArtifactWindow)
      .__rustyWebviewMessages.some((message) => message.kind === 'mountFailed')),
  ).toBe(true);

  await page.evaluate(() => {
    (window as unknown as ArtifactWindow).__rustyEnginePrivateRenderer.readState(91);
  });
  await expect.poll(
    () => page.evaluate(() => (window as unknown as ArtifactWindow)
      .__rustyWebviewMessages.some((message) => message.requestId === 91)),
  ).toBe(true);

  const evidence = await page.evaluate(() => ({
    listeners: (window as unknown as ArtifactWindow).__rustyTrackedListenerCounts(),
    messages: (window as unknown as ArtifactWindow).__rustyWebviewMessages,
  }));
  expect(evidence.listeners).toEqual({
    blur: 0,
    keydown: 0,
    keyup: 0,
    pointerdown: 0,
    pointermove: 0,
    pointerup: 0,
    wheel: 0,
  });
  expect(evidence.messages).toHaveLength(2);
  expect(evidence.messages[0]).toMatchObject({
    kind: 'mountFailed',
    message: 'forced late audio host failure',
  });
  expect(evidence.messages[1]).toMatchObject({
    kind: 'operationFailed',
    operation: 'readState',
    requestId: 91,
  });
  expect(evidence.messages[1]?.message).toContain('mount failed: forced late audio host failure');
});
