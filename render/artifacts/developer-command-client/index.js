import { GENERATED_DEVELOPER_COMMAND_CONTRACT } from './generated-developer-command-contract.js';
import { GENERATED_STANDARD_HOST_WIRE } from './generated-standard-host-wire.js';
/**
 * Public, transport-neutral developer-command client and optional application-host
 * pull-down console.  It intentionally knows no gameplay semantics: a product
 * supplies a bounded adapter, discovery snapshot, and (where it wants a form)
 * an explicit wire schema.  Descriptor help is deliberately not a schema.
 */
export const RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION = GENERATED_DEVELOPER_COMMAND_CONTRACT.protocolVersion;
/** Exact schemas generated from developer-command-standard host DTOs. */
export const RUSTY_STANDARD_HOST_WIRE_SCHEMAS = GENERATED_STANDARD_HOST_WIRE.commands;
/** @deprecated Use `RUSTY_STANDARD_HOST_WIRE_SCHEMAS`; the host wire also includes inspect. */
export const RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS = RUSTY_STANDARD_HOST_WIRE_SCHEMAS;
const MAX_HISTORY = GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.historyEntries;
const MAX_SEQUENCE = 128;
const MAX_COMMANDS = 256;
const MAX_WIRE_DEPTH = 16;
const MAX_SCHEMA_NODES = 256;
const MAX_SCHEMA_FIELDS = 256;
export class RustyDeveloperCommandClientError extends Error {
    code;
    constructor(code, message, options) {
        super(message, options);
        this.name = 'RustyDeveloperCommandClientError';
        this.code = code;
    }
}
export function createRustyDeveloperCommandClient(options) {
    const schemaAttachments = composeSchemas(options.schemas ?? {}, options.extensions ?? []);
    const schemas = schemaAttachments.schemas;
    const issued = new Set();
    const entries = [];
    const correlation = options.createCorrelation ?? (() => `command-${cryptoRandom()}`);
    const now = options.now ?? Date.now;
    let discovery = null;
    let discoveryFloor = null;
    let disposed = false;
    const requireActive = () => {
        if (disposed)
            throw new RustyDeveloperCommandClientError('disposed', 'Developer command client is disposed');
    };
    const trimHistory = () => {
        if (entries.length > MAX_HISTORY)
            entries.splice(0, entries.length - MAX_HISTORY);
    };
    const recordFailure = (phase, lane, failure, request) => {
        const message = failure.message.slice(0, 1024);
        const entry = request === undefined
            ? Object.freeze({ phase, lane, code: failure.code, message, receiptRefs: [], at: now() })
            : Object.freeze({ phase, lane, code: failure.code, message, request, receiptRefs: [], at: now() });
        entries.push(entry);
        trimHistory();
    };
    const resolveDescriptor = (id) => {
        const snapshot = discovery;
        if (snapshot === null)
            return null;
        return snapshot.commands
            .find((candidate) => candidate.id === id || candidate.aliases.includes(id)) ?? null;
    };
    const refresh = async (signal) => {
        requireActive();
        throwIfAborted(signal);
        let raw;
        try {
            raw = await options.adapter.discover(signal);
        }
        catch (cause) {
            throw translateAdapterFailure(cause);
        }
        requireActive();
        throwIfAborted(signal);
        let candidate;
        try {
            candidate = decodeDiscovery(raw, schemaAttachments);
        }
        catch (cause) {
            // A malformed or unreconciled discovery has revoked the only current
            // executable catalog. Keep the monotonic floor so a late stale snapshot
            // is still rejected, but never retain descriptors/codecs from the last
            // successful catalog after authority has denied their binding.
            if (cause instanceof RustyDeveloperCommandClientError
                && (cause.code === 'malformed' || cause.code === 'invalid_extension')) {
                discovery = null;
            }
            throw cause;
        }
        const current = discovery;
        const floor = discoveryFloor;
        const candidateMatchesCatalogFloor = floor !== null
            && candidate.runtime === floor.runtime
            && candidate.profile === floor.profile
            && candidate.catalogEpoch === floor.catalogEpoch;
        if (floor !== null && (candidate.runtime !== floor.runtime || candidate.profile !== floor.profile
            || decimalLessThan(candidate.revision, floor.revision)
            || decimalLessThan(candidate.catalogEpoch, floor.catalogEpoch)
            || (candidateMatchesCatalogFloor
                && floor.contractFingerprint !== undefined
                && candidate.contractFingerprint !== floor.contractFingerprint))) {
            throw new RustyDeveloperCommandClientError('stale_context', 'Developer command discovery regressed or changed its selected runtime/profile context');
        }
        if (current !== null && (candidate.runtime !== current.runtime || candidate.profile !== current.profile
            || decimalLessThan(candidate.revision, current.revision)
            || decimalLessThan(candidate.catalogEpoch, current.catalogEpoch))) {
            throw new RustyDeveloperCommandClientError('stale_context', 'Developer command discovery regressed or changed its selected runtime/profile context');
        }
        discovery = candidate;
        discoveryFloor = discoveryFloorFrom(candidate);
        return candidate;
    };
    return Object.freeze({
        discover: refresh,
        descriptor: (commandOrAlias) => resolveDescriptor(commandOrAlias),
        schema: (command) => {
            const descriptor = resolveDescriptor(command);
            return descriptor?.id === command && hasOwn(schemas, command) ? schemas[command] : null;
        },
        history: () => Object.freeze(entries.slice()),
        exportSequence: () => Object.freeze({
            kind: 'rusty_developer_command.sequence.v1',
            note: 'portable command intent/history; not deterministic replay',
            entries: Object.freeze(entries.filter((entry) => entry.phase === 'completed').slice(-MAX_SEQUENCE)),
        }),
        execute: async (command, payload, signal) => {
            requireActive();
            throwIfAborted(signal);
            const snapshot = discovery ?? await refresh(signal);
            const descriptor = snapshot.commands.find((candidate) => candidate.id === command || candidate.aliases.includes(command)) ?? null;
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
            let requestPayload;
            try {
                validateWireValue(payload, schema.request, '$');
                requestPayload = cloneJson(payload);
            }
            catch (cause) {
                const failure = cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed'
                    ? new RustyDeveloperCommandClientError('invalid_payload', cause.message, { cause })
                    : translateAdapterFailure(cause);
                recordFailure('pre-dispatch', descriptor.lane, failure);
                throw failure;
            }
            let id;
            try {
                id = correlation();
                validateIdentity(id, 'correlation');
            }
            catch (cause) {
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
            }
            catch (cause) {
                const failure = cause instanceof RustyDeveloperCommandClientError
                    ? cause
                    : new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled', { cause: cause instanceof Error ? cause : undefined });
                recordFailure('pre-dispatch', descriptor.lane, failure);
                throw failure;
            }
            issued.add(id);
            const request = Object.freeze({
                protocolVersion: RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION,
                command: descriptor.id,
                correlation: id,
                runtime: snapshot.runtime,
                expected: Object.freeze({
                    profile: snapshot.profile, revision: snapshot.revision, catalogEpoch: snapshot.catalogEpoch,
                }),
                payload: requestPayload,
            });
            let raw;
            try {
                raw = await options.adapter.execute(request, signal);
            }
            catch (cause) {
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
                const floor = discoveryFloor;
                const currentCatalogMatchFloor = floor !== null
                    && current !== null
                    && current.runtime === floor.runtime
                    && current.profile === floor.profile
                    && current.catalogEpoch === floor.catalogEpoch;
                if (current === null
                    || current.runtime !== snapshot.runtime
                    || current.profile !== snapshot.profile
                    || (current.revision === snapshot.revision
                        && current.catalogEpoch === snapshot.catalogEpoch
                        && currentCatalogMatchFloor
                        && floor?.contractFingerprint !== undefined
                        && floor.contractFingerprint !== snapshot.contractFingerprint)
                    || response.runtime !== current.runtime
                    || response.profile !== current.profile
                    || decimalLessThan(response.revision, current.revision)
                    || decimalLessThan(response.catalogEpoch, current.catalogEpoch)) {
                    throw new RustyDeveloperCommandClientError('stale_context', 'Developer command response did not preserve the selected runtime/profile/epoch context');
                }
                requireActive();
                throwIfAborted(signal);
                const entry = Object.freeze({
                    phase: 'completed',
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
                if (decimalLessThan(current.catalogEpoch, response.catalogEpoch)) {
                    // The response establishes a newer monotonic floor, but it carries
                    // no catalog fingerprint.  Keep descriptors invalidated until a
                    // fresh accepted discovery supplies one for these exact facts.
                    discovery = null;
                    discoveryFloor = discoveryFloorFromResponse(response);
                }
                else {
                    discovery = Object.freeze({ ...current, revision: response.revision, catalogEpoch: response.catalogEpoch });
                    discoveryFloor = discoveryFloorFromResponse(response, currentCatalogMatchFloor
                        && response.catalogEpoch === current.catalogEpoch
                        ? floor?.contractFingerprint
                        : undefined);
                }
                return response;
            }
            catch (cause) {
                const failure = cause instanceof RustyDeveloperCommandClientError
                    ? cause
                    : new RustyDeveloperCommandClientError('malformed', 'Developer command response was not valid', { cause: cause instanceof Error ? cause : undefined });
                const phase = failure.code === 'cancelled' || failure.code === 'disposed' || failure.code === 'unavailable'
                    ? 'transport' : 'post-dispatch';
                recordFailure(phase, descriptor.lane, failure, request);
                throw failure;
            }
        },
        dispose: () => { disposed = true; discovery = null; discoveryFloor = null; },
    });
}
function discoveryFloorFrom(snapshot) {
    return Object.freeze({
        runtime: snapshot.runtime,
        profile: snapshot.profile,
        revision: snapshot.revision,
        catalogEpoch: snapshot.catalogEpoch,
        contractFingerprint: snapshot.contractFingerprint,
    });
}
function discoveryFloorFromResponse(response, contractFingerprint) {
    return Object.freeze({
        runtime: response.runtime,
        profile: response.profile,
        revision: response.revision,
        catalogEpoch: response.catalogEpoch,
        ...(contractFingerprint === undefined ? {} : { contractFingerprint }),
    });
}
function composeSchemas(base, extensions) {
    const result = Object.create(null);
    const extensionBindings = [];
    for (const [id, schema] of Object.entries(base)) {
        validateSchemaIdentity(id, `schema.${id}`);
        result[id] = admitWireSchema(schema, `schema.${id}`);
    }
    for (const extension of extensions) {
        const extensionRecord = extensionObject(extension, 'extension');
        extensionExactKeys(extensionRecord, ['namespace', 'schemas'], 'extension');
        const namespace = validateExtensionNamespace(extensionRecord['namespace']);
        const bindings = extensionArray(extensionRecord['schemas'], `extension ${namespace}.schemas`);
        for (const [index, value] of bindings.entries()) {
            const where = `extension ${namespace}.schemas[${index}]`;
            const binding = extensionObject(value, where);
            extensionExactKeys(binding, ['command', 'lane', 'profile', 'schema'], where);
            const command = extensionIdentity(binding['command'], `${where}.command`);
            if (!command.startsWith(`${namespace}.`)) {
                invalidExtension(`${where}.command escapes ${namespace}`);
            }
            if (hasOwn(result, command))
                invalidExtension(`Duplicate wire schema ${command}`);
            const lane = extensionLane(binding['lane'], `${where}.lane`);
            const profile = extensionIdentity(binding['profile'], `${where}.profile`);
            result[command] = admitWireSchema(binding['schema'], `${where}.schema`);
            extensionBindings.push(Object.freeze({ command, lane, profile, schema: result[command] }));
        }
    }
    return Object.freeze({ schemas: Object.freeze(result), extensionBindings: Object.freeze(extensionBindings) });
}
function admitWireSchema(value, where) {
    const context = { active: new Set(), seen: new Set(), nodes: 0 };
    const record = schemaObject(value, where);
    schemaExactKeys(record, ['request', 'result', 'error'], where);
    return Object.freeze({
        request: admitValueSchema(record['request'], `${where}.request`, context, 0),
        result: admitValueSchema(record['result'], `${where}.result`, context, 0),
        error: admitValueSchema(record['error'], `${where}.error`, context, 0),
    });
}
function admitValueSchema(value, where, context, depth) {
    if (depth > MAX_WIRE_DEPTH)
        invalidSchema(`${where} exceeds wire schema depth`);
    const record = schemaObject(value, where);
    if (context.active.has(record))
        invalidSchema(`${where} contains a cyclic schema`);
    if (!context.seen.has(record)) {
        context.seen.add(record);
        context.nodes += 1;
        if (context.nodes > MAX_SCHEMA_NODES)
            invalidSchema(`${where} exceeds the ${MAX_SCHEMA_NODES}-node schema limit`);
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
                if (minimum !== undefined && maximum !== undefined && minimum > maximum)
                    invalidSchema(`${where} minimum exceeds maximum`);
                return Object.freeze({ kind, ...(minimum === undefined ? {} : { minimum }), ...(maximum === undefined ? {} : { maximum }) });
            }
            case 'string': {
                schemaExactKeys(record, ['kind', 'maximumBytes'], where, ['pattern']);
                const maximumBytes = boundedSchemaNumber(record['maximumBytes'], `${where}.maximumBytes`, 0, 1_048_576);
                const pattern = record['pattern'];
                if (pattern !== undefined && pattern !== 'identifier')
                    invalidSchema(`${where}.pattern is not supported`);
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
                if (fieldEntries.length > MAX_SCHEMA_FIELDS)
                    invalidSchema(`${where}.fields exceeds the ${MAX_SCHEMA_FIELDS}-field limit`);
                const fields = Object.create(null);
                for (const [key, field] of fieldEntries) {
                    const fieldWhere = `${where}.fields.${key}`;
                    validateSchemaFieldName(key, fieldWhere);
                    const fieldRecordValue = schemaObject(field, fieldWhere);
                    schemaExactKeys(fieldRecordValue, ['required', 'value'], fieldWhere);
                    if (typeof fieldRecordValue['required'] !== 'boolean')
                        invalidSchema(`${fieldWhere}.required must be boolean`);
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
                if (!Array.isArray(values) || values.length === 0 || values.length > MAX_SCHEMA_FIELDS)
                    invalidSchema(`${where}.values must be a bounded nonempty array`);
                const normalized = values.map((item, index) => {
                    const itemWhere = `${where}.values[${index}]`;
                    const text = schemaText(item, itemWhere, 256);
                    return text;
                });
                if (new Set(normalized).size !== normalized.length)
                    invalidSchema(`${where}.values contains duplicates`);
                return Object.freeze({ kind, values: Object.freeze(normalized) });
            }
            case 'taggedUnion': {
                schemaExactKeys(record, ['kind', 'tag', 'variants'], where);
                const tag = schemaText(record['tag'], `${where}.tag`, 128);
                const variantsRecord = schemaObject(record['variants'], `${where}.variants`);
                const variantsEntries = Object.entries(variantsRecord);
                if (variantsEntries.length === 0 || variantsEntries.length > MAX_SCHEMA_FIELDS)
                    invalidSchema(`${where}.variants must be a bounded nonempty object`);
                const variants = Object.create(null);
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
    }
    finally {
        context.active.delete(record);
    }
}
function schemaObject(value, where) {
    if (typeof value !== 'object' || value === null || Array.isArray(value))
        invalidSchema(`${where} must be an object`);
    const record = value;
    const prototype = Object.getPrototypeOf(record);
    if (prototype !== Object.prototype && prototype !== null)
        invalidSchema(`${where} must be a plain object`);
    if (Object.getOwnPropertySymbols(record).length > 0)
        invalidSchema(`${where} may not contain symbol properties`);
    const descriptors = Object.getOwnPropertyDescriptors(record);
    if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor))) {
        invalidSchema(`${where} may not contain accessors or hidden properties`);
    }
    return record;
}
function schemaExactKeys(record, required, where, optional = []) {
    const allowed = new Set([...required, ...optional]);
    if (Object.keys(record).some((key) => !allowed.has(key)) || required.some((key) => !hasOwn(record, key))) {
        invalidSchema(`${where} has unexpected or missing fields`);
    }
}
function schemaKind(value, where) {
    if (typeof value !== 'string' || !['boolean', 'decimalU64', 'integer', 'string', 'array', 'object', 'enum', 'taggedUnion', 'opaqueJson'].includes(value)) {
        invalidSchema(`${where} is not a supported schema kind`);
    }
    return value;
}
function schemaText(value, where, maximumBytes) {
    if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > maximumBytes)
        invalidSchema(`${where} must be a bounded string`);
    return value;
}
function boundedSchemaNumber(value, where, minimum, maximum) {
    if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum || value > maximum) {
        invalidSchema(`${where} must be a bounded nonnegative integer`);
    }
    return value;
}
function optionalSafeInteger(value, where) {
    if (value === undefined)
        return undefined;
    if (typeof value !== 'number' || !Number.isSafeInteger(value))
        invalidSchema(`${where} must be a safe integer`);
    return value;
}
function validateSchemaFieldName(value, where) {
    if (value.length === 0 || value === '__proto__' || value === 'constructor' || value === 'prototype')
        invalidSchema(`${where} is not a usable field name`);
    schemaText(value, where, 128);
}
function validateSchemaIdentity(value, where) {
    if (!/^[a-z0-9._:-]+$/u.test(value) || new TextEncoder().encode(value).byteLength > GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes) {
        throw new RustyDeveloperCommandClientError('invalid_schema', `${where} is not a valid command identity`);
    }
}
function invalidExtension(message) {
    throw new RustyDeveloperCommandClientError('invalid_extension', message);
}
function extensionObject(value, where) {
    if (typeof value !== 'object' || value === null || Array.isArray(value))
        invalidExtension(`${where} must be an object`);
    const record = value;
    const prototype = Object.getPrototypeOf(record);
    if (prototype !== Object.prototype && prototype !== null)
        invalidExtension(`${where} must be a plain object`);
    if (Object.getOwnPropertySymbols(record).length > 0)
        invalidExtension(`${where} may not contain symbol properties`);
    const descriptors = Object.getOwnPropertyDescriptors(record);
    if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor))) {
        invalidExtension(`${where} may not contain accessors or hidden properties`);
    }
    return record;
}
function extensionExactKeys(record, required, where, optional = []) {
    const allowed = new Set([...required, ...optional]);
    if (Object.keys(record).some((key) => !allowed.has(key)) || required.some((key) => !hasOwn(record, key))) {
        invalidExtension(`${where} has unexpected or missing fields`);
    }
}
function extensionArray(value, where) {
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
function extensionText(value, where, maximumBytes) {
    if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > maximumBytes) {
        invalidExtension(`${where} must be a bounded string`);
    }
    return value;
}
function extensionIdentity(value, where) {
    const result = extensionText(value, where, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes);
    if (!/^[a-z0-9._:-]+$/u.test(result))
        invalidExtension(`${where} must use lower-case command identity characters`);
    return result;
}
function validateExtensionNamespace(value) {
    const namespace = extensionIdentity(value, 'extension namespace');
    if (namespace.includes(':'))
        invalidExtension('extension namespace may not contain colon');
    return namespace;
}
function extensionLane(value, where) {
    const lane = extensionText(value, where, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes);
    if (!GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes.includes(lane))
        invalidExtension(`${where} is invalid`);
    return lane;
}
function decodeDiscovery(value, attachments) {
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
    if (new Set(permittedLanes).size !== permittedLanes.length)
        malformed('discovery.permittedLanes must not contain duplicates');
    const revision = decimalU64(record['revision'], 'discovery.revision');
    const catalogEpoch = decimalU64(record['catalogEpoch'], 'discovery.catalogEpoch');
    const contractFingerprint = identity(record['contractFingerprint'], 'discovery.contractFingerprint');
    if (!Array.isArray(record['commands']))
        malformed('discovery.commands must be a dense ordinary array');
    const commandValues = ordinaryJsonArray(record['commands'], 'discovery.commands');
    if (commandValues.length > MAX_COMMANDS)
        malformed('discovery.commands must be a bounded array');
    const commands = commandValues.map((item, index) => decodeDescriptor(item, `discovery.commands[${index}]`));
    const identities = new Set();
    for (const descriptor of commands) {
        for (const identityValue of [descriptor.id, ...descriptor.aliases]) {
            if (identities.has(identityValue))
                malformed(`duplicate command or alias ${identityValue}`);
            identities.add(identityValue);
        }
    }
    if (commands.some((command) => !permittedLanes.includes(command.lane)))
        malformed('discovery command lane is not permitted by its selected profile');
    const discoveredById = new Map(commands.map((command) => [command.id, command]));
    for (const binding of attachments.extensionBindings) {
        const descriptor = discoveredById.get(binding.command);
        if (descriptor === undefined)
            invalidExtension(`Schema binding ${binding.command} has no available discovered command`);
        if (binding.profile !== profile)
            invalidExtension(`Schema binding ${binding.command} expects profile ${binding.profile}, not ${profile}`);
        if (binding.lane !== descriptor.lane)
            invalidExtension(`Schema binding ${binding.command} expects lane ${binding.lane}, not ${descriptor.lane}`);
    }
    return Object.freeze({ protocolVersion, runtime, profile, permittedLanes: Object.freeze(permittedLanes), revision, catalogEpoch, contractFingerprint, commands: Object.freeze(commands) });
}
function decodeDescriptor(value, where) {
    const record = object(value, where);
    exactKeys(record, ['id', 'aliases', 'lane', 'summary'], where);
    const id = identity(record['id'], `${where}.id`);
    if (!Array.isArray(record['aliases']) || record['aliases'].length > GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.commandAliases)
        malformed(`${where}.aliases must be bounded`);
    const aliases = record['aliases'].map((item, index) => identity(item, `${where}.aliases[${index}]`));
    const lane = decodeLane(record['lane'], `${where}.lane`);
    const summary = boundedString(record['summary'], GENERATED_DEVELOPER_COMMAND_CONTRACT.limits.summaryBytes, `${where}.summary`);
    return Object.freeze({ id, aliases: Object.freeze(aliases), lane, summary, helpOnly: true });
}
function decodeResponse(value, request, schema) {
    const record = object(value, 'response');
    exactKeys(record, ['correlation', 'runtime', 'profile', 'revision', 'catalogEpoch', 'outcome'], 'response');
    const correlation = identity(record['correlation'], 'response.correlation');
    if (correlation !== request.correlation)
        malformed('response correlation does not match request');
    const runtime = identity(record['runtime'], 'response.runtime');
    const profile = identity(record['profile'], 'response.profile');
    const revision = decimalU64(record['revision'], 'response.revision');
    const catalogEpoch = decimalU64(record['catalogEpoch'], 'response.catalogEpoch');
    const outcomeRecord = object(record['outcome'], 'response.outcome');
    const kind = string(outcomeRecord['kind'], 'response.outcome.kind');
    let outcome;
    if (kind === 'success') {
        exactKeys(outcomeRecord, ['kind', 'value', 'receiptRefs'], 'response.outcome');
        if (!Array.isArray(outcomeRecord['receiptRefs']) || outcomeRecord['receiptRefs'].length > 32)
            malformed('response receipt refs must be bounded');
        const receiptRefs = outcomeRecord['receiptRefs'].map((entry, index) => identity(entry, `response.receiptRefs[${index}]`));
        if (schema !== undefined)
            validateWireValue(outcomeRecord['value'], schema.result, '$result');
        outcome = Object.freeze({ kind: 'success', value: cloneJson(outcomeRecord['value']), receiptRefs: Object.freeze(receiptRefs) });
    }
    else if (kind === 'error') {
        const keys = Object.keys(outcomeRecord);
        if (!keys.every((key) => ['kind', 'code', 'message', 'details'].includes(key)) || !hasOwn(outcomeRecord, 'code') || !hasOwn(outcomeRecord, 'message'))
            malformed('response error has invalid fields');
        if (schema !== undefined && hasOwn(outcomeRecord, 'details'))
            validateWireValue(outcomeRecord['details'], schema.error, '$error');
        outcome = Object.freeze({ kind: 'error', code: identity(outcomeRecord['code'], 'response.outcome.code'), message: boundedString(outcomeRecord['message'], 1024, 'response.outcome.message'), ...(hasOwn(outcomeRecord, 'details') ? { details: cloneJson(outcomeRecord['details']) } : {}) });
    }
    else {
        malformed('response outcome kind is invalid');
    }
    return Object.freeze({ correlation, runtime, profile, revision, catalogEpoch, outcome });
}
export function validateRustyDeveloperCommandWireValue(value, schema) { validateWireValue(value, schema, '$'); }
function validateWireValue(value, schema, where, depth = 0) {
    if (depth > MAX_WIRE_DEPTH)
        malformed(`${where} exceeds wire depth`);
    switch (schema.kind) {
        case 'boolean':
            if (typeof value !== 'boolean')
                malformed(`${where} must be boolean`);
            return;
        case 'decimalU64':
            decimalU64(value, where);
            return;
        case 'integer': {
            if (typeof value !== 'number' || !Number.isSafeInteger(value)
                || (schema.minimum !== undefined && value < schema.minimum)
                || (schema.maximum !== undefined && value > schema.maximum))
                malformed(`${where} must be a bounded integer`);
            return;
        }
        case 'string': {
            const result = boundedString(value, schema.maximumBytes, where);
            if (schema.pattern === 'identifier')
                validateIdentity(result, where);
            return;
        }
        case 'array': {
            if (!Array.isArray(value) || value.length > schema.maximumItems)
                malformed(`${where} must be a bounded array`);
            const array = ordinaryJsonArray(value, where);
            array.forEach((item, index) => validateWireValue(item, schema.items, `${where}[${index}]`, depth + 1));
            return;
        }
        case 'enum':
            if (typeof value !== 'string' || !schema.values.includes(value))
                malformed(`${where} must be an admitted enum value`);
            return;
        case 'taggedUnion': {
            const record = ordinaryJsonObject(value, where);
            if (!hasOwn(record, schema.tag))
                malformed(`${where}.${schema.tag} is required`);
            const tag = string(record[schema.tag], `${where}.${schema.tag}`);
            if (!hasOwn(schema.variants, tag))
                malformed(`${where}.${schema.tag} is invalid`);
            const variant = schema.variants[tag];
            validateWireValue(value, variant, where, depth + 1);
            return;
        }
        case 'opaqueJson': {
            validateOpaqueJson(value, schema.maximumBytes, schema.maximumNodes, where);
            return;
        }
        case 'object': {
            const record = ordinaryJsonObject(value, where);
            const fields = schema.fields;
            for (const key of Object.keys(record))
                if (!hasOwn(fields, key))
                    malformed(`${where}.${key} is not allowed`);
            for (const [key, field] of Object.entries(fields)) {
                if (!hasOwn(record, key)) {
                    if (field.required)
                        malformed(`${where}.${key} is required`);
                    continue;
                }
                validateWireValue(record[key], field.value, `${where}.${key}`, depth + 1);
            }
            return;
        }
    }
}
function cryptoRandom() { return typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID().toLowerCase() : `${Date.now()}-${Math.random()}`.replace(/[^a-z0-9.-]/gu, '-'); }
function throwIfAborted(signal) { if (signal?.aborted)
    throw new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled'); }
function translateAdapterFailure(cause) { if (cause instanceof RustyDeveloperCommandClientError)
    return cause; if (cause instanceof DOMException && cause.name === 'AbortError')
    return new RustyDeveloperCommandClientError('cancelled', 'Developer command was cancelled', { cause }); return new RustyDeveloperCommandClientError('unavailable', `Developer command adapter is unavailable: ${errorMessage(cause)}`, { cause: cause instanceof Error ? cause : undefined }); }
function errorMessage(cause) { return cause instanceof Error ? cause.message : String(cause); }
function malformed(message) { throw new RustyDeveloperCommandClientError('malformed', message); }
function invalidSchema(message) { throw new RustyDeveloperCommandClientError('invalid_schema', message); }
function hasOwn(record, key) { return Object.prototype.hasOwnProperty.call(record, key); }
function object(value, where) { return ordinaryJsonObject(value, where); }
function exactKeys(record, expected, where) { if (Object.keys(record).some((key) => !expected.includes(key)) || expected.some((key) => !hasOwn(record, key)))
    malformed(`${where} has unexpected or missing fields`); }
function string(value, where) { if (typeof value !== 'string')
    malformed(`${where} must be string`); return value; }
function boundedString(value, maximum, where) { const result = string(value, where); if (new TextEncoder().encode(result).byteLength > maximum)
    malformed(`${where} exceeds ${maximum} bytes`); return result; }
function identity(value, where) { const result = boundedString(value, GENERATED_DEVELOPER_COMMAND_CONTRACT.identity.commandBytes, where); validateIdentity(result, where); return result; }
function validateIdentity(value, where) { if (!/^[a-z0-9._:-]+$/u.test(value))
    malformed(`${where} must use lower-case command identity characters`); }
function decodeProtocolVersion(value, where) {
    if (value !== RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION)
        malformed(`${where} is unsupported`);
    return RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
}
function decodeLane(value, where) {
    const lane = string(value, where);
    if (!GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes.includes(lane))
        malformed(`${where} is invalid`);
    return lane;
}
function decimalU64(value, where) {
    const result = boundedString(value, 20, where);
    if (!/^(?:0|[1-9][0-9]*)$/u.test(result) || BigInt(result) > 18446744073709551615n)
        malformed(`${where} must be an unsigned 64-bit decimal string`);
    return result;
}
function decimalLessThan(left, right) { return BigInt(left) < BigInt(right); }
function cloneJson(value) { try {
    return JSON.parse(JSON.stringify(value));
}
catch (cause) {
    throw new RustyDeveloperCommandClientError('malformed', 'Developer command values must be JSON-compatible', { cause: cause instanceof Error ? cause : undefined });
} }
function ordinaryJsonObject(value, where) {
    if (typeof value !== 'object' || value === null || Array.isArray(value))
        malformed(`${where} must be an object`);
    const record = value;
    if (Object.getPrototypeOf(record) !== Object.prototype)
        malformed(`${where} must use the ordinary object prototype`);
    if ('toJSON' in record || Object.getOwnPropertySymbols(record).length > 0)
        malformed(`${where} has non-JSON hooks`);
    const descriptors = Object.getOwnPropertyDescriptors(record);
    if (Object.values(descriptors).some((descriptor) => !descriptor.enumerable || !('value' in descriptor)))
        malformed(`${where} has accessor or hidden property`);
    return record;
}
function ordinaryJsonArray(value, where) {
    if (!Array.isArray(value))
        malformed(`${where} must be an array`);
    if (Object.getPrototypeOf(value) !== Array.prototype)
        malformed(`${where} must use the ordinary array prototype`);
    if ('toJSON' in value || Object.getOwnPropertySymbols(value).length > 0)
        malformed(`${where} has non-JSON hooks`);
    const descriptors = Object.getOwnPropertyDescriptors(value);
    for (const [key, descriptor] of Object.entries(descriptors)) {
        if (key === 'length') {
            if (descriptor.enumerable || !('value' in descriptor))
                malformed(`${where} has an invalid length property`);
        }
        else if (!descriptor.enumerable || !('value' in descriptor)) {
            malformed(`${where} has accessor or hidden property`);
        }
    }
    if (Object.keys(value).length !== value.length)
        malformed(`${where} must be a dense ordinary array`);
    for (let index = 0; index < value.length; index += 1) {
        if (!hasOwn(value, index))
            malformed(`${where} must be a dense ordinary array`);
    }
    return value;
}
function validateOpaqueJson(value, maximumBytes, maximumNodes, where) {
    const seen = new Set();
    let nodes = 0;
    const visit = (entry, depth) => {
        if (depth > MAX_WIRE_DEPTH || ++nodes > maximumNodes)
            malformed(`${where} exceeds opaque JSON bounds`);
        if (entry === null || typeof entry === 'string' || typeof entry === 'boolean')
            return;
        if (typeof entry === 'number') {
            if (!Number.isFinite(entry) || Object.is(entry, -0))
                malformed(`${where} has noncanonical number`);
            return;
        }
        if (typeof entry !== 'object' || seen.has(entry))
            malformed(`${where} is not acyclic JSON`);
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
    if (new TextEncoder().encode(JSON.stringify(value)).byteLength > maximumBytes)
        malformed(`${where} exceeds opaque JSON bytes`);
}
