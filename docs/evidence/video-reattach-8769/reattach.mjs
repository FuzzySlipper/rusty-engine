// #8769: a page that leaves mid-clip and a new page that attaches later.
// Screenshots carry their time since Begin, for matching against the clip.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire(new URL('../../../render/package.json', import.meta.url));
const { chromium } = require('@playwright/test');

const URL = 'http://127.0.0.1:4394/';
const OUT = process.env.OUT ?? '.';
const WATCH_MS = 8_000;
const AWAY_MS = 15_000;
const browser = await chromium.launch({ headless: true });
const viewport = { width: 1280, height: 720 };
const observe = async (page) => JSON.parse(await page.evaluate(async () => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: 'playtest.observe',
})).text()));
const frameSequence = (page) => page.evaluate(() => Number(document.querySelector('canvas[data-rusty-application-renderer="engine-owned"]')?.dataset.rustyFrameSequence ?? 0));
const shots = [];
let begun = 0;
const shot = async (page, label) => {
  const at = (Date.now() - begun) / 1000;
  await page.screenshot({ path: `${OUT}/${label}.png` });
  shots.push({ label, secondsSinceBegin: at });
};

const first = await (await browser.newContext({ viewport })).newPage();
await first.goto(URL);
await first.waitForTimeout(6_000);
console.log('mode at attach', (await observe(first)).mode);
await first.getByRole('button', { name: 'Begin' }).click();
begun = Date.now();
await first.waitForTimeout(WATCH_MS);
await shot(first, 'first-page-before-leaving');
await first.context().close();
const left = (Date.now() - begun) / 1000;

await new Promise((resolve) => setTimeout(resolve, AWAY_MS));
const second = await (await browser.newContext({ viewport })).newPage();
const attachedAt = (Date.now() - begun) / 1000;
await second.goto(URL);
while (await frameSequence(second) === 0) await second.waitForTimeout(50);
await shot(second, 'second-page-first-frame');
await second.waitForTimeout(3_000);
await shot(second, 'second-page-plus-3s');
console.log('mode on reattach', (await observe(second)).mode);
writeFileSync(`${OUT}/reattach.json`, JSON.stringify({ leftAt: left, attachedAt, shots }, null, 2));
await browser.close();
