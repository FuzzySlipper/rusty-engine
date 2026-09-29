import { chromium } from '@playwright/test';
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const page = await browser.newPage();
const streams = [];
await page.exposeFunction('reportStream', (url) => streams.push(url));
await page.addInitScript(() => {
  const Native = window.EventSource;
  window.EventSource = class extends Native { constructor(url) { super(url); window.reportStream(String(url)); } };
});
page.on('pageerror', (e) => console.log('pageerror', String(e).slice(0, 300)));
await page.goto(process.env.ORIGIN);
await page.waitForTimeout(10000);
console.log(JSON.stringify({
  failure: await page.evaluate(() => document.querySelector('#rusty-runtime-shell-failure')?.textContent ?? null),
  exercise: await page.evaluate(() => document.body.dataset.exercise ?? null),
  mountedText: await page.evaluate(() => document.querySelector('#application')?.textContent?.includes('ui v1') ?? false),
  streams,
}));
await browser.close();
