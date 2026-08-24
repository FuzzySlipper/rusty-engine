import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const mode = process.argv[2];
if (mode !== '--write' && mode !== '--check') {
  throw new Error('usage: generate-product-model-contract.mjs --write|--check');
}

const repositoryRoot = new URL('../../', import.meta.url);
const outputUrl = new URL(
  '../packages/runtime-composition-authoring/src/generated.ts',
  import.meta.url,
);
const descriptor = JSON.parse(execFileSync(
  'cargo',
  ['run', '--quiet', '-p', 'product-model', '--bin', 'export-product-model-contract', '--locked'],
  { cwd: repositoryRoot, encoding: 'utf8' },
));
validateDescriptor(descriptor);
const output = render(descriptor);
if (mode === '--write') {
  writeFileSync(outputUrl, output);
} else if (readFileSync(outputUrl, 'utf8') !== output) {
  throw new Error('generated Product Model contract is stale; run pnpm run generate');
}

function validateDescriptor(value) {
  exactKeys(value, ['artifact', 'capabilityTargets', 'failures', 'fields', 'identity', 'limits', 'numberEncoding', 'optionalFields', 'ordering'], '$');
  if (value.artifact !== 'compiled-composition') throw new Error('unexpected product-model descriptor artifact');
  exactKeys(value.capabilityTargets, ['namespaces', 'separator'], '$.capabilityTargets');
  if (!Array.isArray(value.capabilityTargets.namespaces) || value.capabilityTargets.namespaces.join(',') !== 'engine,kernel' || value.capabilityTargets.separator !== '.') throw new Error('unexpected capability target contract');
  exactKeys(value.fields, ['capabilityBinding', 'compiledComposition', 'gameplayDefinition', 'inputMap', 'schedule', 'timeline', 'timelineStep'], '$.fields');
  const fields = value.fields;
  assertArray(fields.compiledComposition, ['product', 'inputMap', 'schedule', 'gameplayDefinitions', 'timelines', 'capabilityBindings'], '$.fields.compiledComposition');
  assertArray(fields.inputMap, ['id', 'intent', 'capability', 'payload'], '$.fields.inputMap');
  assertArray(fields.schedule, ['id', 'phase', 'capability', 'definition', 'reads', 'writes', 'payload'], '$.fields.schedule');
  assertArray(fields.gameplayDefinition, ['id', 'payload'], '$.fields.gameplayDefinition');
  assertArray(fields.timeline, ['id', 'steps'], '$.fields.timeline');
  assertArray(fields.timelineStep, ['id', 'capability', 'payload'], '$.fields.timelineStep');
  assertArray(fields.capabilityBinding, ['id', 'target'], '$.fields.capabilityBinding');
  exactKeys(value.identity, ['alphabet', 'forbidAdjacentSeparators', 'maximumBytes', 'startsAndEndsAlphanumeric'], '$.identity');
  if (value.identity.alphabet !== 'lowercase-ascii-alphanumeric-dot-underscore-hyphen' || value.identity.forbidAdjacentSeparators !== true || value.identity.maximumBytes !== 128 || value.identity.startsAndEndsAlphanumeric !== true) throw new Error('unexpected identity contract');
  exactKeys(value.limits, ['maximumCapabilityBindings', 'maximumEncodedBytes', 'maximumGameplayDefinitions', 'maximumInputMapEntries', 'maximumOpaqueJsonArrayEntries', 'maximumOpaqueJsonDepth', 'maximumOpaqueJsonNodes', 'maximumOpaqueJsonObjectEntries', 'maximumOpaqueJsonStringBytes', 'maximumSafeJsonInteger', 'maximumScheduleAccessDeclarations', 'maximumScheduleEntries', 'maximumTimelineSteps', 'maximumTimelines'], '$.limits');
  for (const [key, entry] of Object.entries(value.limits)) if (!Number.isSafeInteger(entry) || entry <= 0) throw new Error(`invalid numeric limit ${key}`);
  exactKeys(value.numberEncoding, ['finiteBinary64', 'integer', 'negativeZero'], '$.numberEncoding');
  if (value.numberEncoding.finiteBinary64 !== 'ecmascript-number-to-string' || value.numberEncoding.negativeZero !== '0' || value.numberEncoding.integer !== 'base10') throw new Error('unexpected canonical number contract');
  exactKeys(value.optionalFields, ['schedule'], '$.optionalFields');
  assertArray(value.optionalFields.schedule, ['definition'], '$.optionalFields.schedule');
  exactKeys(value.ordering, ['capabilityBindings', 'gameplayDefinitions', 'inputMap', 'opaqueArrays', 'opaqueObjectKeys', 'schedule', 'scheduleReads', 'scheduleWrites', 'timelineSteps', 'timelines'], '$.ordering');
  if (Object.values(value.ordering).some((entry) => entry !== 'authored' && entry !== 'canonical-bytewise')) throw new Error('unexpected ordering contract');
  if (!Array.isArray(value.failures) || value.failures.some((entry) => typeof entry !== 'string')) throw new Error('invalid failure vocabulary');
  if (JSON.stringify(value).includes('version')) throw new Error('Product Model descriptor must not introduce version fields');
}

function exactKeys(value, expected, path) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${path} must be an object`);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) throw new Error(`${path} fields drifted: ${JSON.stringify(actual)}`);
}

function assertArray(actual, expected, path) {
  if (!Array.isArray(actual) || JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${path} drifted`);
}

function render(value) {
  return `// Generated from Rust product-model contract descriptor. Do not edit.\n\nexport const PRODUCT_MODEL_ARTIFACT = ${JSON.stringify(value.artifact)} as const;\nexport const PRODUCT_MODEL_CAPABILITY_TARGETS = ${JSON.stringify(value.capabilityTargets, null, 2)} as const;\nexport const PRODUCT_MODEL_FIELDS = ${JSON.stringify(value.fields, null, 2)} as const;\nexport const PRODUCT_MODEL_IDENTITY = ${JSON.stringify(value.identity, null, 2)} as const;\nexport const PRODUCT_MODEL_LIMITS = ${JSON.stringify(value.limits, null, 2)} as const;\nexport const PRODUCT_MODEL_NUMBER_ENCODING = ${JSON.stringify(value.numberEncoding, null, 2)} as const;\nexport const PRODUCT_MODEL_OPTIONAL_FIELDS = ${JSON.stringify(value.optionalFields, null, 2)} as const;\nexport const PRODUCT_MODEL_ORDERING = ${JSON.stringify(value.ordering, null, 2)} as const;\nexport const PRODUCT_MODEL_FAILURES = ${JSON.stringify(value.failures, null, 2)} as const;\n`;
}
