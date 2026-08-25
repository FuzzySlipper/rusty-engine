import { chromium } from '@playwright/test';

const origin = process.argv[2];
if (!origin?.startsWith('http://127.0.0.1:')) {
  throw new Error('expected one explicit Product Dev Host loopback origin');
}

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const failures = [];
  page.on('pageerror', (error) => failures.push(`pageerror: ${error.message}`));
  page.on('console', (message) => {
    if (message.type() === 'error') failures.push(`console: ${message.text()}`);
  });
  // The Product Browser Host owns a long-lived SSE output stream, so network
  // idle is intentionally not a valid readiness signal.
  const response = await page.goto(origin, { waitUntil: 'domcontentloaded', timeout: 30_000 });
  if (!response?.ok()) throw new Error(`browser root returned ${response?.status() ?? 'no response'}`);
  await page.waitForSelector('canvas', { state: 'attached', timeout: 30_000 });
  const canvasCount = await page.locator('canvas').count();
  if (canvasCount !== 1) throw new Error(`expected exactly one Engine canvas, found ${canvasCount}`);
  if (failures.length > 0) throw new Error(failures.join('\n'));
  process.stdout.write(`${JSON.stringify({ status: 'ok', origin, canvasCount })}\n`);
} finally {
  await browser.close();
}
