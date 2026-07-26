import assert from 'node:assert/strict';
import { mkdtemp, mkdir, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { listStudioHostDirectory } from '../scripts/studio-host-files-service.js';

void test('trusted host directory listing navigates, filters, sorts, and excludes symlinks', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-host-files-'));
  await Promise.all([
    mkdir(join(root, 'nested')),
    writeFile(join(root, 'scene.project.json'), '{}'),
    writeFile(join(root, 'mesh.glb'), 'glb'),
    writeFile(join(root, 'notes.txt'), 'notes'),
  ]);
  await symlink(join(root, 'notes.txt'), join(root, 'linked.txt'));

  const readout = await listStudioHostDirectory({ directory: root, extensions: ['.json', '.glb'] });

  assert.deepEqual(readout.entries.map((entry) => [entry.kind, entry.name]), [
    ['directory', 'nested'],
    ['file', 'mesh.glb'],
    ['file', 'scene.project.json'],
  ]);
  assert.equal(readout.parent, tmpdir());
  assert.equal(readout.truncated, false);
  await assert.rejects(
    () => listStudioHostDirectory({ directory: join(root, 'linked.txt') }),
    /Symbolic links are not accepted/,
  );
});
