import {
  STANDARD_EXTENSION,
  STANDARD_FAILURE_CODES,
  STANDARD_FIELD_ORDER,
  STANDARD_FAMILIES,
  STANDARD_IDENTITIES,
  STANDARD_LIMITS,
  StandardContractError,
  type ContinuousDefinitionPayload,
  type ContinuousInput,
  type ContinuousTree,
  type ExactDefinitionPayload,
  type ExactInput,
  type ExactTree,
  type JsonValue,
  type StandardContractErrorCode,
  type StandardDefinitionPayload,
  type StandardExtensionArtifact,
  type StandardRole,
} from './generated.js';

export {
  type ContinuousDefinitionPayload,
  type ContinuousInput,
  type ContinuousTree,
  type ExactDefinitionPayload,
  type ExactInput,
  type ExactTree,
  type JsonValue,
  type StandardDefinitionPayload,
  type StandardExtensionArtifact,
  type StandardRole,
};

type Family = 'exact' | 'continuous';
type Tree = ExactTree | ContinuousTree;
type Input = ExactInput | ContinuousInput;
type Metrics = { nodes: number; work: number; inputs: Set<string> };
type WireObject = Record<string, unknown> & {
  family?: unknown; roles?: unknown; semanticsVersion?: unknown; source?: unknown; subject?: unknown; tree?: unknown;
  namespace?: unknown; schemaVersion?: unknown; kind?: unknown; payload?: unknown; role?: unknown; capabilities?: unknown;
  op?: unknown; value?: unknown; bits?: unknown; input?: unknown; values?: unknown; left?: unknown; right?: unknown;
};

/** Decodes a closed standard payload using only Rust-generated grammar and limits. */
export function decodeStandardPayload(value: unknown): StandardDefinitionPayload {
  const payload = record(value, 'invalid-node', 'definition must be an object');
  fields(payload, [...STANDARD_FIELD_ORDER.Definition], 'definition');
  const family = string(payload.family, 'wrong-family', 'definition.family must be a string');
  if (family !== 'exact' && family !== 'continuous') fail('wrong-family', `unsupported standard family ${family}`);
  const expected = STANDARD_FAMILIES[family].evaluatorSemanticsVersion;
  if (payload.semanticsVersion !== expected) fail('unsupported-semantics-version', `${family} semanticsVersion must be ${expected}`);
  identity(string(payload.subject, 'invalid-identity', 'definition.subject must be a string'), 'subject');
  identity(string(payload.source, 'invalid-identity', 'definition.source must be a string'), 'source');
  const roles = decodeRoles(payload.roles);
  const declared = new Set(roles.map((role) => role.role));
  const metrics: Metrics = { nodes: 0, work: 0, inputs: new Set() };
  const tree = decodeTree(payload.tree, family, declared, metrics, 1);
  const limits = STANDARD_LIMITS[family];
  if (metrics.inputs.size > limits.maximumInputs) fail('input-quota-exceeded', `${family} expression has too many distinct inputs`);
  if (metrics.work > limits.maximumWork) fail('work-quota-exceeded', `${family} expression has too much evaluation work`);
  return family === 'exact'
    ? { family, roles, semanticsVersion: expected, source: payload.source as string, subject: payload.subject as string, tree: tree as ExactTree }
    : { family, roles, semanticsVersion: expected, source: payload.source as string, subject: payload.subject as string, tree: tree as ContinuousTree };
}

/** Validates an author-supplied closed standard payload. */
export function assertStandardPayload(payload: StandardDefinitionPayload): void {
  decodeStandardPayload(payload);
}

/** Decodes the separate bounded exchange artifact; it is never an expression node. */
export function decodeStandardExtensionArtifact(value: unknown): StandardExtensionArtifact {
  const artifact = record(value, 'invalid-node', 'extension artifact must be an object');
  fields(artifact, [...STANDARD_EXTENSION.fieldOrder], 'extension');
  if (artifact.family !== STANDARD_EXTENSION.family) fail('wrong-family', 'extension family must be standardExtension');
  const namespace = string(artifact.namespace, 'invalid-identity', 'extension.namespace must be a string');
  if (new TextEncoder().encode(namespace).byteLength > STANDARD_EXTENSION.namespaceMaximumBytes || !new RegExp(STANDARD_EXTENSION.namespacePattern).test(namespace)) fail('invalid-identity', 'extension.namespace is not canonical');
  const schemaVersion = artifact.schemaVersion;
  if (!Number.isSafeInteger(schemaVersion) || (schemaVersion as number) < 1 || (schemaVersion as number) > STANDARD_EXTENSION.schemaVersionMaximum) fail('extension-schema-mismatch', 'extension.schemaVersion must be a positive Rust u32');
  const kind = string(artifact.kind, 'invalid-identity', 'extension.kind must be a string');
  identity(kind, 'extensionKind');
  const subject = string(artifact.subject, 'invalid-identity', 'extension.subject must be a string');
  const source = string(artifact.source, 'invalid-identity', 'extension.source must be a string');
  identity(subject, 'subject');
  identity(source, 'source');
  if (!isJson(artifact.payload)) fail('invalid-node', 'extension.payload must be JSON');
  if (new TextEncoder().encode(JSON.stringify(artifact.payload)).byteLength > STANDARD_LIMITS.maxExtensionBytes) fail('extension-payload-too-large', 'extension payload exceeds the Rust byte limit');
  return { family: STANDARD_EXTENSION.family, kind, namespace, payload: artifact.payload, schemaVersion: schemaVersion as number, source, subject };
}

export function assertStandardExtensionArtifact(value: StandardExtensionArtifact): void {
  decodeStandardExtensionArtifact(value);
}

function decodeRoles(value: unknown): readonly StandardRole[] {
  if (!Array.isArray(value)) fail('invalid-node', 'definition.roles must be an array');
  let previous = '';
  const seen = new Set<string>();
  return value.map((entry, index) => {
    const role = record(entry, 'invalid-node', `definition.roles[${index}] must be an object`);
    fields(role, [...STANDARD_FIELD_ORDER.Role], `definition.roles[${index}]`);
    const id = string(role.role, 'invalid-identity', 'role must be a string');
    identity(id, 'role');
    if (previous >= id || seen.has(id)) fail('non-canonical-roles', 'roles must be sorted and deduplicated');
    previous = id;
    seen.add(id);
    if (!Array.isArray(role.capabilities) || role.capabilities.length > STANDARD_LIMITS.maxCapabilitiesPerRole) fail('non-canonical-roles', 'role capabilities exceed the Rust limit');
    let priorCapability = '';
    const capabilities = role.capabilities.map((capability, capIndex) => {
      const parsed = string(capability, 'invalid-identity', `role capability ${capIndex} must be a string`);
      identity(parsed, 'capability');
      if (priorCapability >= parsed) fail('non-canonical-roles', 'role capabilities must be sorted and deduplicated');
      priorCapability = parsed;
      return parsed;
    });
    return { role: id, capabilities };
  });
}

function decodeTree(value: unknown, family: Family, roles: ReadonlySet<string>, metrics: Metrics, depth: number): Tree {
  const limits = STANDARD_LIMITS[family];
  if (depth > limits.maximumDepth) fail('depth-quota-exceeded', `${family} expression exceeds Rust depth limit`);
  metrics.nodes += 1;
  metrics.work += 1;
  if (metrics.nodes > limits.maximumNodes) fail('node-quota-exceeded', `${family} expression exceeds Rust node limit`);
  const tree = record(value, 'invalid-node', 'expression node must be an object');
  const op = string(tree.op, 'invalid-node', 'expression op must be a string');
  if (!includes(STANDARD_FAMILIES[family].operations, op)) fail('invalid-node', `${op} is not a ${family} operation`);
  if (op === 'literal') {
    const field = STANDARD_FAMILIES[family].literal.field;
    fields(tree, [...(family === 'exact' ? STANDARD_FIELD_ORDER.Literal : STANDARD_FIELD_ORDER.ContinuousLiteral)], 'literal');
    if (family === 'exact') {
      const exactLimits = STANDARD_LIMITS.exact;
      if (!Number.isSafeInteger(tree.value) || (!STANDARD_FAMILIES.exact.literal.negativeZero && Object.is(tree.value, -0)) || (tree.value as number) < exactLimits.minimumScalar || (tree.value as number) > exactLimits.maximumScalar) fail('invalid-literal', 'exact literal is outside the canonical MechanicsScalar range');
      return { op, value: tree.value as number };
    }
    const bits = string(tree.bits, 'invalid-literal', 'continuous literal bits must be a string');
    const literal = STANDARD_FAMILIES.continuous.literal;
    const leadingBits = Number.parseInt(bits.slice(0, literal.exponentLeadingHexDigits), 16);
    if (!new RegExp(`^[${literal.alphabet}]{${literal.width}}$`).test(bits) || (!literal.negativeZero && bits === literal.negativeZeroBits) || (literal.finite && (leadingBits & literal.exponentMask) === literal.exponentMask)) fail('invalid-literal', 'continuous literal must be finite normalized binary64 bits');
    return { op, bits };
  }
  if (op === 'input') {
    fields(tree, [...STANDARD_FIELD_ORDER.Input], 'input');
    const input = decodeInput(tree.input, family, roles);
    metrics.inputs.add(inputIdentity(input));
    return { op, input } as Tree;
  }
  if (op === 'min' || op === 'max') {
    fields(tree, [...STANDARD_FIELD_ORDER.Aggregate], 'aggregate');
    if (!Array.isArray(tree.values) || tree.values.length === 0) fail('invalid-node', 'aggregate values must be a nonempty array');
    if (tree.values.length > limits.maximumArity) fail('arity-quota-exceeded', `${family} aggregate exceeds Rust arity limit`);
    return { op, values: tree.values.map((child) => decodeTree(child, family, roles, metrics, depth + 1)) } as Tree;
  }
  fields(tree, [...STANDARD_FIELD_ORDER.Binary], 'binary');
  return { op, left: decodeTree(tree.left, family, roles, metrics, depth + 1), right: decodeTree(tree.right, family, roles, metrics, depth + 1) } as Tree;
}

function decodeInput(value: unknown, family: Family, roles: ReadonlySet<string>): Input {
  const input = record(value, 'invalid-node', 'input must be an object');
  const kind = string(input.kind, 'invalid-node', 'input.kind must be a string');
  const descriptor = STANDARD_FAMILIES[family].inputKinds.find((candidate) => candidate.tag === kind);
  if (!descriptor) fail('invalid-node', `${kind} is not a ${family} input kind`);
  fields(input, [...descriptor.fields], 'input');
  const role = string(input.role, 'invalid-identity', 'input.role must be a string');
  identity(role, 'role');
  if (!roles.has(role)) fail('undeclared-input-role', `input role ${role} is not declared`);
  const result: Record<string, string> = { kind, role };
  for (const field of descriptor.fields.slice(2)) {
    const identityType = field === 'id' ? 'input' : field === 'stat' ? 'mechanicsStat' : 'mechanicsTrack';
    const item = string(input[field], 'invalid-identity', `input.${field} must be a string`);
    identity(item, identityType);
    result[field] = item;
  }
  return result as Input;
}

function inputIdentity(input: Input): string {
  return 'id' in input ? `${input.kind}:${input.role}:${input.id}` : 'stat' in input ? `${input.kind}:${input.role}:${input.stat}` : `${input.kind}:${input.role}:${input.track}`;
}

function identity(value: string, kind: keyof typeof STANDARD_IDENTITIES): void {
  const rule = STANDARD_IDENTITIES[kind];
  if (new TextEncoder().encode(value).byteLength > rule.maximumBytes) fail('invalid-identity', `${kind} exceeds Rust byte limit`);
  if (!new RegExp(rule.pattern).test(value) || ('trimmed' in rule && rule.trimmed && value.trim() !== value)) fail('invalid-identity', `${kind} is not canonical`);
}

function fields(value: WireObject, expected: readonly string[], path: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((field, index) => field !== wanted[index])) fail('unknown-field', `${path} has missing, duplicate, or unknown fields`);
}
function record(value: unknown, code: StandardContractErrorCode, message: string): WireObject {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) fail(code, message);
  return value as WireObject;
}
function string(value: unknown, code: StandardContractErrorCode, message: string): string {
  if (typeof value !== 'string') fail(code, message);
  return value;
}
function includes(values: readonly string[], value: string): boolean { return values.includes(value); }
function isJson(value: unknown): value is JsonValue {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJson);
  return typeof value === 'object' && value !== null && Object.values(value).every(isJson);
}
function fail(code: StandardContractErrorCode, message: string): never { throw new StandardContractError(code, message); }

// This forces generated stable codes to remain the only validation-code vocabulary.
void STANDARD_FAILURE_CODES;
