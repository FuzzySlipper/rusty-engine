import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { watch } from 'node:fs';
import { access, mkdir, readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, dirname, isAbsolute, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const STUDIO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ENGINE_ROOT = resolve(STUDIO_ROOT, '..');
const CONSUMER_SOURCE_FILE = resolve(STUDIO_ROOT, 'demo-consumer-source.json');
const HOST_SHUTDOWN_TIMEOUT_MILLISECONDS = 10_000;
const PROCESS_GROUP_POLL_MILLISECONDS = 20;

interface DemoConsumerSource {
  readonly schemaVersion: number;
  readonly publicRepository: string;
  readonly commit: string;
  readonly cargoPackage: string;
  readonly adapterBinary: string;
  readonly adapterId: string;
  readonly protocolVersion: number;
}

interface ManagedAdapter {
  readonly binary: string;
  readonly source: DemoConsumerSource;
  readonly sourceFingerprint: string;
}

export interface DetachedHostResult {
  readonly code: number | null;
  readonly restartRequired: boolean;
}

export interface StudioRestartRequiredReceipt {
  readonly kind: 'studioRestartRequired';
  readonly code: 'consumer_identity_changed' | 'consumer_identity_unreadable';
  readonly manifest: string;
  readonly message?: string;
}

export function watchConsumerIdentity(
  manifest: string,
  initialFingerprint: string,
  restartRequired: (receipt: StudioRestartRequiredReceipt) => void,
): () => void {
  let checking = false;
  let stopped = false;
  const manifestName = basename(manifest);
  // Watch the containing directory so an atomic manifest replacement does not
  // strand the watcher on the old inode after an unchanged replacement.
  const watcher = watch(dirname(manifest), { persistent: false }, (_event, filename) => {
    if (filename !== null && filename.toString() !== manifestName) return;
    if (checking || stopped) return;
    checking = true;
    void readFile(manifest).then((bytes) => {
      const fingerprint = createHash('sha256').update(bytes).digest('hex');
      if (fingerprint !== initialFingerprint) {
        restartRequired({
          kind: 'studioRestartRequired',
          code: 'consumer_identity_changed',
          manifest,
        });
      }
    }).catch((error: unknown) => {
      restartRequired({
        kind: 'studioRestartRequired',
        code: 'consumer_identity_unreadable',
        message: error instanceof Error ? error.message : String(error),
        manifest,
      });
    }).finally(() => { checking = false; });
  });
  return () => {
    stopped = true;
    watcher.close();
  };
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
    && source['adapterBinary'].length > 0
    && typeof source['adapterId'] === 'string'
    && source['adapterId'].length > 0
    && source['protocolVersion'] === 14;
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

async function managedAdapter(): Promise<ManagedAdapter> {
  const source = await consumerSource();
  const sourceFingerprint = createHash('sha256')
    .update(await readFile(CONSUMER_SOURCE_FILE))
    .digest('hex');
  const root = await exactConsumerCheckout(source);
  return {
    binary: await buildConsumerAdapter(root, source),
    source,
    sourceFingerprint,
  };
}

function signalProcessGroup(pid: number, signal: NodeJS.Signals | 0): boolean {
  try {
    process.kill(-pid, signal);
    return true;
  } catch (error: unknown) {
    if (error instanceof Error && 'code' in error && error.code === 'ESRCH') return false;
    throw error;
  }
}

async function waitForProcessGroupExit(pid: number, timeoutMilliseconds: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    if (!signalProcessGroup(pid, 0)) return true;
    await new Promise<void>((resolvePromise) => {
      setTimeout(resolvePromise, PROCESS_GROUP_POLL_MILLISECONDS);
    });
  }
  return !signalProcessGroup(pid, 0);
}

/**
 * Shut down one detached host and every descendant that inherited its process
 * group. The same bounded TERM -> KILL owner is used for signals, manifest
 * restarts, host crashes, and spawn errors so an unexpected host exit cannot
 * orphan a resistant adapter.
 */
export async function shutdownDetachedProcessGroup(
  pid: number | undefined,
  timeoutMilliseconds = HOST_SHUTDOWN_TIMEOUT_MILLISECONDS,
): Promise<void> {
  if (pid === undefined || !signalProcessGroup(pid, 'SIGTERM')) return;
  if (await waitForProcessGroupExit(pid, timeoutMilliseconds)) return;
  if (!signalProcessGroup(pid, 'SIGKILL')) return;
  if (!await waitForProcessGroupExit(pid, timeoutMilliseconds)) {
    throw new Error(`detached Studio host process group ${pid} survived SIGKILL`);
  }
}

export async function runDetachedHostProcess(
  command: string,
  args: readonly string[],
  cwd: string,
  registerRestartRequired?: (restartRequired: () => void) => () => void,
  shutdownTimeoutMilliseconds = HOST_SHUTDOWN_TIMEOUT_MILLISECONDS,
): Promise<DetachedHostResult> {
  return new Promise<DetachedHostResult>((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit', detached: true });
    let restartRequired = false;
    let shutdownPromise: Promise<void> | undefined;
    let settled = false;
    const shutdown = (): Promise<void> => {
      shutdownPromise ??= shutdownDetachedProcessGroup(child.pid, shutdownTimeoutMilliseconds);
      return shutdownPromise;
    };
    const onSignal = (): void => { void shutdown(); };
    process.once('SIGINT', onSignal);
    process.once('SIGTERM', onSignal);
    const stopWatchingManifest = registerRestartRequired?.(() => {
      if (restartRequired || settled) return;
      restartRequired = true;
      void shutdown();
    }) ?? (() => undefined);
    const cleanup = (): void => {
      stopWatchingManifest();
      process.off('SIGINT', onSignal);
      process.off('SIGTERM', onSignal);
    };
    const finish = async (code: number | null, error?: unknown): Promise<void> => {
      if (settled) return;
      settled = true;
      try {
        await shutdown();
        cleanup();
        if (error !== undefined) rejectPromise(error);
        else resolvePromise({ code, restartRequired });
      } catch (shutdownError: unknown) {
        cleanup();
        rejectPromise(shutdownError);
      }
    };
    child.once('error', (error) => { void finish(null, error); });
    child.once('exit', (code) => { void finish(code); });
  });
}

async function runHost(
  managed: ManagedAdapter,
  engineSourceCommit: string,
  host: string,
  port: number,
): Promise<void> {
  const result = await runDetachedHostProcess(
    'pnpm',
    [
        'run',
        'host',
        '--',
        '--adapter-binary',
        managed.binary,
        '--host',
        host,
        '--port',
        String(port),
        '--engine-source-commit',
        engineSourceCommit,
        '--consumer-repository',
        managed.source.publicRepository,
        '--consumer-commit',
        managed.source.commit,
        '--adapter-build-commit',
        managed.source.commit,
        '--expected-adapter-id',
        managed.source.adapterId,
    ],
    STUDIO_ROOT,
    (restartRequired) => watchConsumerIdentity(
      CONSUMER_SOURCE_FILE,
      managed.sourceFingerprint,
      (receipt) => {
        process.stderr.write(`${JSON.stringify({
          ...receipt,
          previousConsumerCommit: managed.source.commit,
        })}\n`);
        restartRequired();
      },
    ),
  );
  const code = result.restartRequired ? 75 : result.code;
  if (code !== 0 && code !== null) process.exitCode = code;
}

async function engineSourceCommit(): Promise<string> {
  const commit = (await commandOutput('git', ['rev-parse', 'HEAD'], ENGINE_ROOT)).trim();
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    throw new Error(`Engine source did not resolve one exact commit: ${commit}`);
  }
  return commit;
}

async function main(): Promise<void> {
  rejectUnsupportedManagedOverrides();
  const host = requiredArgument('--host');
  const port = Number(requiredArgument('--port'));
  if (host.length === 0 || /\s/.test(host)) throw new Error('--host must be a non-empty address');
  if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
    throw new Error('--port must be an integer from 1 through 65535');
  }
  const [managed, engineCommit] = await Promise.all([
    managedAdapter(),
    engineSourceCommit(),
    runChecked('pnpm', ['run', 'build'], STUDIO_ROOT),
  ]);
  await runHost(managed, engineCommit, host, port);
}

function rejectUnsupportedManagedOverrides(): void {
  const argumentsPresent = ['--adapter-binary', '--consumer-root']
    .filter((name) => process.argv.includes(name));
  const environmentPresent = ['RUSTY_STUDIO_ADAPTER_BINARY', 'RUSTY_STUDIO_CONSUMER_ROOT']
    .filter((name) => process.env[name] !== undefined);
  const present = [...argumentsPresent, ...environmentPresent];
  if (present.length === 0) return;
  throw new Error(
    `managed Studio does not accept ${present.join(', ')}; `
    + 'build Studio and use `pnpm run host -- --adapter-binary /absolute/path` '
    + 'for an explicit downstream adapter',
  );
}

const invokedPath = process.argv[1] === undefined ? null : pathToFileURL(resolve(process.argv[1])).href;
if (invokedPath === import.meta.url) await main();
