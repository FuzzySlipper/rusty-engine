import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const DEMO_REPOSITORY = 'FuzzySlipper/rusty-engine-demo';
const DEMO_PUBLIC_REPOSITORY = 'https://github.com/FuzzySlipper/rusty-engine-demo';
const ENGINE_REPOSITORY = 'https://github.com/FuzzySlipper/rusty-engine';
const EXACT_COMMIT = /^[0-9a-f]{40}$/u;

export function certifyDemoConsumerRevision(pin, engineSource) {
  requireRecord(pin, 'demo consumer pin');
  if (
    pin.schemaVersion !== 1
    || pin.repository !== DEMO_REPOSITORY
    || pin.publicRepository !== DEMO_PUBLIC_REPOSITORY
  ) {
    throw new Error('demo consumer pin has an unsupported repository identity');
  }
  requireCommit(pin.commit, 'demo consumer pin commit');
  requireCommit(pin.engineCommit, 'demo consumer pin engineCommit');
  if (
    pin.projectFile !== 'content/projects/loading-bay.project.json'
    || pin.voxelProjectFile !== 'content/projects/converted-wall.project.json'
    || pin.cargoPackage !== 'loading-bay-game'
    || pin.adapterBinary !== 'studio-adapter'
    || pin.studioApplication !== 'apps/loading-bay-studio'
    || !isRecord(pin.entityInspectorConsumer)
    || pin.entityInspectorConsumer.componentTypeId !== 'rusty-engine-demo.loading-bay.weapon'
    || pin.entityInspectorConsumer.contractId !== 'rusty-engine-demo.loading-bay.weapon-authoring'
    || pin.entityInspectorConsumer.contractVersion !== 1
  ) {
    throw new Error('demo consumer pin has an unsupported integration target');
  }

  requireRecord(engineSource, 'consumer engine-source.json');
  if (engineSource.schemaVersion !== 1 || engineSource.repository !== ENGINE_REPOSITORY) {
    throw new Error('consumer engine-source.json has an unsupported Engine repository identity');
  }
  requireCommit(engineSource.commit, 'consumer engine-source.json commit');
  if (engineSource.commit !== pin.engineCommit) {
    throw new Error(
      `Engine reverse pin mismatch: studio/demo-consumer-source.json records ${pin.engineCommit}, `
      + `but the certified consumer engine-source.json records ${engineSource.commit}`,
    );
  }

  return Object.freeze({
    kind: 'studioDemoRevisionPreflight',
    consumerRepository: pin.repository,
    consumerCommit: pin.commit,
    engineRepository: engineSource.repository,
    engineCommit: engineSource.commit,
  });
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    throw new Error(
      `${label} is not readable JSON: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

function requireRecord(value, label) {
  if (!isRecord(value)) throw new Error(`${label} must be a JSON object`);
}

function isRecord(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function requireCommit(value, label) {
  if (typeof value !== 'string' || !EXACT_COMMIT.test(value)) {
    throw new Error(`${label} must be one lowercase 40-character commit`);
  }
}

function main() {
  const [pinPath, engineSourcePath, outputMode, ...extra] = process.argv.slice(2);
  if (
    pinPath === undefined
    || engineSourcePath === undefined
    || extra.length !== 0
    || (outputMode !== undefined && outputMode !== '--shell-values')
  ) {
    throw new Error(
      'usage: node studio/scripts/check-demo-consumer-revision.mjs '
      + '<demo-consumer-source.json> <consumer-engine-source.json> [--shell-values]',
    );
  }
  const evidence = certifyDemoConsumerRevision(
    readJson(pinPath, 'demo consumer pin'),
    readJson(engineSourcePath, 'consumer engine-source.json'),
  );
  if (outputMode === '--shell-values') {
    process.stdout.write([
      evidence.consumerRepository,
      evidence.consumerCommit,
      evidence.engineCommit,
      JSON.stringify(evidence),
      '',
    ].join('\n'));
    return;
  }
  process.stdout.write(`${JSON.stringify(evidence)}\n`);
}

const invokedPath = process.argv[1] === undefined
  ? null
  : pathToFileURL(resolve(process.argv[1])).href;
if (invokedPath === import.meta.url) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
