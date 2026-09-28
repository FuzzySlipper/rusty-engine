// #8765 phase B: gameplay input, remapping and pause/resume on a live dagger world.
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
  const ex = { contexts: 0, contextsLost: 0, sourcesStarted: 0, sourcesStopped: 0, sourcesEnded: 0 };
  window.__ex = ex;
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, ...rest) {
    const context = getContext.call(this, type, ...rest);
    if (context && (type === 'webgl2' || type === 'webgl') && !this.__counted) {
      this.__counted = true; ex.contexts += 1;
      this.addEventListener('webglcontextlost', () => { ex.contextsLost += 1; });
    }
    return context;
  };
  const Base = window.AudioContext;
  window.AudioContext = class extends Base {
    createBufferSource() {
      const source = super.createBufferSource();
      const start = source.start.bind(source);
      const stop = source.stop.bind(source);
      source.start = (...args) => { ex.sourcesStarted += 1; return start(...args); };
      source.stop = (...args) => { ex.sourcesStopped += 1; return stop(...args); };
      source.addEventListener('ended', () => { ex.sourcesEnded += 1; });
      return source;
    }
  };
});
const network = { fresh: 0, resources: 0, keyFacts: 0, clears: [] };
let binding = null;
page.on('request', (request) => {
  const url = request.url();
  if (url.includes('/runtime/outputs/fresh')) network.fresh += 1;
  if (url.includes('/runtime/resource') || url.includes('?content=')) network.resources += 1;
  const body = request.postData();
  if (!url.includes('/runtime/input') || body === null) return;
  const batch = JSON.parse(body).batch ?? [];
  if (batch[0]?.runtime) binding = batch[0].runtime;
  for (const fact of batch) {
    if (fact.fact?.kind === 'key') network.keyFacts += 1;
    if (fact.fact?.kind === 'clear') network.clears.push(`${fact.runtime.controlRevision}:${fact.fact.reason}`);
  }
});
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));
const debug = (command) => page.evaluate(async (text) => JSON.parse(await (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text()), command);
const world = async () => {
  const observed = await debug('playtest.observe');
  const time = await debug('engine.time');
  return { mode: observed.mode, position: observed.player?.position, forward: observed.controls?.['move.forward'],
    step: Number(time.simulationStep) };
};
const lifecycle = async (operation) => {
  const result = await page.evaluate(async ([op, runtime]) => JSON.parse(await (await fetch(`/__rusty/product/runtime/lifecycle/${op}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ runtime }),
  })).text()), [operation, binding]);
  if (result.binding) binding = result.binding;
  return { accepted: result.accepted, code: result.code, controlRevision: result.binding?.controlRevision };
};
const state = async (label) => {
  const entry = { label, ...(await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (window.__canvas === undefined) window.__canvas = canvas;
    return { ...window.__ex, sameCanvas: window.__canvas === canvas,
      contextAlive: !(canvas?.getContext('webgl2')?.isContextLost() ?? true) };
  })), ...(await world()), controlRevision: binding?.controlRevision, fresh: network.fresh,
    resourceFetches: network.resources };
  await page.screenshot({ path: `${out}/${label}.png` });
  return entry;
};
const focusGame = async () => { await page.mouse.click(640, 420); await page.waitForTimeout(600); };
const walk = async (code) => {
  await focusGame();
  const before = (await world()).position;
  await page.keyboard.down(code);
  await page.waitForTimeout(1200);
  await page.keyboard.up(code);
  await page.waitForTimeout(500);
  const after = (await world()).position;
  return Math.round(Math.hypot(after.x - before.x, after.z - before.z) * 100) / 100;
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
};

await page.goto(origin);
await page.waitForTimeout(7000);
await focusGame();
const checkpoints = [await state('attached')];
const walkW = await walk('KeyW');
checkpoints.push(await state('walked-w'));
await rebindForward('KeyK');
checkpoints.push(await state('rebound-k'));
const walkOldW = await walk('KeyW');
const walkNewK = await walk('KeyK');
checkpoints.push(await state('walked-after-rebind'));
const pause = await lifecycle('pause');
await page.waitForTimeout(800);
const heldA = (await world()).step;
await page.waitForTimeout(1500);
const heldB = (await world()).step;
checkpoints.push(await state('paused'));
const resume = await lifecycle('resume');
await page.waitForTimeout(1200);
const walkAfterResume = await walk('KeyK');
checkpoints.push(await state('resumed-walked-k'));
await rebindForward('KeyW');
const walkRestored = await walk('KeyW');
checkpoints.push(await state('restored-w'));
const result = { walkW, walkOldW, walkNewK, pause, pausedStepsAdvanced: heldB - heldA, resume, walkAfterResume,
  walkRestored, keyFacts: network.keyFacts, clears: network.clears, checkpoints, errors };
writeFileSync(`${out}/gameplay-result.json`, JSON.stringify(result, null, 2));
await browser.close();
