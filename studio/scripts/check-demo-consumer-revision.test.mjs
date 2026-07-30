import assert from 'node:assert/strict';
import test from 'node:test';

import { certifyDemoConsumerRevision } from './check-demo-consumer-revision.mjs';

const CONSUMER_COMMIT = 'fbcd352abf09661a5066c5f622140ac244321ebb';
const ENGINE_COMMIT = 'af8d34c7fef57965fa27f9b14b08e2a45c357439';

function consumerPin(overrides = {}) {
  return {
    schemaVersion: 1,
    repository: 'FuzzySlipper/rusty-engine-demo',
    publicRepository: 'https://github.com/FuzzySlipper/rusty-engine-demo',
    commit: CONSUMER_COMMIT,
    engineCommit: ENGINE_COMMIT,
    projectFile: 'content/projects/loading-bay.project.json',
    voxelProjectFile: 'content/projects/converted-wall.project.json',
    cargoPackage: 'loading-bay-game',
    adapterBinary: 'studio-adapter',
    studioApplication: 'apps/loading-bay-studio',
    entityInspectorConsumer: {
      componentTypeId: 'rusty-engine-demo.loading-bay.weapon',
      contractId: 'rusty-engine-demo.loading-bay.weapon-authoring',
      contractVersion: 1,
    },
    ...overrides,
  };
}

function engineSource(overrides = {}) {
  return {
    schemaVersion: 1,
    repository: 'https://github.com/FuzzySlipper/rusty-engine',
    commit: ENGINE_COMMIT,
    ...overrides,
  };
}

test('certifies the exact reviewed consumer and provider pair', () => {
  assert.deepEqual(
    certifyDemoConsumerRevision(consumerPin(), engineSource()),
    {
      kind: 'studioDemoRevisionPreflight',
      consumerRepository: 'FuzzySlipper/rusty-engine-demo',
      consumerCommit: CONSUMER_COMMIT,
      engineRepository: 'https://github.com/FuzzySlipper/rusty-engine',
      engineCommit: ENGINE_COMMIT,
    },
  );
});

test('rejects drift between the Engine reverse pin and consumer manifest', () => {
  const observed = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  assert.throws(
    () => certifyDemoConsumerRevision(consumerPin(), engineSource({ commit: observed })),
    new RegExp(`${ENGINE_COMMIT}.*${observed}`, 'u'),
  );
});

test('rejects floating commits on both sides of the reverse certification', () => {
  assert.throws(
    () => certifyDemoConsumerRevision(consumerPin({ engineCommit: 'main' }), engineSource()),
    /pin engineCommit must be one lowercase 40-character commit/u,
  );
  assert.throws(
    () => certifyDemoConsumerRevision(consumerPin(), engineSource({ commit: 'main' })),
    /engine-source\.json commit must be one lowercase 40-character commit/u,
  );
});

test('rejects a noncanonical provider or changed product target', () => {
  assert.throws(
    () => certifyDemoConsumerRevision(
      consumerPin(),
      engineSource({ repository: 'https://example.invalid/rusty-engine' }),
    ),
    /unsupported Engine repository identity/u,
  );
  assert.throws(
    () => certifyDemoConsumerRevision(
      consumerPin({ projectFile: 'content/projects/other.project.json' }),
      engineSource(),
    ),
    /unsupported integration target/u,
  );
});
