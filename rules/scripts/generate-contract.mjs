import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const mode = process.argv[2];
if (mode !== '--check' && mode !== '--write') {
  throw new Error('usage: node scripts/generate-contract.mjs --check|--write');
}

const repositoryRoot = fileURLToPath(new URL('../../', import.meta.url));
const generatedUrl = new URL(
  '../packages/gameplay-rules-contracts/src/generated.ts',
  import.meta.url,
);
const result = spawnSync(
  'cargo',
  [
    'run',
    '--quiet',
    '-p',
    'gameplay-rules',
    '--bin',
    'export-gameplay-rules-contract',
  ],
  {
    cwd: repositoryRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  },
);
if (result.status !== 0) {
  throw new Error(`Rust gameplay-rules contract export failed with status ${String(result.status)}`);
}

const descriptor = JSON.parse(result.stdout);
validateDescriptor(descriptor);
const generated = renderDescriptor(descriptor);

if (mode === '--write') {
  writeFileSync(generatedUrl, generated);
  console.log(`wrote ${fileURLToPath(generatedUrl)}`);
} else {
  const current = readFileSync(generatedUrl, 'utf8');
  if (current !== generated) {
    throw new Error(
      'generated gameplay-rules contract drifted; run pnpm --dir rules run generate',
    );
  }
  console.log('generated gameplay-rules contract matches Rust owner');
}

function validateDescriptor(value) {
  if (!isRecord(value) || value.contractVersion !== 2) {
    throw new Error('unsupported Rust gameplay-rules contract descriptor');
  }
  for (const key of [
    'artifactKind',
    'schemaVersion',
    'binary64SchemaVersion',
    'brands',
    'unions',
    'records',
    'limits',
    'fieldOrder',
  ]) {
    if (!(key in value)) {
      throw new Error(`Rust gameplay-rules contract descriptor is missing ${key}`);
    }
  }
}

function renderDescriptor(descriptor) {
  const lines = [
    '// Generated from the Rust gameplay-rules contract. Do not edit by hand.',
    '// Run: pnpm --dir rules run generate',
    '',
    `export const RULE_CONTRACT_DESCRIPTOR_VERSION = ${String(descriptor.contractVersion)} as const;`,
    `export const RULE_PACKAGE_ARTIFACT_KIND = ${JSON.stringify(descriptor.artifactKind)} as const;`,
    `export const RULE_PACKAGE_SCHEMA_VERSION = ${String(descriptor.schemaVersion)} as const;`,
    `export const RULE_PACKAGE_BINARY64_SCHEMA_VERSION = ${String(descriptor.binary64SchemaVersion)} as const;`,
    '',
    'export const RULE_LIMITS = Object.freeze({',
  ];
  for (const [name, value] of Object.entries(descriptor.limits)) {
    lines.push(`  ${name}: ${String(value)},`);
  }
  lines.push('} as const);', '');

  lines.push('export const RULE_FIELD_ORDER = Object.freeze({');
  for (const [name, fields] of Object.entries(descriptor.fieldOrder)) {
    lines.push(`  ${name}: Object.freeze(${JSON.stringify(fields)} as const),`);
  }
  lines.push('} as const);', '');

  lines.push(
    'export type JsonPrimitive = null | boolean | number | string;',
    'export type JsonValue = JsonPrimitive | readonly JsonValue[] | { readonly [key: string]: JsonValue };',
    '',
  );

  for (const brand of descriptor.brands) {
    lines.push(
      `declare const ${brand}Brand: unique symbol;`,
      `export type ${brand} = string & { readonly [${brand}Brand]: true };`,
      '',
    );
  }

  for (const union of descriptor.unions) {
    lines.push(
      `export type ${union.name} = ${union.values
        .map((value) => JSON.stringify(value))
        .join(' | ')};`,
      '',
    );
  }

  for (const record of descriptor.records) {
    const typeParameter =
      typeof record.typeParameter === 'string' ? `<${record.typeParameter}>` : '';
    lines.push(`export interface ${record.name}${typeParameter} {`);
    for (const field of record.fields) {
      lines.push(
        `  readonly ${field.name}${field.optional === true ? '?' : ''}: ${field.type};`,
      );
    }
    lines.push('}', '');
  }

  while (lines.at(-1) === '') lines.pop();
  return `${lines.join('\n')}\n`;
}

function isRecord(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
