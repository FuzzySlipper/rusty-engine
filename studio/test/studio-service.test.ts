import assert from 'node:assert/strict';
import { lstat, mkdtemp, readlink, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  classifyUpdate,
  promoteRelease,
  requireCleanCheckout,
  serviceCommand,
} from '../scripts/studio-service.js';

const HEAD = '1'.repeat(40);
const UPSTREAM = '2'.repeat(40);

test('service command accepts direct and pnpm separator argv shapes', () => {
  assert.equal(serviceCommand(['install']), 'install');
  assert.equal(serviceCommand(['--', 'install']), 'install');
  assert.equal(serviceCommand(['--']), undefined);
});

test('update admission rejects dirty and divergent source while accepting fast-forward only', () => {
  requireCleanCheckout('');
  assert.throws(() => requireCleanCheckout(' M AGENTS.md\n'), /dirty_checkout/u);
  assert.equal(classifyUpdate(HEAD, HEAD, true), 'current');
  assert.equal(classifyUpdate(HEAD, UPSTREAM, true), 'fast-forward');
  assert.throws(() => classifyUpdate(HEAD, UPSTREAM, false), /not_fast_forward/u);
});

test('failed candidate smoke preserves current and successful promotion records rollback', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-service-'));
  const first = join(root, 'releases', HEAD);
  const second = join(root, 'releases', UPSTREAM);
  try {
    await promoteRelease(root, first, async () => undefined);
    assert.equal(await readlink(join(root, 'current')), first);
    await assert.rejects(
      promoteRelease(root, second, async () => { throw new Error('candidate failed'); }),
      /candidate failed/u,
    );
    assert.equal(await readlink(join(root, 'current')), first);
    await promoteRelease(root, second, async () => undefined);
    assert.equal(await readlink(join(root, 'current')), second);
    assert.equal(await readlink(join(root, 'previous')), first);
    assert.equal((await lstat(join(root, 'current'))).isSymbolicLink(), true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
