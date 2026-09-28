// #8768 visible exercise: rusty-dagger with no per-tick readout/progress events.
// Checks realtime cadence (UI input reaches the product), playtest time
// inspection (manual mode, advance, back to realtime) and live debug.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');

const [origin, out] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--autoplay-policy=no-user-gesture-required', '--use-gl=angle', '--use-angle=swiftshader'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.addInitScript(() => {
  // Count SSE output kinds as the browser receives them.
  const counts = { messages: 0, kinds: {}, lastReadout: null };
  window.__sse = counts;
  const Base = window.EventSource;
  window.EventSource = class extends Base {
    constructor(...args) {
      super(...args);
      this.addEventListener('message', (event) => {
        counts.messages += 1;
        try {
          for (const output of JSON.parse(event.data).outputs ?? []) {
            counts.kinds[output.kind] = (counts.kinds[output.kind] ?? 0) + 1;
            if (output.kind === 'runtime-readout') {
              const r = output.readout;
              counts.lastReadout = { state: r.state, inspectionTime: r.inspectionTime ?? null,
                steps: r.admittedSimulationSteps, controlRevision: r.runtime.controlRevision };
            }
          }
        } catch { /* fragments are counted as messages only */ }
      });
    }
  };
});
let inputPosts = 0;
page.on('request', (request) => { if (request.url().includes('/runtime/input')) inputPosts += 1; });
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));

const debug = (command) => page.evaluate(async (text) => JSON.parse(await (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text()), command);
const sse = () => page.evaluate(() => ({ messages: window.__sse.messages, kinds: { ...window.__sse.kinds } }));
// The product shell does not expose the host object, so read the last
// readout the browser received and the host's health dataset.
const hostReadout = () => page.evaluate(() => window.__sse.lastReadout);
const hostState = () => page.evaluate(() => document.body.dataset.rustyProductHostState ?? null);
const playtest = (request) => page.evaluate((r) => globalThis.__rustyPlaytest(r), request);
const diagnostics = () => page.evaluate(async () => JSON.parse(await (await fetch('/__rusty/product/runtime/diagnostics/read', {
  method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}',
})).text()));
const canvasState = () => page.evaluate(() => {
  const canvas = document.querySelector('canvas');
  if (window.__canvas === undefined) window.__canvas = canvas;
  return { sameCanvas: window.__canvas === canvas,
    contextAlive: !(canvas?.getContext('webgl2')?.isContextLost() ?? true),
    videos: document.querySelectorAll('video').length };
});

const result = { checkpoints: [] };
const mark = async (label, extra = {}) => {
  const entry = { label, hostState: await hostState(), readout: await hostReadout(), sse: await sse(),
    inputPosts, ...(await canvasState()), mode: (await debug('playtest.observe')).mode,
    step: Number((await debug('engine.time')).simulationStep), ...extra };
  result.checkpoints.push(entry);
  await page.screenshot({ path: `${out}/${String(result.checkpoints.length).padStart(2, '0')}-${label}.png` });
  return entry;
};

await page.goto(origin);
await page.waitForTimeout(6000);
await mark('attached');
const idleBefore = await sse();
await page.waitForTimeout(5000);
const idleAfter = await sse();
result.idleFiveSeconds = {
  messages: idleAfter.messages - idleBefore.messages,
  readouts: (idleAfter.kinds['runtime-readout'] ?? 0) - (idleBefore.kinds['runtime-readout'] ?? 0),
  progress: (idleAfter.kinds['runtime-progress'] ?? 0) - (idleBefore.kinds['runtime-progress'] ?? 0),
};

// Realtime cadence: a UI click reaches the product through the renderer cadence.
const postsBeforeBegin = inputPosts;
await page.getByRole('button', { name: 'Begin' }).click({ force: true });
await page.waitForTimeout(3000);
await mark('begin-clicked', { beginInputPosts: inputPosts - postsBeforeBegin });

// Playtest time inspection.
const manual = await playtest({ op: 'time', mode: 'manual' });
await page.waitForTimeout(500);
const manualReadout = await hostReadout();
const heldA = Number((await debug('engine.time')).simulationStep);
await page.waitForTimeout(1500);
const heldB = Number((await debug('engine.time')).simulationStep);
const advance = await playtest({ op: 'advance', ms: 500 });
await page.waitForTimeout(500);
const advancedReadout = await hostReadout();
await mark('manual-advanced');
const realtime = await playtest({ op: 'time', mode: 'realtime' });
await page.waitForTimeout(1500);
const realtimeReadout = await hostReadout();
await mark('realtime-again');
result.inspection = {
  manual, manualReadout, heldStepsOver1500ms: heldB - heldA, advance, advancedReadout, realtime, realtimeReadout,
};

// Input cadence: remap move.forward through dagger's Controls UI and back.
const rebindForward = async (code) => {
  const settings = page.getByRole('button', { name: 'Control settings' });
  for (let i = 0; i < 4 && !(await settings.isVisible()); i++) {
    await page.keyboard.press('Escape');
    await page.waitForTimeout(700);
  }
  await settings.click({ force: true });
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: 'Rebind' }).first().click({ force: true });
  await page.waitForTimeout(400);
  await page.keyboard.press(code);
  await page.waitForTimeout(1200);
  const back = page.getByRole('button', { name: 'Return to game' });
  for (let i = 0; i < 3 && !(await back.isVisible()); i++) {
    const menu = page.getByRole('button', { name: 'Back to menu' });
    if (await menu.isVisible()) await menu.click({ force: true }); else await page.keyboard.press('Escape');
    await page.waitForTimeout(500);
  }
  if (await back.isVisible()) await back.click({ force: true });
  await page.waitForTimeout(800);
  return (await debug('playtest.observe')).controls?.['move.forward'];
};
const forwardBefore = (await debug('playtest.observe')).controls?.['move.forward'];
const postsBeforeRebind = inputPosts;
const forwardAfterK = await rebindForward('KeyK');
await mark('rebound-k');
const forwardRestored = await rebindForward('KeyW');
await mark('restored-w');
result.rebind = { forwardBefore, forwardAfterK, forwardRestored, inputPosts: inputPosts - postsBeforeRebind };

// Live debug.
const diag = await diagnostics();
result.liveDebug = {
  engineTime: await debug('engine.time'),
  runtimeProgressRateMillihertz: diag.telemetry?.runtimeProgressRateMillihertz ?? null,
  runtimeProgressAgeMs: diag.telemetry?.runtimeProgressAgeMs ?? null,
  telemetryKeys: Object.keys(diag.telemetry ?? {}),
};
result.errors = errors;
writeFileSync(`${out}/result.json`, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
