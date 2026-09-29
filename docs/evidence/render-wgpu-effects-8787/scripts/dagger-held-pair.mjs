// Held Dagger frame: Three screenshot and presentation capture of the same state.
// usage: node dagger-held-pair.mjs <origin> <out-dir> <capture-presentation.py> <name>
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');
const [origin, out, captureScript, name] = process.argv.slice(2);
const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const debug = (command) => page.evaluate(async (text) => (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text(), command);
await page.goto(origin);
await page.waitForTimeout(12000);
console.log(await debug('engine.time.mode manual'));
await page.waitForTimeout(2000);
await page.screenshot({ path: `${out}/${name}-three.png` });
execFileSync('python3', [captureScript, origin, `${out}/${name}`], { stdio: 'inherit' });
await browser.close();
