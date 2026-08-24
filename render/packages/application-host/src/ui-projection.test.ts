import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';

import {
  RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
  RustyApplicationUiProjectionError,
  createRustyApplicationUiProjection,
  type RustyApplicationUiProjectionEnvelope,
} from './ui-projection.js';

const RUST_UI_FIXTURE = new URL(
  '../../../../fixtures/runtime-ui/stealth.ui-projection.json',
  import.meta.url,
);

const RUNTIME = {
  instanceId: '7',
  generation: '3',
  controlRevision: '11',
} as const;

const OPTIONS = {
  expectedStream: 'product.hud',
  expectedContract: 'product.hud.v1',
  binding: RUNTIME,
} as const;

function envelope(sequence: string, value: unknown = { health: 72 }): RustyApplicationUiProjectionEnvelope {
  return {
    artifact: RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
    runtime: { ...RUNTIME },
    sequence,
    stream: 'product.hud',
    contract: 'product.hud.v1',
    value: value as RustyApplicationUiProjectionEnvelope['value'],
  };
}

void test('strict UI projection detaches and freezes the exact worker envelope', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  const source = { health: { current: 72 }, tags: ['stealth'] };
  assert.equal(projection.ingest(envelope('0', source)), true);
  source.health.current = 0;
  source.tags[0] = 'mutated';
  const current = projection.current();
  assert.ok(current !== null);
  assert.deepEqual(current, envelope('0', { health: { current: 72 }, tags: ['stealth'] }));
  assert.equal(Object.isFrozen(current), true);
  assert.equal(Object.isFrozen(current.value), true);
  assert.equal(Object.isFrozen((current.value as { readonly health: unknown }).health), true);
  assert.throws(
    () => (current.value as { readonly health: { current: number } }).health.current = 1,
    TypeError,
  );
  projection.dispose();
});

void test('projection accepts only exact fields, expected identity, and strictly increasing sequences', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  assert.throws(
    () => projection.ingest({ ...envelope('0'), extra: true }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'invalid_envelope',
  );
  assert.throws(
    () => projection.ingest({ ...envelope('0'), sequence: '01' }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'invalid_sequence',
  );
  assert.throws(
    () => projection.ingest({ ...envelope('0'), stream: 'product.other' }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'stream_mismatch',
  );
  assert.throws(
    () => projection.ingest({ ...envelope('0'), runtime: { ...RUNTIME, generation: '4' } }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'runtime_mismatch',
  );
  projection.ingest(envelope('4'));
  assert.throws(
    () => projection.ingest(envelope('4')),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'sequence_not_increasing',
  );
  assert.throws(
    () => projection.ingest(envelope('3')),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'sequence_not_increasing',
  );
  projection.dispose();
});

void test('rebind clears the snapshot, notifies but retains subscribers, and resets sequence', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  const mutableObserved: Array<string | null> = [];
  const unsubscribe = projection.subscribe((value) => {
    mutableObserved.push(value?.sequence ?? null);
  });
  projection.ingest(envelope('0'));
  assert.deepEqual(mutableObserved, [null, '0']);
  assert.equal(projection.bindRuntime({ ...RUNTIME, generation: '4', controlRevision: '12' }), true);
  assert.equal(projection.current(), null);
  assert.deepEqual(mutableObserved, [null, '0', null]);
  projection.ingest({
    ...envelope('0'),
    runtime: { ...RUNTIME, generation: '4', controlRevision: '12' },
  });
  assert.deepEqual(mutableObserved, [null, '0', null, '0']);
  assert.equal(projection.readout().subscriberCount, 1);
  unsubscribe();
  projection.dispose();
  assert.equal(projection.readout().state, 'disposed');
  assert.equal(projection.readout().subscriberCount, 0);
});

void test('projection rejects unbound, non-JSON, cyclic, and over-limit values', () => {
  const unbound = createRustyApplicationUiProjection({
    expectedStream: 'product.hud',
    expectedContract: 'product.hud.v1',
  });
  assert.throws(
    () => unbound.ingest(envelope('0')),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'runtime_unbound',
  );

  const projection = createRustyApplicationUiProjection({
    ...OPTIONS,
    maximumNodes: 3,
  });
  const cyclic: { self?: unknown } = {};
  cyclic.self = cyclic;
  assert.throws(
    () => projection.ingest(envelope('0', cyclic)),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'value_invalid',
  );
  assert.throws(
    () => projection.ingest(envelope('0', { one: 1, two: 2, three: 3 })),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'value_limit_exceeded',
  );
  assert.throws(
    () => projection.ingest(envelope('0', new Date(0))),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'value_invalid',
  );
  projection.dispose();
});

void test('projection admits finite fractions but rejects unsafe integer-valued numbers', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  assert.equal(projection.ingest(envelope('0', { fraction: 0.5 })), true);
  assert.equal((projection.current()?.value as { readonly fraction: number }).fraction, 0.5);
  assert.throws(
    () => projection.ingest(envelope('1', { integer: Number.MAX_SAFE_INTEGER + 2 })),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'value_invalid',
  );
  projection.dispose();
});

void test('projection rejects hostile accessors, array subclasses, holes, and overridden map without invoking them', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  let getterInvoked = false;
  const getterValue = {} as Record<string, unknown>;
  Object.defineProperty(getterValue, 'health', {
    enumerable: true,
    get: () => {
      getterInvoked = true;
      return 100;
    },
  });
  assert.throws(() => projection.ingest(envelope('0', getterValue)), RustyApplicationUiProjectionError);
  assert.equal(getterInvoked, false);

  let mapInvoked = false;
  const array = [1, 2] as number[] & { map: () => never };
  Object.defineProperty(array, 'map', {
    configurable: true,
    enumerable: false,
    value: () => {
      mapInvoked = true;
      throw new Error('map must not run');
    },
  });
  assert.throws(() => projection.ingest(envelope('0', array)), RustyApplicationUiProjectionError);
  assert.equal(mapInvoked, false);

  const hole: unknown[] = [];
  hole.length = 1;
  assert.throws(() => projection.ingest(envelope('0', hole)), RustyApplicationUiProjectionError);
  class ProductArray extends Array<number> {}
  assert.throws(() => projection.ingest(envelope('0', new ProductArray(1))), RustyApplicationUiProjectionError);
  projection.dispose();
});

void test('projection enforces the subscriber bound and monotonic runtime rebinding', () => {
  const projection = createRustyApplicationUiProjection({ ...OPTIONS, maximumSubscribers: 2 });
  const unsubscribeOne = projection.subscribe(() => undefined);
  const unsubscribeTwo = projection.subscribe(() => undefined);
  assert.throws(
    () => projection.subscribe(() => undefined),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'subscriber_limit_exceeded',
  );
  assert.throws(
    () => projection.bindRuntime({ ...RUNTIME, generation: '2', controlRevision: '12' }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'runtime_mismatch',
  );
  assert.throws(
    () => projection.bindRuntime({ ...RUNTIME, generation: '4', controlRevision: '11' }),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'runtime_mismatch',
  );
  unsubscribeOne();
  unsubscribeTwo();
  projection.dispose();
});

void test('TS projection admits the exact Rust runtime-ui fixture wire', async () => {
  const fixture: unknown = JSON.parse(await readFile(RUST_UI_FIXTURE, 'utf8'));
  assert.ok(typeof fixture === 'object' && fixture !== null);
  const value = fixture as {
    readonly runtime: typeof RUNTIME;
    readonly stream: string;
    readonly contract: string;
  };
  const projection = createRustyApplicationUiProjection({
    binding: value.runtime,
    expectedStream: value.stream,
    expectedContract: value.contract,
  });
  assert.equal(projection.ingest(fixture), true);
  assert.deepEqual(projection.current(), fixture);
  projection.dispose();
});
