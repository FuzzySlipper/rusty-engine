// Record the gaps between frames the page draws (streaming mode) and rAF gaps.
// Usage: node draw-gaps.mjs <width> <height>   (product on http://127.0.0.1:4394)
import { createRequire } from 'node:module';
const require = createRequire(new URL('../../../../render/node_modules/.pnpm/playwright-core@1.61.1/node_modules/playwright-core/package.json', import.meta.url));
const { chromium } = require('playwright-core');
const [w, h] = process.argv.slice(2).map(Number);
const browser = await chromium.launch({ args: ['--enable-gpu', '--use-angle=vulkan', '--disable-vulkan-surface', '--ignore-gpu-blocklist', '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan'] });
const page = await browser.newPage({ viewport: { width: w, height: h } });
await page.goto('http://127.0.0.1:4394', { waitUntil: 'domcontentloaded' });
await page.waitForFunction(() => document.querySelector('#application')?.dataset.rustyApplicationState === 'ready', null, { timeout: 60000 });
await page.waitForTimeout(3000);
const result = await page.evaluate(async () => {
  const canvas = document.querySelector('canvas');
  const draws = [];
  const observer = new MutationObserver(() => draws.push([performance.now(), Number(canvas.dataset.rustyFrameSequence), Number(canvas.dataset.rustyFrameStep)]));
  observer.observe(canvas, { attributes: true, attributeFilter: ['data-rusty-frame-sequence'] });
  const rafs = []; let stop = false; const tick = (t) => { rafs.push(t); if (!stop) requestAnimationFrame(tick); }; requestAnimationFrame(tick);
  await new Promise((r) => setTimeout(r, 5000)); stop = true; observer.disconnect();
  const gaps = draws.slice(1).map((d, i) => d[0] - draws[i][0]).sort((a, b) => a - b);
  const rafGaps = rafs.slice(1).map((t, i) => t - rafs[i]).sort((a, b) => a - b);
  const q = (v, p) => Math.round(v[Math.floor(p * (v.length - 1))] * 10) / 10;
  const skipped = draws.length > 1 ? draws.at(-1)[1] - draws[0][1] + 1 - draws.length : null;
  return { draws: draws.length, drawGapMedian: q(gaps, .5), drawGapP90: q(gaps, .9), drawGapMax: q(gaps, 1), skippedSequences: skipped, rafs: rafs.length, rafGapP90: q(rafGaps, .9), rafGapMax: q(rafGaps, 1) };
});
console.log(w, h, JSON.stringify(result));
await browser.close();
