import { defineConfig } from '@playwright/test';

const adapterBinary = requiredEnvironment('RUSTY_STUDIO_ADAPTER_BINARY');
const settingsRoot = requiredEnvironment('RUSTY_STUDIO_SETTINGS_ROOT');
const port = Number(process.env['RUSTY_STUDIO_PORT'] ?? '4302');
if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error('RUSTY_STUDIO_PORT must be a valid TCP port');
}
const baseURL = `http://127.0.0.1:${String(port)}`;
const executablePath = process.env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'];

export default defineConfig({
  testDir: './test/voxel-consumer-browser',
  timeout: 90_000,
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL,
    browserName: 'chromium',
    headless: true,
    launchOptions: {
      ...(executablePath === undefined ? {} : { executablePath }),
      args: ['--enable-webgl', '--ignore-gpu-blocklist', '--use-angle=swiftshader'],
    },
  },
  webServer: {
    command: `pnpm run host -- --adapter-binary ${shellArgument(adapterBinary)} --settings-root ${shellArgument(settingsRoot)} --port ${String(port)}`,
    url: `${baseURL}/health`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}

function shellArgument(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}
