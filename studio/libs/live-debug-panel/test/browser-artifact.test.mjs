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
      response.end('<main><div id="ready"></div><div id="inert"></div><div id="hanging"></div></main>');
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
    let diagnosticCalls = 0;
    const commands = [];
    let hangingSignal;
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
      async diagnostics() {
        diagnosticCalls += 1;
        return diagnosticCalls === 1
          ? {
            events: [{ sequence: '8', monotonicNanoseconds: '2000000000', severity: 'warning', disposition: 'degraded', source: 'browser-host', code: 'BROWSER_HOST_STATUS', message: 'stopped', fields: [{ key: 'renderer-observation-age-ms', value: '100' }, { key: 'transport', value: 'closed' }] }],
            floorSequence: '8', throughSequence: '8', nextCursor: '8', readMonotonicNanoseconds: '2000000000', lagged: false, warningCount: '1', errorCount: '0', droppedCount: '0',
            telemetry: {
              inFlightOperation: 'advance-realtime', inFlightAgeMs: '4',
              lastProductAdmissionLatencyMs: '6', lastInputAdmissionLatencyMs: '2',
              queuedInputBatches: 3, queuedInputEvents: 4, inputBatchCapacity: 256,
              oldestInputAgeMs: '9', inputOverflowPending: false,
              runtimeProgressRateMillihertz: '60000', runtimeProgressAgeMs: '1',
              connections: 1, subscribers: 1, outputQueueItems: 2, outputQueueCapacity: 256,
              outputQueueFloor: '7', outputBindingActive: true,
            },
          }
          : { events: [], floorSequence: '8', throughSequence: '8', nextCursor: '8', readMonotonicNanoseconds: '3000000000', lagged: false, warningCount: '1', errorCount: '0', droppedCount: '0' };
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
    const hanging = await mountLiveDebugPanel(document.querySelector('#hanging'), {
      enabled: true,
      transport: {
        catalog: transport.catalog,
        execute(_command, signal) {
          hangingSignal = signal;
          return new Promise(() => {});
        },
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    const input = document.querySelector('#ready input');
    input.value = 'inspect';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#ready form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    const hangingInput = document.querySelector('#hanging input');
    hangingInput.value = 'inspect';
    hangingInput.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#hanging form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    const ids = [...document.querySelectorAll('input')].map((element) => element.id);
    const logs = [...document.querySelectorAll('#ready [role="log"]')].map((element) => element.textContent).join('\n');
    const panelText = document.querySelector('#ready .rusty-live-debug-panel')?.textContent ?? '';
    const diagnosticStyles = getComputedStyle(document.querySelector('#ready .rusty-live-debug-panel__diagnostics'));
    const diagnosticSelection = {
      cursor: diagnosticStyles.cursor,
      userSelect: diagnosticStyles.userSelect,
    };
    ready.dispose();
    inert.dispose();
    hanging.dispose();
    return { catalogCalls, commands, hangingAborted: hangingSignal?.aborted, ids, logs, panelText, diagnosticSelection };
  });

  assert.equal(result.catalogCalls, 2, 'the disabled panel must stay inert');
  assert.deepEqual(result.commands, ['inspect']);
  assert.equal(result.hangingAborted, true, 'disposing a panel must abort its in-flight command');
  assert.equal(new Set(result.ids).size, result.ids.length, 'mounted panels must not reuse DOM IDs');
  assert.match(result.logs ?? '', /ran inspect/);
  assert.match(result.logs ?? '', /renderer-observation-age-ms=100/);
  assert.match(result.logs ?? '', /event-age-ms=1000/);
  assert.match(result.panelText ?? '', /Product\/runtime lane/);
  assert.match(result.panelText ?? '', /Runtime progress: 60\.000\/s/);
  assert.deepEqual(result.diagnosticSelection, { cursor: 'text', userSelect: 'text' });
});

test('browser artifact can remount after disposal into the same caller-owned host', async (context) => {
  const bundle = await readFile(new URL('index.js', artifactRoot), 'utf8');
  const server = createServer(async (request, response) => {
    if (request.url === '/fixture.html') {
      response.writeHead(200, { 'content-type': 'text/html' });
      response.end('<main><div id="host"></div></main>');
      return;
    }
    if (request.url === '/index.js') {
      response.writeHead(200, { 'content-type': 'text/javascript' });
      response.end(bundle);
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
    const transport = {
      async catalog() {
        return { available: true, commands: [] };
      },
      async execute() {
        return { succeeded: true, message: 'ok' };
      },
    };
    const host = document.querySelector('#host');
    const first = await mountLiveDebugPanel(host, { enabled: true, transport });
    await new Promise((resolve) => setTimeout(resolve, 20));
    const firstConnected = host.querySelector('.rusty-live-debug-panel')?.textContent?.includes('Connected');
    first.dispose();
    const cleared = host.childElementCount === 0;
    const second = await mountLiveDebugPanel(host, { enabled: true, transport });
    await new Promise((resolve) => setTimeout(resolve, 20));
    const secondConnected = host.querySelector('.rusty-live-debug-panel')?.textContent?.includes('Connected');
    second.dispose();
    return { firstConnected, cleared, secondConnected, finalChildCount: host.childElementCount };
  });

  assert.equal(result.firstConnected, true);
  assert.equal(result.cleared, true, 'disposing a panel must remove its owned host node');
  assert.equal(result.secondConnected, true, 'a caller-owned host must support a later remount');
  assert.equal(result.finalChildCount, 0);
});

test('renderer metrics widget establishes visibility once, refreshes admitted facts, and disposes its polling', async (context) => {
  const bundle = await readFile(new URL('index.js', artifactRoot), 'utf8');
  const server = createServer(async (request, response) => {
    if (request.url === '/fixture.html') {
      response.writeHead(200, { 'content-type': 'text/html' });
      response.end('<main><div id="first"></div><div id="second"></div></main>');
      return;
    }
    if (request.url === '/index.js') {
      response.writeHead(200, { 'content-type': 'text/javascript' });
      response.end(bundle);
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
    const { mountRendererMetricsWidget } = await import('/index.js');
    let visible = false;
    let statusCalls = 0;
    const commands = [];
    const summary = () => JSON.stringify({
      schemaVersion: 1,
      available: true,
      widget: { visible },
      renderer: { class: 'accelerated', name: 'Fixture GPU', vendor: null },
      canvas: { cssWidth: 800, cssHeight: 600, backingWidth: 1600, backingHeight: 1200, effectivePixelRatio: 2 },
      frame: { fps: 60, intervalMs: 16.67, syncSubmissionMs: 2.5 },
      pacing: { timerDurationMs: null, effectiveDurationMs: 3.2, state: 'ready', mode: 'timerQuery' },
      statistics: {
        drawCallCount: { value: 4 }, triangleCount: { value: 12 }, renderHandleCount: { value: 2 },
        geometryResourceCount: { value: 2 }, materialResourceCount: { value: 1 }, textureResourceCount: { value: 3 },
      },
      resources: { definedTextureCount: 3, spriteFallbackCount: 0, materialFallbackCount: 1 },
    });
    const transport = {
      async catalog() { return { available: true, commands: [] }; },
      async execute(command) {
        commands.push(command);
        if (command === 'engine.renderer.show') visible = true;
        if (command === 'engine.renderer.hide') visible = false;
        if (command === 'engine.renderer.toggle') visible = !visible;
        if (command === 'engine.renderer.status') statusCalls += 1;
        return { succeeded: true, message: summary() };
      },
    };
    const firstHost = document.querySelector('#first');
    const secondHost = document.querySelector('#second');
    const first = mountRendererMetricsWidget(firstHost, { initiallyVisible: true, transport });
    const second = mountRendererMetricsWidget(secondHost, { transport });
    await new Promise((resolve) => setTimeout(resolve, 30));
    const firstText = firstHost.textContent;
    const secondVisible = secondHost.querySelector('.rusty-renderer-metrics-widget')?.hidden === false;
    await transport.execute('engine.renderer.hide');
    await new Promise((resolve) => setTimeout(resolve, 800));
    const hiddenAfterConsole = firstHost.querySelector('.rusty-renderer-metrics-widget')?.hidden === true
      && secondHost.querySelector('.rusty-renderer-metrics-widget')?.hidden === true;
    first.dispose();
    second.dispose();
    const callsAtDispose = statusCalls;
    await new Promise((resolve) => setTimeout(resolve, 800));
    return {
      commands,
      firstText,
      secondVisible,
      hiddenAfterConsole,
      callsAtDispose,
      callsAfterDispose: statusCalls,
      firstChildren: firstHost.childElementCount,
      secondChildren: secondHost.childElementCount,
    };
  });

  assert.ok(result.commands.includes('engine.renderer.show'));
  assert.ok(result.commands.filter((command) => command === 'engine.renderer.status').length >= 2);
  assert.match(result.firstText ?? '', /FPS: 60\.0/);
  assert.match(result.firstText ?? '', /GPU timer: unavailable/);
  assert.equal(result.secondVisible, true, 'separate mounts read the one shared runtime visibility state');
  assert.equal(result.hiddenAfterConsole, true, 'a console state update reaches every mounted widget');
  assert.equal(result.callsAfterDispose, result.callsAtDispose, 'disposed widgets must stop polling');
  assert.equal(result.firstChildren, 0);
  assert.equal(result.secondChildren, 0);
});
