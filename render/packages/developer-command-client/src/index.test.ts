import test from 'node:test';
import assert from 'node:assert/strict';

import {
  RustyDeveloperCommandClientError,
  createRustyDeveloperCommandClient,
  type RustyDeveloperCommandAdapter,
  type RustyDeveloperCommandExtension,
  type RustyDeveloperCommandRequest,
  type RustyDeveloperCommandValueSchema,
  type RustyDeveloperCommandWireSchema,
  RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS,
  validateRustyDeveloperCommandWireValue,
} from './index.js';

const inspectSchema: RustyDeveloperCommandWireSchema = {
  request: { kind: 'object', fields: { entity: { required: true, value: { kind: 'integer', minimum: 0 } } } },
  result: { kind: 'object', fields: { entity: { required: true, value: { kind: 'integer', minimum: 0 } } } },
  error: { kind: 'object', fields: {} },
};

function adapter(execute?: (request: RustyDeveloperCommandRequest) => unknown): RustyDeveloperCommandAdapter {
  return {
    discover: async () => ({
      protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect', 'play', 'admin'],
      revision: '4', catalogEpoch: '7', contractFingerprint: 'test-contract',
      commands: [{ id: 'standard.inspect.entity', aliases: ['inspect.entity'], lane: 'inspect', summary: 'Inspect one entity.' }],
    }),
    execute: async (request) => execute?.(request) ?? ({
      correlation: request.correlation, runtime: request.runtime, profile: request.expected.profile,
      revision: '4', catalogEpoch: '7', outcome: { kind: 'success', value: request.payload, receiptRefs: ['receipt-1'] },
    }),
  };
}

test('developer client strictly decodes discovery, payloads, responses, and history', async () => {
  const client = createRustyDeveloperCommandClient({
    adapter: adapter(), schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'unique-1', now: () => 12,
  });
  const discovery = await client.discover();
  assert.equal(discovery.runtime, 'test-runtime');
  const response = await client.execute('inspect.entity', { entity: 3 });
  assert.equal(response.outcome.kind, 'success');
  assert.equal(client.history()[0]?.at, 12);
  assert.equal(client.history()[0]?.lane, 'inspect');
  assert.equal(client.history()[0]?.phase, 'completed');
  assert.deepEqual(client.exportSequence().note, 'portable command intent/history; not deterministic replay');
  await assert.rejects(client.execute('inspect.entity', { entity: 3 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'correlation_reused');
  await assert.rejects(client.execute('inspect.entity', { entity: 3, surprise: true }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');
});

test('developer client rejects malformed and stale adapter responses without appending history', async () => {
  const malformed = createRustyDeveloperCommandClient({ adapter: adapter(() => ({ unexpected: true })), schemas: { 'standard.inspect.entity': inspectSchema } });
  await malformed.discover();
  await assert.rejects(malformed.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
  assert.equal(malformed.history().length, 1);
  assert.equal(malformed.history()[0]?.phase, 'post-dispatch');
  assert.equal(malformed.history()[0]?.lane, 'inspect');
  assert.deepEqual(malformed.history()[0]?.receiptRefs, []);
  assert.equal('request' in (malformed.history()[0] ?? {}), true);

  const stale = createRustyDeveloperCommandClient({ adapter: adapter((request) => ({
    correlation: request.correlation, runtime: request.runtime, profile: request.expected.profile,
    revision: '3', catalogEpoch: '7', outcome: { kind: 'success', value: { entity: 1 }, receiptRefs: [] },
  })), schemas: { 'standard.inspect.entity': inspectSchema } });
  await stale.discover();
  await assert.rejects(stale.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'stale_context');
  assert.equal(stale.history()[0]?.phase, 'post-dispatch');
  assert.equal(stale.history()[0]?.request?.command, 'standard.inspect.entity');
  assert.deepEqual(stale.history()[0]?.receiptRefs, []);
});

test('error responses require own code and message fields', async () => {
  const client = createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => ({
        protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect'],
        revision: '1', catalogEpoch: '1', contractFingerprint: 'test-contract',
        commands: [{ id: 'standard.inspect.entity', aliases: [], lane: 'inspect', summary: 'Inspect one entity.' }],
      }),
      execute: async (request) => ({
        correlation: request.correlation, runtime: 'test-runtime', profile: request.expected.profile,
        revision: '1', catalogEpoch: '1', outcome: { kind: 'error' },
      }),
    },
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'prototype-error',
  });
  await client.discover();
  const previousCode = Object.getOwnPropertyDescriptor(Object.prototype, 'code');
  const previousMessage = Object.getOwnPropertyDescriptor(Object.prototype, 'message');
  try {
    Object.defineProperty(Object.prototype, 'code', { configurable: true, enumerable: false, writable: true, value: 'inherited-code' });
    Object.defineProperty(Object.prototype, 'message', { configurable: true, enumerable: false, writable: true, value: 'inherited-message' });
    await assert.rejects(client.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
      cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
  } finally {
    if (previousCode === undefined) delete (Object.prototype as unknown as Record<string, unknown>)['code'];
    else Object.defineProperty(Object.prototype, 'code', previousCode);
    if (previousMessage === undefined) delete (Object.prototype as unknown as Record<string, unknown>)['message'];
    else Object.defineProperty(Object.prototype, 'message', previousMessage);
  }
});

test('discovery admits each lane once and rejects over-bounded or duplicate lane lists', async () => {
  const lanes = ['inspect', 'preview', 'play', 'admin', 'session', 'author', 'fault'] as const;
  const makeClient = (permittedLanes: readonly string[]) => createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => ({
        protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes,
        revision: '1', catalogEpoch: '1', contractFingerprint: 'test-contract', commands: [],
      }),
      execute: async () => null,
    },
  });

  const exact = await makeClient(lanes).discover();
  assert.deepEqual(exact.permittedLanes, lanes);

  await assert.rejects(makeClient([...lanes, 'inspect']).discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
  await assert.rejects(makeClient(['inspect', 'preview', 'inspect']).discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
});

test('discovery rejects sparse and inherited envelope arrays', async () => {
  const makeClient = (permittedLanes: unknown, commands: unknown) => createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => ({
        protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes,
        revision: '1', catalogEpoch: '1', contractFingerprint: 'test-contract', commands,
      }),
      execute: async () => null,
    },
  });
  const sparseLanes = new Array<string>(1);
  await assert.rejects(makeClient(sparseLanes, []).discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
  const sparseCommands = new Array<unknown>(1);
  await assert.rejects(makeClient(['inspect'], sparseCommands).discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');

  const previous = Object.getOwnPropertyDescriptor(Array.prototype, '0');
  try {
    Object.defineProperty(Array.prototype, '0', { configurable: true, enumerable: false, writable: true, value: 'inspect' });
    const inheritedLane = new Array<string>(1);
    await assert.rejects(makeClient(inheritedLane, []).discover(), (cause: unknown) =>
      cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
  } finally {
    if (previous === undefined) delete (Array.prototype as unknown as Record<string, unknown>)['0'];
    else Object.defineProperty(Array.prototype, '0', previous);
  }
});

test('product extensions must be namespaced and may provide their own real codec', async () => {
  const client = createRustyDeveloperCommandClient({
    adapter: adapter(),
    extensions: [{
      namespace: 'product',
      descriptors: [{ id: 'product.play.attack', aliases: ['product.attack'], lane: 'play', summary: 'Product play command.' }],
      schemas: { 'product.play.attack': inspectSchema },
    }],
  });
  await client.discover();
  assert.equal(client.descriptor('product.attack')?.id, 'product.play.attack');
  assert.throws(() => createRustyDeveloperCommandClient({ adapter: adapter(), extensions: [{
    namespace: 'product', descriptors: [{ id: 'wrong.command', aliases: [], lane: 'play', summary: 'Wrong.' }], schemas: {},
  }] }), RustyDeveloperCommandClientError);
});

test('discovery rejects aliases colliding across base and extension commands', async () => {
  assert.throws(() => createRustyDeveloperCommandClient({
    adapter: adapter(),
    extensions: [{ namespace: 'product', descriptors: [{ id: 'product.play.attack', aliases: ['inspect.entity'], lane: 'play', summary: 'Conflict.' }], schemas: {} }],
  }), (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_extension');
});

test('help-only commands and unavailable adapters cannot fabricate an invocation', async () => {
  const helpOnly = createRustyDeveloperCommandClient({ adapter: adapter() });
  await helpOnly.discover();
  await assert.rejects(helpOnly.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'codec_unavailable');
  const unavailable = createRustyDeveloperCommandClient({
    adapter: { discover: async () => { throw new Error('host disconnected'); }, execute: async () => null },
  });
  await assert.rejects(unavailable.discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'unavailable');
});

test('generated standard schemas admit every source variant and reject bad policy and opaque values', () => {
  const stat = RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS['standard.admin.stat.set-base']!;
  for (const source of [
    { kind: 'intrinsic', entity: '1', instance: 'source' },
    { kind: 'effect', entity: '1', effect: 'effect', stack: 1, source: 'source' },
    { kind: 'equippedItem', owner: '1', item: '2', source: 'source' },
    { kind: 'request', operation: 'operation', instance: 'source' },
  ]) validateRustyDeveloperCommandWireValue({ operation: 'operation', source, entity: '1', stat: 'stat', base: 1 }, stat.request);
  const track = RUSTY_STANDARD_ADMIN_WIRE_SCHEMAS['standard.admin.track.set']!;
  assert.throws(() => validateRustyDeveloperCommandWireValue({ operation: 'operation', source: { kind: 'request', operation: 'operation', instance: 'source' }, entity: '1', track: 'track', value: 1, policy: 'freestyle' }, track.request));
  const circular: { self?: unknown } = {}; circular.self = circular;
  assert.throws(() => validateRustyDeveloperCommandWireValue(circular, { kind: 'opaqueJson', maximumBytes: 32, maximumNodes: 4 }));
});

test('resolved pre-dispatch failures are visible without pretending a request was issued', async () => {
  const helpOnly = createRustyDeveloperCommandClient({ adapter: adapter() });
  await helpOnly.discover();
  await assert.rejects(helpOnly.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'codec_unavailable');
  const helpFailure = helpOnly.history()[0];
  assert.equal(helpFailure?.phase, 'pre-dispatch');
  assert.equal(helpFailure?.lane, 'inspect');
  assert.equal(helpFailure?.code, 'codec_unavailable');
  assert.deepEqual(helpFailure?.receiptRefs, []);
  assert.equal(helpFailure !== undefined && 'request' in helpFailure, false);

  const invalid = createRustyDeveloperCommandClient({ adapter: adapter(), schemas: { 'standard.inspect.entity': inspectSchema } });
  await invalid.discover();
  await assert.rejects(invalid.execute('standard.inspect.entity', { entity: -1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');
  const invalidFailure = invalid.history()[0];
  assert.equal(invalidFailure?.phase, 'pre-dispatch');
  assert.equal(invalidFailure?.lane, 'inspect');
  assert.equal(invalidFailure?.code, 'invalid_payload');
  assert.deepEqual(invalidFailure?.receiptRefs, []);
  assert.equal(invalidFailure !== undefined && 'request' in invalidFailure, false);

  let calls = 0;
  const reused = createRustyDeveloperCommandClient({
    adapter: adapter(), schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => { calls += 1; return 'reused'; },
  });
  await reused.discover();
  await reused.execute('standard.inspect.entity', { entity: 1 });
  await assert.rejects(reused.execute('standard.inspect.entity', { entity: 2 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'correlation_reused');
  assert.equal(calls, 2);
  const reuseFailure = reused.history()[1];
  assert.equal(reuseFailure?.phase, 'pre-dispatch');
  assert.equal(reuseFailure?.lane, 'inspect');
  assert.equal(reuseFailure?.code, 'correlation_reused');
  assert.deepEqual(reuseFailure?.receiptRefs, []);
  assert.equal(reuseFailure !== undefined && 'request' in reuseFailure, false);
});

test('transport failures and cancellation retain the issued request but never receipts', async () => {
  const unavailable = createRustyDeveloperCommandClient({
    adapter: adapter(() => { throw new Error('wire disconnected'); }),
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'transport-1',
  });
  await unavailable.discover();
  await assert.rejects(unavailable.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'unavailable');
  const unavailableFailure = unavailable.history()[0];
  assert.equal(unavailableFailure?.phase, 'transport');
  assert.equal(unavailableFailure?.lane, 'inspect');
  assert.equal(unavailableFailure?.request?.correlation, 'transport-1');
  assert.deepEqual(unavailableFailure?.receiptRefs, []);

  const controller = new AbortController();
  const cancelled = createRustyDeveloperCommandClient({
    adapter: adapter((request) => {
      controller.abort();
      return { unexpected: true };
    }),
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'cancel-1',
  });
  await cancelled.discover();
  await assert.rejects(cancelled.execute('standard.inspect.entity', { entity: 1 }, controller.signal), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'cancelled');
  const cancellationFailure = cancelled.history()[0];
  assert.equal(cancellationFailure?.phase, 'transport');
  assert.equal(cancellationFailure?.lane, 'inspect');
  assert.equal(cancellationFailure?.request?.correlation, 'cancel-1');
  assert.deepEqual(cancellationFailure?.receiptRefs, []);
});

test('disposal while pending fails closed and late responses cannot append completed history', async () => {
  let client: ReturnType<typeof createRustyDeveloperCommandClient>;
  const disposing = createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => ({
        protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect'],
        revision: '4', catalogEpoch: '7', contractFingerprint: 'test-contract',
        commands: [{ id: 'standard.inspect.entity', aliases: [], lane: 'inspect', summary: 'Inspect one entity.' }],
      }),
      execute: async (request) => {
        client.dispose();
        return {
          correlation: request.correlation, runtime: 'test-runtime', profile: 'developer', revision: '4', catalogEpoch: '7',
          outcome: { kind: 'success', value: { entity: 1 }, receiptRefs: ['late-receipt'] },
        };
      },
    },
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'dispose-1',
  });
  client = disposing;
  await client.discover();
  await assert.rejects(client.execute('standard.inspect.entity', { entity: 1 }), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'disposed');
  assert.equal(client.history().some((entry) => entry.phase === 'completed'), false);
  const failure = client.history()[0];
  assert.equal(failure?.phase, 'transport');
  assert.equal(failure?.code, 'disposed');
  assert.equal(failure?.request?.correlation, 'dispose-1');
  assert.deepEqual(failure?.receiptRefs, []);
});

test('signal cancellation is checked before response decoding', async () => {
  const controller = new AbortController();
  const client = createRustyDeveloperCommandClient({
    adapter: adapter(() => {
      controller.abort();
      return { unexpected: true };
    }),
    schemas: { 'standard.inspect.entity': inspectSchema },
  });
  await client.discover();
  await assert.rejects(client.execute('standard.inspect.entity', { entity: 1 }, controller.signal), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'cancelled');
  const cancellationFailure = client.history()[0];
  assert.equal(cancellationFailure?.phase, 'transport');
  if (cancellationFailure !== undefined && 'code' in cancellationFailure) {
    assert.equal(cancellationFailure.code, 'cancelled');
  }
});

test('portable sequences contain only completed schema-valid requests, not local failures', async () => {
  const client = createRustyDeveloperCommandClient({
    adapter: adapter(), schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: (() => { let count = 0; return () => `sequence-${++count}`; })(),
  });
  await client.discover();
  await assert.rejects(client.execute('standard.inspect.entity', { entity: -1 }));
  await assert.rejects(client.execute('missing.command', { entity: 1 }));
  await client.execute('inspect.entity', { entity: 3 });
  const sequence = client.exportSequence();
  assert.equal(sequence.note, 'portable command intent/history; not deterministic replay');
  assert.equal(sequence.entries.length, 1);
  assert.equal(sequence.entries[0]?.request.command, 'standard.inspect.entity');
  assert.deepEqual(sequence.entries[0]?.request.payload, { entity: 3 });
  assert.deepEqual(sequence.entries[0]?.receiptRefs, ['receipt-1']);
});

test('object and array codecs admit only ordinary own-data JSON values', async () => {
  const strictObjectSchema: RustyDeveloperCommandWireSchema = {
    request: {
      kind: 'object',
      fields: {
        entity: { required: true, value: { kind: 'integer', minimum: 0 } },
        toJSON: { required: false, value: { kind: 'string', maximumBytes: 64 } },
      },
    },
    result: { kind: 'opaqueJson', maximumBytes: 16_384, maximumNodes: 256 },
    error: { kind: 'opaqueJson', maximumBytes: 16_384, maximumNodes: 256 },
  };
  const client = createRustyDeveloperCommandClient({
    adapter: adapter(), schemas: { 'standard.inspect.entity': strictObjectSchema },
  });
  await client.discover();

  const accessor: Record<string, unknown> = {};
  Object.defineProperty(accessor, 'entity', { enumerable: true, get: () => 1 });
  await assert.rejects(client.execute('standard.inspect.entity', accessor), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const hidden: Record<string, unknown> = {};
  Object.defineProperty(hidden, 'entity', { enumerable: false, value: 1 });
  await assert.rejects(client.execute('standard.inspect.entity', hidden), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const withToJson = { entity: 1, toJSON: () => ({ entity: 999 }) };
  await assert.rejects(client.execute('standard.inspect.entity', withToJson), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const withSymbol = { entity: 1, [Symbol('extra')]: true };
  await assert.rejects(client.execute('standard.inspect.entity', withSymbol), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const specialPrototype = Object.create({ inherited: true }) as { entity: number };
  specialPrototype.entity = 1;
  await assert.rejects(client.execute('standard.inspect.entity', specialPrototype), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const nullPrototype = Object.create(null) as { entity: number };
  nullPrototype.entity = 1;
  await assert.rejects(client.execute('standard.inspect.entity', nullPrototype), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_payload');

  const arraySchema: RustyDeveloperCommandValueSchema = {
    kind: 'array', items: { kind: 'integer', minimum: 0 }, maximumItems: 4,
  };
  const withArrayToJson = [1] as number[] & { toJSON?: () => unknown };
  withArrayToJson.toJSON = () => [99];
  assert.throws(() => validateRustyDeveloperCommandWireValue(withArrayToJson, arraySchema));
  const sparse = new Array<number>(1);
  assert.throws(() => validateRustyDeveloperCommandWireValue(sparse, arraySchema));
  const specialArrayPrototype = [1];
  Object.setPrototypeOf(specialArrayPrototype, { custom: true });
  assert.throws(() => validateRustyDeveloperCommandWireValue(specialArrayPrototype, arraySchema));
});

function schemaWithRequest(request: RustyDeveloperCommandValueSchema): RustyDeveloperCommandWireSchema {
  const opaque = { kind: 'opaqueJson', maximumBytes: 16_384, maximumNodes: 256 } as const;
  return { request, result: opaque, error: opaque };
}

function assertSchemaRejected(schema: RustyDeveloperCommandWireSchema): void {
  assert.throws(
    () => createRustyDeveloperCommandClient({ adapter: adapter(), schemas: { 'standard.inspect.entity': schema } }),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_schema',
  );
}

test('schema catalog admission rejects cyclic, over-depth, over-node, and invalid extension schemas', () => {
  const cyclic = { kind: 'array', maximumItems: 1 } as { kind: 'array'; maximumItems: number; items?: unknown };
  cyclic.items = cyclic;
  assertSchemaRejected(schemaWithRequest(cyclic as unknown as RustyDeveloperCommandValueSchema));

  let deep: RustyDeveloperCommandValueSchema = { kind: 'boolean' };
  for (let index = 0; index < 20; index += 1) deep = { kind: 'array', items: deep, maximumItems: 1 };
  assertSchemaRejected(schemaWithRequest(deep));

  const broad = (depth: number): RustyDeveloperCommandValueSchema => depth === 0
    ? { kind: 'boolean' }
    : { kind: 'object', fields: {
      left: { required: true, value: broad(depth - 1) },
      right: { required: true, value: broad(depth - 1) },
    } };
  assertSchemaRejected(schemaWithRequest(broad(8)));
  assertSchemaRejected(schemaWithRequest({ kind: 'integer', minimum: 5, maximum: 1 }));
  assertSchemaRejected(schemaWithRequest({ kind: 'unknown' } as unknown as RustyDeveloperCommandValueSchema));

  const extensionSchema = schemaWithRequest({ kind: 'array', maximumItems: 1, items: { kind: 'array', maximumItems: 1 } as unknown as RustyDeveloperCommandValueSchema });
  assert.throws(
    () => createRustyDeveloperCommandClient({
      adapter: adapter(),
      extensions: [{
        namespace: 'product',
        descriptors: [{ id: 'product.play.attack', aliases: [], lane: 'play', summary: 'Attack.' }],
        schemas: { 'product.play.attack': extensionSchema },
      }],
    }),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_schema',
  );
});

test('decimalU64 schemas admit only canonical unsigned 64-bit decimal strings', () => {
  const schema = { kind: 'decimalU64' } as const;
  for (const value of ['0', '1', '18446744073709551615']) {
    validateRustyDeveloperCommandWireValue(value, schema);
  }
  for (const value of ['', '00', '01', '+1', '-1', '18446744073709551616', '1.0', 1, null]) {
    assert.throws(
      () => validateRustyDeveloperCommandWireValue(value, schema),
      (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed',
    );
  }
  assertSchemaRejected(schemaWithRequest({ kind: 'decimalU64', maximumBytes: 20 } as unknown as RustyDeveloperCommandValueSchema));
});

test('opaque JSON preserves finite binary64 fractions and rejects only non-JSON numbers', async () => {
  const schema = schemaWithRequest({ kind: 'opaqueJson', maximumBytes: 16_384, maximumNodes: 256 });
  const input = {
    fraction: 0.125,
    tiny: Number.MIN_VALUE,
    huge: Number.MAX_VALUE,
    impreciseInteger: Number.MAX_SAFE_INTEGER + 2,
  };
  const client = createRustyDeveloperCommandClient({
    adapter: adapter((request) => ({
      correlation: request.correlation, runtime: request.expected.profile === 'developer' ? 'test-runtime' : 'test-runtime',
      profile: request.expected.profile, revision: '4', catalogEpoch: '7',
      outcome: { kind: 'success', value: request.payload, receiptRefs: [] },
    })),
    schemas: { 'standard.inspect.entity': schema },
    createCorrelation: () => 'opaque-fraction',
  });
  const response = await client.execute('standard.inspect.entity', input);
  assert.deepEqual(response.outcome.kind === 'success' ? response.outcome.value : null, input);
  assert.notEqual(response.outcome.kind === 'success' ? response.outcome.value : null, input);
  for (const value of [-0, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    assert.throws(
      () => validateRustyDeveloperCommandWireValue(value, { kind: 'opaqueJson', maximumBytes: 64, maximumNodes: 4 }),
      (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed',
    );
  }
});

test('schema field and tagged-union lookups cannot use inherited constructor names', () => {
  const objectSchema: RustyDeveloperCommandValueSchema = {
    kind: 'object',
    fields: { entity: { required: true, value: { kind: 'integer', minimum: 0 } } },
  };
  const inheritedField = JSON.parse('{"constructor":1}') as unknown;
  assert.throws(
    () => validateRustyDeveloperCommandWireValue(inheritedField, objectSchema),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed',
  );

  const unionSchema: RustyDeveloperCommandValueSchema = {
    kind: 'taggedUnion',
    tag: 'kind',
    variants: { safe: { kind: 'string', maximumBytes: 16 } },
  };
  assert.throws(
    () => validateRustyDeveloperCommandWireValue({ kind: 'constructor' }, unionSchema),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed',
  );
  assert.throws(
    () => validateRustyDeveloperCommandWireValue({ kind: 'prototype' }, unionSchema),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed',
  );
});

test('extension descriptors are fully admitted and aggregate with discovered commands', async () => {
  const extension = (descriptor: Record<string, unknown> = {
    id: 'product.play.attack', aliases: ['product.attack'], lane: 'play', summary: 'Attack.',
  }): RustyDeveloperCommandExtension => ({
    namespace: 'product', descriptors: [descriptor], schemas: {},
  } as unknown as RustyDeveloperCommandExtension);
  const invalid = [
    { id: 'Product.play.attack', aliases: [], lane: 'play', summary: 'Bad identity.' },
    { id: `product.${'x'.repeat(130)}`, aliases: [], lane: 'play', summary: 'Too long.' },
    { id: 'product.play.attack', aliases: Array.from({ length: 9 }, (_, index) => `product.alias${index}`), lane: 'play', summary: 'Too many aliases.' },
    { id: 'product.play.attack', aliases: ['Product.alias'], lane: 'play', summary: 'Bad alias.' },
    { id: 'product.play.attack', aliases: [], lane: 'not-a-lane', summary: 'Bad lane.' },
    { id: 'product.play.attack', aliases: [], lane: 'play', summary: 'x'.repeat(257) },
    { id: 'product.play.attack', aliases: ['product.play.attack'], lane: 'play', summary: 'Self collision.' },
  ];
  for (const descriptor of invalid) {
    assert.throws(
      () => createRustyDeveloperCommandClient({ adapter: adapter(), extensions: [extension(descriptor)] }),
      (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_extension',
    );
  }
  assert.throws(
    () => createRustyDeveloperCommandClient({ adapter: adapter(), extensions: [
      extension(), extension({ id: 'product.play.other', aliases: ['product.attack'], lane: 'play', summary: 'Alias collision.' }),
    ] }),
    (cause: unknown) => cause instanceof RustyDeveloperCommandClientError && cause.code === 'invalid_extension',
  );

  const commands = Array.from({ length: 256 }, (_, index) => ({
    id: `standard.inspect.entity${index}`, aliases: [], lane: 'inspect', summary: 'Entity.',
  }));
  const aggregate = createRustyDeveloperCommandClient({
    adapter: {
      ...adapter(),
      discover: async () => ({
        protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect', 'play'],
        revision: '4', catalogEpoch: '7', contractFingerprint: 'test-contract', commands,
      }),
    },
    extensions: [extension()],
  });
  await assert.rejects(aggregate.discover(), (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'malformed');
});

test('concurrent discovery rejects older or mismatched late snapshots without regressing context', async () => {
  const pending: Array<(value: unknown) => void> = [];
  const snapshot = (revision: string, catalogEpoch: string, profile = 'developer', contractFingerprint = 'test-contract') => ({
    protocolVersion: 1, runtime: 'test-runtime', profile, permittedLanes: ['inspect'],
    revision, catalogEpoch, contractFingerprint,
    commands: [{ id: 'standard.inspect.entity', aliases: [], lane: 'inspect', summary: 'Inspect one entity.' }],
  });
  const client = createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => new Promise((resolve) => pending.push(resolve)),
      execute: async (request) => ({
        correlation: request.correlation, runtime: 'test-runtime', profile: request.expected.profile,
        revision: '2', catalogEpoch: '8', outcome: { kind: 'success', value: request.payload, receiptRefs: [] },
      }),
    },
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'discovery-context',
  });
  const older = client.discover();
  const newer = client.discover();
  assert.equal(pending.length, 2);
  pending[1]!(snapshot('2', '8'));
  assert.equal((await newer).revision, '2');
  pending[0]!(snapshot('1', '7'));
  await assert.rejects(older, (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'stale_context');

  const mismatched = client.discover();
  assert.equal(pending.length, 3);
  pending[2]!(snapshot('3', '9', 'other-profile'));
  await assert.rejects(mismatched, (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'stale_context');
  const mismatchedFingerprint = client.discover();
  assert.equal(pending.length, 4);
  pending[3]!(snapshot('2', '8', 'developer', 'different-contract'));
  await assert.rejects(mismatchedFingerprint, (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'stale_context');
  const response = await client.execute('standard.inspect.entity', { entity: 1 });
  assert.equal(response.revision, '2');
  assert.equal(response.catalogEpoch, '8');
});

test('delayed responses use the current discovery and preserve its context', async () => {
  type Deferred = { resolve: (value: unknown) => void };
  const pending: Deferred[] = [];
  let discoverCount = 0;
  const snapshot = (revision: string, catalogEpoch: string, contractFingerprint: string) => ({
    protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect'],
    revision, catalogEpoch, contractFingerprint,
    commands: [
      { id: 'standard.inspect.entity', aliases: [], lane: 'inspect', summary: 'Inspect one entity.' },
      ...(revision === '5' ? [{ id: 'standard.inspect.new', aliases: [], lane: 'inspect', summary: 'Inspect another entity.' }] : []),
    ],
  });
  const client = createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => snapshot(
        discoverCount++ === 0 ? '4' : '5',
        discoverCount === 1 ? '7' : '8',
        discoverCount === 1 ? 'old-contract' : 'new-contract',
      ),
      execute: async (request) => new Promise((resolve) => {
        pending.push({ resolve });
        assert.deepEqual(request.expected, { profile: 'developer', revision: '4', catalogEpoch: '7' });
      }),
    },
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'delayed-response',
  });

  await client.discover();
  const result = client.execute('standard.inspect.entity', { entity: 1 });
  assert.equal(pending.length, 1);
  await client.discover();
  pending[0]!.resolve({
    correlation: 'delayed-response', runtime: 'test-runtime', profile: 'developer',
    revision: '4', catalogEpoch: '7', outcome: { kind: 'success', value: { entity: 1 }, receiptRefs: [] },
  });
  await assert.rejects(result, (cause: unknown) =>
    cause instanceof RustyDeveloperCommandClientError && cause.code === 'stale_context');
  assert.equal(client.descriptor('standard.inspect.new')?.summary, 'Inspect another entity.');
  const current = await client.discover();
  assert.equal(current.revision, '5');
  assert.equal(current.catalogEpoch, '8');
  assert.equal(current.contractFingerprint, 'new-contract');
  assert.equal(client.history()[0]?.phase, 'post-dispatch');
});

test('accepted delayed responses advance facts without replacing a newer discovery catalog', async () => {
  const pending: Array<(value: unknown) => void> = [];
  let discoverCount = 0;
  const snapshot = (revision: string, catalogEpoch: string, contractFingerprint: string) => ({
    protocolVersion: 1, runtime: 'test-runtime', profile: 'developer', permittedLanes: ['inspect'],
    revision, catalogEpoch, contractFingerprint,
    commands: [
      { id: 'standard.inspect.entity', aliases: [], lane: 'inspect', summary: 'Inspect one entity.' },
      ...(revision !== '4' ? [{ id: 'standard.inspect.new', aliases: [], lane: 'inspect', summary: 'Inspect another entity.' }] : []),
    ],
  });
  const client = createRustyDeveloperCommandClient({
    adapter: {
      discover: async () => {
        discoverCount += 1;
        return discoverCount === 1
          ? snapshot('4', '7', 'old-contract')
          : discoverCount === 2
            ? snapshot('5', '8', 'new-contract')
            : snapshot('6', '9', 'new-contract');
      },
      execute: async () => new Promise((resolve) => pending.push(resolve)),
    },
    schemas: { 'standard.inspect.entity': inspectSchema },
    createCorrelation: () => 'delayed-advance',
  });

  await client.discover();
  const result = client.execute('standard.inspect.entity', { entity: 1 });
  assert.equal(pending.length, 1);
  await client.discover();
  pending[0]!({
    correlation: 'delayed-advance', runtime: 'test-runtime', profile: 'developer',
    revision: '6', catalogEpoch: '9', outcome: { kind: 'success', value: { entity: 1 }, receiptRefs: [] },
  });
  await result;
  const current = await client.discover();
  assert.equal(current.revision, '6');
  assert.equal(current.catalogEpoch, '9');
  assert.equal(current.contractFingerprint, 'new-contract');
  assert.equal(client.descriptor('standard.inspect.new')?.summary, 'Inspect another entity.');
});
