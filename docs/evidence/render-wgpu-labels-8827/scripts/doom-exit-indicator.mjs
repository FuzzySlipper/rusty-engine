// Turn the shared Doom player with a held key, then screenshot the Three + DOM
// frame and print the DOM billboard boxes.
// usage: node doom-exit-indicator.mjs <origin/> <out.png> [key holdMs]
import { createRequire } from 'node:module';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');

const [url, out, key, holdMs] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--no-sandbox', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-webgl', '--ignore-gpu-blocklist'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.goto(url);
await page.waitForTimeout(12000);
// Focus the canvas (this also fires one shot).
await page.mouse.click(640, 360);
if (key && Number(holdMs) > 0) {
  await page.keyboard.down(key);
  await page.waitForTimeout(Number(holdMs));
  await page.keyboard.up(key);
}
await page.waitForTimeout(3000);
const boxes = await page.evaluate(() => [...document.querySelectorAll('[data-rusty-billboard-layer]')].map((element) => {
  const rect = element.getBoundingClientRect();
  return [element.style.display, Math.round(rect.left), Math.round(rect.top), Math.round(rect.width), Math.round(rect.height), element.textContent];
}));
console.log(JSON.stringify(boxes));
await page.screenshot({ path: out });
await browser.close();
