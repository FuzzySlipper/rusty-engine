import assert from 'node:assert/strict';
import test from 'node:test';

import { decodeStudioHostStatus } from './host-status.js';

const ENGINE = '1'.repeat(40);
const CONSUMER = '2'.repeat(40);
const BINARY = '3'.repeat(64);

test('decodes and freezes one exact managed Studio host identity', () => {
  const decoded = decodeStudioHostStatus(managedStatus());
  assert.equal(decoded.mode, 'managed');
  assert.equal(decoded.engineSourceCommit, ENGINE);
  assert.equal(decoded.configuredConsumer?.commit, CONSUMER);
  assert.equal(decoded.runningAdapter.buildCommit, CONSUMER);
  assert.equal(decoded.runningAdapter.binarySha256, BINARY);
  assert.ok(Object.isFrozen(decoded));
  assert.ok(Object.isFrozen(decoded.configuredConsumer));
  assert.ok(Object.isFrozen(decoded.runningAdapter));
});

test('rejects consumer/build drift and malformed extra status state', () => {
  const drift = managedStatus();
  drift.runningAdapter.buildCommit = '4'.repeat(40);
  assert.throws(
    () => decodeStudioHostStatus(drift),
    /buildCommit.*configured consumer commit/u,
  );

  const extra = managedStatus() as Record<string, unknown>;
  extra['branch'] = 'main';
  assert.throws(() => decodeStudioHostStatus(extra), /must contain exactly/u);
});

test('unmanaged status cannot claim exact managed source identity', () => {
  const unmanaged = managedStatus();
  unmanaged.mode = 'unmanaged';
  assert.throws(
    () => decodeStudioHostStatus(unmanaged),
    /unmanaged status must not claim managed source identity/u,
  );
});

test('decodes generic interactive status without managed identity', () => {
  const managed = managedStatus();
  const generic = {
    ...managed,
    mode: 'generic' as const,
    engineSourceCommit: null,
    configuredConsumer: null,
    runningAdapter: { ...managed.runningAdapter, buildCommit: null },
  };
  const decoded = decodeStudioHostStatus(generic);
  assert.equal(decoded.mode, 'generic');
  assert.equal(decoded.engineSourceCommit, null);
  assert.equal(decoded.configuredConsumer, null);
  assert.equal(decoded.runningAdapter.buildCommit, null);
});

test('generic rolling-development status may identify Engine without claiming a consumer', () => {
  const managed = managedStatus();
  const generic = {
    ...managed,
    mode: 'generic' as const,
    configuredConsumer: null,
    runningAdapter: { ...managed.runningAdapter, buildCommit: null },
  };
  const decoded = decodeStudioHostStatus(generic);
  assert.equal(decoded.engineSourceCommit, ENGINE);
  assert.equal(decoded.configuredConsumer, null);
});

function managedStatus() {
  return {
    schemaVersion: 1,
    project: 'rusty-engine-studio',
    status: 'ok',
    mode: 'managed',
    engineSourceCommit: ENGINE,
    configuredConsumer: {
      repository: 'https://github.com/FuzzySlipper/studio-test-fixture',
      commit: CONSUMER,
    },
    activeProjectRoot: '/work/studio-test-fixture',
    activeProjectFile: 'content/projects/studio-test-fixture.project.json',
    runningAdapter: {
      adapterId: 'studio.test-fixture',
      adapterVersion: 1,
    protocolVersion: 15,
      buildCommit: CONSUMER,
      binarySha256: BINARY,
    },
  };
}
