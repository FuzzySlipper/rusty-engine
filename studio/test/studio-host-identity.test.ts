import assert from 'node:assert/strict';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

import {
  STUDIO_ADAPTER_OPERATIONS,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  decodeStudioHostStatus,
} from '../libs/adapter-client/src/index.js';

const STUDIO_ROOT = resolve(import.meta.dirname, '..');
const ENGINE = '1'.repeat(40);
const CONSUMER = '2'.repeat(40);

test('rejects an old adapter before listening, then exposes the exact current identity', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-host-identity-'));
  try {
    const staticRoot = join(root, 'static');
    const oldAdapter = join(root, 'old-adapter.mjs');
    const currentAdapter = join(root, 'current-adapter.mjs');
    await Promise.all([
      writeFile(join(root, 'index.html'), '<!doctype html><title>fixture</title>'),
      writeAdapter(oldAdapter, 12),
      writeAdapter(currentAdapter, STUDIO_ADAPTER_PROTOCOL_VERSION),
    ]);
    await mkdir(staticRoot);
    await rename(join(root, 'index.html'), join(staticRoot, 'index.html'));

    const rejectedPort = await freePort();
    const rejected = hostProcess(oldAdapter, staticRoot, rejectedPort);
    const rejection = await processResult(rejected);
    assert.notEqual(rejection.code, 0);
    assert.match(
      rejection.stderr,
      new RegExp(`protocolVersion.*must equal ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)}`, 'u'),
    );
    await assert.rejects(fetch(`http://127.0.0.1:${String(rejectedPort)}/health`));

    const port = await freePort();
    const current = hostProcess(currentAdapter, staticRoot, port);
    try {
      const response = await waitForStatus(port);
      const status = decodeStudioHostStatus(response);
      assert.equal(status.engineSourceCommit, ENGINE);
      assert.equal(status.configuredConsumer?.commit, CONSUMER);
      assert.equal(status.runningAdapter.buildCommit, CONSUMER);
      assert.equal(status.runningAdapter.protocolVersion, STUDIO_ADAPTER_PROTOCOL_VERSION);
      assert.equal(status.activeProjectRoot, null);
      assert.equal(status.activeProjectFile, null);
      assert.equal(
        status.runningAdapter.binarySha256,
        createHash('sha256').update(await readFile(currentAdapter)).digest('hex'),
      );
    } finally {
      current.kill('SIGTERM');
      const result = await processResult(current);
      assert.ok(result.code === 0 || result.signal === 'SIGTERM');
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

async function writeAdapter(path: string, protocolVersion: number): Promise<void> {
  await writeFile(path, `#!/usr/bin/env node
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const request = JSON.parse(line);
  process.stdout.write(JSON.stringify({
    type: 'described',
    protocolVersion: ${String(protocolVersion)},
    requestId: request.requestId,
    adapter: {
      adapterId: 'studio.test-fixture',
      adapterVersion: 1,
      protocolVersion: ${String(protocolVersion)},
      projectKind: 'fixture',
      projectSchemaVersion: 1,
      operations: ${JSON.stringify(STUDIO_ADAPTER_OPERATIONS)},
      entityInspectorContracts: [],
    },
  }) + '\\n');
});
`);
  await chmod(path, 0o755);
}

function hostProcess(
  adapter: string,
  staticRoot: string,
  port: number,
): ChildProcessWithoutNullStreams {
  return spawn('pnpm', [
    'exec',
    'tsx',
    'scripts/studio-host.ts',
    '--adapter-binary', adapter,
    '--static-root', staticRoot,
    '--port', String(port),
    '--engine-source-commit', ENGINE,
    '--consumer-repository', 'https://github.com/FuzzySlipper/studio-test-fixture',
    '--consumer-commit', CONSUMER,
    '--adapter-build-commit', CONSUMER,
    '--expected-adapter-id', 'studio.test-fixture',
  ], { cwd: STUDIO_ROOT, stdio: ['pipe', 'pipe', 'pipe'] });
}

async function waitForStatus(port: number): Promise<unknown> {
  const deadline = Date.now() + 10_000;
  let lastError: unknown;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${String(port)}/api/studio-status`);
      if (response.ok) return await response.json() as unknown;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 25));
  }
  throw new Error(`Studio status did not become ready: ${String(lastError)}`);
}

async function freePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolvePromise) => server.listen(0, '127.0.0.1', resolvePromise));
  const address = server.address();
  if (address === null || typeof address === 'string') throw new Error('fixture port is unavailable');
  await new Promise<void>((resolvePromise, rejectPromise) =>
    server.close((error) => error === undefined ? resolvePromise() : rejectPromise(error)),
  );
  return address.port;
}

async function processResult(process: ChildProcessWithoutNullStreams): Promise<{
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;
  readonly stderr: string;
}> {
  let stderr = '';
  process.stderr.setEncoding('utf8');
  process.stderr.on('data', (chunk: string) => { stderr += chunk; });
  const [code, signal] = await new Promise<[number | null, NodeJS.Signals | null]>((resolvePromise) => {
    process.once('exit', (exitCode, exitSignal) => resolvePromise([exitCode, exitSignal]));
  });
  return { code, signal, stderr };
}
