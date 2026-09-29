// Open the product page headless, wait, screenshot, and print what the page shows.
// Usage: node page-check.mjs <origin> <out.png> [waitMs]
import { createRequire } from 'node:module';
// playwright-core from the render workspace's pnpm store.
const require = createRequire(new URL('../../../../render/node_modules/.pnpm/playwright-core@1.61.1/node_modules/playwright-core/package.json', import.meta.url));
const { chromium } = require('playwright-core');
const [origin, out, waitMs = '10000'] = process.argv.slice(2);
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const messages = [];
page.on('console', (message) => messages.push(`${message.type()}: ${message.text()}`));
page.on('pageerror', (error) => messages.push(`pageerror: ${error.message}`));
await page.goto(origin, { waitUntil: 'domcontentloaded' });
await page.waitForTimeout(Number(waitMs));
await page.screenshot({ path: out });
const facts = await page.evaluate(() => {
  const canvases = [...document.querySelectorAll('canvas')].map((canvas) => ({
    renderer: canvas.dataset.rustyApplicationRenderer,
    backing: [canvas.width, canvas.height],
    css: [canvas.clientWidth, canvas.clientHeight],
    frameSequence: canvas.dataset.rustyFrameSequence,
    frameStep: canvas.dataset.rustyFrameStep,
    held: canvas.dataset.rustyFrameHeld,
  }));
  const root = document.querySelector('#application');
  return { canvases, state: root?.dataset.rustyApplicationState, host: root?.dataset.rustyProductHostState, failure: document.querySelector('#rusty-runtime-shell-failure')?.textContent ?? null };
});
console.log(JSON.stringify({ facts, messages: messages.slice(0, 30) }, null, 1));
await browser.close();
