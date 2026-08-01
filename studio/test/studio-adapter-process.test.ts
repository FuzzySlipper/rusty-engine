import assert from 'node:assert/strict';
import { chmod, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  AdapterProcess,
  StudioAdapterResponseLimitError,
} from '../scripts/studio-adapter-process.js';

test('an unavailable adapter rejects promptly and bounded close completes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-adapter-process-missing-'));
  const adapter = new AdapterProcess(join(root, 'missing-adapter'));
  try {
    await assert.rejects(
      adapter.exchange(JSON.stringify({ requestId: 'missing' })),
      /ENOENT|spawn/u,
    );
    await Promise.race([
      adapter.close(),
      new Promise<never>((_resolve, reject) => {
        setTimeout(() => reject(new Error('adapter close did not complete after spawn failure')), 500);
      }),
    ]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('an oversized response fails one exchange without killing the adapter process', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-adapter-process-'));
  const fixture = join(root, 'fixture-adapter.mjs');
  await writeFile(fixture, `#!/usr/bin/env node
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const request = JSON.parse(line);
  if (request.kind === 'oversized') {
    process.stdout.write(JSON.stringify({ payload: 'x'.repeat(96) }) + '\\n');
    return;
  }
  process.stdout.write(JSON.stringify({ ok: true, requestId: request.requestId }) + '\\n');
});
`);
  await chmod(fixture, 0o755);

  const adapter = new AdapterProcess(fixture, 64);
  try {
    await assert.rejects(
      adapter.exchange(JSON.stringify({ kind: 'oversized', requestId: 'too-large' })),
      (error: unknown) => {
        assert.ok(error instanceof StudioAdapterResponseLimitError);
        assert.equal(error.code, 'studio_adapter_response_too_large');
        assert.equal(error.limitBytes, 64);
        assert.ok(error.actualBytes > error.limitBytes);
        return true;
      },
    );

    assert.deepEqual(
      JSON.parse(await adapter.exchange(JSON.stringify({ kind: 'valid', requestId: 'next' }))),
      { ok: true, requestId: 'next' },
    );
  } finally {
    await adapter.close();
    await rm(root, { recursive: true, force: true });
  }
});

test('an unterminated oversized response rejects promptly and drains before the next exchange', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-adapter-process-unterminated-'));
  const fixture = join(root, 'fixture-adapter.mjs');
  const release = join(root, 'release-oversized-response');
  await writeFile(fixture, `#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const request = JSON.parse(line);
  if (request.kind === 'unterminated-oversized') {
    process.stdout.write('x'.repeat(65));
    const waitForRelease = setInterval(() => {
      if (!existsSync(request.releasePath)) return;
      clearInterval(waitForRelease);
      process.stdout.write('\\n');
    }, 5);
    return;
  }
  process.stdout.write(JSON.stringify({ ok: true, requestId: request.requestId }) + '\\n');
});
`);
  await chmod(fixture, 0o755);

  const adapter = new AdapterProcess(fixture, 64);
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    const oversized = adapter.exchange(JSON.stringify({
      kind: 'unterminated-oversized',
      requestId: 'too-large-without-newline',
      releasePath: release,
    }));
    const promptly = Promise.race([
      oversized,
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(
          () => reject(new Error('oversized unterminated response did not reject promptly')),
          2_000,
        );
      }),
    ]);
    await assert.rejects(promptly, (error: unknown) => {
      assert.ok(error instanceof StudioAdapterResponseLimitError);
      assert.equal(error.limitBytes, 64);
      assert.equal(error.actualBytes, 65);
      return true;
    });
    clearTimeout(timeout);
    timeout = undefined;

    const next = adapter.exchange(JSON.stringify({ kind: 'valid', requestId: 'after-drain' }));
    await writeFile(release, 'release');
    assert.deepEqual(JSON.parse(await next), { ok: true, requestId: 'after-drain' });
  } finally {
    clearTimeout(timeout);
    await writeFile(release, 'release').catch(() => undefined);
    await adapter.close();
    await rm(root, { recursive: true, force: true });
  }
});
