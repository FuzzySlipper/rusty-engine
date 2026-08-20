import assert from 'node:assert/strict';
import test from 'node:test';

import { HttpStudioHostStatusClient } from './transport.js';

test('reads the exact frozen managed Studio runtime identity', async () => {
  const client = new HttpStudioHostStatusClient('/api/studio-status', async (input, init) => {
    assert.equal(input, '/api/studio-status');
    assert.equal(init.method, 'GET');
    return response(200, {
      schemaVersion: 1,
      project: 'rusty-engine-studio',
      status: 'ok',
      mode: 'managed',
      engineSourceCommit: '1'.repeat(40),
      configuredConsumer: {
        repository: 'https://github.com/FuzzySlipper/studio-test-fixture',
        commit: '2'.repeat(40),
      },
      activeProjectRoot: '/work/studio-test-fixture',
      activeProjectFile: 'content/projects/studio-test-fixture.project.json',
      runningAdapter: {
        adapterId: 'studio.test-fixture',
        adapterVersion: 1,
        protocolVersion: 14,
        buildCommit: '2'.repeat(40),
        binarySha256: '3'.repeat(64),
      },
    });
  });
  const status = await client.read();
  assert.equal(status.engineSourceCommit, '1'.repeat(40));
  assert.equal(status.configuredConsumer?.commit, '2'.repeat(40));
  assert.equal(status.runningAdapter.protocolVersion, 14);
  assert.ok(Object.isFrozen(status));
});

test('rejects a host status whose adapter build does not match its consumer', async () => {
  const client = new HttpStudioHostStatusClient('/api/studio-status', async () => response(200, {
    schemaVersion: 1,
    project: 'rusty-engine-studio',
    status: 'ok',
    mode: 'managed',
    engineSourceCommit: '1'.repeat(40),
    configuredConsumer: {
        repository: 'https://github.com/FuzzySlipper/studio-test-fixture',
      commit: '2'.repeat(40),
    },
      activeProjectRoot: '/work/studio-test-fixture',
      activeProjectFile: 'content/projects/studio-test-fixture.project.json',
    runningAdapter: {
        adapterId: 'studio.test-fixture',
      adapterVersion: 1,
      protocolVersion: 14,
      buildCommit: '4'.repeat(40),
      binarySha256: '3'.repeat(64),
    },
  }));
  await assert.rejects(client.read(), /buildCommit.*configured consumer commit/u);
});

function response(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => JSON.stringify(body),
    headers: new Headers(),
  };
}
