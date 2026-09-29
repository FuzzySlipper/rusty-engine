// Same-moment Three screenshot and presentation capture of the Doom room study.
// usage: node doom-held-pair.mjs <origin> <out-dir> <capture-presentation.py>
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');

const [origin, out, captureScript] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--use-gl=angle', '--use-angle=swiftshader', '--autoplay-policy=no-user-gesture-required'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));
const debug = (command) => page.evaluate(async (text) => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text(), command);
const capture = (name) => execFileSync('python3', [captureScript, origin, `${out}/${name}`], { stdio: 'inherit' });
const settle = (ms) => page.waitForTimeout(ms);

await page.goto(origin);
await settle(20000);
const log = { steps: [] };
log.steps.push(['hold', await debug('engine.time.mode manual')]);
await settle(1500);
// Fire once while held (Left Ctrl), then step one frame at a time until the
// weapon shows its flash, and capture that held frame both ways.
await page.click('canvas');
await settle(300);
await page.keyboard.down('ControlLeft');
await settle(300);
for (let step = 1; step <= 12; step += 1) {
  const advanced = await debug('engine.time.advance 17');
  if (step === 3) await page.keyboard.up('ControlLeft');
  await settle(1200);
  capture(`fire-${step}`);
  const sprites = execFileSync('python3', ['-c', `
import json
ops = json.load(open('${out}/fire-${step}/world-frame.json'))['ops']
print(json.dumps([[o['sprite']['asset'], o['sprite']['metadata'].get('sourceEntity')] for o in ops if o['op'] == 'createSprite' and o['sprite']['layer'] == 'viewmodel']))
`]).toString().trim();
  log.steps.push([`step-${step}`, advanced, sprites]);
  await page.screenshot({ path: `${out}/fire-${step}-three.png` });
  if (sprites.includes('35001')) break;
}

log.errors = errors;
writeFileSync(`${out}/pair.json`, JSON.stringify(log, null, 2));
await browser.close();
