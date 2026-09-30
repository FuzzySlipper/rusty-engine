// Open the staged page in headless Chromium, wait for the viewer to finish its
// frames, and write the canvas screenshot and the page's measurements.
//
//   node run-browser.mjs <origin> <out dir> [label [width height]] [--no-unsafe-flag | --plain] [--shaders]
//
// --shaders compiles every render-wgpu WGSL module instead (shaders.html).
//
// PLAYWRIGHT_CORE: a playwright-core directory. CHROMIUM: an executable.
import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';

const require = createRequire(`${process.env.PLAYWRIGHT_CORE}/package.json`);
const { chromium } = require('playwright-core');

const [origin, out, label = 'chromium', width = '1280', height = '720'] = process.argv.slice(2).filter((arg) => !arg.startsWith('--'));
mkdirSync(out, { recursive: true });
// The crew-services GPU flags (streaming-8786), which include WebGPU.
// --no-unsafe-flag drops `--enable-unsafe-webgpu`; --plain passes no flags,
// as a stock browser would run.
const args = process.argv.includes('--plain')
  ? []
  : ['--enable-gpu', '--use-angle=vulkan', '--disable-vulkan-surface', '--ignore-gpu-blocklist', '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan'];
if (!process.argv.includes('--plain') && !process.argv.includes('--no-unsafe-flag')) args.push('--enable-unsafe-webgpu');
const browser = await chromium.launch({
  ...(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {}),
  args,
});
const page = await browser.newPage({ viewport: { width: Number(width), height: Number(height) }, deviceScaleFactor: 1 });
const console_ = [];
page.on('console', (message) => console_.push(`${message.type()}: ${message.text()}`));
page.on('pageerror', (error) => console_.push(`pageerror: ${error.message}`));
if (process.argv.includes('--shaders')) {
  await page.goto(`${origin}/shaders.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__shaders, null, { timeout: 60000 });
  const shaders = await page.evaluate(() => window.__shaders);
  writeFileSync(`${out}/${label}.json`, JSON.stringify(shaders, null, 2));
  console.log(JSON.stringify(shaders, null, 2));
  await browser.close();
  process.exit(0);
}
await page.goto(`${origin}/index.html?width=${width}&height=${height}`, { waitUntil: 'domcontentloaded' });
await page.waitForFunction(() => ['done', 'failed'].includes(window.__spike?.state), null, { timeout: 120000 });
const result = await page.evaluate(async () => {
  const adapter = await navigator.gpu?.requestAdapter();
  const info = adapter?.info;
  const gl = document.createElement('canvas').getContext('webgl2');
  const glInfo = gl?.getExtension('WEBGL_debug_renderer_info');
  return {
    ...window.__spike,
    userAgent: navigator.userAgent,
    gpuAdapter: info ? { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description } : null,
    preferredCanvasFormat: navigator.gpu?.getPreferredCanvasFormat?.() ?? null,
    webgl2Renderer: glInfo ? gl.getParameter(glInfo.UNMASKED_RENDERER_WEBGL) : null,
  };
});
result.console = console_;
if (result.state === 'done') {
  await page.locator('#engine').screenshot({ path: `${out}/${label}.png` });
}
writeFileSync(`${out}/${label}.json`, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
