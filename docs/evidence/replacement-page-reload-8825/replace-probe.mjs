// Drives one page through a C#-only edit and a combined C#+UI edit under
// `rusty dev`, polling what the page shows and which product instance runs.
import { chromium } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';

const origin = process.env.ORIGIN;
const product = process.env.PRODUCT;
const csPath = `${product}/Product.cs`;
const uiPath = `${product}/ui/main.ts`;
const identity = async () => {
  try {
    const response = await fetch(`${origin}/__rusty/product/runtime/debug/execute`, {
      method: 'POST', headers: { 'content-type': 'text/plain; charset=utf-8' }, body: 'exercise.identity',
    });
    return response.ok ? (await response.text()).split('instance=')[1]?.slice(0, 8) : `http-${response.status}`;
  } catch { return 'down'; }
};
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const page = await browser.newPage();
let navigations = 0;
page.on('framenavigated', (frame) => { if (frame === page.mainFrame()) navigations += 1; });
await page.goto(origin);
const sample = async () => ({
  navigations,
  state: await page.evaluate(() => document.querySelector('#application')?.dataset.rustyProductHostState ?? null).catch(() => 'navigating'),
  ui: await page.evaluate(() => document.body.dataset.exercise ?? null).catch(() => 'navigating'),
  instance: await identity(),
});
const settle = async (label, done) => {
  const start = Date.now();
  let last = '';
  for (;;) {
    const now = await sample();
    const line = JSON.stringify(now);
    if (line !== last) { console.log(`${label} +${String(Date.now() - start).padStart(6)}ms ${line}`); last = line; }
    if (done(now) || Date.now() - start > 90_000) return now;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
};
const initial = await settle('start', (s) => s.state === 'ready' && s.instance?.length === 8);
const edit = (path, from, to) => writeFileSync(path, readFileSync(path, 'utf8').replace(from, to));

edit(csPath, 'to show no restart.', 'to show no restart (edit 1).');
const afterCs = await settle('cs-only', (s) => s.instance !== initial.instance && s.instance?.length === 8 && s.state === 'ready');

edit(csPath, '(edit 1)', '(edit 2)');
edit(uiPath, '"ui v1"', '"ui v2"');
await settle('cs+ui', (s) => s.instance !== afterCs.instance && s.instance?.length === 8 && s.state === 'ready' && s.ui === 'ui v2');
await browser.close();
