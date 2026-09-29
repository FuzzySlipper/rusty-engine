// #8771 exercise: the churn product on a real rusty-product-host with Chromium.
// Counts the page's /runtime/resource responses, fresh output streams (each
// recovery rebaselines) and the page state sampled every 100 ms.
// Usage: node run-churn.mjs <rusty-product-host> <Product dir> <out.json> <mode> <kind> <seconds>
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire(new URL("../../../../render/package.json", import.meta.url));
const { chromium } = require('@playwright/test');

const [host, product, out, mode, kind, seconds] = process.argv.slice(2);
const origin = 'http://127.0.0.1:40831/';
const child = spawn(host, ['--product', product, '--loader', 'coreclr'], {
  env: { ...process.env, CHURN_MODE: mode, CHURN_KIND: kind }, stdio: ['ignore', 'pipe', 'pipe'], detached: true,
});
let log = '';
child.stdout.on('data', (d) => { log += d; });
child.stderr.on('data', (d) => { log += d; });
const until = async (predicate, ms) => {
  const start = Date.now();
  while (Date.now() - start < ms) { if (await predicate()) return true; await new Promise((r) => setTimeout(r, 100)); }
  return false;
};
await until(async () => { try { return (await fetch(origin)).ok; } catch { return false; } }, 60000);

const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader'] });
const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
const network = { resource200: 0, resource404: 0, resourceOther: 0, fresh: 0 };
page.on('request', (r) => { if (r.url().includes('/runtime/outputs/fresh')) network.fresh += 1; });
page.on('response', (r) => {
  if (!r.url().includes('/runtime/resource')) return;
  if (r.status() === 200) network.resource200 += 1;
  else if (r.status() === 404) network.resource404 += 1;
  else network.resourceOther += 1;
});
const errors = [];
page.on('pageerror', (e) => errors.push(String(e).slice(0, 200)));
const states = { ready: 0, degraded: 0, failed: 0, other: 0 };
const state = () => page.evaluate(() => document.body.dataset.rustyProductHostState ?? null);
const failure = () => page.evaluate(() => document.body.dataset.rustyProductRuntimeFailure ?? null);
await page.goto(origin);
await until(async () => (await state()) === 'ready', 30000);
const firstReadyFresh = network.fresh;
const end = Date.now() + Number(seconds) * 1000;
while (Date.now() < end) {
  const s = await state().catch(() => null);
  states[s in states ? s : 'other'] += 1;
  await page.waitForTimeout(100);
}
const result = {
  mode, kind, seconds: Number(seconds),
  finalState: await state(), finalFailure: await failure(), stateSamples: states, network,
  freshStreamsAfterFirstReady: network.fresh - firstReadyFresh,
  churnCreated: (log.match(/CHURN created/g) ?? []).length,
  churnReleased: (log.match(/CHURN released/g) ?? []).length,
  pageErrors: errors,
};
writeFileSync(out, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));
await browser.close();
process.kill(-child.pid, 'SIGINT');
