// Opening a menu from pointer-locked gameplay in a real browser: the menu's
// claim is made before the browser reports the lock and focus lost, and must
// still reach the product exactly once, after the held input it releases.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { after, before, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium, type Browser } from '@playwright/test';

const dist = fileURLToPath(new URL('.', import.meta.url));

// A product UI as an ordinary game writes it: Escape in gameplay claims the
// menu and asks for the interface, which lets go of the pointer.
const PAGE = `<!doctype html>
<html><head><style>html,body{margin:0;height:100%}</style></head>
<body><div id="root" style="height:100%"></div>
<script type="module">
import { mountRustyApplication } from '/application-host.js';
window.host = await mountRustyApplication({
  root: document.getElementById('root'),
  output: 'window',
  initialInteractionMode: 'gameplay',
  runtimeInput: { binding: { runtime: { instanceId: '1', generation: '1', controlRevision: '1' }, context: 'gameplay' } },
  mountUi: (root, context) => {
    const text = document.createElement('input');
    text.id = 'text-entry';
    root.append(text);
    document.addEventListener('keydown', (event) => {
      if (event.code !== 'Escape' || context.ui.interactionMode() !== 'gameplay') return;
      context.intents.claim('menu', { kind: 'product-payload', contract: 'example.menu.v1', data: { open: true } });
      context.ui.setInteractionMode('interface');
    });
    return { dispose() {} };
  },
});
window.ready = true;
</script>
</body></html>`;

let server: Server;
let origin: string;
let browser: Browser;

before(async () => {
  server = createServer(async (request, response) => {
    const path = request.url === '/' ? null : request.url!.replace(/^\//, '');
    if (path === null) {
      response.writeHead(200, { 'content-type': 'text/html' }).end(PAGE);
      return;
    }
    try {
      const body = await readFile(`${dist}${path}`);
      response.writeHead(200, { 'content-type': 'text/javascript' }).end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await new Promise((resolve) => server?.close(resolve));
});

type Drained = ReadonlyArray<{ sequence: string; fact?: { kind: string; reason?: string; code?: string; edge?: string }; intent?: string }>;

test('a menu claimed from pointer-locked gameplay reaches the product once, after held input clears', async () => {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  await page.goto(origin);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true);
  await page.mouse.click(320, 180);
  await page.waitForFunction(() => document.pointerLockElement !== null);
  const drain = (): Promise<Drained> => page.evaluate(
    () => (window as unknown as { host: { input: { drain: () => Drained } } }).host.input.drain(),
  );
  await drain();

  await page.keyboard.down('KeyW');
  await page.keyboard.press('Escape');
  // The browser lets go of the lock after the claim, in its own time.
  await page.waitForFunction(() => document.pointerLockElement === null);
  await page.waitForTimeout(50);
  const drained = await drain();

  const claims = drained.filter((entry) => entry.intent === 'menu');
  assert.equal(claims.length, 1, JSON.stringify(drained));
  const claimAt = drained.indexOf(claims[0]!);
  const pressAt = drained.findIndex((entry) => entry.fact?.kind === 'key' && entry.fact.code === 'key-w');
  assert.ok(pressAt >= 0 && pressAt < claimAt, JSON.stringify(drained));
  const clears = drained.slice(claimAt + 1).map((entry) => entry.fact?.reason);
  assert.ok(clears.includes('interaction-mode-loss') && clears.includes('pointer-lock-loss'), JSON.stringify(drained));
  // Nothing but clears follows the claim, and the sequence has no gaps.
  assert.ok(drained.slice(claimAt + 1).every((entry) => entry.fact?.kind === 'clear'));
  const first = BigInt(drained[0]!.sequence);
  assert.deepEqual(drained.map((entry) => BigInt(entry.sequence) - first), drained.map((_, index) => BigInt(index)));
  await page.close();
});


test('Backspace and F1-F12 deliver both edges, consume gameplay shortcuts, and respect text focus and context clearing', async () => {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  try {
    await page.goto(origin);
    await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true);
    await page.mouse.click(320, 180);
    await page.waitForFunction(() => document.pointerLockElement !== null);
    const drain = (): Promise<Drained> => page.evaluate(
      () => (window as unknown as { host: { input: { drain: () => Drained } } }).host.input.drain(),
    );
    await drain();
    // Observe cancellation after Engine ingress handled the same DOM event.
    await page.evaluate(() => {
      const events: Array<{ code: string; consumed: boolean }> = [];
      (window as unknown as { keyEvents: typeof events }).keyEvents = events;
      document.addEventListener('keydown', event => events.push({ code: event.code, consumed: event.defaultPrevented }));
    });
    const keys = ['Backspace', ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`)];
    for (const key of keys) {
      await page.keyboard.down(key);
      await page.keyboard.down(key); // OS repeat is consumed without another press.
      await page.keyboard.up(key);
      const events = (await drain()).filter(entry => entry.fact?.kind === 'key');
      assert.deepEqual(events.map(entry => entry.fact), [
        { kind: 'key', code: key.toLowerCase(), edge: 'pressed' },
        { kind: 'key', code: key.toLowerCase(), edge: 'released' },
      ]);
    }
    const consumed = await page.evaluate(() => (window as unknown as { keyEvents: Array<{ consumed: boolean }> }).keyEvents);
    assert.equal(consumed.length, keys.length * 2);
    assert.ok(consumed.every(event => event.consumed));
    // A held key is cleared when text owns focus. Backspace still edits text.
    await page.keyboard.down('F1');
    await drain();
    const field = page.locator('#text-entry');
    await field.fill('abc');
    await page.waitForFunction(() => document.pointerLockElement === null);
    assert.ok((await drain()).some(entry => entry.fact?.kind === 'clear'));
    await page.keyboard.up('F1');
    await page.keyboard.press('Backspace');
    assert.equal(await field.inputValue(), 'ab');
    // F5 would legitimately reload text-focused browser UI; check its admission
    // with a DOM event while preserving the real text editing check above.
    for (const code of keys) {
      const prevented = await field.evaluate((element, code) => {
        const event = new KeyboardEvent('keydown', { code, bubbles: true, cancelable: true });
        element.dispatchEvent(event);
        element.dispatchEvent(new KeyboardEvent('keyup', { code, bubbles: true }));
        return event.defaultPrevented;
      }, code);
      assert.equal(prevented, false);
    }
    assert.ok((await drain()).every(entry => entry.fact?.kind === 'clear'));
    await page.mouse.click(320, 180);
    await page.waitForFunction(() => document.pointerLockElement !== null);
    await drain();
    await page.keyboard.down('F12');
    await drain();
    await page.evaluate(() => (window as unknown as { host: { input: { setContext: (value: string) => void } } }).host.input.setContext('gameplay.other'));
    assert.ok((await drain()).some(entry => entry.fact?.kind === 'clear'));
    await page.keyboard.up('F12');
    assert.deepEqual(await drain(), []);
    await page.keyboard.press('F12');
    assert.equal((await drain()).filter(entry => entry.fact?.kind === 'key').length, 2);
  } finally {
    await page.close();
  }
});
