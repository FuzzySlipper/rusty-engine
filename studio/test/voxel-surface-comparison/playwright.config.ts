import { defineConfig } from '@playwright/test';

const executablePath = process.env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'];
const port = Number(process.env['RUSTY_SURFACE_PORT'] ?? '4319');

export default defineConfig({
  testDir: '.',
  testMatch: 'voxel-surface-comparison.spec.ts',
  timeout: 120_000,
  workers: 1,
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
    command: `pnpm --dir ../../../render exec vite --config ../studio/test/voxel-surface-comparison/vite.config.ts --host 127.0.0.1 --port ${String(port)}`,
    url: `http://127.0.0.1:${String(port)}`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
