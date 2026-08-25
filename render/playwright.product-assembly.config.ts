import { defineConfig } from '@playwright/test';

const origin = process.env['PLAYWRIGHT_PRODUCT_HOST_ORIGIN'];
if (origin === undefined || !/^http:\/\/127\.0\.0\.1:\d+$/u.test(origin)) {
  throw new Error('PLAYWRIGHT_PRODUCT_HOST_ORIGIN must be the generated 127.0.0.1 origin');
}

const executablePath = process.env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'];

/**
 * Explicit #7262 gate configuration. There is intentionally no webServer:
 * the Rust integration test starts the generated ProductDevHost and supplies
 * its exact loopback origin. Playwright creates a fresh browser context for
 * this run, so no shared product profile or existing server is reused.
 */
export default defineConfig({
  testDir: './browser',
  testMatch: 'product-assembly-generated.browser.spec.ts',
  timeout: 30_000,
  workers: 1,
  use: {
    baseURL: origin,
    browserName: 'chromium',
    headless: true,
    launchOptions: {
      ...(executablePath === undefined ? {} : { executablePath }),
      args: ['--enable-webgl', '--ignore-gpu-blocklist', '--use-angle=swiftshader'],
    },
  },
});
