import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import {
  access,
  constants,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readlink,
  rename,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const STUDIO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DEFAULT_ENGINE_ROOT = resolve(STUDIO_ROOT, '..');
const DEFAULT_RUNTIME_ROOT = '/home/system/rusty-studio';
const SERVICE_NAME = 'rusty-studio.service';
const COMMIT = /^[0-9a-f]{40}$/u;

export type UpdateDisposition = 'current' | 'fast-forward';

export function serviceCommand(arguments_: readonly string[]): string | undefined {
  return arguments_.find((value) => value !== '--');
}

export function requireCleanCheckout(status: string): void {
  if (status.trim().length !== 0) {
    throw new Error('studio_service_update_dirty_checkout: commit or remove local changes before updating');
  }
}

export function classifyUpdate(
  head: string,
  upstream: string,
  headIsAncestor: boolean,
): UpdateDisposition {
  if (!COMMIT.test(head) || !COMMIT.test(upstream)) {
    throw new Error('studio_service_update_invalid_revision');
  }
  if (head === upstream) return 'current';
  if (!headIsAncestor) {
    throw new Error('studio_service_update_not_fast_forward: local and upstream history diverged');
  }
  return 'fast-forward';
}

export async function promoteRelease(
  runtimeRoot: string,
  releaseRoot: string,
  smoke: () => Promise<void>,
): Promise<{ readonly previous: string | null }> {
  await smoke();
  await mkdir(runtimeRoot, { recursive: true });
  const currentLink = join(runtimeRoot, 'current');
  const previousLink = join(runtimeRoot, 'previous');
  const previous = await optionalLink(currentLink);
  if (previous !== null && previous !== releaseRoot) {
    await replaceLink(previousLink, previous);
  }
  await replaceLink(currentLink, releaseRoot);
  return { previous };
}

async function optionalLink(path: string): Promise<string | null> {
  try {
    const metadata = await lstat(path);
    if (!metadata.isSymbolicLink()) throw new Error(`${path} must be a symbolic link`);
    return await readlink(path);
  } catch (error) {
    if (isMissing(error)) return null;
    throw error;
  }
}

async function replaceLink(path: string, target: string): Promise<void> {
  const candidate = `${path}.next-${String(process.pid)}`;
  await rm(candidate, { force: true });
  await symlink(target, candidate);
  await rename(candidate, path);
}

interface ServiceConfiguration {
  readonly engineRoot: string;
  readonly runtimeRoot: string;
  readonly host: string;
  readonly port: number;
  readonly pnpm: string;
}

function configuration(): ServiceConfiguration {
  const engineRoot = resolve(process.env['RUSTY_STUDIO_ENGINE_ROOT'] ?? DEFAULT_ENGINE_ROOT);
  const runtimeRoot = resolve(process.env['RUSTY_STUDIO_RUNTIME_ROOT'] ?? DEFAULT_RUNTIME_ROOT);
  const host = process.env['RUSTY_STUDIO_HOST'] ?? '127.0.0.1';
  const port = Number(process.env['RUSTY_STUDIO_PORT'] ?? '4310');
  const pnpm = process.env['RUSTY_STUDIO_PNPM'] ?? 'pnpm';
  if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
    throw new Error('RUSTY_STUDIO_PORT must be an integer from 1 through 65535');
  }
  if (host.length === 0 || /\s/u.test(host)) throw new Error('RUSTY_STUDIO_HOST is invalid');
  return { engineRoot, runtimeRoot, host, port, pnpm };
}

async function prepare(config: ServiceConfiguration): Promise<string> {
  const commit = (await output('git', ['rev-parse', 'HEAD'], config.engineRoot)).trim();
  if (!COMMIT.test(commit)) throw new Error('Engine HEAD did not resolve one exact commit');
  const releasesRoot = join(config.runtimeRoot, 'releases');
  const releaseRoot = join(releasesRoot, commit);
  await mkdir(releasesRoot, { recursive: true });
  if (!await pathExists(join(releaseRoot, 'studio', 'dist', 'apps', 'studio-app', 'browser', 'index.html'))) {
    const candidate = await mkdtemp(join(releasesRoot, `.candidate-${commit.slice(0, 12)}-`));
    try {
      await extractArchive(config.engineRoot, commit, candidate);
      await checked(config.pnpm, ['--dir', join(candidate, 'studio'), 'install', '--frozen-lockfile'], candidate);
      await checked(config.pnpm, ['--dir', join(candidate, 'studio'), 'run', 'build'], candidate);
      await writeFile(join(candidate, 'studio-service-identity.json'), `${JSON.stringify({
        schemaVersion: 1,
        engineSourceCommit: commit,
      }, null, 2)}\n`);
      await rename(candidate, releaseRoot);
    } catch (error) {
      await rm(candidate, { recursive: true, force: true });
      throw error;
    }
  }
  await promoteRelease(config.runtimeRoot, releaseRoot, () => smokeRelease(config, releaseRoot, commit));
  process.stdout.write(`${releaseRoot}\n`);
  return releaseRoot;
}

async function runService(config: ServiceConfiguration): Promise<void> {
  const current = await optionalLink(join(config.runtimeRoot, 'current'));
  if (current === null) throw new Error('rusty_studio_release_missing: run prepare first');
  const identity = JSON.parse(await readFile(join(current, 'studio-service-identity.json'), 'utf8')) as {
    engineSourceCommit?: unknown;
  };
  if (typeof identity.engineSourceCommit !== 'string' || !COMMIT.test(identity.engineSourceCommit)) {
    throw new Error('rusty_studio_release_identity_invalid');
  }
  const child = spawnHost(config, current, identity.engineSourceCommit, config.host, config.port, 'inherit');
  const interrupt = (): void => { child.kill('SIGINT'); };
  const terminate = (): void => { child.kill('SIGTERM'); };
  process.once('SIGINT', interrupt);
  process.once('SIGTERM', terminate);
  const result = await childResult(child);
  process.removeListener('SIGINT', interrupt);
  process.removeListener('SIGTERM', terminate);
  if (result.signal !== null) process.kill(process.pid, result.signal);
  process.exitCode = result.code ?? 1;
}

async function update(config: ServiceConfiguration): Promise<void> {
  requireCleanCheckout(await output('git', ['status', '--porcelain'], config.engineRoot));
  await checked('git', ['fetch', '--prune'], config.engineRoot);
  const head = (await output('git', ['rev-parse', 'HEAD'], config.engineRoot)).trim();
  const upstream = (await output('git', ['rev-parse', '@{upstream}'], config.engineRoot)).trim();
  const ancestor = await succeeds('git', ['merge-base', '--is-ancestor', head, upstream], config.engineRoot);
  const disposition = classifyUpdate(head, upstream, ancestor);
  if (disposition === 'fast-forward') {
    await checked('git', ['merge', '--ff-only', upstream], config.engineRoot);
  }
  await checked(config.pnpm, ['--dir', join(config.engineRoot, 'studio'), 'install', '--frozen-lockfile'], config.engineRoot);
  const previous = await optionalLink(join(config.runtimeRoot, 'current'));
  await prepare(config);
  try {
    await checked('systemctl', ['--user', 'restart', SERVICE_NAME], config.engineRoot);
    await waitForHealth(config.host, config.port);
  } catch (error) {
    if (previous !== null) {
      await replaceLink(join(config.runtimeRoot, 'current'), previous);
      await checked('systemctl', ['--user', 'restart', SERVICE_NAME], config.engineRoot);
    }
    throw error;
  }
}

async function rollback(config: ServiceConfiguration): Promise<void> {
  const previous = await optionalLink(join(config.runtimeRoot, 'previous'));
  if (previous === null) throw new Error('rusty_studio_previous_release_missing');
  const current = await optionalLink(join(config.runtimeRoot, 'current'));
  if (current !== null) await replaceLink(join(config.runtimeRoot, 'previous'), current);
  await replaceLink(join(config.runtimeRoot, 'current'), previous);
  await checked('systemctl', ['--user', 'restart', SERVICE_NAME], config.engineRoot);
  await waitForHealth(config.host, config.port);
}

async function install(config: ServiceConfiguration): Promise<void> {
  const configRoot = join(config.runtimeRoot, 'config');
  await mkdir(configRoot, { recursive: true });
  const environmentPath = join(configRoot, 'service.env');
  if (!await pathExists(environmentPath)) {
    await writeFile(environmentPath, [
      `RUSTY_STUDIO_ENGINE_ROOT=${config.engineRoot}`,
      `RUSTY_STUDIO_RUNTIME_ROOT=${config.runtimeRoot}`,
      'RUSTY_STUDIO_HOST=127.0.0.1',
      `RUSTY_STUDIO_PORT=${String(config.port)}`,
      '',
    ].join('\n'), { mode: 0o600 });
  }
  const template = await readFile(join(STUDIO_ROOT, 'ops', SERVICE_NAME), 'utf8');
  const pnpm = await executablePath('pnpm');
  const rendered = template
    .replaceAll('@ENGINE_ROOT@', config.engineRoot)
    .replaceAll('@RUNTIME_ROOT@', config.runtimeRoot)
    .replaceAll('@PNPM@', pnpm)
    .replaceAll('@PNPM_DIR@', dirname(pnpm));
  const unitRoot = join(process.env['HOME'] ?? '', '.config', 'systemd', 'user');
  if (!unitRoot.startsWith('/')) throw new Error('HOME must resolve an absolute user service root');
  await mkdir(unitRoot, { recursive: true });
  await writeFile(join(unitRoot, SERVICE_NAME), rendered);
  await prepare(config);
  await checked('systemctl', ['--user', 'daemon-reload'], config.engineRoot);
  await checked('systemctl', ['--user', 'enable', '--now', SERVICE_NAME], config.engineRoot);
  await waitForHealth(config.host, config.port);
}

async function executablePath(name: string): Promise<string> {
  const search = process.env['PATH']?.split(':') ?? [];
  for (const root of search) {
    if (!root.startsWith('/')) continue;
    const candidate = join(root, name);
    try {
      await access(candidate, constants.X_OK);
      return candidate;
    } catch (error) {
      if (!isMissing(error) && !(error instanceof Error && 'code' in error && error.code === 'EACCES')) {
        throw error;
      }
    }
  }
  throw new Error(`${name} was not found on the installer PATH`);
}

async function uninstall(config: ServiceConfiguration): Promise<void> {
  await succeeds('systemctl', ['--user', 'disable', '--now', SERVICE_NAME], config.engineRoot);
  const unitRoot = join(process.env['HOME'] ?? '', '.config', 'systemd', 'user');
  await rm(join(unitRoot, SERVICE_NAME), { force: true });
  await checked('systemctl', ['--user', 'daemon-reload'], config.engineRoot);
  process.stdout.write(`Preserved runtime releases and settings at ${config.runtimeRoot}\n`);
}

async function smokeRelease(
  config: ServiceConfiguration,
  releaseRoot: string,
  commit: string,
): Promise<void> {
  const port = await freePort();
  const child = spawnHost(config, releaseRoot, commit, '127.0.0.1', port, 'ignore');
  const result = childResult(child);
  try {
    await waitForHealth('127.0.0.1', port, commit);
  } finally {
    child.kill('SIGTERM');
    await result;
  }
}

function spawnHost(
  config: ServiceConfiguration,
  releaseRoot: string,
  commit: string,
  host: string,
  port: number,
  stdio: 'ignore' | 'inherit',
) {
  return spawn(config.pnpm, [
    '--dir', join(releaseRoot, 'studio'),
    'run', 'host', '--',
    '--static-root', join(releaseRoot, 'studio', 'dist', 'apps', 'studio-app', 'browser'),
    '--settings-root', join(config.runtimeRoot, 'settings'),
    '--host', host,
    '--port', String(port),
    '--rolling-engine-source-commit', commit,
  ], { cwd: releaseRoot, stdio, detached: false });
}

async function extractArchive(engineRoot: string, commit: string, target: string): Promise<void> {
  const archive = spawn('git', ['archive', '--format=tar', commit], {
    cwd: engineRoot,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const extract = spawn('tar', ['-x', '-C', target], { stdio: ['pipe', 'inherit', 'inherit'] });
  archive.stdout.pipe(extract.stdin);
  const [archiveResult, extractResult] = await Promise.all([childResult(archive), childResult(extract)]);
  if (archiveResult.code !== 0 || extractResult.code !== 0) {
    throw new Error('rusty_studio_release_archive_failed');
  }
}

async function waitForHealth(host: string, port: number, commit?: string): Promise<void> {
  const deadline = Date.now() + 60_000;
  let detail = 'not ready';
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://${host}:${String(port)}/health`);
      const body = await response.json() as { engineSourceCommit?: unknown };
      if (response.ok && (commit === undefined || body.engineSourceCommit === commit)) return;
      detail = `HTTP ${String(response.status)} ${JSON.stringify(body)}`;
    } catch (error) {
      detail = String(error);
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw new Error(`rusty_studio_health_timeout: ${detail}`);
}

async function freePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolvePromise) => server.listen(0, '127.0.0.1', resolvePromise));
  const address = server.address();
  if (address === null || typeof address === 'string') throw new Error('free port unavailable');
  await new Promise<void>((resolvePromise, rejectPromise) =>
    server.close((error) => error === undefined ? resolvePromise() : rejectPromise(error)),
  );
  return address.port;
}

async function output(command: string, args: readonly string[], cwd: string): Promise<string> {
  const child = spawn(command, args, { cwd, stdio: ['ignore', 'pipe', 'inherit'] });
  let value = '';
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk: string) => { value += chunk; });
  const result = await childResult(child);
  if (result.code !== 0) throw new Error(`${command} ${args.join(' ')} failed`);
  return value;
}

async function checked(command: string, args: readonly string[], cwd: string): Promise<void> {
  const child = spawn(command, args, { cwd, stdio: 'inherit' });
  const result = await childResult(child);
  if (result.code !== 0) throw new Error(`${command} ${args.join(' ')} failed`);
}

async function succeeds(command: string, args: readonly string[], cwd: string): Promise<boolean> {
  const child = spawn(command, args, { cwd, stdio: 'ignore' });
  return (await childResult(child)).code === 0;
}

async function childResult(child: ReturnType<typeof spawn>): Promise<{
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;
}> {
  return new Promise((resolvePromise, rejectPromise) => {
    child.once('error', rejectPromise);
    child.once('exit', (code, signal) => resolvePromise({ code, signal }));
  });
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
}

function isMissing(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

async function main(): Promise<void> {
  const command = serviceCommand(process.argv.slice(2));
  await loadServiceEnvironment();
  const config = configuration();
  switch (command) {
    case 'prepare': await prepare(config); break;
    case 'run': await runService(config); break;
    case 'update': await update(config); break;
    case 'rollback': await rollback(config); break;
    case 'install': await install(config); break;
    case 'uninstall': await uninstall(config); break;
    default: throw new Error('usage: pnpm run service -- prepare|run|update|rollback|install|uninstall');
  }
}

async function loadServiceEnvironment(): Promise<void> {
  const runtimeRoot = resolve(process.env['RUSTY_STUDIO_RUNTIME_ROOT'] ?? DEFAULT_RUNTIME_ROOT);
  const path = join(runtimeRoot, 'config', 'service.env');
  let contents: string;
  try {
    contents = await readFile(path, 'utf8');
  } catch (error) {
    if (isMissing(error)) return;
    throw error;
  }
  const accepted = new Set([
    'RUSTY_STUDIO_ENGINE_ROOT',
    'RUSTY_STUDIO_RUNTIME_ROOT',
    'RUSTY_STUDIO_HOST',
    'RUSTY_STUDIO_PORT',
    'RUSTY_STUDIO_PNPM',
  ]);
  for (const [index, line] of contents.split(/\r?\n/u).entries()) {
    if (line.length === 0 || line.startsWith('#')) continue;
    const match = /^([A-Z0-9_]+)=([^\0\r\n]*)$/u.exec(line);
    if (match === null || !accepted.has(match[1] as string)) {
      throw new Error(`invalid Studio service environment entry at ${path}:${String(index + 1)}`);
    }
    const name = match[1] as string;
    if (process.env[name] === undefined) process.env[name] = match[2] as string;
  }
}

const invoked = process.argv[1] === undefined ? null : resolve(process.argv[1]);
if (invoked === fileURLToPath(import.meta.url)) await main();
