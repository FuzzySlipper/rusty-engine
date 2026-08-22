import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const mode = process.argv[2];
if (mode !== '--check' && mode !== '--write') throw new Error('usage: node scripts/generate-standard-contract.mjs --check|--write');
const root = fileURLToPath(new URL('../../', import.meta.url));
const output = new URL('../packages/gameplay-standard-contracts/src/generated.ts', import.meta.url);
const result = spawnSync('cargo', ['run', '--quiet', '-p', 'gameplay-standard', '--bin', 'export-gameplay-standard-contract'], { cwd: root, encoding: 'utf8' });
if (result.status !== 0) throw new Error(`Rust gameplay-standard contract export failed:\n${result.stderr}`);
const descriptor = JSON.parse(result.stdout);
if (descriptor.contractVersion !== 1 || !Array.isArray(descriptor.families) || descriptor.families.length !== 2) throw new Error('unsupported Rust gameplay-standard descriptor');
const exact = descriptor.families.find((family) => family.id === 'exact');
const continuous = descriptor.families.find((family) => family.id === 'continuous');
const composed = descriptor.composedExact;
if (!exact || !continuous || !descriptor.fieldOrder || !descriptor.identities || !descriptor.failures || !descriptor.extensions || !composed) throw new Error('Rust gameplay-standard descriptor is incomplete');

const inputFieldType = (field) => field === 'minimum' || field === 'maximum' ? 'number' : 'string';
const inputUnion = (kinds) => kinds.map(({ tag, fields }) => {
  if (!Array.isArray(fields) || fields[0] !== 'kind') throw new Error(`invalid Rust input table for ${tag}`);
  return `{ readonly kind: ${JSON.stringify(tag)}; ${fields.slice(1).map((field) => `readonly ${field}: ${inputFieldType(field)};`).join(' ')} }`;
}).join('\n  | ');
const binaryOps = (family) => family.operations.filter((op) => !['literal', 'input', 'min', 'max', 'fixedPower'].includes(op));
const fixedPowerArm = (treeName) => `| { readonly op: 'fixedPower'; readonly base: ${treeName}; readonly exponent: ${treeName}; readonly scale: number }`;
const literalFields = (family) => family.literal.field;
const familySource = (family) => `Object.freeze({ schemaVersion: ${family.schemaVersion}, evaluatorSemanticsVersion: ${family.evaluatorSemanticsVersion}, operations: Object.freeze(${JSON.stringify(family.operations)} as const), inputKinds: Object.freeze(${JSON.stringify(family.inputKinds)} as const), literal: Object.freeze(${JSON.stringify(family.literal)} as const) })`;
const generated = `// Generated from the Rust gameplay-standard contract. Do not edit by hand.
// Run: pnpm --dir rules run generate

export const STANDARD_CONTRACT_DESCRIPTOR_VERSION = ${descriptor.contractVersion} as const;
export const STANDARD_FAMILIES = Object.freeze({
  exact: ${familySource(exact)},
  continuous: ${familySource(continuous)},
} as const);
export const STANDARD_FIELD_ORDER = Object.freeze(${JSON.stringify(descriptor.fieldOrder)} as const);
export const STANDARD_IDENTITIES = Object.freeze(${JSON.stringify(descriptor.identities)} as const);
export const STANDARD_FAILURE_CODES = Object.freeze(${JSON.stringify(descriptor.failures)} as const);
export const STANDARD_LIMITS = Object.freeze({
  maxRoleIdBytes: ${descriptor.limits.maxRoleIdBytes},
  maxCapabilitiesPerRole: ${descriptor.limits.maxCapabilitiesPerRole},
  exact: Object.freeze(${JSON.stringify(descriptor.limits.exact)} as const),
  continuous: Object.freeze(${JSON.stringify(descriptor.limits.continuous)} as const),
  maxExtensionBytes: ${descriptor.extensions.maximumBytes},
} as const);
export const STANDARD_EXTENSION = Object.freeze(${JSON.stringify(descriptor.extensions)} as const);
export const STANDARD_COMPOSED_EXACT = Object.freeze(${JSON.stringify(composed)} as const);

export type StandardFamily = keyof typeof STANDARD_FAMILIES;
export type ExactOperation = (typeof STANDARD_FAMILIES.exact.operations)[number];
export type ContinuousOperation = (typeof STANDARD_FAMILIES.continuous.operations)[number];
export type StandardContractErrorCode = (typeof STANDARD_FAILURE_CODES)[number];
export type JsonValue = null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue };
export type ExactInput = ${inputUnion(exact.inputKinds)};
export type ContinuousInput = ${inputUnion(continuous.inputKinds)};
export type ExactTree =
  | { readonly op: 'literal'; readonly ${literalFields(exact)}: number }
  | { readonly op: 'input'; readonly input: ExactInput }
  | { readonly op: ${binaryOps(exact).map(JSON.stringify).join(' | ')}; readonly left: ExactTree; readonly right: ExactTree }
  ${fixedPowerArm('ExactTree')}
  | { readonly op: 'min' | 'max'; readonly values: readonly ExactTree[] };
export type ContinuousTree =
  | { readonly op: 'literal'; readonly ${literalFields(continuous)}: string }
  | { readonly op: 'input'; readonly input: ContinuousInput }
  | { readonly op: ${binaryOps(continuous).map(JSON.stringify).join(' | ')}; readonly left: ContinuousTree; readonly right: ContinuousTree }
  | { readonly op: 'min' | 'max'; readonly values: readonly ContinuousTree[] };
export interface StandardRole { readonly role: string; readonly capabilities: readonly string[] }
export interface ExactDefinitionPayload { readonly family: 'exact'; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_FAMILIES.exact.evaluatorSemanticsVersion; readonly source: string; readonly subject: string; readonly tree: ExactTree }
export interface ContinuousDefinitionPayload { readonly family: 'continuous'; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_FAMILIES.continuous.evaluatorSemanticsVersion; readonly source: string; readonly subject: string; readonly tree: ContinuousTree }
export type StandardDefinitionPayload = ExactDefinitionPayload | ContinuousDefinitionPayload;
export interface StandardExtensionArtifact { readonly family: typeof STANDARD_EXTENSION.family; readonly kind: string; readonly namespace: string; readonly payload: JsonValue; readonly schemaVersion: number; readonly source: string; readonly subject: string }
export interface ComposedExactExtensionSchema { readonly namespace: string; readonly schemaVersion: number }
export type ComposedExactProductLeaf<Payload extends JsonValue> = { readonly op: typeof STANDARD_COMPOSED_EXACT.productOp; readonly kind: string; readonly payload: Payload; readonly source: string; readonly subject: string };
export type ComposedExactTree<Payload extends JsonValue> =
  | { readonly op: 'literal'; readonly ${literalFields(exact)}: number }
  | { readonly op: 'input'; readonly input: ExactInput }
  | { readonly op: ${binaryOps(exact).map(JSON.stringify).join(' | ')}; readonly left: ComposedExactTree<Payload>; readonly right: ComposedExactTree<Payload> }
  ${fixedPowerArm('ComposedExactTree<Payload>')}
  | { readonly op: 'min' | 'max'; readonly values: readonly ComposedExactTree<Payload>[] }
  | ComposedExactProductLeaf<Payload>;
export interface ComposedExactDefinitionPayload<Payload extends JsonValue> { readonly family: typeof STANDARD_COMPOSED_EXACT.family; readonly extension: ComposedExactExtensionSchema; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_COMPOSED_EXACT.semanticsVersion; readonly source: string; readonly subject: string; readonly tree: ComposedExactTree<Payload> }
export class StandardContractError extends Error {
  public constructor(public readonly code: StandardContractErrorCode, message: string) { super(message); this.name = 'StandardContractError'; }
}
`;
if (mode === '--write') writeFileSync(output, generated);
else if (readFileSync(output, 'utf8') !== generated) throw new Error('generated gameplay-standard contract drifted; run pnpm --dir rules run generate');
