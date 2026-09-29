// Attach after the first clip ended unwatched, and watch the opening finish.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire(new URL('../../../render/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const OUT = process.env.OUT ?? '.';
const browser = await chromium.launch({ headless: true });
const page = await (await browser.newContext({ viewport: { width: 1280, height: 720 } })).newPage();
const observe = async () => JSON.parse(await page.evaluate(async () => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: 'playtest.observe',
})).text()));
const started = Date.now();
await page.goto('http://127.0.0.1:4394/');
const modes = [];
let last = null;
while (Date.now() - started < 300_000) {
  const mode = (await observe()).mode;
  if (mode !== last) { modes.push({ mode, secondsSinceAttach: (Date.now() - started) / 1000 }); last = mode; }
  if (mode === 'Playing') break;
  await page.waitForTimeout(1_000);
}
await page.screenshot({ path: `${OUT}/finished.png` });
writeFileSync(`${OUT}/finish.json`, JSON.stringify({ modes }, null, 2));
console.log(JSON.stringify(modes));
await browser.close();
