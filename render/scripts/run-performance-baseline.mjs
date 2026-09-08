#!/usr/bin/env node
import { createServer } from 'vite';
import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { execFileSync } from 'node:child_process';
const { values } = parseArgs({ options: {
  output: { type: 'string' }, port: { type: 'string', default: '4190' },
  serve: { type: 'boolean', default: false }, environment: { type: 'string' },
} });
if (!values.output || !values.environment) throw new Error('Usage: --output DIR --environment LABEL [--serve] [--port 4190]');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(values.output);
await mkdir(output, { recursive: true });
let finish;
const completed = new Promise((resolve) => { finish = resolve; });
const server = await createServer({ root, configFile: resolve(root, 'vite.config.ts'),
  server: { host: '0.0.0.0', port: Number(values.port), strictPort: true, headers: {
    'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp',
  } },
  plugins: [{ name: 'performance-capture', configureServer(server) {
    server.middlewares.use('/performance-results', async (request, response) => {
      if (request.method !== 'POST') { response.statusCode = 405; response.end(); return; }
      try {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        const result = JSON.parse(Buffer.concat(chunks).toString());
        await writeFile(resolve(output, 'browser-result.json'), JSON.stringify(result, null, 2));
        if (result.error || !Array.isArray(result.records) || !result.records.length) throw new Error(result.error ?? 'Missing records');
        await writeFile(resolve(output, 'browser.log'), result.records.map((r) => `RUSTY_PERF ${JSON.stringify(r)}`).join('\n') + '\n');
        response.end('saved');
        finish({ ok: true });
      } catch (error) {
        response.statusCode = 500; response.end(String(error)); finish({ ok: false, error: String(error) });
      }
    });
  } }],
});
let browser;
try {
  await server.listen();
  const url = `http://127.0.0.1:${values.port}/browser/performance-baseline.html`;
  console.log(`Performance page: ${url}`);
  if (!values.serve) {
    browser = await chromium.launch({ executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ?? '/usr/bin/chromium',
      headless: true, args: ['--enable-webgl', '--ignore-gpu-blocklist', '--use-angle=swiftshader'] });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
    page.on('pageerror', (error) => finish({ ok: false, error: String(error) }));
    await page.goto(url);
  } else console.log('Open the page in the owned GPU harness session. Installed harness configuration is unchanged.');
  const timeout = setTimeout(() => finish({ ok: false, error: 'Capture timed out after 15 minutes' }), 15 * 60_000);
  const result = await completed;
  clearTimeout(timeout);
  if (!result.ok) throw new Error(result.error);
  execFileSync(process.execPath, [resolve(root, '../scripts/performance-results.mjs'), 'capture', '--output', resolve(output, 'baseline.json'),
    '--environment', values.environment, resolve(output, 'browser.log')], { stdio: 'inherit', cwd: resolve(root, '..') });
  console.log(`Saved ${output}/baseline.json`);
} finally {
  await browser?.close();
  await server.close();
}
