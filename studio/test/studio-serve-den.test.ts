import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  watchConsumerIdentity,
  type StudioRestartRequiredReceipt,
} from '../scripts/serve-den.js';

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
