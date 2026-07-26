import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import test from 'node:test';

import { readStudioRenderResource } from '../scripts/studio-render-resource-service.js';

test('render resources require an in-project regular file with the admitted hash', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-render-resource-'));
  try {
    const directory = join(root, 'content', 'assets');
    await mkdir(directory, { recursive: true });
    const bytes = Buffer.from('bounded animated GLB fixture');
    await writeFile(join(directory, 'character.glb'), bytes);
    const contentHash = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;

    assert.deepEqual(await readStudioRenderResource({
      projectRoot: root,
      sourcePath: 'content/assets/character.glb',
      contentHash,
    }), bytes);

    await assert.rejects(
      readStudioRenderResource({
        projectRoot: root,
        sourcePath: 'content/assets/character.glb',
        contentHash: `sha256:${'0'.repeat(64)}`,
      }),
      /does not match/u,
    );
    await assert.rejects(
      readStudioRenderResource({
        projectRoot: root,
        sourcePath: '../character.glb',
        contentHash,
      }),
      /project-relative GLB/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('render resources reject symbolic links in the trusted path', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-render-symlink-'));
  try {
    const outside = await mkdtemp(join(tmpdir(), 'rusty-studio-render-outside-'));
    try {
      const bytes = Buffer.from('outside bytes');
      const outsideFile = join(outside, 'character.glb');
      await writeFile(outsideFile, bytes);
      await mkdir(join(root, 'content'), { recursive: true });
      await symlink(outside, join(root, 'content', 'assets'));
      const contentHash = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;

      await assert.rejects(
        readStudioRenderResource({
          projectRoot: root,
          sourcePath: 'content/assets/character.glb',
          contentHash,
        }),
        /Symbolic links/u,
      );
    } finally {
      await rm(outside, { recursive: true, force: true });
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
