import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';

import type { RuntimeUiProjectionEnvelope } from './generated/contracts.js';
import {
  RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
  RustyApplicationUiProjectionError,
  createRustyApplicationUiProjection,
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

function envelope(sequence: string, value: unknown = { health: 72 }): RuntimeUiProjectionEnvelope {
  return {
    artifact: RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
    runtime: { ...RUNTIME },
    sequence,
    stream: 'product.hud',
    contract: 'product.hud.v1',
    value: value as RuntimeUiProjectionEnvelope['value'],
  };
}

void test('a projection freezes the envelope every subscriber shares', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
  assert.equal(projection.ingest(envelope('0', { health: { current: 72 }, tags: ['stealth'] })), true);
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

void test('projection admits its stream, contract and binding, in increasing sequence', () => {
  const projection = createRustyApplicationUiProjection(OPTIONS);
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

void test('projection rejects an envelope before a runtime binding', () => {
  const unbound = createRustyApplicationUiProjection({
    expectedStream: 'product.hud',
    expectedContract: 'product.hud.v1',
  });
  assert.throws(
    () => unbound.ingest(envelope('0')),
    (error: unknown) => error instanceof RustyApplicationUiProjectionError
      && error.code === 'runtime_unbound',
  );
  unbound.dispose();
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

void test('projection admits the Rust runtime-ui fixture', async () => {
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
  assert.equal(projection.ingest(fixture as RuntimeUiProjectionEnvelope), true);
  assert.deepEqual(projection.current(), fixture);
  projection.dispose();
});
