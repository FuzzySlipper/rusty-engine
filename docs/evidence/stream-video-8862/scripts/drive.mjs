// Dagger in the streaming mode: Begin, then watch the opening cinematics play
// in the streamed frames above the page UI until the game starts.
// usage: node drive.mjs <origin> <out-dir> <live-debug> <seconds>
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const require = createRequire('/home/agent/dev/crew-services/internal/playtest/browser/');
const { chromium } = require('playwright-core');

const [origin, out, live, seconds] = process.argv.slice(2);
const browser = await chromium.launch({
  executablePath: '/home/system/crew-services/playtest/browser-binaries/chromium-1243/chrome-linux64/chrome',
  args: ['--autoplay-policy=no-user-gesture-required'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.goto(origin);
const canvas = page.locator('canvas[data-rusty-application-renderer]');
await page.waitForFunction(() => Number(document.querySelector('canvas[data-rusty-application-renderer]')?.dataset.rustyFrameSequence ?? 0) > 0, null, { timeout: 60_000 });
const state = () => page.evaluate(() => {
  const canvas = document.querySelector('canvas[data-rusty-application-renderer]');
  const ui = document.querySelector('[data-rusty-application-ui]');
  return {
    sequence: Number(canvas.dataset.rustyFrameSequence),
    video: canvas.dataset.rustyFrameVideo,
    canvasZ: getComputedStyle(canvas).zIndex,
    uiZ: getComputedStyle(ui).zIndex,
    videoElements: document.querySelectorAll('video').length,
  };
});
const presentation = () => JSON.parse(execFileSync(live, ['--origin', origin, '--command', 'engine.renderer.presentation']).toString());
const log = [];
const note = async (label) => {
  const entry = { t: Date.now(), label, ...(await state()) };
  log.push(entry);
  console.log(JSON.stringify(entry));
  return entry;
};

await page.locator('.dagger-entry-begin').waitFor({ state: 'visible', timeout: 60_000 });
await note('title');
await page.screenshot({ path: `${out}/01-title.png` });
const begun = Date.now();
await page.locator('.dagger-entry-begin').click();
await page.waitForFunction(() => document.querySelector('canvas[data-rusty-application-renderer]')?.dataset.rustyFrameVideo === 'true', null, { timeout: 20_000 });
await page.waitForTimeout(3000);
await note('anim0000 +3s');
await page.screenshot({ path: `${out}/02-anim0000.png` });
const submitted = presentation().presentation.submitted;
writeFileSync(`${out}/presentation-during-video.json`, JSON.stringify({ frameSequence: submitted.frameSequence, simulationStep: submitted.simulationStep, held: submitted.held, video: submitted.video }, null, 2));

// Sample until the video flag has been off for 5 s after the cinematics, or time runs out.
let lastVideo = true;
let offSince = null;
const shots = new Set();
while (Date.now() - begun < Number(seconds) * 1000) {
  const entry = await state();
  const elapsed = (Date.now() - begun) / 1000;
  if ((entry.video === 'true') !== lastVideo) {
    lastVideo = entry.video === 'true';
    await note(`video ${lastVideo ? 'on' : 'off'} at ${elapsed.toFixed(1)}s`);
  }
  for (const [at, name] of [[47, '03-after-anim0000'], [60, '04-dag2'], [175, '05-after-dag2']]) {
    if (elapsed >= at && !shots.has(name)) {
      shots.add(name);
      await note(name);
      await page.screenshot({ path: `${out}/${name}.png` });
    }
  }
  if (!lastVideo) {
    offSince ??= Date.now();
    if (Date.now() - offSince > 5000 && elapsed > 60) break;
  } else offSince = null;
  await page.waitForTimeout(250);
}
await note('end');
await page.screenshot({ path: `${out}/06-end.png` });
writeFileSync(`${out}/log.json`, JSON.stringify(log, null, 2));
await browser.close();
