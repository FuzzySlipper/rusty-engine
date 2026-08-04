import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  STUDIO_ADAPTER_OPERATIONS,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
} from '../libs/adapter-client/src/index.js';
import {
  StudioAdapterHost,
  STUDIO_ROOT_BOOTSTRAP_FILE,
} from '../scripts/studio-adapter-host.js';

test('generic host discovers a root-local adapter and preserves the old session on switch failure', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-adapter-host-'));
  const firstRoot = join(root, 'first');
  const brokenRoot = join(root, 'broken');
  await Promise.all([
    writeFixtureRoot(firstRoot, 'fixture.first'),
    mkdir(brokenRoot, { recursive: true }).then(() => writeFile(join(brokenRoot, STUDIO_ROOT_BOOTSTRAP_FILE), JSON.stringify({
      schemaVersion: 1,
      adapter: { command: ['./missing-adapter.mjs'], cwd: '.' },
    }))),
  ]);
  const host = await StudioAdapterHost.create({ adapterBinary: undefined, managedIdentity: null });
  try {
    assert.equal(host.status(), null);
    await assert.rejects(
      host.exchange(JSON.stringify({ type: 'describe', protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION })),
      /studio_adapter_not_selected/u,
    );

    await host.selectProject(firstRoot, 'content/projects/first.project.json');
    const status = host.status();
    assert.ok(status);
    assert.equal(status.mode, 'unmanaged');
    assert.equal(status.activeProjectRoot, null);
    assert.equal(status.runningAdapter.adapterId, 'fixture.first');
    assert.match(
      await host.exchange(JSON.stringify({ type: 'describe', protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION })),
      /fixture\.first/u,
    );

    await assert.rejects(
      host.selectProject(brokenRoot, 'content/projects/broken.project.json'),
      /studio_adapter_bootstrap_command_not_file/u,
    );
    const preserved = host.status();
    assert.ok(preserved);
    assert.equal(preserved.runningAdapter.adapterId, 'fixture.first');
  } finally {
    await host.close();
    await rm(root, { recursive: true, force: true });
  }
});

test('generic host reports a typed missing bootstrap without creating an adapter', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-adapter-host-missing-'));
  const host = await StudioAdapterHost.create({ adapterBinary: undefined, managedIdentity: null });
  try {
    await assert.rejects(
      host.selectProject(root, 'content/projects/missing.project.json'),
      /studio_adapter_bootstrap_missing/u,
    );
    assert.equal(host.status(), null);
  } finally {
    await host.close();
    await rm(root, { recursive: true, force: true });
  }
});

async function writeFixtureRoot(root: string, adapterId: string): Promise<void> {
  await mkdir(root, { recursive: true });
  const adapter = join(root, 'fixture-adapter.mjs');
  await writeFile(adapter, `#!/usr/bin/env node
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const request = JSON.parse(line);
  process.stdout.write(JSON.stringify({
    type: 'described',
    protocolVersion: ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)},
    requestId: request.requestId ?? 'fixture',
    adapter: {
      adapterId: ${JSON.stringify(adapterId)},
      adapterVersion: 1,
      protocolVersion: ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)},
      projectKind: 'fixture',
      projectSchemaVersion: 1,
      operations: ${JSON.stringify(STUDIO_ADAPTER_OPERATIONS)},
      entityInspectorContracts: [],
    },
  }) + '\\n');
});
`);
  await chmod(adapter, 0o755);
  await writeFile(join(root, STUDIO_ROOT_BOOTSTRAP_FILE), JSON.stringify({
    schemaVersion: 1,
    adapter: { command: ['./fixture-adapter.mjs'], cwd: '.' },
  }));
}
