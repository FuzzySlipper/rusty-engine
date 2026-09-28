// #8765 visible exercise against a real rusty-product-host serving rusty-dagger
// (Engine source override at the tested Engine head).
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
  const ex = { contexts: 0, contextsLost: 0, audioContexts: 0, sourcesStarted: 0, sourcesStopped: 0,
    sourcesEnded: 0, videosCreated: 0, videosRemoved: 0 };
  window.__ex = ex;
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, ...rest) {
    const context = getContext.call(this, type, ...rest);
    if (context && (type === 'webgl2' || type === 'webgl') && !this.__counted) {
      this.__counted = true;
      ex.contexts += 1;
      this.addEventListener('webglcontextlost', () => { ex.contextsLost += 1; });
    }
    return context;
  };
  const Base = window.AudioContext;
  window.AudioContext = class extends Base {
    constructor(...args) { super(...args); ex.audioContexts += 1; }
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
  const create = Document.prototype.createElement;
  Document.prototype.createElement = function (name, ...rest) {
    const element = create.call(this, name, ...rest);
    if (String(name).toLowerCase() === 'video') {
      ex.videosCreated += 1;
      const remove = element.remove.bind(element);
      element.remove = () => { ex.videosRemoved += 1; remove(); };
    }
    return element;
  };
});

const network = { fresh: 0, resumed: 0, resources: 0 };
let binding = null;
page.on('request', (request) => {
  const url = request.url();
  if (url.includes('/runtime/outputs/fresh')) network.fresh += 1;
  else if (url.includes('/runtime/outputs')) network.resumed += 1;
  if (url.includes('/runtime/resource') || url.includes('?content=')) network.resources += 1;
  const body = request.postData();
  if (body === null || !body.includes('"instanceId"')) return;
  try {
    const parsed = JSON.parse(body);
    const runtime = parsed.runtime ?? parsed.batch?.[0]?.runtime;
    if (runtime?.instanceId !== undefined) binding = runtime;
  } catch { /* non-JSON */ }
});
const errors = [];
page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text().slice(0, 200)); });
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));

const debug = (command) => page.evaluate(async (text) => {
  const response = await fetch('/__rusty/product/runtime/debug/execute', {
    method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
  });
  return JSON.parse(await response.text());
}, command);
const world = async () => {
  const observed = await debug('playtest.observe');
  const time = await debug('engine.time');
  return { mode: observed.mode, position: observed.player?.position, controls: observed.controls,
    step: Number(time.simulationStep) };
};
const post = (path, body) => page.evaluate(async ([route, payload]) => {
  const response = await fetch(route, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload),
  });
  const parsed = JSON.parse(await response.text());
  return { accepted: parsed.accepted, code: parsed.code };
}, [path, body]);
const settle = () => page.waitForTimeout(1500);
const media = () => page.evaluate(() => {
  const video = document.querySelector('video');
  if (window.__video === undefined && video !== null) window.__video = video;
  const canvas = document.querySelector('canvas');
  if (window.__canvas === undefined) window.__canvas = canvas;
  return {
    ...window.__ex,
    sameVideo: window.__video !== undefined && window.__video === video,
    videoConnected: window.__video?.isConnected ?? false,
    videoTime: window.__video?.currentTime ?? null,
    videoPaused: window.__video?.paused ?? null,
    sameCanvas: window.__canvas === canvas,
    canvasContextAlive: canvas !== null && !(canvas.getContext('webgl2')?.isContextLost() ?? true),
  };
});
const checkpoints = [];
const snap = async (label) => {
  const entry = { label, ...(await media()), ...(await world()), binding: binding?.controlRevision,
    freshConnections: network.fresh, resumedConnections: network.resumed, resourceFetches: network.resources };
  delete entry.controls;
  checkpoints.push(entry);
  await page.screenshot({ path: `${out}/${String(checkpoints.length).padStart(2, '0')}-${label}.png` });
  return entry;
};
const openControls = async () => {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: 'Control settings' }).click({ force: true });
  await page.waitForTimeout(800);
};
const closeMenus = async () => {
  for (let i = 0; i < 3; i++) {
    const back = page.getByRole('button', { name: 'Return to game' });
    if (await back.isVisible()) { await back.click({ force: true }); break; }
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);
  }
  await page.waitForTimeout(800);
};
const rebindForward = async (code) => {
  await openControls();
  await page.getByRole('button', { name: 'Rebind' }).first().click({ force: true });
  await page.waitForTimeout(400);
  await page.keyboard.press(code);
  await page.waitForTimeout(1200);
  await closeMenus();
};
const walk = async (code) => {
  const before = (await world()).position;
  await page.keyboard.down(code);
  await page.waitForTimeout(1200);
  await page.keyboard.up(code);
  await page.waitForTimeout(400);
  const after = (await world()).position;
  return Math.round(Math.hypot(after.x - before.x, after.z - before.z) * 100) / 100;
};

await page.goto(origin);
await page.waitForTimeout(6000);
await page.getByRole('button', { name: 'Begin' }).click({ force: true });
await page.waitForTimeout(4000);
await snap('intro-playing');

// Phase A: control fences while the retained intro video plays.
const pauseA = await post('/__rusty/product/runtime/lifecycle/pause', { runtime: binding });
await settle();
await snap('intro-paused');
const resumeA = await post('/__rusty/product/runtime/lifecycle/resume', { runtime: binding });
await settle();
await snap('intro-resumed');
const replaceA = await post('/__rusty/product/runtime/control/replace', { runtime: binding });
await settle();
await snap('intro-control-replaced');
await rebindForward('KeyW');
await snap('intro-rebound-forward-w');

// Wait for the intro to finish and gameplay to begin.
let modeNow = (await world()).mode;
for (let i = 0; i < 90 && modeNow === "Title"; i++) {
  await page.waitForTimeout(5000);
  modeNow = (await world()).mode;
}
await closeMenus();
await page.waitForTimeout(3000);
await snap('gameplay');

// Phase B: gameplay input across fences.
const walkW = await walk('KeyW');
await rebindForward('KeyK');
const controlsAfterRebind = (await world()).controls?.['move.forward'];
const walkOldW = await walk('KeyW');
const walkNewK = await walk('KeyK');
await snap('gameplay-rebound-k');
const pauseB = await post('/__rusty/product/runtime/lifecycle/pause', { runtime: binding });
await page.waitForTimeout(800);
const heldA = (await world()).step;
await page.waitForTimeout(1500);
const heldB = (await world()).step;
await snap('gameplay-paused');
const resumeB = await post('/__rusty/product/runtime/lifecycle/resume', { runtime: binding });
await settle();
const walkAfterResume = await walk('KeyK');
await snap('gameplay-resumed');
await rebindForward('KeyW');
await snap('restored-w');

const result = {
  fences: { pauseA, resumeA, replaceA, pauseB, resumeB },
  gameplay: { modeReached: modeNow, walkW, controlsAfterRebind, walkOldW, walkNewK,
    pausedStepsAdvanced: heldB - heldA, walkAfterResume },
  checkpoints, errors: errors.slice(0, 20),
};
writeFileSync(`${out}/result.json`, JSON.stringify(result, null, 2));
await browser.close();
