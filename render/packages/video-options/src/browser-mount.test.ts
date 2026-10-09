// The mounted panel in a real browser: keyboard focus survives updates, and
// the look's custom properties set on an ancestor take effect.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { after, before, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium, type Browser, type Page } from '@playwright/test';

const dist = fileURLToPath(new URL('.', import.meta.url));

// The page mounts the panel on a fake transport that answers a change with
// the catalogue that change makes, and records what it was sent.
const PAGE = `<!doctype html>
<html><body>
<div id="holder"></div>
<script type="module">
import { mountVideoOptions } from '/browser-mount.js';
const option = (id, group, kind, value, extra) => ({
  id, label: id, group, description: '', kind, value, requested: value, gameDefault: value,
  chosen: false, restart: false, cost: '', refused: null, ...extra,
});
let options = [
  option('strength', 'Lighting', 'range', 1, { min: 0, max: 2, step: 0.1, unit: '' }),
  option('shadows', 'Lighting', 'toggle', true),
];
const catalogue = () => ({ version: 1, stored: true, presets: [{ id: 'low', label: 'Low' }], options });
window.changes = [];
window.transport = {
  read: async () => catalogue(),
  change: async (change) => {
    window.changes.push(change);
    if (change.choose) {
      options = options.map((o) => o.id === change.choose.id
        ? { ...o, value: change.choose.value, requested: change.choose.value, chosen: true } : o);
    }
    return catalogue();
  },
  // An Engine change made elsewhere, as another page or the game would.
  setElsewhere: (id, value) => {
    options = options.map((o) => o.id === id ? { ...o, value, requested: value, chosen: true } : o);
  },
};
// The game's own option, applied and published back as a game does.
let fov = 90;
const productOptions = () => [{
  id: 'fov', label: 'Field of view', group: 'Game', description: '', kind: 'range',
  min: 60, max: 110, step: 1, value: fov,
  onChange: (value) => { fov = value; window.panel.setProductOptions(productOptions()); },
}];
window.panel = await mountVideoOptions(document.getElementById('holder'), {
  transport: window.transport, productOptions: productOptions(),
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

async function open(style = ''): Promise<Page> {
  const page = await browser.newPage();
  await page.goto(origin);
  if (style) await page.evaluate((css) => { document.getElementById('holder')!.style.cssText = css; }, style);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true);
  return page;
}

const slider = (id: string) => `[data-option="${id}"] input[type=range]`;

/** Marks the element, so a later check can tell it is the same node, still focused. */
async function markFocused(page: Page, selector: string): Promise<void> {
  await page.focus(selector);
  await page.$eval(selector, (element) => { (element as unknown as { mark: boolean }).mark = true; });
}

async function sameFocusedNode(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const active = document.activeElement as (Element & { mark?: boolean }) | null;
    return active?.mark === true && active.isConnected;
  });
}

const settled = (page: Page, text: string) =>
  page.waitForFunction((expected) => document.querySelector('.rusty-video-options__status')?.textContent === expected, text);

test('arrow keys keep stepping an Engine slider across the change it sends', async () => {
  const page = await open();
  await markFocused(page, slider('strength'));
  await page.keyboard.press('ArrowRight');
  await settled(page, 'Saved for this install.');
  assert.ok(await sameFocusedNode(page), 'the slider lost focus or was replaced');
  // The chosen option grew a Game default button beside the same slider.
  assert.equal(await page.locator('[data-option="strength"] .rusty-video-options__reset').count(), 1);
  await page.evaluate(() => { document.querySelector('.rusty-video-options__status')!.textContent = ''; });
  await page.keyboard.press('ArrowRight');
  await settled(page, 'Saved for this install.');
  assert.ok(await sameFocusedNode(page));
  const sent = await page.evaluate(() => (window as unknown as { changes: unknown[] }).changes);
  assert.deepEqual(sent, [
    { choose: { id: 'strength', value: 1.1 } },
    { choose: { id: 'strength', value: 1.2 } },
  ]);
  assert.equal(await page.$eval(slider('strength'), (input) => (input as HTMLInputElement).value), '1.2');
  await page.close();
});

test('arrow keys keep stepping a game slider across setProductOptions', async () => {
  const page = await open();
  await markFocused(page, slider('fov'));
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  assert.ok(await sameFocusedNode(page), 'the slider lost focus or was replaced');
  assert.equal(await page.$eval(slider('fov'), (input) => (input as HTMLInputElement).value), '92');
  assert.equal(await page.$eval('[data-option="fov"] output', (output) => output.textContent), '92');
  await page.close();
});

test('an Engine change from elsewhere updates other rows in place and leaves the focused slider alone', async () => {
  const page = await open();
  await markFocused(page, slider('strength'));
  await page.$eval(slider('fov'), (input) => { (input as unknown as { mark2: boolean }).mark2 = true; });
  await page.evaluate(async () => {
    const w = window as unknown as {
      transport: { setElsewhere(id: string, value: unknown): void };
      panel: { refresh(): Promise<void> };
    };
    w.transport.setElsewhere('strength', 0.4);
    w.transport.setElsewhere('shadows', false);
    await w.panel.refresh();
  });
  assert.ok(await sameFocusedNode(page));
  // The player's slider keeps what they set; the toggle follows the Engine.
  assert.equal(await page.$eval(slider('strength'), (input) => (input as HTMLInputElement).value), '1');
  assert.equal(await page.$eval('[data-option="shadows"] input', (input) => (input as HTMLInputElement).checked), false);
  assert.ok(await page.$eval(slider('fov'), (input) => (input as unknown as { mark2?: boolean }).mark2 === true));
  await page.close();
});

test("the look's custom properties set on an ancestor take effect, with defaults otherwise", async () => {
  const styled = await open(
    '--rusty-video-options-background: rgb(10, 20, 30); --rusty-video-options-font: 13px/1 monospace',
  );
  const look = (page: Page) =>
    page.$eval('.rusty-video-options', (panel) => {
      const style = getComputedStyle(panel);
      return [style.backgroundColor, style.fontFamily, style.fontSize];
    });
  assert.deepEqual(await look(styled), ['rgb(10, 20, 30)', 'monospace', '13px']);
  await styled.close();
  const plain = await open();
  const [background, family] = await look(plain);
  assert.equal(background, 'rgba(14, 18, 26, 0.94)');
  assert.match(family ?? '', /system-ui/);
  await plain.close();
});
