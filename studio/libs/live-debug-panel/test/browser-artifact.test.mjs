import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { chromium } from '@playwright/test';

const artifactRoot = new URL('../../../artifacts/live-debug-panel/', import.meta.url);

test('browser artifact mounts independently and routes through an injected transport', async (context) => {
  const bundle = await readFile(new URL('index.js', artifactRoot), 'utf8');
  assert.doesNotMatch(bundle, /^\s*import\s/m, 'the copied artifact must not need a bare-import resolver');
  assert.doesNotMatch(bundle, /node_modules/u, 'the emitted artifact must not encode workspace paths');

  const server = createServer(async (request, response) => {
    if (request.url === '/fixture.html') {
      response.writeHead(200, { 'content-type': 'text/html' });
      response.end('<main><div id="ready"></div><div id="inert"></div></main>');
      return;
    }
    if (request.url === '/index.js') {
      response.writeHead(200, { 'content-type': 'text/javascript' });
      response.end(await readFile(new URL('index.js', artifactRoot)));
      return;
    }
    response.writeHead(404).end();
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  context.after(() => server.close());
  const address = server.address();
  assert.ok(address && typeof address !== 'string');

  const browser = await chromium.launch({ headless: true });
  context.after(() => browser.close());
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${address.port}/fixture.html`);

  const result = await page.evaluate(async () => {
    const { mountLiveDebugPanel } = await import('/index.js');
    let catalogCalls = 0;
    const commands = [];
    const transport = {
      async catalog() {
        catalogCalls += 1;
        return {
          available: true,
          commands: [{ name: 'inspect', description: 'Shows the current fact.', parameters: [] }],
        };
      },
      async execute(command) {
        commands.push(command);
        return { succeeded: true, message: `ran ${command}` };
      },
    };
    const ready = await mountLiveDebugPanel(document.querySelector('#ready'), {
      enabled: true,
      presentation: 'dock',
      transport,
    });
    const inert = await mountLiveDebugPanel(document.querySelector('#inert'), {
      enabled: false,
      transport,
    });
    await new Promise((resolve) => setTimeout(resolve, 20));
    const input = document.querySelector('#ready input');
    input.value = 'inspect';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#ready form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    const ids = [...document.querySelectorAll('input')].map((element) => element.id);
    const transcript = document.querySelector('#ready [role="log"]')?.textContent;
    ready.dispose();
    inert.dispose();
    return { catalogCalls, commands, ids, transcript };
  });

  assert.equal(result.catalogCalls, 1, 'the disabled panel must stay inert');
  assert.deepEqual(result.commands, ['inspect']);
  assert.equal(new Set(result.ids).size, result.ids.length, 'mounted panels must not reuse DOM IDs');
  assert.match(result.transcript ?? '', /ran inspect/);
});
