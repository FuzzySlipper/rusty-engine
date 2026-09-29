import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test, type Page } from '@playwright/test';
import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

type PanelArtifact = typeof import('@rusty-engine/live-debug-panel');

const ORIGIN = 'http://live-debug-panel.test';
const ARTIFACT_PATH = '/index.js';
const bundleUrl = new URL('../artifacts/live-debug-panel/index.js', import.meta.url);

/** Serves one fixture page and the built panel artifact from a fake origin. */
async function openFixture(page: Page, body: string): Promise<void> {
  const bundle = await readFile(bundleUrl, 'utf8');
  await page.route(`${ORIGIN}/**`, (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/fixture.html') {
      return route.fulfill({ contentType: 'text/html', body: `<main>${body}</main>` });
    }
    if (path === ARTIFACT_PATH) return route.fulfill({ contentType: 'text/javascript', body: bundle });
    return route.fulfill({ status: 404 });
  });
  await page.goto(`${ORIGIN}/fixture.html`);
}

test('browser artifact mounts independently and routes through an injected transport', async ({ page }) => {
  await openFixture(page, '<div id="ready"></div><div id="inert"></div><div id="hanging"></div>');
  const result = await page.evaluate(async (ARTIFACT_PATH) => {
    const { mountLiveDebugPanel } = await import(ARTIFACT_PATH) as PanelArtifact;
    const q = <T extends Element>(selector: string): T => {
      const element = document.querySelector<T>(selector);
      if (element === null) throw new Error(`missing ${selector}`);
      return element;
    };
    Object.defineProperty(globalThis.navigator, 'clipboard', { configurable: true, value: undefined });
    let copiedText: string | null = null;
    document.addEventListener('copy', (event) => {
      const active = document.activeElement;
      copiedText = active instanceof HTMLTextAreaElement ? active.value : null;
      event.preventDefault();
    });
    let catalogCalls = 0;
    let diagnosticCalls = 0;
    const commands: string[] = [];
    let hangingSignal: AbortSignal | undefined;
    const transport: LiveDebugTransport = {
      async catalog() {
        catalogCalls += 1;
        return {
          available: true,
          commands: [{ name: 'inspect', description: 'Shows the current fact.', parameters: [] }],
        };
      },
      async execute(command: string) {
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
              runtimeProgressUnavailableReason: null,
              connections: 1, subscribers: 1, outputQueueItems: 2, outputQueueCapacity: 256,
              outputBindingActive: true, updateAttribution: null,
            },
          }
          : { events: [], floorSequence: '8', throughSequence: '8', nextCursor: '8', readMonotonicNanoseconds: '3000000000', lagged: false, warningCount: '1', errorCount: '0', droppedCount: '0' };
      },
    };
    const ready = await mountLiveDebugPanel(q<HTMLElement>('#ready'), {
      enabled: true,
      presentation: 'dock',
      transport,
    });
    const inert = await mountLiveDebugPanel(q<HTMLElement>('#inert'), {
      enabled: false,
      transport,
    });
    const hanging = await mountLiveDebugPanel(q<HTMLElement>('#hanging'), {
      enabled: true,
      transport: {
        catalog: transport.catalog,
        execute(_command: string, signal?: AbortSignal) {
          hangingSignal = signal;
          return new Promise(() => {});
        },
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    const input = q<HTMLInputElement>('#ready input');
    input.value = 'inspect';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    q('#ready form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    const hangingInput = q<HTMLInputElement>('#hanging input');
    hangingInput.value = 'inspect';
    hangingInput.dispatchEvent(new Event('input', { bubbles: true }));
    q('#hanging form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    const copyButton = [...document.querySelectorAll<HTMLButtonElement>('#ready button')]
      .find((button) => button.textContent?.trim() === 'Copy');
    copyButton?.click();
    await new Promise((resolve) => setTimeout(resolve, 20));
    const ids = [...document.querySelectorAll('input')].map((element) => element.id);
    const logs = [...document.querySelectorAll('#ready [role="log"]')].map((element) => element.textContent).join('\n');
    const panelText = document.querySelector('#ready .rusty-live-debug-panel')?.textContent ?? '';
    const diagnosticStyles = getComputedStyle(q('#ready .rusty-live-debug-panel__diagnostics'));
    const diagnosticSelection = {
      cursor: diagnosticStyles.cursor,
      userSelect: diagnosticStyles.userSelect,
    };
    const transcriptStyles = getComputedStyle(q('#ready .rusty-live-debug-panel__transcript'));
    const transcriptSelection = {
      cursor: transcriptStyles.cursor,
      userSelect: transcriptStyles.userSelect,
    };
    const panelOwnsUiInput = document.querySelector('#ready .rusty-live-debug-panel')
      ?.hasAttribute('data-rusty-ui-interactive') ?? false;
    const fallbackTextareaCount = document.querySelectorAll('textarea[aria-hidden="true"]').length;
    ready.dispose();
    inert.dispose();
    hanging.dispose();
    return {
      catalogCalls, commands, copiedText, fallbackTextareaCount, hangingAborted: hangingSignal?.aborted,
      ids, logs, panelText, panelOwnsUiInput, diagnosticSelection, transcriptSelection,
    };
  }, ARTIFACT_PATH);

  assert.equal(result.catalogCalls, 2, 'the disabled panel must stay inert');
  assert.deepEqual(result.commands, ['inspect']);
  assert.equal(result.hangingAborted, true, 'disposing a panel must abort its in-flight command');
  assert.equal(new Set(result.ids).size, result.ids.length, 'mounted panels must not reuse DOM IDs');
  assert.match(result.logs ?? '', /ran inspect/);
  assert.match(result.logs ?? '', /renderer-observation-age-ms=100/);
  assert.match(result.logs ?? '', /event-age-ms=1000/);
  assert.match(result.panelText ?? '', /Product\/runtime lane/);
  assert.match(result.panelText ?? '', /Runtime progress: 60\.000\/s/);
  assert.match(result.copiedText ?? '', /> inspect\nran inspect/u);
  assert.equal(result.fallbackTextareaCount, 0, 'the LAN clipboard fallback must remove its temporary textarea');
  assert.equal(result.panelOwnsUiInput, true, 'the whole panel must stay outside gameplay input admission');
  assert.deepEqual(result.diagnosticSelection, { cursor: 'text', userSelect: 'text' });
  assert.deepEqual(result.transcriptSelection, { cursor: 'text', userSelect: 'text' });
});

test('browser artifact can remount after disposal into the same caller-owned host', async ({ page }) => {
  await openFixture(page, '<div id="host"></div>');
  const result = await page.evaluate(async (ARTIFACT_PATH) => {
    const { mountLiveDebugPanel } = await import(ARTIFACT_PATH) as PanelArtifact;
    const q = <T extends Element>(selector: string): T => {
      const element = document.querySelector<T>(selector);
      if (element === null) throw new Error(`missing ${selector}`);
      return element;
    };
    const transport: LiveDebugTransport = {
      async catalog() {
        return { available: true, commands: [] };
      },
      async execute() {
        return { succeeded: true, message: 'ok' };
      },
    };
    const host = q<HTMLElement>('#host');
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
  }, ARTIFACT_PATH);

  assert.equal(result.firstConnected, true);
  assert.equal(result.cleared, true, 'disposing a panel must remove its owned host node');
  assert.equal(result.secondConnected, true, 'a caller-owned host must support a later remount');
  assert.equal(result.finalChildCount, 0);
});

test('renderer metrics widget establishes visibility once, refreshes admitted facts, and disposes its polling', async ({ page }) => {
  await openFixture(page, '<div id="first"></div><div id="second"></div>');
  const result = await page.evaluate(async (ARTIFACT_PATH) => {
    const { mountRendererMetricsWidget } = await import(ARTIFACT_PATH) as PanelArtifact;
    const q = <T extends Element>(selector: string): T => {
      const element = document.querySelector<T>(selector);
      if (element === null) throw new Error(`missing ${selector}`);
      return element;
    };
    let visible = false;
    let statusCalls = 0;
    const commands: string[] = [];
    const summary = () => JSON.stringify({
      schemaVersion: 1,
      available: true,
      widget: { visible },
      renderer: { class: 'accelerated', name: 'Fixture GPU', vendor: null },
      canvas: { cssWidth: 800, cssHeight: 600, backingWidth: 1600, backingHeight: 1200, effectivePixelRatio: 2 },
      frame: { submissionRateHz: 60, intervalMs: 16.67, syncSubmissionMs: 2.5 },
      pacing: { timerDurationMs: null, effectiveDurationMs: 3.2, state: 'ready', mode: 'timerQuery' },
      statistics: {
        drawCallCount: { value: 4 }, triangleCount: { value: 12 }, renderHandleCount: { value: 2 },
        geometryResourceCount: { value: 2 }, materialResourceCount: { value: 1 }, textureResourceCount: { value: 3 },
      },
      resources: { definedTextureCount: 3, spriteFallbackCount: 0, materialFallbackCount: 1 },
    });
    const transport: LiveDebugTransport = {
      async catalog() { return { available: true, commands: [] }; },
      async execute(command: string) {
        commands.push(command);
        if (command === 'engine.renderer.show') visible = true;
        if (command === 'engine.renderer.hide') visible = false;
        if (command === 'engine.renderer.toggle') visible = !visible;
        if (command === 'engine.renderer.status') statusCalls += 1;
        return { succeeded: true, message: summary() };
      },
    };
    const firstHost = q<HTMLElement>('#first');
    const secondHost = q<HTMLElement>('#second');
    const first = mountRendererMetricsWidget(firstHost, { initiallyVisible: true, transport });
    const second = mountRendererMetricsWidget(secondHost, { transport });
    await new Promise((resolve) => setTimeout(resolve, 30));
    const firstText = firstHost.textContent;
    const secondVisible = secondHost.querySelector<HTMLElement>('.rusty-renderer-metrics-widget')?.hidden === false;
    await transport.execute('engine.renderer.hide');
    await new Promise((resolve) => setTimeout(resolve, 800));
    const hiddenAfterConsole = firstHost.querySelector<HTMLElement>('.rusty-renderer-metrics-widget')?.hidden === true
      && secondHost.querySelector<HTMLElement>('.rusty-renderer-metrics-widget')?.hidden === true;
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
  }, ARTIFACT_PATH);

  assert.ok(result.commands.includes('engine.renderer.show'));
  assert.ok(result.commands.filter((command) => command === 'engine.renderer.status').length >= 2);
  assert.match(result.firstText ?? '', /Submission rate: 60\.0 Hz/);
  assert.match(result.firstText ?? '', /GPU timer: unavailable/);
  assert.equal(result.secondVisible, true, 'separate mounts read the one shared runtime visibility state');
  assert.equal(result.hiddenAfterConsole, true, 'a console state update reaches every mounted widget');
  assert.equal(result.callsAfterDispose, result.callsAtDispose, 'disposed widgets must stop polling');
  assert.equal(result.firstChildren, 0);
  assert.equal(result.secondChildren, 0);
});
