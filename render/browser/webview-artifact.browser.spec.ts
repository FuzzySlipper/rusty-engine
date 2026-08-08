import { expect, test } from '@playwright/test';

interface WebviewMessage {
  readonly kind: string;
  readonly operation?: string;
  readonly requestId?: number;
  readonly value?: unknown;
}

interface PrivateRendererApi {
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
