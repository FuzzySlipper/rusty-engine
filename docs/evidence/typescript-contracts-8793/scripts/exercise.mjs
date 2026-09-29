// Exercises the browser shell against a running product on a runtime pack
// built from this tree: the typed bootstrap, the array output events, the
// playtest adapter's typed answers, the diagnostics read and the renderer
// metrics widget.
//
// usage: node exercise.mjs <origin> <out-prefix>
// PLAYWRIGHT_CORE (a playwright-core directory) and CHROMIUM (an executable)
// select the browser.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';

const require = createRequire(`${process.env.PLAYWRIGHT_CORE}/package.json`);
const { chromium } = require('playwright-core');
const [origin, out] = process.argv.slice(2);
const browser = await chromium.launch({
  ...(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {}),
  args: ['--no-sandbox'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 300)));
page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text().slice(0, 300)); });
const sse = [];
page.on('response', async (response) => {
  if (response.url().includes('/outputs/fresh')) sse.push(response.status());
});
await page.goto(origin);
await page.waitForTimeout(15000);
// A product may not answer every step; record its error and go on.
const playtest = (request) => page.evaluate((value) => globalThis.__rustyPlaytest(value), request)
  .catch((error) => ({ error: String(error.message ?? error).split('\n')[0] }));
const debug = (command) => page.evaluate(async (text) => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text(), command);
const report = {
  shellFailure: await page.evaluate(() => document.querySelector('#rusty-runtime-shell-failure')?.textContent ?? null),
  title: await page.title(),
  frameSequence: await page.evaluate(() => document.querySelector('canvas[data-rusty-application-renderer="engine-owned"]')?.dataset.rustyFrameSequence ?? null),
  discover: await playtest({ op: 'discover' }).then((answer) => (answer.error ? answer : {
    commands: answer.commands.length, timeModes: answer.timeModes, drawingModes: answer.drawingModes,
  })),
  time: await playtest({ op: 'time', mode: 'manual' }),
  drawing: await playtest({ op: 'drawing', mode: 'on-demand' }),
  camera: await playtest({ op: 'camera' }),
  timeRealtime: await playtest({ op: 'time', mode: 'realtime' }),
  diagnostics: await page.evaluate(async () => {
    const response = await fetch('/__rusty/product/runtime/diagnostics/read', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}',
    });
    const body = await response.json();
    return { status: response.status, keys: Object.keys(body).sort(), telemetry: Object.keys(body.telemetry ?? {}).length };
  }),
  rendererStatus: JSON.parse(await debug('engine.renderer.show')),
};
// The template's counter: a UI intent claim out, its UI projection back.
report.templateCounter = await page.evaluate(async () => {
  const button = document.querySelector('[data-rusty-template-increment]');
  const output = document.querySelector('#rusty-template-counter');
  if (button === null || output === null) return null;
  const before = output.textContent;
  button.click();
  button.click();
  await new Promise((resolve) => setTimeout(resolve, 1500));
  return { before, after: output.textContent };
});
// The runtime pack's live-debug panel bundle, as a product would mount it.
report.widget = await page.evaluate(async () => {
  const panel = await import('/engine/live-debug-panel/index.js');
  const host = document.createElement('div');
  document.body.append(host);
  const widget = panel.mountRendererMetricsWidget(host, { initiallyVisible: true });
  await new Promise((resolve) => setTimeout(resolve, 2000));
  const text = host.textContent;
  widget.dispose();
  return text;
});
report.errors = errors;
report.freshStreams = sse;
await page.screenshot({ path: `${out}.png` });
writeFileSync(`${out}.json`, JSON.stringify(report, null, 1));
console.log(JSON.stringify(report, null, 1));
await browser.close();
