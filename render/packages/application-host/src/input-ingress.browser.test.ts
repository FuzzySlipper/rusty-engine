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
