// Held-time captures of Doom E1M1 or Dagger, as the viewer sees them, for the
// Three browser path and the streamed wgpu path alike. Run once per path on
// the same build; every step is in held Engine time, so both runs reach the
// same simulation state.
//
// usage: node parity-capture.mjs <origin> <product: doom|dagger> <out-prefix>
// PLAYWRIGHT_CORE (a playwright-core directory) and CHROMIUM (an executable)
// select the browser.
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const require = createRequire(`${process.env.PLAYWRIGHT_CORE}/package.json`);
const { chromium } = require('playwright-core');
const [origin, product, out] = process.argv.slice(2);
const here = new URL('.', import.meta.url).pathname;

const browser = await chromium.launch({
  ...(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {}),
  // The crew-services GPU flags: ANGLE on Vulkan for WebGL.
  args: ['--no-sandbox', '--enable-gpu', '--use-angle=vulkan', '--disable-vulkan-surface', '--ignore-gpu-blocklist',
    '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan', '--autoplay-policy=no-user-gesture-required'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 300)));
const debug = (command) => page.evaluate(async (text) => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text(), command);
const log = [];
const step = async (command) => { const answer = await debug(command); log.push([command, answer.slice(0, 400)]); return answer; };
const shot = async (name) => { await page.waitForTimeout(2500); await page.screenshot({ path: `${out}-${name}.png` }); };

await page.goto(origin);
await page.waitForTimeout(20000);
if (product === 'dagger') {
  // Into `playing` with Dagger's own UI intents, as #8784 and #8787 did.
  execFileSync('python3', [`${here}dagger-ui-intent.py`, origin.replace(/\/$/u, ''), 'begin', 'cinematic-skip', 'cinematic-skip', 'cinematic-skip'], { stdio: 'inherit' });
  await page.waitForTimeout(8000);
}
await step('engine.time.mode manual');
await step('engine.time.advance 17');
await shot('start');
// Turn right for 900 ms of held time (Doom's "l"; Dagger's "ArrowRight"),
// then step once more after the release.
await page.mouse.click(640, 360);
const turn = product === 'doom' ? 'l' : 'ArrowRight';
await page.keyboard.down(turn);
await page.waitForTimeout(300);
await step('engine.time.advance 900');
await page.keyboard.up(turn);
await page.waitForTimeout(300);
await step('engine.time.advance 17');
if (product === 'doom') await step('loading-bay.readout');
await shot('turned');
writeFileSync(`${out}-log.json`, JSON.stringify({ log, errors }, null, 2));
await browser.close();
