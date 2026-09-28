// #8766 visible exercise: rusty-dagger under `rusty dev` with the runtime
// serving the browser directly. Covers reload, a source-restage runtime
// replacement with the page left open, and input before and after.
import { createRequire } from 'node:module';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');

const [origin, out, touchFile] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--autoplay-policy=no-user-gesture-required', '--use-gl=angle', '--use-angle=swiftshader'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.addInitScript(() => {
  const seen = { messages: 0, bindings: [], lastBinding: null };
  window.__sse = seen;
  const Base = window.EventSource;
  window.EventSource = class extends Base {
    constructor(...args) {
      super(...args);
      this.addEventListener('rusty-output-baseline', (event) => {
        try {
          const r = JSON.parse(event.data).binding;
          if (r) {
            seen.lastBinding = `${r.instanceId}/${r.generation}/${r.controlRevision}`;
            seen.bindings.push(seen.lastBinding);
          }
        } catch { /* ignore */ }
      });
      this.addEventListener('message', (event) => {
        seen.messages += 1;
        try {
          for (const output of JSON.parse(event.data).outputs ?? []) {
            if (output.kind === 'binding') {
              const r = output.runtime;
              seen.lastBinding = `${r.instanceId}/${r.generation}/${r.controlRevision}`;
              seen.bindings.push(seen.lastBinding);
            }
          }
        } catch { /* fragments */ }
      });
    }
  };
});
const network = { fresh: 0, resumed: 0, inputPosts: 0, status503: 0 };
page.on('request', (request) => {
  const url = request.url();
  if (url.includes('/runtime/outputs/fresh')) network.fresh += 1;
  else if (url.includes('/runtime/outputs')) network.resumed += 1;
  if (url.includes('/runtime/input')) network.inputPosts += 1;
});
const timeline = [];
page.on('response', (response) => {
  if (response.status() === 503) network.status503 += 1;
  if (response.url().includes('/runtime/outputs/fresh')) timeline.push([Date.now(), response.status()]);
});
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));

const debug = (command) => page.evaluate(async (text) => JSON.parse(await (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text()), command);
const snapshot = async (label) => {
  const view = await page.evaluate(() => ({
    hostState: document.body.dataset.rustyProductHostState ?? null,
    binding: window.__sse.lastBinding,
    bindings: window.__sse.bindings.length,
    contextAlive: !(document.querySelector('canvas')?.getContext('webgl2')?.isContextLost() ?? true),
  }));
  let world = null;
  try {
    const observed = await debug('playtest.observe');
    world = { mode: observed.mode, forward: observed.controls?.['move.forward'],
      step: Number((await debug('engine.time')).simulationStep) };
  } catch (error) { world = { error: String(error).slice(0, 120) }; }
  const entry = { label, at: Date.now(), ...view, ...world, network: { ...network } };
  result.checkpoints.push(entry);
  await page.screenshot({ path: `${out}/${String(result.checkpoints.length).padStart(2, '0')}-${label}.png` });
  return entry;
};
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

const result = { checkpoints: [] };
await page.goto(origin);
await page.waitForTimeout(6000);
await snapshot('attached');
result.rebindBefore = await rebindForward('KeyK');
await snapshot('rebound-k');

// Reload: a fresh attachment to the same runtime.
await page.reload();
await page.waitForTimeout(6000);
await snapshot('reloaded');

// Source restage: rusty dev replaces the runtime; the page stays open.
const beforeBinding = await page.evaluate(() => window.__sse.lastBinding);
const edited = Date.now();
appendFileSync(touchFile, `\n// #8766 restage ${edited}\n`);
let replacedAt = null;
for (let i = 0; i < 240; i++) {
  await page.waitForTimeout(500);
  const binding = await page.evaluate(() => window.__sse.lastBinding);
  const state = await page.evaluate(() => document.body.dataset.rustyProductHostState ?? null);
  if (binding !== beforeBinding && binding !== null && state === 'ready') { replacedAt = Date.now(); break; }
}
result.replacement = { beforeBinding, afterBinding: await page.evaluate(() => window.__sse.lastBinding),
  secondsFromEdit: replacedAt === null ? null : (replacedAt - edited) / 1000,
  freshResponsesAfterEdit: timeline.filter(([at]) => at >= edited).map(([at, status]) => [(at - edited) / 1000, status]) };
await page.waitForTimeout(3000);
await snapshot('replaced');
result.rebindAfter = await rebindForward('KeyW');
await snapshot('restored-w');
result.errors = errors;
writeFileSync(`${out}/result.json`, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
