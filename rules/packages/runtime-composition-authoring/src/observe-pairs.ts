import { RuntimeCompositionAuthoringError } from './error.js';
import {
  PRODUCT_MODEL_CAPABILITY_TARGETS,
  PRODUCT_MODEL_IDENTITY,
  RUNTIME_STANDARD_CAPABILITIES,
} from './generated.js';
import { cadence, system } from './author.js';
import type { CapabilityBinding, ScheduleCadence, ScheduleSystem } from './types.js';

/** The only authored quotas accepted by the closed Engine observe-pairs system. */
export interface ObservePairsQuotas {
  readonly observers: number;
  readonly targets: number;
  readonly pairs: number;
  readonly aggregates: number;
}

/**
 * The build-time inputs for one fixed typed observation system.
 *
 * The observer and target role identities name product component facts. Range,
 * facing, origin, target center, and evidence are read from those typed Rust
 * facts at runtime; the author cannot provide predicates, expressions, or
 * callbacks here.
 */
export interface ObservePairsDraft {
  readonly id: string;
  readonly engineBinding: CapabilityBinding;
  readonly operationBinding: CapabilityBinding;
  readonly observerRole: string;
  readonly targetRole: string;
  readonly quotas: ObservePairsQuotas;
  readonly cadence: number | ScheduleCadence | { readonly everySteps: number; readonly offsetSteps?: number };
}

/**
 * Creates the complete simulation system for the Engine-owned, center-ray
 * observe-pairs mechanism. Its payload is a closed wire shape emitted from
 * Rust's descriptor; Product Kernel alert meaning remains in the selected
 * operation binding.
 */
export function observePairs(draft: ObservePairsDraft): ScheduleSystem {
  const source = record(draft, '$.observePairs');
  known(source, ['id', 'engineBinding', 'operationBinding', 'observerRole', 'targetRole', 'quotas', 'cadence'], '$.observePairs');
  const contract = RUNTIME_STANDARD_CAPABILITIES.observePairs;
  const engineBinding = binding(required(source, 'engineBinding', '$.observePairs'), '$.observePairs.engineBinding');
  if (engineBinding.target !== contract.target) {
    fail('unknown-engine-capability', '$.observePairs.engineBinding.target', `expected the generated ${contract.target} capability`);
  }
  const operationBinding = binding(required(source, 'operationBinding', '$.observePairs'), '$.observePairs.operationBinding');
  const separator = PRODUCT_MODEL_CAPABILITY_TARGETS.separator;
  if (!operationBinding.target.startsWith(`kernel${separator}`)) {
    fail('invalid-capability-target', '$.observePairs.operationBinding.target', 'observe-pairs operations must be Product Kernel capability bindings');
  }
  if (engineBinding.id === operationBinding.id) {
    fail('duplicate-entry', '$.observePairs.operationBinding.id', 'the Engine system and Product Kernel operation bindings must have distinct identities');
  }
  const observerRole = identity(requiredString(source, 'observerRole', '$.observePairs'), '$.observePairs.observerRole');
  const targetRole = identity(requiredString(source, 'targetRole', '$.observePairs'), '$.observePairs.targetRole');
  if (observerRole === targetRole) {
    fail('duplicate-entry', '$.observePairs.targetRole', 'observer and target roles must have distinct identities');
  }
  const quotas = admitQuotas(required(source, 'quotas', '$.observePairs'), '$.observePairs.quotas');
  const scheduleCadence = cadence(required(source, 'cadence', '$.observePairs') as number | ScheduleCadence | { readonly everySteps: number; readonly offsetSteps?: number });
  const payload = Object.freeze({
    kind: contract.payload.kind,
    observerRole,
    targetRole,
    operationBinding: operationBinding.id,
    // The standard result is also the exact Product Kernel operation input
    // contract. The Rust assembly validates the selected binding agrees.
    operationType: contract.payload.resultKind,
    quotas,
  });
  return system(requiredString(source, 'id', '$.observePairs'), {
    capability: engineBinding.id,
    reads: contract.access.reads,
    writes: contract.access.writes,
    cadence: scheduleCadence,
    payload,
  });
}

function admitQuotas(value: unknown, path: string): ObservePairsQuotas {
  const source = record(value, path);
  const fields = RUNTIME_STANDARD_CAPABILITIES.observePairs.payload.quotaFields;
  known(source, fields, path);
  const limits = RUNTIME_STANDARD_CAPABILITIES.observePairs.quotas;
  return Object.freeze({
    observers: quota(required(source, 'observers', path), `${path}.observers`, limits.observers),
    targets: quota(required(source, 'targets', path), `${path}.targets`, limits.targets),
    pairs: quota(required(source, 'pairs', path), `${path}.pairs`, limits.pairs),
    aggregates: quota(required(source, 'aggregates', path), `${path}.aggregates`, limits.aggregates),
  });
}

function binding(value: unknown, path: string): CapabilityBinding {
  const source = record(value, path);
  known(source, ['id', 'target'], path);
  const id = identity(requiredString(source, 'id', path), `${path}.id`);
  const target = requiredString(source, 'target', path);
  const separator = PRODUCT_MODEL_CAPABILITY_TARGETS.separator;
  const separatorIndex = target.indexOf(separator);
  const namespace = separatorIndex < 0 ? '' : target.slice(0, separatorIndex);
  const local = separatorIndex < 0 ? '' : target.slice(separatorIndex + separator.length);
  if (!PRODUCT_MODEL_CAPABILITY_TARGETS.namespaces.includes(namespace as 'engine' | 'kernel')) {
    fail('invalid-capability-target', `${path}.target`, 'capability targets must use engine.<id> or kernel.<id>');
  }
  return Object.freeze({ id, target: `${namespace}${separator}${identity(local, `${path}.target`)}` });
}

function quota(value: unknown, path: string, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1 || (value as number) > maximum) {
    fail('quota-exceeded', path, `must be an integer in 1..=${String(maximum)}`);
  }
  return value as number;
}

function identity(value: string, path: string): string {
  const pattern = /^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/;
  if (new TextEncoder().encode(value).length > PRODUCT_MODEL_IDENTITY.maximumBytes || !pattern.test(value)) {
    fail('invalid-identity', path, 'identities must be 1..=128 lowercase ASCII segments with only single separators between alphanumerics');
  }
  return value;
}

function record(value: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value) || (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null)) {
    fail('invalid-field-type', path, 'expected a plain object');
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key === 'symbol') fail('invalid-json-value', path, 'authoring objects cannot contain symbol keys');
    const descriptor = descriptors[key];
    if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) fail('invalid-json-value', `${path}.${key}`, 'authoring objects cannot contain accessors or non-enumerable fields');
    if (descriptor.value === undefined) fail('invalid-json-value', `${path}.${key}`, 'authoring objects cannot contain undefined fields');
  }
  return value as Readonly<Record<string, unknown>>;
}

function known(source: Readonly<Record<string, unknown>>, fields: readonly string[], path: string): void {
  for (const key of Object.keys(source)) if (!fields.includes(key)) fail('unknown-field', `${path}.${key}`, `unknown field ${key}`);
}
function required(source: Readonly<Record<string, unknown>>, field: string, path: string): unknown {
  if (!Object.hasOwn(source, field)) fail('missing-field', `${path}.${field}`, `missing required field ${field}`);
  return source[field];
}
function requiredString(source: Readonly<Record<string, unknown>>, field: string, path: string): string {
  const value = required(source, field, path);
  if (typeof value !== 'string') fail('invalid-field-type', `${path}.${field}`, 'expected a string');
  return value;
}
function fail(code: RuntimeCompositionAuthoringError['code'], path: string, message: string): never {
  throw new RuntimeCompositionAuthoringError(code, path, message);
}
