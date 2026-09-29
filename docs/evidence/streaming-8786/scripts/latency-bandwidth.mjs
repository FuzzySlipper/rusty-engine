// Measure what a viewer receives and how long input takes to show, for the
// Three path and the streaming path alike.
//
// Usage: node latency-bandwidth.mjs <origin> <label> [width height] [trials]
//
// Bandwidth: every byte the page receives over a quiet 5 s window (CDP
// Network.dataReceived), split by URL: SSE outputs, frame stream, other.
// Latency: a keydown on the focused canvas ("l" turns right in Doom) is
// timestamped in the page; a requestAnimationFrame sampler copies the Engine
// canvas into a small 2D canvas each frame and records the first frame whose
// pixels differ from the frame before the key. The turn is undone with "j"
// between trials.
import { createRequire } from 'node:module';
const require = createRequire(new URL('../../../../render/node_modules/.pnpm/playwright-core@1.61.1/node_modules/playwright-core/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const [origin, label, width = '1280', height = '720', trials = '15'] = process.argv.slice(2);
// The crew-services GPU flags: ANGLE on Vulkan (RADV) for WebGL.
const browser = await chromium.launch({
  args: ['--enable-gpu', '--use-angle=vulkan', '--disable-vulkan-surface', '--enable-unsafe-webgpu', '--ignore-gpu-blocklist', '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan'],
});
const page = await browser.newPage({ viewport: { width: Number(width), height: Number(height) } });
const cdp = await page.context().newCDPSession(page);
await cdp.send('Network.enable');
const urls = new Map();
let counting = false;
const received = { sse: 0, frames: 0, other: 0 };
cdp.on('Network.requestWillBeSent', (event) => urls.set(event.requestId, event.request.url));
cdp.on('Network.dataReceived', (event) => {
  if (!counting) return;
  const url = urls.get(event.requestId) ?? '';
  const bucket = url.includes('/outputs/fresh') ? 'sse' : url.includes('/runtime/frames') ? 'frames' : 'other';
  received[bucket] += event.encodedDataLength || event.dataLength;
});
await page.goto(origin, { waitUntil: 'domcontentloaded' });
await page.waitForFunction(() => document.querySelector('#application')?.dataset.rustyApplicationState === 'ready', null, { timeout: 60000 });
await page.waitForTimeout(4000);
const gl = await page.evaluate(() => {
  const probe = document.createElement('canvas').getContext('webgl2');
  const info = probe?.getExtension('WEBGL_debug_renderer_info');
  return info ? probe.getParameter(info.UNMASKED_RENDERER_WEBGL) : null;
});
counting = true;
await page.waitForTimeout(5000);
counting = false;
const bandwidth = Object.fromEntries(Object.entries(received).map(([key, bytes]) => [key, Math.round(bytes / 5)]));

await page.locator('canvas').first().focus();
await page.evaluate(() => {
  const canvas = document.querySelector('canvas');
  const sampleCanvas = document.createElement('canvas');
  sampleCanvas.width = 96; sampleCanvas.height = 54;
  const context = sampleCanvas.getContext('2d', { willReadFrequently: true });
  const sample = () => { context.drawImage(canvas, 0, 0, 96, 54); return context.getImageData(0, 0, 96, 54).data; };
  const state = { previous: sample(), keyAt: null, changedAt: null, blank: 0 };
  document.addEventListener('keydown', (event) => { if (event.key === 'l' && state.keyAt === null) state.keyAt = performance.now(); }, true);
  const tick = () => {
    const current = sample();
    let difference = 0, lit = 0;
    for (let i = 0; i < current.length; i += 4) {
      difference += Math.abs(current[i] - state.previous[i]) + Math.abs(current[i + 1] - state.previous[i + 1]);
      lit += current[i] + current[i + 1] + current[i + 2];
    }
    if (lit === 0) state.blank += 1;
    if (state.keyAt !== null && state.changedAt === null && difference > 96 * 54 * 4) state.changedAt = performance.now();
    if (state.keyAt === null) state.previous = current;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  window.__latencyProbe = state;
});
const latencies = [];
let blankSamples = 0;
for (let trial = 0; trial < Number(trials); trial += 1) {
  await page.evaluate(() => { const s = window.__latencyProbe; s.keyAt = null; s.changedAt = null; });
  await page.waitForTimeout(300);
  await page.keyboard.down('l');
  await page.waitForTimeout(150);
  await page.keyboard.up('l');
  await page.waitForTimeout(600);
  const { keyAt, changedAt, blank } = await page.evaluate(() => window.__latencyProbe);
  blankSamples = blank;
  if (keyAt !== null && changedAt !== null) latencies.push(changedAt - keyAt);
  await page.keyboard.down('j');
  await page.waitForTimeout(150);
  await page.keyboard.up('j');
  await page.waitForTimeout(400);
}
latencies.sort((a, b) => a - b);
const pick = (q) => latencies.length === 0 ? null : Math.round(latencies[Math.min(latencies.length - 1, Math.floor(q * latencies.length))] * 10) / 10;
console.log(JSON.stringify({
  label, origin, viewport: [Number(width), Number(height)], webglRenderer: gl,
  bytesPerSecond: bandwidth,
  inputToVisibleChangeMs: { trials: Number(trials), measured: latencies.length, median: pick(0.5), p90: pick(0.9), min: pick(0), max: pick(1) },
  blankSamples,
}, null, 1));
await browser.close();
