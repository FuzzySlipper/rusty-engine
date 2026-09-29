// #8770 visible exercise: rusty-dagger under `rusty dev --engine-source`.
// Counts what the browser receives while idle, then changes product state
// (Begin, open Control settings, return) and checks that the HUD projection
// and the DOM follow. Usage:
//   node exercise-dagger.mjs <origin> <out-dir>
import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
const require = createRequire(new URL('../../../../render/package.json', import.meta.url));
const { chromium } = require('@playwright/test');

const [origin, out] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const browser = await chromium.launch({
  args: ['--autoplay-policy=no-user-gesture-required', '--use-gl=angle', '--use-angle=swiftshader'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.addInitScript(() => {
  const seen = { batches: 0, kinds: {}, emptyFrames: 0, ui: {}, uiModes: [] };
  window.__seen = seen;
  const Base = window.EventSource;
  window.EventSource = class extends Base {
    constructor(...args) {
      super(...args);
      const record = (event) => {
        seen.batches += 1;
        try {
          for (const output of JSON.parse(event.data).outputs ?? []) {
            seen.kinds[output.kind] = (seen.kinds[output.kind] ?? 0) + 1;
            if (output.kind === 'frame' && (output.frame?.ops ?? []).length === 0) seen.emptyFrames += 1;
            if (output.kind === 'ui-projection') {
              const stream = output.envelope.stream;
              seen.ui[stream] = (seen.ui[stream] ?? 0) + 1;
              const mode = output.envelope.value?.mode;
              if (typeof mode === 'string' && seen.uiModes.at(-1) !== mode) seen.uiModes.push(mode);
            }
          }
        } catch { /* counted as a batch only */ }
      };
      this.addEventListener('message', record);
      this.addEventListener('rusty-output-baseline', record);
    }
  };
});
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));
const seen = () => page.evaluate(() => JSON.parse(JSON.stringify(window.__seen)));
const hostState = () => page.evaluate(() => document.body.dataset.rustyProductHostState ?? null);
const debug = (command) => page.evaluate(async (text) => JSON.parse(await (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text()), command);
const delta = (a, b) => ({
  batches: b.batches - a.batches,
  emptyFrames: b.emptyFrames - a.emptyFrames,
  kinds: Object.fromEntries(Object.keys(b.kinds).map((k) => [k, b.kinds[k] - (a.kinds[k] ?? 0)]).filter(([, n]) => n !== 0)),
  ui: Object.fromEntries(Object.keys(b.ui).map((k) => [k, b.ui[k] - (a.ui[k] ?? 0)]).filter(([, n]) => n !== 0)),
});
const visible = async (name) => page.getByRole('button', { name }).isVisible().catch(() => false);

const result = { checkpoints: [] };
const mark = async (label, extra = {}) => {
  const observe = await debug('playtest.observe').catch(() => ({}));
  result.checkpoints.push({ label, hostState: await hostState(), mode: observe.mode ?? null,
    step: Number((await debug('engine.time').catch(() => ({}))).simulationStep ?? NaN), ...extra });
  await page.screenshot({ path: `${out}/${String(result.checkpoints.length).padStart(2, '0')}-${label}.png` });
};

await page.goto(origin);
await page.waitForTimeout(8000);
await mark('title', { beginVisible: await visible('Begin') });

let before = await seen();
await page.waitForTimeout(5000);
result.idleTitleFiveSeconds = delta(before, await seen());

before = await seen();
await page.getByRole('button', { name: 'Begin' }).click({ force: true });
await page.waitForTimeout(4000);
result.begin = { ...delta(before, await seen()), beginStillVisible: await visible('Begin') };
await mark('began');

before = await seen();
await page.waitForTimeout(5000);
result.idlePlayingFiveSeconds = delta(before, await seen());

before = await seen();
for (let i = 0; i < 4 && !(await visible('Control settings')); i++) {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(700);
}
const menuVisible = await visible('Control settings');
if (menuVisible) await page.getByRole('button', { name: 'Control settings' }).click({ force: true });
await page.waitForTimeout(1500);
result.controls = { ...delta(before, await seen()), menuVisible,
  rebindVisible: await page.getByRole('button', { name: 'Rebind' }).first().isVisible().catch(() => false) };
await mark('controls-open');

// A product-state change: remap move.forward. The product's control settings
// change, so the HUD projection must be published again and the DOM follow.
const forwardText = () => page.getByText(/^move\.forward: /).first().textContent().catch(() => null);
const rebindForward = async (code) => {
  const start = await seen();
  await page.getByRole('button', { name: 'Rebind' }).first().click({ force: true });
  await page.waitForTimeout(400);
  await page.keyboard.press(code);
  await page.waitForTimeout(1500);
  return { ...delta(start, await seen()), domText: await forwardText(),
    productBinding: (await debug('playtest.observe').catch(() => ({}))).controls?.['move.forward'] ?? null };
};
result.rebind = { domBefore: await forwardText(), toK: await rebindForward('KeyK') };
await mark('rebound-k');
result.rebind.toW = await rebindForward('KeyW');
await mark('restored-w');

before = await seen();
for (let i = 0; i < 4 && !(await visible('Return to game')); i++) {
  const back = page.getByRole('button', { name: 'Back to menu' });
  if (await back.isVisible().catch(() => false)) await back.click({ force: true }); else await page.keyboard.press('Escape');
  await page.waitForTimeout(600);
}
if (await visible('Return to game')) await page.getByRole('button', { name: 'Return to game' }).click({ force: true });
await page.waitForTimeout(1500);
result.returned = { ...delta(before, await seen()), rebindStillVisible:
  await page.getByRole('button', { name: 'Rebind' }).first().isVisible().catch(() => false) };
await mark('returned');

result.total = await seen();
result.errors = errors;
writeFileSync(`${out}/result.json`, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
