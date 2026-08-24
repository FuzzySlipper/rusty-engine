import { defineConfig } from '@playwright/test';

const executablePath = process.env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'];
const port = Number(process.env['PLAYWRIGHT_RENDER_PORT'] ?? '4173');

export default defineConfig({
  testDir: './browser',
  // This spec requires the Rust integration test to launch an actual
  // generated ProductDevHost and is owned by
  // playwright.product-assembly.config.ts, not the ordinary Vite fixture.
  testIgnore: 'product-assembly-generated.browser.spec.ts',
  timeout: 30_000,
  use: {
    baseURL: `http://127.0.0.1:${String(port)}`,
    browserName: 'chromium',
    headless: true,
    launchOptions: {
      ...(executablePath === undefined ? {} : { executablePath }),
      args: ['--enable-webgl', '--ignore-gpu-blocklist', '--use-angle=swiftshader'],
    },
  },
  webServer: {
    command: `pnpm exec vite --config vite.config.ts --host 127.0.0.1 --port ${String(port)}`,
    port,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
