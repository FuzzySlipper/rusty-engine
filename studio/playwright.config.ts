import { defineConfig } from '@playwright/test';

const adapterBinary = requiredEnvironment('RUSTY_STUDIO_ADAPTER_BINARY');
const settingsRoot = requiredEnvironment('RUSTY_STUDIO_SETTINGS_ROOT');
const port = Number(process.env['RUSTY_STUDIO_PORT'] ?? '4301');
if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error('RUSTY_STUDIO_PORT must be a valid TCP port');
}
const baseURL = `http://127.0.0.1:${String(port)}`;
const executablePath = process.env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'];
const managedIdentityArguments = managedIdentityArgs();

export default defineConfig({
  testDir: './test/browser',
  timeout: 45_000,
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
    command: `pnpm run host -- --adapter-binary ${shellArgument(adapterBinary)} --settings-root ${shellArgument(settingsRoot)} --port ${String(port)}${managedIdentityArguments}`,
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

function managedIdentityArgs(): string {
  const entries = [
    ['--engine-source-commit', 'RUSTY_STUDIO_ENGINE_SOURCE_COMMIT'],
    ['--consumer-repository', 'RUSTY_STUDIO_CONSUMER_REPOSITORY'],
    ['--consumer-commit', 'RUSTY_STUDIO_CONSUMER_COMMIT'],
    ['--adapter-build-commit', 'RUSTY_STUDIO_ADAPTER_BUILD_COMMIT'],
    ['--expected-adapter-id', 'RUSTY_STUDIO_EXPECTED_ADAPTER_ID'],
  ] as const;
  const values = entries.map(([, environment]) => process.env[environment]);
  if (values.every((value) => value === undefined)) return '';
  if (values.some((value) => value === undefined || value.length === 0)) {
    throw new Error(`managed Studio identity requires ${entries.map(([, name]) => name).join(', ')}`);
  }
  return entries.map(([argument], index) =>
    ` ${argument} ${shellArgument(values[index] as string)}`,
  ).join('');
}
