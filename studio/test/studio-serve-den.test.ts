import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { access, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  runDetachedHostProcess,
  watchConsumerIdentity,
  type StudioRestartRequiredReceipt,
} from '../scripts/serve-den.js';

async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

test('managed serve rejects an obsolete explicit-adapter override instead of ignoring it', async () => {
  const child = spawn('pnpm', [
    'exec',
    'tsx',
    'scripts/serve-den.ts',
    '--',
    '--adapter-binary',
    '/tmp/obsolete-adapter',
    '--host',
    '127.0.0.1',
    '--port',
    '4300',
  ], { cwd: join(import.meta.dirname, '..'), stdio: ['ignore', 'ignore', 'pipe'] });
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk: string) => { stderr += chunk; });
  const code = await new Promise<number | null>((resolve) => child.once('exit', resolve));
  assert.notEqual(code, 0);
  assert.match(stderr, /managed Studio does not accept --adapter-binary/u);
  assert.match(stderr, /pnpm run host/u);
});

test('the managed serve watcher ignores identical bytes and stops on consumer identity drift', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-serve-watch-'));
  const manifest = join(root, 'consumer.json');
  const initial = '{"commit":"1111111111111111111111111111111111111111"}\n';
  await writeFile(manifest, initial);
  let resolveReceipt!: (receipt: StudioRestartRequiredReceipt) => void;
  const receipt = new Promise<StudioRestartRequiredReceipt>((resolve) => {
    resolveReceipt = resolve;
  });
  const stop = watchConsumerIdentity(
    manifest,
    createHash('sha256').update(initial).digest('hex'),
    resolveReceipt,
  );
  try {
    const unchangedReplacement = join(root, 'unchanged.json');
    await writeFile(unchangedReplacement, initial);
    await rename(unchangedReplacement, manifest);
    const unchanged = await Promise.race([
      receipt.then(() => false),
      new Promise<true>((resolve) => setTimeout(() => resolve(true), 100)),
    ]);
    assert.equal(unchanged, true);

    const changedReplacement = join(root, 'changed.json');
    await writeFile(changedReplacement, '{"commit":"2222222222222222222222222222222222222222"}\n');
    await rename(changedReplacement, manifest);
    assert.deepEqual(await receipt, {
      kind: 'studioRestartRequired',
      code: 'consumer_identity_changed',
      manifest,
    });
  } finally {
    stop();
    await rm(root, { recursive: true, force: true });
  }
});

test('a crashed detached host cannot orphan a SIGTERM-resistant adapter descendant', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-serve-crash-cleanup-'));
  const descendant = join(root, 'descendant.mjs');
  const host = join(root, 'host.mjs');
  const ready = join(root, 'ready');
  const pidFile = join(root, 'descendant.pid');
  await writeFile(descendant, `
    import { writeFileSync } from 'node:fs';
    process.on('SIGTERM', () => undefined);
    writeFileSync(process.argv[2], 'ready');
    setInterval(() => undefined, 1000);
  `);
  await writeFile(host, `
    import { spawn } from 'node:child_process';
    import { existsSync, writeFileSync } from 'node:fs';
    const child = spawn(process.execPath, [process.argv[2], process.argv[3]], { stdio: 'ignore' });
    writeFileSync(process.argv[4], String(child.pid));
    const waitForReady = () => {
      if (existsSync(process.argv[3])) process.exit(17);
      else setTimeout(waitForReady, 5);
    };
    waitForReady();
  `);
  try {
    const result = await runDetachedHostProcess(
      process.execPath,
      [host, descendant, ready, pidFile],
      root,
      undefined,
      100,
    );
    assert.deepEqual(result, { code: 17, restartRequired: false });
    const descendantPid = Number(await readFile(pidFile, 'utf8'));
    assert.equal(Number.isSafeInteger(descendantPid), true);
    assert.equal(await exists(`/proc/${descendantPid}`), false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
