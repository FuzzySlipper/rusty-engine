// Time browser JPEG and raw RGBA decode-and-draw for two saved frames.
// Usage: node decode-bench.mjs <dir with probe-720b.jpg and probe-1080.jpg>
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
const require = createRequire(new URL('../../../../render/node_modules/.pnpm/playwright-core@1.61.1/node_modules/playwright-core/package.json', import.meta.url));
const { chromium } = require('playwright-core');
const browser = await chromium.launch({ args: ['--enable-gpu', '--use-angle=vulkan', '--disable-vulkan-surface', '--ignore-gpu-blocklist', '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan'] });
const page = await browser.newPage();
for (const [name, size] of [['probe-720b.jpg', [1280, 720]], ['probe-1080.jpg', [1920, 1080]]]) {
  const bytes = [...readFileSync(`${process.argv[2]}/${name}`)];
  const result = await page.evaluate(async ({ bytes, size }) => {
    const data = new Uint8Array(bytes);
    const canvas = document.createElement('canvas'); canvas.width = size[0]; canvas.height = size[1];
    const context = canvas.getContext('2d', { alpha: false });
    const raw = new Uint8ClampedArray(size[0] * size[1] * 4);
    const times = { jpeg: [], rgba: [] };
    for (let i = 0; i < 40; i++) {
      let t = performance.now();
      const bitmap = await createImageBitmap(new Blob([data], { type: 'image/jpeg' }));
      context.drawImage(bitmap, 0, 0); bitmap.close(); times.jpeg.push(performance.now() - t);
      t = performance.now();
      const rgba = await createImageBitmap(new ImageData(raw, size[0], size[1]));
      context.drawImage(rgba, 0, 0); rgba.close(); times.rgba.push(performance.now() - t);
    }
    const median = (v) => v.sort((a, b) => a - b)[v.length >> 1];
    return { jpegDecodeDrawMs: median(times.jpeg.slice(5)), rgbaDecodeDrawMs: median(times.rgba.slice(5)) };
  }, { bytes, size });
  console.log(name, JSON.stringify(result));
}
await browser.close();
