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
const standardCapabilities = JSON.parse(execFileSync(
  'cargo',
  ['run', '--quiet', '-p', 'runtime-standard-capabilities', '--bin', 'export-runtime-standard-capabilities-contract', '--locked'],
  { cwd: repositoryRoot, encoding: 'utf8' },
));
validateDescriptor(descriptor);
validateStandardCapabilities(standardCapabilities);
const output = render(descriptor, standardCapabilities);
if (mode === '--write') {
  writeFileSync(outputUrl, output);
} else if (readFileSync(outputUrl, 'utf8') !== output) {
  throw new Error('generated Product Model contract is stale; run pnpm run generate');
}

function validateDescriptor(value) {
  exactKeys(value, ['artifact', 'capabilityCatalog', 'capabilityTargets', 'failures', 'fields', 'identity', 'input', 'limits', 'numberEncoding', 'optionalFields', 'ordering', 'schedule'], '$');
  if (value.artifact !== 'compiled-composition') throw new Error('unexpected product-model descriptor artifact');
  exactKeys(value.capabilityTargets, ['namespaces', 'separator'], '$.capabilityTargets');
  if (!Array.isArray(value.capabilityTargets.namespaces) || value.capabilityTargets.namespaces.join(',') !== 'engine,kernel' || value.capabilityTargets.separator !== '.') throw new Error('unexpected capability target contract');
  validateCapabilityCatalog(value.capabilityCatalog);
  exactKeys(value.fields, ['capabilityBinding', 'compiledComposition', 'gameplayDefinition', 'inputMap', 'intentDescriptor', 'schedule', 'scheduleCadence', 'scheduleSystem', 'timeline', 'timelineStep'], '$.fields');
  const fields = value.fields;
  assertArray(fields.compiledComposition, ['product', 'intentDescriptors', 'inputMap', 'schedule', 'gameplayDefinitions', 'timelines', 'capabilityBindings'], '$.fields.compiledComposition');
  assertArray(fields.intentDescriptor, ['id', 'valueKind', 'payloadContract', 'capability', 'payload'], '$.fields.intentDescriptor');
  assertArray(fields.inputMap, ['id', 'intent', 'trigger'], '$.fields.inputMap');
  assertArray(fields.schedule, ['phase', 'mode', 'systems', 'before', 'after'], '$.fields.schedule');
  assertArray(fields.scheduleSystem, ['id', 'capability', 'definition', 'after', 'reads', 'writes', 'cadence', 'payload'], '$.fields.scheduleSystem');
  assertArray(fields.scheduleCadence, ['everySteps', 'offsetSteps'], '$.fields.scheduleCadence');
  assertArray(fields.gameplayDefinition, ['id', 'payload'], '$.fields.gameplayDefinition');
  assertArray(fields.timeline, ['id', 'steps'], '$.fields.timeline');
  assertArray(fields.timelineStep, ['id', 'capability', 'payload'], '$.fields.timelineStep');
  assertArray(fields.capabilityBinding, ['id', 'target'], '$.fields.capabilityBinding');
  exactKeys(value.identity, ['alphabet', 'forbidAdjacentSeparators', 'maximumBytes', 'startsAndEndsAlphanumeric'], '$.identity');
  if (value.identity.alphabet !== 'lowercase-ascii-alphanumeric-dot-underscore-hyphen' || value.identity.forbidAdjacentSeparators !== true || value.identity.maximumBytes !== 128 || value.identity.startsAndEndsAlphanumeric !== true) throw new Error('unexpected identity contract');
  exactKeys(value.limits, ['maximumCapabilityBindings', 'maximumDirectIntentProductPayloadBytes', 'maximumEncodedBytes', 'maximumGameplayDefinitions', 'maximumInputChordControls', 'maximumInputMapEntries', 'maximumIntentDescriptors', 'maximumOpaqueJsonArrayEntries', 'maximumOpaqueJsonDepth', 'maximumOpaqueJsonNodes', 'maximumOpaqueJsonObjectEntries', 'maximumOpaqueJsonStringBytes', 'maximumSafeJsonInteger', 'maximumScheduleAccessDeclarations', 'maximumScheduleDependencies', 'maximumScheduleEntries', 'maximumTimelineSteps', 'maximumTimelines', 'schedulePhaseCount'], '$.limits');
  for (const [key, entry] of Object.entries(value.limits)) if (!Number.isSafeInteger(entry) || entry <= 0) throw new Error(`invalid numeric limit ${key}`);
  exactKeys(value.numberEncoding, ['finiteBinary64', 'integer', 'negativeZero'], '$.numberEncoding');
  if (value.numberEncoding.finiteBinary64 !== 'ecmascript-number-to-string' || value.numberEncoding.negativeZero !== '0' || value.numberEncoding.integer !== 'base10') throw new Error('unexpected canonical number contract');
  exactKeys(value.optionalFields, ['intentDescriptor', 'scheduleSystem'], '$.optionalFields');
  assertArray(value.optionalFields.intentDescriptor, ['payloadContract', 'capability'], '$.optionalFields.intentDescriptor');
  assertArray(value.optionalFields.scheduleSystem, ['definition'], '$.optionalFields.scheduleSystem');
  exactKeys(value.schedule, ['defaultCadence', 'modes', 'phases', 'placements'], '$.schedule');
  assertArray(value.schedule.phases, ['input', 'simulation', 'consequences', 'commit', 'projection'], '$.schedule.phases');
  assertArray(value.schedule.modes, ['append', 'prepend', 'extend', 'replace'], '$.schedule.modes');
  assertArray(value.schedule.placements, ['append', 'prepend', 'extend-before', 'extend-after', 'replace'], '$.schedule.placements');
  exactKeys(value.schedule.defaultCadence, ['everySteps', 'offsetSteps'], '$.schedule.defaultCadence');
  if (value.schedule.defaultCadence.everySteps !== 1 || value.schedule.defaultCadence.offsetSteps !== 0) throw new Error('unexpected default schedule cadence');
  exactKeys(value.ordering, ['capabilityBindings', 'gameplayDefinitions', 'inputMap', 'intentDescriptors', 'opaqueArrays', 'opaqueObjectKeys', 'schedule', 'scheduleAfter', 'scheduleReads', 'scheduleSystems', 'scheduleWrites', 'timelineSteps', 'timelines'], '$.ordering');
  if (Object.values(value.ordering).some((entry) => entry !== 'authored' && entry !== 'canonical-bytewise' && entry !== 'canonical-phases')) throw new Error('unexpected ordering contract');
  if (!Array.isArray(value.failures) || value.failures.some((entry) => typeof entry !== 'string')) throw new Error('invalid failure vocabulary');
  exactKeys(value.input, ['axes', 'controllerAxes', 'controllerButtons', 'edges', 'intentValueKinds', 'keyboardControls', 'pointerButtons', 'triggerKinds'], '$.input');
  assertArray(value.input.intentValueKinds, ['digital', 'axis', 'product-payload'], '$.input.intentValueKinds');
  assertArray(value.input.edges, ['held', 'pressed', 'released'], '$.input.edges');
  assertArray(value.input.triggerKinds, ['key', 'pointer-button', 'pointer-axis', 'wheel', 'controller-button', 'controller-axis'], '$.input.triggerKinds');
  if ([value.input.axes, value.input.keyboardControls, value.input.pointerButtons, value.input.controllerButtons, value.input.controllerAxes].some((list) => !Array.isArray(list) || list.some((entry) => typeof entry !== 'string'))) throw new Error('invalid input vocabulary');
  if (JSON.stringify(value).includes('version')) throw new Error('Product Model descriptor must not introduce version fields');
}

function validateCapabilityCatalog(value) {
  exactKeys(value, ['engine', 'kinds'], '$.capabilityCatalog');
  assertArray(value.kinds, ['system', 'operation', 'query', 'projection', 'migration'], '$.capabilityCatalog.kinds');
  if (!Array.isArray(value.engine)) throw new Error('$.capabilityCatalog.engine must be an array');
  const targets = value.engine.map((entry, index) => {
    const path = `$.capabilityCatalog.engine[${String(index)}]`;
    exactKeys(entry, ['access', 'availability', 'budget', 'kind', 'provenance', 'target', 'uses'], path);
    if (typeof entry.target !== 'string' || !entry.target.startsWith('engine.')) throw new Error(`${path}.target must be an Engine target`);
    if (!value.kinds.includes(entry.kind)) throw new Error(`${path}.kind is not a closed capability kind`);
    if (entry.availability !== 'linkable' && entry.availability !== 'unavailable') throw new Error(`${path}.availability is invalid`);
    if (!Array.isArray(entry.uses) || entry.uses.some((use) => !['input-map', 'schedule', 'timeline'].includes(use))) throw new Error(`${path}.uses is invalid`);
    exactKeys(entry.access, ['reads', 'writes'], `${path}.access`);
    if (!Array.isArray(entry.access.reads) || !Array.isArray(entry.access.writes) || [...entry.access.reads, ...entry.access.writes].some((item) => typeof item !== 'string')) throw new Error(`${path}.access must contain string declarations`);
    exactKeys(entry.budget, ['maximumCompactJsonPayloadBytes'], `${path}.budget`);
    if (!Number.isSafeInteger(entry.budget.maximumCompactJsonPayloadBytes) || entry.budget.maximumCompactJsonPayloadBytes <= 0) throw new Error(`${path}.budget.maximumCompactJsonPayloadBytes is invalid`);
    exactKeys(entry.provenance, ['logicalPath', 'owner', 'source'], `${path}.provenance`);
    if (Object.values(entry.provenance).some((item) => typeof item !== 'string' || item.length === 0)) throw new Error(`${path}.provenance must be complete`);
    return entry.target;
  });
  if (new Set(targets).size !== targets.length) throw new Error('$.capabilityCatalog.engine contains duplicate targets');
  if (JSON.stringify(targets) !== JSON.stringify([...targets].sort())) throw new Error('$.capabilityCatalog.engine targets must be deterministic bytewise order');
}

function validateStandardCapabilities(value) {
  exactKeys(value, ['artifact', 'observePairs'], '$runtimeStandardCapabilities');
  if (value.artifact !== 'runtime-standard-capabilities') throw new Error('unexpected runtime-standard-capabilities descriptor artifact');
  const observePairs = value.observePairs;
  exactKeys(observePairs, ['access', 'kind', 'maximumCompactJsonPayloadBytes', 'payload', 'quotas', 'target'], '$runtimeStandardCapabilities.observePairs');
  if (observePairs.target !== 'engine.runtime.observe-pairs' || observePairs.kind !== 'system') throw new Error('unexpected observe-pairs capability target');
  if (!Number.isSafeInteger(observePairs.maximumCompactJsonPayloadBytes) || observePairs.maximumCompactJsonPayloadBytes <= 0) throw new Error('invalid observe-pairs payload limit');
  exactKeys(observePairs.access, ['reads', 'writes'], '$runtimeStandardCapabilities.observePairs.access');
  assertArray(observePairs.access.reads, ['entity-state.components', 'entity-state.transforms', 'engine-spatial.occlusion'], '$runtimeStandardCapabilities.observePairs.access.reads');
  assertArray(observePairs.access.writes, ['runtime-mutation.operations'], '$runtimeStandardCapabilities.observePairs.access.writes');
  exactKeys(observePairs.payload, ['fields', 'kind', 'quotaFields', 'resultKind', 'visibility'], '$runtimeStandardCapabilities.observePairs.payload');
  assertArray(observePairs.payload.fields, ['kind', 'observerRole', 'targetRole', 'operationBinding', 'operationType', 'quotas'], '$runtimeStandardCapabilities.observePairs.payload.fields');
  assertArray(observePairs.payload.quotaFields, ['observers', 'targets', 'pairs', 'aggregates'], '$runtimeStandardCapabilities.observePairs.payload.quotaFields');
  if (observePairs.payload.kind !== 'engine.runtime.observe-pairs.v1' || observePairs.payload.resultKind !== 'engine.runtime.observe-pairs.result.v1' || observePairs.payload.visibility !== 'center-ray') throw new Error('unexpected observe-pairs closed payload contract');
  exactKeys(observePairs.quotas, ['aggregates', 'observers', 'pairs', 'targets'], '$runtimeStandardCapabilities.observePairs.quotas');
  for (const [name, limit] of Object.entries(observePairs.quotas)) if (!Number.isSafeInteger(limit) || limit <= 0) throw new Error(`invalid observe-pairs quota ${name}`);
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

function render(value, standardCapabilities) {
  return `// Generated from Rust product-model contract descriptor. Do not edit.\n// Runtime standard capability constants are generated from their Rust descriptor.\n\nexport const PRODUCT_MODEL_ARTIFACT = ${JSON.stringify(value.artifact)} as const;\nexport const PRODUCT_MODEL_CAPABILITY_TARGETS = ${JSON.stringify(value.capabilityTargets, null, 2)} as const;\nexport const PRODUCT_MODEL_CAPABILITY_CATALOG = ${JSON.stringify(value.capabilityCatalog, null, 2)} as const;\nexport type EngineCapabilityTarget = typeof PRODUCT_MODEL_CAPABILITY_CATALOG.engine[number]['target'];\nexport type EngineCapabilityName = EngineCapabilityTarget extends \`engine.\${infer Name}\` ? Name : never;\nexport const RUNTIME_STANDARD_CAPABILITIES = ${JSON.stringify(standardCapabilities, null, 2)} as const;\nexport const PRODUCT_MODEL_FIELDS = ${JSON.stringify(value.fields, null, 2)} as const;\nexport const PRODUCT_MODEL_IDENTITY = ${JSON.stringify(value.identity, null, 2)} as const;\nexport const PRODUCT_MODEL_INPUT = ${JSON.stringify(value.input, null, 2)} as const;\nexport const PRODUCT_MODEL_LIMITS = ${JSON.stringify(value.limits, null, 2)} as const;\nexport const PRODUCT_MODEL_NUMBER_ENCODING = ${JSON.stringify(value.numberEncoding, null, 2)} as const;\nexport const PRODUCT_MODEL_OPTIONAL_FIELDS = ${JSON.stringify(value.optionalFields, null, 2)} as const;\nexport const PRODUCT_MODEL_ORDERING = ${JSON.stringify(value.ordering, null, 2)} as const;\nexport const PRODUCT_MODEL_SCHEDULE = ${JSON.stringify(value.schedule, null, 2)} as const;\nexport const PRODUCT_MODEL_FAILURES = ${JSON.stringify(value.failures, null, 2)} as const;\n`;
}
