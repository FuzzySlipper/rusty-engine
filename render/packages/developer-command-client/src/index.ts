import { GENERATED_DEVELOPER_COMMAND_CONTRACT } from './generated-developer-command-contract.js';
import { GENERATED_STANDARD_HOST_WIRE } from './generated-standard-host-wire.js';

/**
 * Public, transport-neutral developer-command client and optional application-host
 * pull-down console.  It intentionally knows no gameplay semantics: a product
 * supplies a bounded adapter, discovery snapshot, and (where it wants a form)
 * an explicit wire schema.  Descriptor help is deliberately not a schema.
 */

export const RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION =
  GENERATED_DEVELOPER_COMMAND_CONTRACT.protocolVersion;
/** Exact schemas generated from developer-command-standard host DTOs. */
export const RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS =
  GENERATED_STANDARD_HOST_WIRE.commands as unknown as Readonly<Record<string, RustyDeveloperCommandWireSchema>>;
const MAX_HISTORY = GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.historyEntries;
const MAX_SEQUENCE = 128;
const MAX_COMMANDS = 256;
const MAX_WIRE_DEPTH = 16;
const MAX_SCHEMA_NODES = 256;
const MAX_SCHEMA_FIELDS = 256;

export type RustyDeveloperCommandLane =
  typeof GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes[number];

export type RustyDeveloperCommandValueSchema =
  | { readonly kind: 'boolean' }
  | { readonly kind: 'decimalU64' }
  | { readonly kind: 'integer'; readonly minimum?: number; readonly maximum?: number }
  | { readonly kind: 'string'; readonly maximumBytes: number; readonly pattern?: 'identifier' }
  | { readonly kind: 'array'; readonly items: RustyDeveloperCommandValueSchema; readonly maximumItems: number }
  | { readonly kind: 'object'; readonly fields: Readonly<Record<string, RustyDeveloperCommandWireField>> }
  | { readonly kind: 'enum'; readonly values: readonly string[] }
  | { readonly kind: 'taggedUnion'; readonly tag: string; readonly variants: Readonly<Record<string, RustyDeveloperCommandValueSchema>> }
  | { readonly kind: 'opaqueJson'; readonly maximumBytes: number; readonly maximumNodes: number };

export interface RustyDeveloperCommandWireField {
  readonly required: boolean;
  readonly value: RustyDeveloperCommandValueSchema;
}

/**
 * An explicit value codec supplied by a Rust/product host adapter.  This is
 * deliberately separate from `developer-command::TypeDescriptor`, which is a
 * bounded help/discovery summary and cannot safely describe all owner DTOs.
 */
export interface RustyDeveloperCommandWireSchema {
  readonly request: RustyDeveloperCommandValueSchema;
  readonly result: RustyDeveloperCommandValueSchema;
  readonly error: RustyDeveloperCommandValueSchema;
}

export interface RustyDeveloperCommandDescriptor {
  readonly id: string;
  readonly aliases: readonly string[];
  readonly lane: RustyDeveloperCommandLane;
  readonly summary: string;
  /** Discovery/help only; never used to encode a request. */
  readonly helpOnly?: boolean;
}

export interface RustyDeveloperCommandDiscovery {
  readonly protocolVersion: typeof RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
  readonly runtime: string;
  readonly profile: string;
  readonly permittedLanes: readonly RustyDeveloperCommandLane[];
  readonly revision: string;
  readonly catalogEpoch: string;
  readonly contractFingerprint: string;
  readonly commands: readonly RustyDeveloperCommandDescriptor[];
}

export interface RustyDeveloperCommandRequest {
  readonly protocolVersion: typeof RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
  readonly command: string;
  readonly correlation: string;
  readonly runtime: string;
  readonly expected: { readonly profile: string; readonly revision: string; readonly catalogEpoch: string };
  readonly payload: unknown;
}

export type RustyDeveloperCommandOutcome =
  | { readonly kind: 'success'; readonly value: unknown; readonly receiptRefs: readonly string[] }
  | { readonly kind: 'error'; readonly code: string; readonly message: string; readonly details?: unknown };

export interface RustyDeveloperCommandResponse {
  readonly correlation: string;
  readonly runtime: string;
  readonly profile: string;
  readonly revision: string;
  readonly catalogEpoch: string;
  readonly outcome: RustyDeveloperCommandOutcome;
}

/** Product-owned adapter: dispatch, authorization, safe points and mutation stay behind it. */
export interface RustyDeveloperCommandAdapter {
  readonly discover: (signal?: AbortSignal) => Promise<unknown>;
  readonly execute: (request: Readonly<RustyDeveloperCommandRequest>, signal?: AbortSignal) => Promise<unknown>;
}

export interface RustyDeveloperCommandExtension {
  readonly namespace: string;
  readonly descriptors: readonly RustyDeveloperCommandDescriptor[];
  readonly schemas: Readonly<Record<string, RustyDeveloperCommandWireSchema>>;
}

export interface RustyDeveloperCommandHistoryEntry {
  readonly phase: 'completed';
  readonly request: RustyDeveloperCommandRequest;
  readonly lane: RustyDeveloperCommandLane;
  readonly outcome: RustyDeveloperCommandOutcome;
  readonly receiptRefs: readonly string[];
  readonly runtime: string;
  readonly profile: string;
  readonly revision: string;
  readonly catalogEpoch: string;
  readonly at: number;
}
export interface RustyDeveloperCommandLocalFailure {
  readonly phase: 'pre-dispatch' | 'transport' | 'post-dispatch';
  readonly lane: RustyDeveloperCommandLane | null;
  readonly code: RustyDeveloperCommandClientError['code'];
  readonly message: string;
  /** Present only once the transport-bound request has been issued. */
  readonly request?: RustyDeveloperCommandRequest;
  readonly receiptRefs: readonly [];
  readonly at: number;
}

/** A portable command transcript, deliberately not a deterministic replay format. */
export interface RustyDeveloperCommandSequence {
  readonly kind: 'rusty_developer_command.sequence.v1';
  readonly note: 'portable command intent/history; not deterministic replay';
  readonly entries: readonly RustyDeveloperCommandHistoryEntry[];
}

export class RustyDeveloperCommandClientError extends Error {
  readonly code:
    | 'disposed' | 'malformed' | 'unavailable' | 'correlation_reused' | 'stale_context'
    | 'unknown_command' | 'invalid_payload' | 'codec_unavailable' | 'cancelled' | 'invalid_extension'
    | 'invalid_schema';

  constructor(code: RustyDeveloperCommandClientError['code'], message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'RustyDeveloperCommandClientError';
    this.code = code;
  }
}

export interface RustyDeveloperCommandClient {
  readonly discover: (signal?: AbortSignal) => Promise<RustyDeveloperCommandDiscovery>;
  readonly execute: (command: string, payload: unknown, signal?: AbortSignal) => Promise<RustyDeveloperCommandResponse>;
  readonly descriptor: (commandOrAlias: string) => RustyDeveloperCommandDescriptor | null;
  readonly schema: (command: string) => RustyDeveloperCommandWireSchema | null;
  readonly history: () => readonly (RustyDeveloperCommandHistoryEntry | RustyDeveloperCommandLocalFailure)[];
  readonly exportSequence: () => RustyDeveloperCommandSequence;
  readonly dispose: () => void;
}

export interface RustyDeveloperCommandClientOptions {
  readonly adapter: RustyDeveloperCommandAdapter;
  readonly schemas?: Readonly<Record<string, RustyDeveloperCommandWireSchema>>;
  readonly extensions?: readonly RustyDeveloperCommandExtension[];
  readonly createCorrelation?: () => string;
  readonly now?: () => number;
}

export function createRustyDeveloperCommandClient(
  options: RustyDeveloperCommandClientOptions,
): RustyDeveloperCommandClient {
  const extensionDescriptors = composeDescriptors(options.extensions ?? []);
  const schemas = composeSchemas(options.schemas ?? {}, options.extensions ?? []);
  const issued = new Set<string>();
  const entries: (RustyDeveloperCommandHistoryEntry | RustyDeveloperCommandLocalFailure)[] = [];
  const correlation = options.createCorrelation ?? (() => `command-${cryptoRandom()}`);
  const now = options.now ?? Date.now;
  let discovery: RustyDeveloperCommandDiscovery | null = null;
  let disposed = false;
  const requireActive = (): void => {
    if (disposed) throw new RustyDeveloperCommandClientError('disposed', 'Developer command client is disposed');
  };
  const trimHistory = (): void => {
    if (entries.length > MAX_HISTORY) entries.splice(0, entries.length - MAX_HISTORY);
  };
  const recordFailure = (
    phase: RustyDeveloperCommandLocalFailure['phase'],
    lane: RustyDeveloperCommandLane | null,
    failure: RustyDeveloperCommandClientError,
    request?: RustyDeveloperCommandRequest,
  ): void => {
    const message = failure.message.slice(0, 1024);
    const entry: RustyDeveloperCommandLocalFailure = request === undefined
      ? Object.freeze({ phase, lane, code: failure.code, message, receiptRefs: [] as const, at: now() })
      : Object.freeze({ phase, lane, code: failure.code, message, request, receiptRefs: [] as const, at: now() });
    entries.push(entry);
    trimHistory();
  };
  const resolveDescriptor = (id: string): RustyDeveloperCommandDescriptor | null => {
    const snapshot = discovery;
    if (snapshot === null) return null;
    return snapshot.commands
      .find((candidate) => candidate.id === id || candidate.aliases.includes(id)) ?? null;
  };
  const refresh = async (signal?: AbortSignal): Promise<RustyDeveloperCommandDiscovery> => {
    requireActive();
    throwIfAborted(signal);
    let raw: unknown;
    try {
      raw = await options.adapter.discover(signal);
    } catch (cause) {
      throw translateAdapterFailure(cause);
    }
    requireActive();
    throwIfAborted(signal);
    const candidate = decodeDiscovery(raw, extensionDescriptors);
    const current = discovery;
    if (current !== null && (candidate.runtime !== current.runtime || candidate.profile !== current.profile
      || decimalLessThan(candidate.revision, current.revision)
      || decimalLessThan(candidate.catalogEpoch, current.catalogEpoch)
      || (candidate.revision === current.revision
        && candidate.catalogEpoch === current.catalogEpoch
        && candidate.contractFingerprint !== current.contractFingerprint))) {
      throw new RustyDeveloperCommandClientError(
        'stale_context',
        'Developer command discovery regressed or changed its selected runtime/profile context',
      );
    }
    discovery = candidate;
    return candidate;
  };
  return Object.freeze({
    discover: refresh,
    descriptor: (commandOrAlias: string) => resolveDescriptor(commandOrAlias),
    schema: (command: string) => hasOwn(schemas, command) ? schemas[command]! : null,
    history: () => Object.freeze(entries.slice()),
    exportSequence: () => Object.freeze({
      kind: 'rusty_developer_command.sequence.v1' as const,
      note: 'portable command intent/history; not deterministic replay' as const,
      entries: Object.freeze(entries.filter((entry): entry is RustyDeveloperCommandHistoryEntry => entry.phase === 'completed').slice(-MAX_SEQUENCE)),
    }),
    execute: async (command: string, payload: unknown, signal?: AbortSignal) => {
      requireActive();
      throwIfAborted(signal);
      const snapshot = discovery ?? await refresh(signal);
      const descriptor = snapshot.commands.find((candidate) =>
        candidate.id === command || candidate.aliases.includes(command)) ?? null;
      if (descriptor === null) {
        // There is no admitted lane, schema, or issued request to preserve for
        // an unknown command, so it is intentionally absent from local history
        // and portable sequences.
        throw new RustyDeveloperCommandClientError('unknown_command', `Unknown developer command ${command}`);
      }
      const schema = schemas[descriptor.id];
      if (schema === undefined) {
        const failure = new RustyDeveloperCommandClientError('codec_unavailable', `${descriptor.id} has help only; its product has not supplied an exact wire codec`);
        recordFailure('pre-dispatch', descriptor.lane, failure);
        throw failure;
      }
      let requestPayload: unknown;
      try {
        validateWireValue(payload, schema.request, '$');
        requestPayload = cloneJson(payload);
      } catch (cause) {
        const failure = cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed'
          ? new RustyDeveloperCommandClientError('invalid_payload', cause.message, { cause })
          : translateAdapterFailure(cause);
        recordFailure('pre-dispatch', descriptor.lane, failure);
        throw failure;
      }
      let id: string;
      try {
        id = correlation();
        validateIdentity(id, 'correlation');
      } catch (cause) {
        const failure = cause instanceof RustyDeveloperCommandClientError
          ? cause
          : new RustyDeveloperCommandClientError('malformed', 'Correlation factory returned an invalid identity', { cause: cause instanceof Error ? cause : undefined });
        recordFailure('pre-dispatch', descriptor.lane, failure);
        throw failure;
      }
      if (issued.has(id)) {
        const failure = new RustyDeveloperCommandClientError('correlation_reused', `Correlation ${id} was already issued`);
        recordFailure('pre-dispatch', descriptor.lane, failure);
        throw failure;
      }
      try {
        requireActive();
        throwIfAborted(signal);
      } catch (cause) {
        const failure = cause instanceof RustyDeveloperCommandClientError
          ? cause
          : new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled', { cause: cause instanceof Error ? cause : undefined });
        recordFailure('pre-dispatch', descriptor.lane, failure);
        throw failure;
      }
      issued.add(id);
      const request: RustyDeveloperCommandRequest = Object.freeze({
        protocolVersion: RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION,
        command: descriptor.id,
        correlation: id,
        runtime: snapshot.runtime,
        expected: Object.freeze({
          profile: snapshot.profile, revision: snapshot.revision, catalogEpoch: snapshot.catalogEpoch,
        }),
        payload: requestPayload,
      });
      let raw: unknown;
      try {
        raw = await options.adapter.execute(request, signal);
      } catch (cause) {
        const failure = translateAdapterFailure(cause);
        recordFailure('transport', descriptor.lane, failure, request);
        throw failure;
      }
      try {
        // The adapter promise may have resolved after cancellation or disposal.
        // These checks must precede decode and the history write so a late reply
        // can never become a completed command entry.
        requireActive();
        throwIfAborted(signal);
        const response = decodeResponse(raw, request, schema);
        const current = discovery;
        if (current === null
          || current.runtime !== snapshot.runtime
          || current.profile !== snapshot.profile
          || (current.revision === snapshot.revision
            && current.catalogEpoch === snapshot.catalogEpoch
            && current.contractFingerprint !== snapshot.contractFingerprint)
          || response.runtime !== current.runtime
          || response.profile !== current.profile
          || decimalLessThan(response.revision, current.revision)
          || decimalLessThan(response.catalogEpoch, current.catalogEpoch)) {
          throw new RustyDeveloperCommandClientError('stale_context', 'Developer command response did not preserve the selected runtime/profile/epoch context');
        }
        requireActive();
        throwIfAborted(signal);
        const entry = Object.freeze({
          phase: 'completed' as const,
          request,
          lane: descriptor.lane,
          outcome: response.outcome,
          receiptRefs: response.outcome.kind === 'success' ? response.outcome.receiptRefs : Object.freeze([]),
          runtime: response.runtime,
          profile: response.profile,
          revision: response.revision,
          catalogEpoch: response.catalogEpoch,
          at: now(),
        });
        entries.push(entry);
        trimHistory();
        discovery = Object.freeze({ ...current, revision: response.revision, catalogEpoch: response.catalogEpoch });
        return response;
      } catch (cause) {
        const failure = cause instanceof RustyDeveloperCommandClientError
          ? cause
          : new RustyDeveloperCommandClientError('malformed', 'Developer command response was not valid', { cause: cause instanceof Error ? cause : undefined });
        const phase = failure.code === 'cancelled' || failure.code === 'disposed' || failure.code === 'unavailable'
          ? 'transport' : 'post-dispatch';
        recordFailure(phase, descriptor.lane, failure, request);
        throw failure;
      }
    },
    dispose: () => { disposed = true; discovery = null; },
  });
}

function composeSchemas(
  base: Readonly<Record<string, RustyDeveloperCommandWireSchema>>,
  extensions: readonly RustyDeveloperCommandExtension[],
): Readonly<Record<string, RustyDeveloperCommandWireSchema>> {
  const result: Record<string, RustyDeveloperCommandWireSchema> = Object.create(null) as Record<string, RustyDeveloperCommandWireSchema>;
  for (const [id, schema] of Object.entries(base)) {
    validateSchemaIdentity(id, `schema.${id}`);
    result[id] = admitWireSchema(schema, `schema.${id}`);
  }
  for (const extension of extensions) {
    const extensionRecord = extensionObject(extension, 'extension');
    extensionExactKeys(extensionRecord, ['namespace', 'descriptors', 'schemas'], 'extension');
    const namespace = validateExtensionNamespace(extensionRecord['namespace']);
    const schemaRecord = extensionObject(extensionRecord['schemas'], `extension ${namespace}.schemas`);
    for (const [id, schema] of Object.entries(schemaRecord)) {
      const normalizedId = extensionIdentity(id, `extension schema ${id}`);
      if (!normalizedId.startsWith(`${namespace}.`)) {
        invalidExtension(`Extension schema ${id} escapes ${namespace}`);
      }
      if (hasOwn(result, normalizedId)) invalidExtension(`Duplicate wire schema ${normalizedId}`);
      result[normalizedId] = admitWireSchema(schema, `extension schema ${normalizedId}`);
    }
  }
  return Object.freeze(result);
}

interface SchemaAdmissionContext {
  readonly active: Set<object>;
  readonly seen: Set<object>;
  nodes: number;
}

function admitWireSchema(value: unknown, where: string): RustyDeveloperCommandWireSchema {
  const context: SchemaAdmissionContext = { active: new Set(), seen: new Set(), nodes: 0 };
  const record = schemaObject(value, where);
  schemaExactKeys(record, ['request', 'result', 'error'], where);
  return Object.freeze({
    request: admitValueSchema(record['request'], `${where}.request`, context, 0),
    result: admitValueSchema(record['result'], `${where}.result`, context, 0),
    error: admitValueSchema(record['error'], `${where}.error`, context, 0),
  });
}

function admitValueSchema(
  value: unknown,
  where: string,
  context: SchemaAdmissionContext,
  depth: number,
): RustyDeveloperCommandValueSchema {
  if (depth > MAX_WIRE_DEPTH) invalidSchema(`${where} exceeds wire schema depth`);
  const record = schemaObject(value, where);
  if (context.active.has(record)) invalidSchema(`${where} contains a cyclic schema`);
  if (!context.seen.has(record)) {
    context.seen.add(record);
    context.nodes += 1;
    if (context.nodes > MAX_SCHEMA_NODES) invalidSchema(`${where} exceeds the ${MAX_SCHEMA_NODES}-node schema limit`);
  }
  context.active.add(record);
  try {
    const kind = schemaKind(record['kind'], `${where}.kind`);
    switch (kind) {
      case 'boolean':
        schemaExactKeys(record, ['kind'], where);
        return Object.freeze({ kind });
      case 'decimalU64':
        schemaExactKeys(record, ['kind'], where);
        return Object.freeze({ kind });
      case 'integer': {
        schemaExactKeys(record, ['kind'], where, ['minimum', 'maximum']);
        const minimum = optionalSafeInteger(record['minimum'], `${where}.minimum`);
        const maximum = optionalSafeInteger(record['maximum'], `${where}.maximum`);
        if (minimum !== undefined && maximum !== undefined && minimum > maximum) invalidSchema(`${where} minimum exceeds maximum`);
        return Object.freeze({ kind, ...(minimum === undefined ? {} : { minimum }), ...(maximum === undefined ? {} : { maximum }) });
      }
      case 'string': {
        schemaExactKeys(record, ['kind', 'maximumBytes'], where, ['pattern']);
        const maximumBytes = boundedSchemaNumber(record['maximumBytes'], `${where}.maximumBytes`, 0, 1_048_576);
        const pattern = record['pattern'];
        if (pattern !== undefined && pattern !== 'identifier') invalidSchema(`${where}.pattern is not supported`);
        return Object.freeze({ kind, maximumBytes, ...(pattern === undefined ? {} : { pattern }) });
      }
      case 'array': {
        schemaExactKeys(record, ['kind', 'items', 'maximumItems'], where);
        const maximumItems = boundedSchemaNumber(record['maximumItems'], `${where}.maximumItems`, 0, 65_536);
        return Object.freeze({ kind, items: admitValueSchema(record['items'], `${where}.items`, context, depth + 1), maximumItems });
      }
      case 'object': {
        schemaExactKeys(record, ['kind', 'fields'], where);
        const fieldRecord = schemaObject(record['fields'], `${where}.fields`);
        const fieldEntries = Object.entries(fieldRecord);
        if (fieldEntries.length > MAX_SCHEMA_FIELDS) invalidSchema(`${where}.fields exceeds the ${MAX_SCHEMA_FIELDS}-field limit`);
        const fields: Record<string, RustyDeveloperCommandWireField> = Object.create(null) as Record<string, RustyDeveloperCommandWireField>;
        for (const [key, field] of fieldEntries) {
          const fieldWhere = `${where}.fields.${key}`;
          validateSchemaFieldName(key, fieldWhere);
          const fieldRecordValue = schemaObject(field, fieldWhere);
          schemaExactKeys(fieldRecordValue, ['required', 'value'], fieldWhere);
          if (typeof fieldRecordValue['required'] !== 'boolean') invalidSchema(`${fieldWhere}.required must be boolean`);
          fields[key] = Object.freeze({
            required: fieldRecordValue['required'],
            value: admitValueSchema(fieldRecordValue['value'], `${fieldWhere}.value`, context, depth + 1),
          });
        }
        return Object.freeze({ kind, fields: Object.freeze(fields) });
      }
      case 'enum': {
        schemaExactKeys(record, ['kind', 'values'], where);
        const values = record['values'];
        if (!Array.isArray(values) || values.length === 0 || values.length > MAX_SCHEMA_FIELDS) invalidSchema(`${where}.values must be a bounded nonempty array`);
        const normalized = values.map((item, index) => {
          const itemWhere = `${where}.values[${index}]`;
          const text = schemaText(item, itemWhere, 256);
          return text;
        });
        if (new Set(normalized).size !== normalized.length) invalidSchema(`${where}.values contains duplicates`);
        return Object.freeze({ kind, values: Object.freeze(normalized) });
      }
      case 'taggedUnion': {
        schemaExactKeys(record, ['kind', 'tag', 'variants'], where);
        const tag = schemaText(record['tag'], `${where}.tag`, 128);
        const variantsRecord = schemaObject(record['variants'], `${where}.variants`);
        const variantsEntries = Object.entries(variantsRecord);
        if (variantsEntries.length === 0 || variantsEntries.length > MAX_SCHEMA_FIELDS) invalidSchema(`${where}.variants must be a bounded nonempty object`);
        const variants: Record<string, RustyDeveloperCommandValueSchema> = Object.create(null) as Record<string, RustyDeveloperCommandValueSchema>;
        for (const [variant, variantSchema] of variantsEntries) {
          validateSchemaFieldName(variant, `${where}.variants.${variant}`);
          variants[variant] = admitValueSchema(variantSchema, `${where}.variants.${variant}`, context, depth + 1);
        }
        return Object.freeze({ kind, tag, variants: Object.freeze(variants) });
      }
      case 'opaqueJson': {
        schemaExactKeys(record, ['kind', 'maximumBytes', 'maximumNodes'], where);
        const maximumBytes = boundedSchemaNumber(record['maximumBytes'], `${where}.maximumBytes`, 0, 1_048_576);
        const maximumNodes = boundedSchemaNumber(record['maximumNodes'], `${where}.maximumNodes`, 1, 65_536);
        return Object.freeze({ kind, maximumBytes, maximumNodes });
      }
    }
  } finally {
    context.active.delete(record);
  }
}

function schemaObject(value: unknown, where: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) invalidSchema(`${where} must be an object`);
  const record = value as Record<string, unknown>;
  const prototype = Object.getPrototypeOf(record);
  if (prototype !== Object.prototype && prototype !== null) invalidSchema(`${where} must be a plain object`);
  if (Object.getOwnPropertySymbols(record).length > 0) invalidSchema(`${where} may not contain symbol properties`);
  const descriptors = Object.getOwnPropertyDescriptors(record);
  if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor))) {
    invalidSchema(`${where} may not contain accessors or hidden properties`);
  }
  return record;
}

function schemaExactKeys(
  record: Record<string, unknown>,
  required: readonly string[],
  where: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set([...required, ...optional]);
  if (Object.keys(record).some((key) => !allowed.has(key)) || required.some((key) => !hasOwn(record, key))) {
    invalidSchema(`${where} has unexpected or missing fields`);
  }
}

function schemaKind(value: unknown, where: string): RustyDeveloperCommandValueSchema['kind'] {
  if (typeof value !== 'string' || !['boolean', 'decimalU64', 'integer', 'string', 'array', 'object', 'enum', 'taggedUnion', 'opaqueJson'].includes(value)) {
    invalidSchema(`${where} is not a supported schema kind`);
  }
  return value as RustyDeveloperCommandValueSchema['kind'];
}

function schemaText(value: unknown, where: string, maximumBytes: number): string {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > maximumBytes) invalidSchema(`${where} must be a bounded string`);
  return value;
}

function boundedSchemaNumber(value: unknown, where: string, minimum: number, maximum: number): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum || value > maximum) {
    invalidSchema(`${where} must be a bounded nonnegative integer`);
  }
  return value;
}

function optionalSafeInteger(value: unknown, where: string): number | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) invalidSchema(`${where} must be a safe integer`);
  return value;
}

function validateSchemaFieldName(value: string, where: string): void {
  if (value.length === 0 || value === '__proto__' || value === 'constructor' || value === 'prototype') invalidSchema(`${where} is not a usable field name`);
  schemaText(value, where, 128);
}

function validateSchemaIdentity(value: string, where: string): void {
  if (!/^[a-z0-9._:-]+$/u.test(value) || new TextEncoder().encode(value).byteLength > GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes) {
    throw new RustyDeveloperCommandClientError('invalid_schema', `${where} is not a valid command identity`);
  }
}

function invalidExtension(message: string): never {
  throw new RustyDeveloperCommandClientError('invalid_extension', message);
}

function extensionObject(value: unknown, where: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) invalidExtension(`${where} must be an object`);
  const record = value as Record<string, unknown>;
  const prototype = Object.getPrototypeOf(record);
  if (prototype !== Object.prototype && prototype !== null) invalidExtension(`${where} must be a plain object`);
  if (Object.getOwnPropertySymbols(record).length > 0) invalidExtension(`${where} may not contain symbol properties`);
  const descriptors = Object.getOwnPropertyDescriptors(record);
  if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor))) {
    invalidExtension(`${where} may not contain accessors or hidden properties`);
  }
  return record;
}

function extensionExactKeys(
  record: Record<string, unknown>,
  required: readonly string[],
  where: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set([...required, ...optional]);
  if (Object.keys(record).some((key) => !allowed.has(key)) || required.some((key) => !hasOwn(record, key))) {
    invalidExtension(`${where} has unexpected or missing fields`);
  }
}

function extensionArray(value: unknown, where: string): readonly unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype
    || Object.keys(value).length !== value.length || Object.getOwnPropertySymbols(value).length > 0) {
    invalidExtension(`${where} must be a dense ordinary array`);
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Object.entries(descriptors).some(([key, descriptor]) => key !== 'length' && (!descriptor.enumerable || !('value' in descriptor)))) {
    invalidExtension(`${where} may not contain accessors or hidden properties`);
  }
  return value;
}

function extensionText(value: unknown, where: string, maximumBytes: number): string {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > maximumBytes) {
    invalidExtension(`${where} must be a bounded string`);
  }
  return value;
}

function extensionIdentity(value: unknown, where: string): string {
  const result = extensionText(value, where, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes);
  if (!/^[a-z0-9._:-]+$/u.test(result)) invalidExtension(`${where} must use lower-case command identity characters`);
  return result;
}

function validateExtensionNamespace(value: unknown): string {
  const namespace = extensionIdentity(value, 'extension namespace');
  if (namespace.includes(':')) invalidExtension('extension namespace may not contain colon');
  return namespace;
}

function extensionLane(value: unknown, where: string): RustyDeveloperCommandLane {
  const lane = extensionText(value, where, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes) as RustyDeveloperCommandLane;
  if (!(GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes as readonly string[]).includes(lane)) invalidExtension(`${where} is invalid`);
  return lane;
}

function composeDescriptors(extensions: readonly RustyDeveloperCommandExtension[]): readonly RustyDeveloperCommandDescriptor[] {
  const result: RustyDeveloperCommandDescriptor[] = [];
  const identities = new Set<string>();
  for (const extension of extensions) {
    const extensionRecord = extensionObject(extension, 'extension');
    extensionExactKeys(extensionRecord, ['namespace', 'descriptors', 'schemas'], 'extension');
    const namespace = validateExtensionNamespace(extensionRecord['namespace']);
    const descriptors = extensionArray(extensionRecord['descriptors'], `extension ${namespace}.descriptors`);
    if (result.length + descriptors.length > MAX_COMMANDS) {
      invalidExtension(`extensions exceed the ${MAX_COMMANDS}-command limit`);
    }
    descriptors.forEach((value, index) => {
      const where = `extension ${namespace}.descriptors[${index}]`;
      const record = extensionObject(value, where);
      extensionExactKeys(record, ['id', 'aliases', 'lane', 'summary'], where, ['helpOnly']);
      const id = extensionIdentity(record['id'], `${where}.id`);
      if (!id.startsWith(`${namespace}.`)) invalidExtension(`${where}.id escapes ${namespace}`);
      const aliases = extensionArray(record['aliases'], `${where}.aliases`);
      if (aliases.length > GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.commandAliases) {
        invalidExtension(`${where}.aliases exceeds the alias limit`);
      }
      const normalizedAliases = aliases.map((alias, aliasIndex) => {
        const normalized = extensionIdentity(alias, `${where}.aliases[${aliasIndex}]`);
        if (!normalized.startsWith(`${namespace}.`)) invalidExtension(`${where}.aliases[${aliasIndex}] escapes ${namespace}`);
        return normalized;
      });
      const lane = extensionLane(record['lane'], `${where}.lane`);
      const summary = extensionText(record['summary'], `${where}.summary`, GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.summaryBytes);
      const helpOnly = record['helpOnly'];
      if (helpOnly !== undefined && typeof helpOnly !== 'boolean') invalidExtension(`${where}.helpOnly must be boolean`);
      for (const identityValue of [id, ...normalizedAliases]) {
        if (identities.has(identityValue)) invalidExtension(`duplicate command or alias ${identityValue}`);
        identities.add(identityValue);
      }
      result.push(Object.freeze({
        id,
        aliases: Object.freeze(normalizedAliases),
        lane,
        summary,
        ...(helpOnly === undefined ? {} : { helpOnly }),
      }));
    }
    );
  }
  return Object.freeze(result);
}

function decodeDiscovery(value: unknown, extensionDescriptors: readonly RustyDeveloperCommandDescriptor[]): RustyDeveloperCommandDiscovery {
  const record = object(value, 'discovery');
  exactKeys(record, GENERATED_DEVELOPER_COMMAND_CONTRACT.discoveryFields, 'discovery');
  const protocolVersion = decodeProtocolVersion(record['protocolVersion'], 'discovery.protocolVersion');
  const runtime = identity(record['runtime'], 'runtime');
  const profile = identity(record['profile'], 'profile');
  if (!Array.isArray(record['permittedLanes'])) {
    malformed('discovery.permittedLanes must be a dense ordinary array');
  }
  const permittedLaneValues = ordinaryJsonArray(record['permittedLanes'], 'discovery.permittedLanes');
  if (permittedLaneValues.length === 0
    || permittedLaneValues.length > GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes.length) {
    malformed(`discovery.permittedLanes must contain 1-${GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes.length} lanes`);
  }
  const permittedLanes = permittedLaneValues.map((lane, index) => decodeLane(lane, `discovery.permittedLanes[${index}]`));
  if (new Set(permittedLanes).size !== permittedLanes.length) malformed('discovery.permittedLanes must not contain duplicates');
  const revision = decimalU64(record['revision'], 'discovery.revision');
  const catalogEpoch = decimalU64(record['catalogEpoch'], 'discovery.catalogEpoch');
  const contractFingerprint = identity(record['contractFingerprint'], 'discovery.contractFingerprint');
  if (!Array.isArray(record['commands'])) malformed('discovery.commands must be a dense ordinary array');
  const commandValues = ordinaryJsonArray(record['commands'], 'discovery.commands');
  if (commandValues.length > MAX_COMMANDS) malformed('discovery.commands must be a bounded array');
  const commands = commandValues.map((item, index) => decodeDescriptor(item, `discovery.commands[${index}]`));
  const combined = [...commands, ...extensionDescriptors];
  if (combined.length > MAX_COMMANDS) malformed(`discovery.commands exceeds the ${MAX_COMMANDS}-command aggregate limit`);
  const identities = new Set<string>();
  for (const descriptor of combined) {
    for (const identityValue of [descriptor.id, ...descriptor.aliases]) {
      if (identities.has(identityValue)) malformed(`duplicate command or alias ${identityValue}`);
      identities.add(identityValue);
    }
  }
  if (combined.some((command) => !permittedLanes.includes(command.lane))) malformed('discovery command lane is not permitted by its selected profile');
  return Object.freeze({ protocolVersion, runtime, profile, permittedLanes: Object.freeze(permittedLanes), revision, catalogEpoch, contractFingerprint, commands: Object.freeze(combined) });
}

function decodeDescriptor(value: unknown, where: string): RustyDeveloperCommandDescriptor {
  const record = object(value, where);
  exactKeys(record, ['id', 'aliases', 'lane', 'summary'], where);
  const id = identity(record['id'], `${where}.id`);
  if (!Array.isArray(record['aliases']) || record['aliases'].length > GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.commandAliases) malformed(`${where}.aliases must be bounded`);
  const aliases = record['aliases'].map((item, index) => identity(item, `${where}.aliases[${index}]`));
  const lane = decodeLane(record['lane'], `${where}.lane`);
  const summary = boundedString(record['summary'], GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.summaryBytes, `${where}.summary`);
  return Object.freeze({ id, aliases: Object.freeze(aliases), lane, summary, helpOnly: true });
}

function decodeResponse(
  value: unknown,
  request: RustyDeveloperCommandRequest,
  schema: RustyDeveloperCommandWireSchema | undefined,
): RustyDeveloperCommandResponse {
  const record = object(value, 'response');
  exactKeys(record, ['correlation', 'runtime', 'profile', 'revision', 'catalogEpoch', 'outcome'], 'response');
  const correlation = identity(record['correlation'], 'response.correlation');
  if (correlation !== request.correlation) malformed('response correlation does not match request');
  const runtime = identity(record['runtime'], 'response.runtime');
  const profile = identity(record['profile'], 'response.profile');
  const revision = decimalU64(record['revision'], 'response.revision');
  const catalogEpoch = decimalU64(record['catalogEpoch'], 'response.catalogEpoch');
  const outcomeRecord = object(record['outcome'], 'response.outcome');
  const kind = string(outcomeRecord['kind'], 'response.outcome.kind');
  let outcome: RustyDeveloperCommandOutcome;
  if (kind === 'success') {
    exactKeys(outcomeRecord, ['kind', 'value', 'receiptRefs'], 'response.outcome');
    if (!Array.isArray(outcomeRecord['receiptRefs']) || outcomeRecord['receiptRefs'].length > 32) malformed('response receipt refs must be bounded');
    const receiptRefs = outcomeRecord['receiptRefs'].map((entry, index) => identity(entry, `response.receiptRefs[${index}]`));
    if (schema !== undefined) validateWireValue(outcomeRecord['value'], schema.result, '$result');
    outcome = Object.freeze({ kind: 'success', value: cloneJson(outcomeRecord['value']), receiptRefs: Object.freeze(receiptRefs) });
  } else if (kind === 'error') {
    const keys = Object.keys(outcomeRecord);
    if (!keys.every((key) => ['kind', 'code', 'message', 'details'].includes(key)) || !hasOwn(outcomeRecord, 'code') || !hasOwn(outcomeRecord, 'message')) malformed('response error has invalid fields');
    if (schema !== undefined && hasOwn(outcomeRecord, 'details')) validateWireValue(outcomeRecord['details'], schema.error, '$error');
    outcome = Object.freeze({ kind: 'error', code: identity(outcomeRecord['code'], 'response.outcome.code'), message: boundedString(outcomeRecord['message'], 1024, 'response.outcome.message'), ...(hasOwn(outcomeRecord, 'details') ? { details: cloneJson(outcomeRecord['details']) } : {}) });
  } else {
    malformed('response outcome kind is invalid');
  }
  return Object.freeze({ correlation, runtime, profile, revision, catalogEpoch, outcome });
}

export function validateRustyDeveloperCommandWireValue(
  value: unknown,
  schema: RustyDeveloperCommandValueSchema,
): void { validateWireValue(value, schema, '$'); }

function validateWireValue(value: unknown, schema: RustyDeveloperCommandValueSchema, where: string, depth = 0): void {
  if (depth > MAX_WIRE_DEPTH) malformed(`${where} exceeds wire depth`);
  switch (schema.kind) {
    case 'boolean': if (typeof value !== 'boolean') malformed(`${where} must be boolean`); return;
    case 'decimalU64': decimalU64(value, where); return;
    case 'integer': {
      if (typeof value !== 'number' || !Number.isSafeInteger(value)
        || (schema.minimum !== undefined && value < schema.minimum)
        || (schema.maximum !== undefined && value > schema.maximum)) malformed(`${where} must be a bounded integer`);
      return;
    }
    case 'string': {
      const result = boundedString(value, schema.maximumBytes, where);
      if (schema.pattern === 'identifier') validateIdentity(result, where);
      return;
    }
    case 'array': {
      if (!Array.isArray(value) || value.length > schema.maximumItems) malformed(`${where} must be a bounded array`);
      const array = ordinaryJsonArray(value, where);
      array.forEach((item, index) => validateWireValue(item, schema.items, `${where}[${index}]`, depth + 1));
      return;
    }
    case 'enum': if (typeof value !== 'string' || !schema.values.includes(value)) malformed(`${where} must be an admitted enum value`); return;
    case 'taggedUnion': {
      const record = ordinaryJsonObject(value, where);
      if (!hasOwn(record, schema.tag)) malformed(`${where}.${schema.tag} is required`);
      const tag = string(record[schema.tag], `${where}.${schema.tag}`);
      if (!hasOwn(schema.variants, tag)) malformed(`${where}.${schema.tag} is invalid`);
      const variant = schema.variants[tag]!;
      validateWireValue(value, variant, where, depth + 1); return;
    }
    case 'opaqueJson': { validateOpaqueJson(value, schema.maximumBytes, schema.maximumNodes, where); return; }
    case 'object': {
      const record = ordinaryJsonObject(value, where);
      const fields = schema.fields;
      for (const key of Object.keys(record)) if (!hasOwn(fields, key)) malformed(`${where}.${key} is not allowed`);
      for (const [key, field] of Object.entries(fields)) {
        if (!hasOwn(record, key)) { if (field.required) malformed(`${where}.${key} is required`); continue; }
        validateWireValue(record[key], field.value, `${where}.${key}`, depth + 1);
      }
      return;
    }
  }
}

function cryptoRandom(): string { return typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID().toLowerCase() : `${Date.now()}-${Math.random()}`.replace(/[^a-z0-9.-]/gu, '-'); }
function throwIfAborted(signal: AbortSignal | undefined): void { if (signal?.aborted) throw new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled'); }
function translateAdapterFailure(cause: unknown): RustyDeveloperCommandClientError { if (cause instanceof RustyDeveloperCommandClientError) return cause; if (cause instanceof DOMException && cause.name === 'AbortError') return new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled', { cause }); return new RustyDeveloperCommandClientError('unavailable', `Developer command adapter is unavailable: ${errorMessage(cause)}`, { cause: cause instanceof Error ? cause : undefined }); }
function errorMessage(cause: unknown): string { return cause instanceof Error ? cause.message : String(cause); }
function malformed(message: string): never { throw new RustyDeveloperCommandClientError('malformed', message); }
function invalidSchema(message: string): never { throw new RustyDeveloperCommandClientError('invalid_schema', message); }
function hasOwn(record: object, key: PropertyKey): boolean { return Object.prototype.hasOwnProperty.call(record, key); }
function object(value: unknown, where: string): Record<string, unknown> { return ordinaryJsonObject(value, where); }
function exactKeys(record: Record<string, unknown>, expected: readonly string[], where: string): void { if (Object.keys(record).some((key) => !expected.includes(key)) || expected.some((key) => !hasOwn(record, key))) malformed(`${where} has unexpected or missing fields`); }
function string(value: unknown, where: string): string { if (typeof value !== 'string') malformed(`${where} must be string`); return value; }
function boundedString(value: unknown, maximum: number, where: string): string { const result = string(value, where); if (new TextEncoder().encode(result).byteLength > maximum) malformed(`${where} exceeds ${maximum} bytes`); return result; }
function identity(value: unknown, where: string): string { const result = boundedString(value, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes, where); validateIdentity(result, where); return result; }
function validateIdentity(value: string, where: string): void { if (!/^[a-z0-9._:-]+$/u.test(value)) malformed(`${where} must use lower-case command identity characters`); }
function decodeProtocolVersion(value: unknown, where: string): typeof RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION {
  if (value !== RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION) malformed(`${where} is unsupported`);
  return RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
}
function decodeLane(value: unknown, where: string): RustyDeveloperCommandLane {
  const lane = string(value, where) as RustyDeveloperCommandLane;
  if (!(GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes as readonly string[]).includes(lane)) malformed(`${where} is invalid`);
  return lane;
}
function decimalU64(value: unknown, where: string): string {
  const result = boundedString(value, 20, where);
  if (!/^(?:0|[1-9][0-9]*)$/u.test(result) || BigInt(result) > 18_446_744_073_709_551_615n) malformed(`${where} must be an unsigned 64-bit decimal string`);
  return result;
}
function decimalLessThan(left: string, right: string): boolean { return BigInt(left) < BigInt(right); }
function cloneJson(value: unknown): unknown { try { return JSON.parse(JSON.stringify(value)) as unknown; } catch (cause) { throw new RustyDeveloperCommandClientError('malformed', 'Developer command values must be JSON-compatible', { cause: cause instanceof Error ? cause : undefined }); } }
function ordinaryJsonObject(value: unknown, where: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) malformed(`${where} must be an object`);
  const record = value as Record<string, unknown>;
  if (Object.getPrototypeOf(record) !== Object.prototype) malformed(`${where} must use the ordinary object prototype`);
  if ('toJSON' in record || Object.getOwnPropertySymbols(record).length > 0) malformed(`${where} has non-JSON hooks`);
  const descriptors = Object.getOwnPropertyDescriptors(record);
  if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor))) malformed(`${where} has accessor or hidden property`);
  return record;
}
function ordinaryJsonArray(value: unknown, where: string): readonly unknown[] {
  if (!Array.isArray(value)) malformed(`${where} must be an array`);
  if (Object.getPrototypeOf(value) !== Array.prototype) malformed(`${where} must use the ordinary array prototype`);
  if ('toJSON' in value || Object.getOwnPropertySymbols(value).length > 0) malformed(`${where} has non-JSON hooks`);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const [key, descriptor] of Object.entries(descriptors)) {
    if (key === 'length') {
      if (descriptor.enumerable || !('value' in descriptor)) malformed(`${where} has an invalid length property`);
    } else if (!descriptor.enumerable || !('value' in descriptor)) {
      malformed(`${where} has accessor or hidden property`);
    }
  }
  if (Object.keys(value).length !== value.length) malformed(`${where} must be a dense ordinary array`);
  for (let index = 0; index < value.length; index += 1) {
    if (!hasOwn(value, index)) malformed(`${where} must be a dense ordinary array`);
  }
  return value;
}
function validateOpaqueJson(value: unknown, maximumBytes: number, maximumNodes: number, where: string): void {
  const seen = new Set<unknown>();
  let nodes = 0;
  const visit = (entry: unknown, depth: number): void => {
    if (depth > MAX_WIRE_DEPTH || ++nodes > maximumNodes) malformed(`${where} exceeds opaque JSON bounds`);
    if (entry === null || typeof entry === 'string' || typeof entry === 'boolean') return;
    if (typeof entry === 'number') {
      if (!Number.isFinite(entry) || Object.is(entry, -0)) malformed(`${where} has noncanonical number`);
      return;
    }
    if (typeof entry !== 'object' || seen.has(entry)) malformed(`${where} is not acyclic JSON`);
    seen.add(entry);
    if (Array.isArray(entry)) {
      ordinaryJsonArray(entry, where);
      entry.forEach((child) => visit(child, depth + 1));
      return;
    }
    ordinaryJsonObject(entry, where);
    Object.values(entry).forEach((child) => visit(child, depth + 1));
  };
  visit(value, 0);
  if (new TextEncoder().encode(JSON.stringify(value)).byteLength > maximumBytes) malformed(`${where} exceeds opaque JSON bytes`);
}
