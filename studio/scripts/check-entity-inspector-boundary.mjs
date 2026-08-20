import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studioRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(studioRoot, '..');
const genericFiles = [
  'libs/adapter-client/src/protocol.ts',
  'libs/editor-shell/src/entity-inspector.ts',
  'libs/editor-shell/src/studio-shell.component.ts',
  'libs/editor-shell/src/studio-shell.component.html',
  'apps/studio-app/src/app/app.ts',
];
const genericSources = new Map(genericFiles.map((path) => [
  path,
  readFileSync(join(studioRoot, path), 'utf8'),
]));
const violations = [];

const forbiddenGenericVocabulary = [
  'rusty-engine-demo',
  'LoadingBay',
  'loadingBay',
  'readLoadingBayWeapon',
  'replaceLoadingBayWeapon',
  'readComponent',
  'writeComponent',
  'invokeExtension',
  'extensionPayload',
  'moduleUrl',
];
for (const [path, source] of genericSources) {
  for (const forbidden of forbiddenGenericVocabulary) {
    if (source.includes(forbidden)) {
      violations.push(`${path}: generic inspector surface contains forbidden ${forbidden}`);
    }
  }
  if (/\bimport\s*\(/u.test(source)) {
    violations.push(`${path}: generic inspector surface dynamically imports runtime code`);
  }
}

const inspectorContract = genericSources.get('libs/editor-shell/src/entity-inspector.ts') ?? '';
for (const forbidden of [
  'StudioWorkspaceStore',
  'STUDIO_WORKSPACE',
  'EnvironmentInjector',
  'Injector',
  'ServiceLocator',
  'Record<string, unknown>',
]) {
  if (inspectorContract.includes(forbidden)) {
    violations.push(`entity-inspector.ts exposes forbidden store/service/payload surface ${forbidden}`);
  }
}
for (const [interfaceName, forbiddenFields] of [
  ['StudioEntityInspectorContext', ['payload', 'value', 'store', 'transport', 'execute', 'invoke']],
  ['StudioEntityInspectorContribution', ['payload', 'value', 'module', 'loader', 'registry']],
  ['StudioEntityInspectorPanel', ['store', 'serviceLocator', 'transport', 'execute', 'invoke']],
]) {
  const block = inspectorContract.match(
    new RegExp(`interface ${interfaceName} \\{([\\s\\S]*?)\\n\\}`, 'u'),
  )?.[1] ?? null;
  if (block === null) {
    violations.push(`entity-inspector.ts is missing ${interfaceName}`);
    continue;
  }
  for (const field of forbiddenFields) {
    if (new RegExp(`\\b${field}\\b`, 'iu').test(block)) {
      violations.push(`${interfaceName} exposes forbidden generic field ${field}`);
    }
  }
}
if (!/interface StudioEntityInspectorPanel[\s\S]*?readonly context:[\s\S]*?readonly mutationPort:[\s\S]*?\n\}/u.test(inspectorContract)) {
  violations.push('entity-inspector.ts no longer exposes the bounded context + mutation-port panel contract');
}
if (!/entityInspectorContributions\s*=\s*input<readonly StudioEntityInspectorContribution\[\]>/u.test(
  genericSources.get('libs/editor-shell/src/studio-shell.component.ts') ?? '',
)) {
  violations.push('Studio shell no longer admits one explicit immutable contribution input');
}
if (!(genericSources.get('apps/studio-app/src/app/app.ts') ?? '').includes(
  'RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS',
)) {
  violations.push('stock Studio app no longer composes built-in contributions statically');
}

for (const ordinaryGate of ['scripts/verify.sh', 'scripts/verify-studio.sh']) {
  const source = readFileSync(join(repoRoot, ordinaryGate), 'utf8');
  if (source.includes('rusty-engine-demo')) {
    violations.push(`${ordinaryGate}: ordinary verification depends on the retired demo checkout`);
  }
}

if (violations.length !== 0) {
  for (const violation of violations) console.error(violation);
  process.exit(1);
}

console.log('Studio Entity inspector boundary check passed');
