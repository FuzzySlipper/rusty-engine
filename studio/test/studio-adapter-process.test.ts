import assert from 'node:assert/strict';
import { chmod, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  AdapterProcess,
  StudioAdapterResponseLimitError,
} from '../scripts/studio-adapter-process.js';

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
    adapter.close();
    await rm(root, { recursive: true, force: true });
  }
});
