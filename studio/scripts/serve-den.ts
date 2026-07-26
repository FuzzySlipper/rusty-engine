import { spawn } from 'node:child_process';
import { access, mkdir, readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, isAbsolute, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const STUDIO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CONSUMER_SOURCE_FILE = resolve(STUDIO_ROOT, 'demo-consumer-source.json');

interface DemoConsumerSource {
  readonly schemaVersion: number;
  readonly publicRepository: string;
  readonly commit: string;
  readonly cargoPackage: string;
  readonly adapterBinary: string;
}

function argumentValue(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`);
  return value;
}

function requiredArgument(name: string): string {
  const value = argumentValue(name);
  if (value === undefined) throw new Error(`${name} is required`);
  return value;
}

function validConsumerSource(value: unknown): value is DemoConsumerSource {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  const source = value as Record<string, unknown>;
  return source['schemaVersion'] === 1
    && typeof source['publicRepository'] === 'string'
    && /^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(source['publicRepository'])
    && typeof source['commit'] === 'string'
    && /^[0-9a-f]{40}$/.test(source['commit'])
    && typeof source['cargoPackage'] === 'string'
    && source['cargoPackage'].length > 0
    && typeof source['adapterBinary'] === 'string'
    && source['adapterBinary'].length > 0;
}

async function consumerSource(): Promise<DemoConsumerSource> {
  const decoded: unknown = JSON.parse(await readFile(CONSUMER_SOURCE_FILE, 'utf8'));
  if (!validConsumerSource(decoded)) {
    throw new Error(`${CONSUMER_SOURCE_FILE} is not a supported exact consumer source`);
  }
  return decoded;
}

async function fileExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function runChecked(command: string, args: readonly string[], cwd: string): Promise<void> {
  const code = await new Promise<number | null>((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' });
    child.once('error', rejectPromise);
    child.once('exit', (exitCode) => resolvePromise(exitCode));
  });
  if (code !== 0) throw new Error(`${command} exited with status ${String(code)}`);
}

async function commandOutput(command: string, args: readonly string[], cwd: string): Promise<string> {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { cwd, stdio: ['ignore', 'pipe', 'inherit'] });
    let output = '';
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk: string) => { output += chunk; });
    child.once('error', rejectPromise);
    child.once('exit', (code) => {
      if (code === 0) resolvePromise(output);
      else rejectPromise(new Error(`${command} exited with status ${String(code)}`));
    });
  });
}

async function exactConsumerCheckout(source: DemoConsumerSource): Promise<string> {
  const cacheBase = process.env['RUSTY_STUDIO_CONSUMER_CACHE_ROOT']
    ?? resolve(
      process.env['XDG_CACHE_HOME'] ?? resolve(homedir(), '.cache'),
      'rusty-engine-studio',
      'consumers',
    );
  if (!isAbsolute(cacheBase)) throw new Error('RUSTY_STUDIO_CONSUMER_CACHE_ROOT must be absolute');
  const checkout = resolve(cacheBase, source.commit);
  const gitDirectory = resolve(checkout, '.git');
  if (!await fileExists(gitDirectory)) {
    await mkdir(cacheBase, { recursive: true });
    await runChecked(
      'git',
      ['clone', '--filter=blob:none', '--no-checkout', source.publicRepository, checkout],
      cacheBase,
    );
  }
  const head = (await commandOutput('git', ['-C', checkout, 'rev-parse', 'HEAD'], cacheBase)).trim();
  if (head !== source.commit) {
    await runChecked('git', ['-C', checkout, 'fetch', '--depth', '1', 'origin', source.commit], cacheBase);
  }
  await runChecked('git', ['-C', checkout, 'checkout', '--detach', source.commit], cacheBase);
  const resolvedHead = (await commandOutput(
    'git',
    ['-C', checkout, 'rev-parse', 'HEAD'],
    cacheBase,
  )).trim();
  if (resolvedHead !== source.commit) {
    throw new Error(`reference consumer resolved ${resolvedHead}, expected ${source.commit}`);
  }
  const worktreeStatus = (await commandOutput(
    'git',
    ['-C', checkout, 'status', '--porcelain'],
    cacheBase,
  )).trim();
  if (worktreeStatus.length !== 0) {
    throw new Error(`reference consumer cache is dirty and will not be executed: ${checkout}`);
  }
  return checkout;
}

async function buildConsumerAdapter(root: string, source: DemoConsumerSource): Promise<string> {
  if (!isAbsolute(root)) throw new Error('Studio consumer root must be absolute');
  const manifest = resolve(root, 'Cargo.toml');
  if (!await fileExists(manifest)) throw new Error(`Studio consumer has no Cargo.toml: ${root}`);
  await runChecked(
    'cargo',
    [
      'build',
      '--locked',
      '--manifest-path',
      manifest,
      '--package',
      source.cargoPackage,
      '--bin',
      source.adapterBinary,
    ],
    root,
  );
  const binary = resolve(root, 'target', 'debug', source.adapterBinary);
  if (!await fileExists(binary)) throw new Error(`Studio adapter build did not produce ${binary}`);
  return binary;
}

async function adapterBinary(): Promise<string> {
  const explicitBinary = argumentValue('--adapter-binary')
    ?? process.env['RUSTY_STUDIO_ADAPTER_BINARY'];
  if (explicitBinary !== undefined) {
    if (!isAbsolute(explicitBinary)) throw new Error('Studio adapter binary must be absolute');
    if (!await fileExists(explicitBinary)) {
      throw new Error(`Studio adapter binary is unavailable: ${explicitBinary}`);
    }
    return explicitBinary;
  }

  const source = await consumerSource();
  const explicitConsumer = argumentValue('--consumer-root')
    ?? process.env['RUSTY_STUDIO_CONSUMER_ROOT'];
  const root = explicitConsumer ?? await exactConsumerCheckout(source);
  return buildConsumerAdapter(root, source);
}

async function runHost(binary: string, host: string, port: number): Promise<void> {
  const code = await new Promise<number | null>((resolvePromise, rejectPromise) => {
    const child = spawn(
      'pnpm',
      [
        'run',
        'host',
        '--',
        '--adapter-binary',
        binary,
        '--host',
        host,
        '--port',
        String(port),
      ],
      { cwd: STUDIO_ROOT, stdio: 'inherit' },
    );
    child.once('error', rejectPromise);
    child.once('exit', (exitCode) => resolvePromise(exitCode));
  });
  if (code !== 0 && code !== null) process.exitCode = code;
}

async function main(): Promise<void> {
  const host = requiredArgument('--host');
  const port = Number(requiredArgument('--port'));
  if (host.length === 0 || /\s/.test(host)) throw new Error('--host must be a non-empty address');
  if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
    throw new Error('--port must be an integer from 1 through 65535');
  }
  const [binary] = await Promise.all([
    adapterBinary(),
    runChecked('pnpm', ['run', 'build'], STUDIO_ROOT),
  ]);
  await runHost(binary, host, port);
}

await main();
