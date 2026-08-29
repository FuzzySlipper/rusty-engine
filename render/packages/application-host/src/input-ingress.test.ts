import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';

import {
  createRustyApplicationInputIngress,
  createRustyApplicationInputQueue,
  normalizeRustyApplicationKeyboardControl,
  RUSTY_APPLICATION_INPUT_U64_MAXIMUM,
} from './input-ingress.js';

const INITIAL = {
  runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
  context: 'gameplay.default',
} as const;
const HOST_NEUTRAL_WIRE_FIXTURE = new URL(
  '../../../../fixtures/runtime-input/host-neutral-input-envelope.json',
  import.meta.url,
);

void test('input ingress normalizes exactly the Engine keyboard catalog', () => {
  assert.equal(normalizeRustyApplicationKeyboardControl('KeyW'), 'key-w');
  assert.equal(normalizeRustyApplicationKeyboardControl('Digit7'), 'digit-7');
  assert.equal(normalizeRustyApplicationKeyboardControl('ShiftLeft'), 'shift-left');
  assert.equal(normalizeRustyApplicationKeyboardControl('ControlRight'), 'control-right');
  assert.equal(normalizeRustyApplicationKeyboardControl('ArrowUp'), null);
  assert.equal(normalizeRustyApplicationKeyboardControl('Tab'), null);
  assert.equal(normalizeRustyApplicationKeyboardControl('KeyAA'), null);
});

void test('input ingress preserves physical and direct UI observation order with lossless sequences', () => {
  const queue = createRustyApplicationInputQueue(8);
  assert.equal(queue.bindRuntime(INITIAL), true);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' }), false);
  assert.equal(queue.claim('move.forward', { kind: 'digital', active: true }), false);
  assert.deepEqual(queue.drain(), [
    {
      runtime: INITIAL.runtime,
      sequence: '0',
      context: 'gameplay.default',
      fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
    },
    {
      runtime: INITIAL.runtime,
      sequence: '1',
      context: 'gameplay.default',
      intent: 'move.forward',
      value: { kind: 'digital', active: true },
    },
  ]);
});

void test('direct product payload claims are deeply plain, bounded, and immutable', () => {
  const queue = createRustyApplicationInputQueue(8);
  queue.bindRuntime(INITIAL);
  queue.claim('inventory.drop', {
    kind: 'product-payload',
    contract: 'example.inventory.drop.v1',
    data: { sourceSlot: 3, targetSlot: 5, selected: true },
  });
  const [entry] = queue.drain();
  assert.ok(entry !== undefined && 'value' in entry);
  assert.equal(entry.value.kind, 'product-payload');
  if (entry.value.kind === 'product-payload') {
    assert.equal(entry.value.contract, 'example.inventory.drop.v1');
    assert.deepEqual({ ...(entry.value.data as Record<string, unknown>) }, {
      selected: true, sourceSlot: 3, targetSlot: 5,
    });
    assert.ok(Object.isFrozen(entry.value.data));
  }
  assert.throws(() => queue.claim('inventory.drop', {
    kind: 'product-payload',
    contract: 'example.inventory.drop.v1',
    data: Object.create({ inherited: true }) as never,
  }));
  const accessor: Record<string, unknown> = {};
  Object.defineProperty(accessor, 'slot', { enumerable: true, get: () => 3 });
  assert.throws(() => queue.claim('inventory.drop', {
    kind: 'product-payload', contract: 'example.inventory.drop.v1', data: accessor as never,
  }));
  const executableArray: unknown[] = [3];
  Object.defineProperty(executableArray, '4294967295', {
    enumerable: true,
    get: () => { throw new Error('must not read extra array property'); },
  });
  assert.throws(() => queue.claim('inventory.drop', {
    kind: 'product-payload', contract: 'example.inventory.drop.v1', data: executableArray as never,
  }));
  assert.throws(() => queue.claim('inventory.drop', {
    kind: 'product-payload', contract: 'example.inventory.drop.v1', data: { slot: 9_007_199_254_740_992 },
  }));
});

void test('input ingress rebinding and context changes clear with the exact epoch ordering', () => {
  const queue = createRustyApplicationInputQueue(8);
  queue.bindRuntime(INITIAL);
  queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' });
  assert.equal(queue.bindRuntime({
    runtime: { instanceId: '7', generation: '4', controlRevision: '12' },
    context: 'gameplay.default',
  }), true);
  assert.deepEqual(queue.drain(), [{
    runtime: { instanceId: '7', generation: '4', controlRevision: '12' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'restart' },
  }]);
  assert.equal(queue.bindRuntime({
    runtime: { instanceId: '7', generation: '4', controlRevision: '13' },
    context: 'gameplay.default',
  }), true);
  assert.deepEqual(queue.drain(), [{
    runtime: { instanceId: '7', generation: '4', controlRevision: '13' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'control-revision-change' },
  }]);
  assert.equal(queue.bindRuntime({
    runtime: { instanceId: '7', generation: '4', controlRevision: '13' },
    context: 'gameplay.default',
  }), false);
  assert.equal(queue.setContext('interface.menu'), true);
  assert.deepEqual(queue.drain(), [{
    runtime: { instanceId: '7', generation: '4', controlRevision: '13' },
    sequence: '1',
    context: 'interface.menu',
    fact: { kind: 'clear', reason: 'interaction-mode-loss' },
  }]);
  queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'released' });
  assert.deepEqual(queue.drain(), [{
    runtime: { instanceId: '7', generation: '4', controlRevision: '13' },
    sequence: '2',
    context: 'interface.menu',
    fact: { kind: 'key', code: 'key-w', edge: 'released' },
  }]);
});

void test('input ingress fails closed on bounded-queue overflow', () => {
  const queue = createRustyApplicationInputQueue(2);
  queue.bindRuntime(INITIAL);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' }), false);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'released' }), false);
  assert.equal(queue.enqueueFact({ kind: 'wheel', x: 0, y: 1 }), true);
  assert.deepEqual(queue.drain(), [{
    runtime: INITIAL.runtime,
    sequence: '0',
    context: INITIAL.context,
    fact: { kind: 'clear', reason: 'ingress-overflow' },
  }]);
});

void test('same-epoch clears replace undispatched input at its first sequence without gaps', () => {
  const queue = createRustyApplicationInputQueue(4);
  queue.bindRuntime(INITIAL);
  queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' });
  queue.enqueueFact({ kind: 'wheel', x: 0, y: 1 });
  queue.clear('focus-loss');
  queue.clear('pointer-lock-loss');
  assert.deepEqual(queue.drain(), [{
    runtime: INITIAL.runtime,
    sequence: '0',
    context: INITIAL.context,
    fact: { kind: 'clear', reason: 'pointer-lock-loss' },
  }]);
  queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' });
  assert.deepEqual(queue.drain(), [{
    runtime: INITIAL.runtime,
    sequence: '1',
    context: INITIAL.context,
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  }]);
});

void test('input ingress reserves u64 maximum for one terminal fail-closed clear until rebind', () => {
  const queue = createRustyApplicationInputQueue(4, RUSTY_APPLICATION_INPUT_U64_MAXIMUM);
  queue.bindRuntime(INITIAL);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' }), true);
  assert.deepEqual(queue.drain(), [{
    runtime: INITIAL.runtime,
    sequence: '18446744073709551615',
    context: INITIAL.context,
    fact: { kind: 'clear', reason: 'ingress-overflow' },
  }]);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'released' }), true);
  assert.deepEqual(queue.drain(), []);
  assert.equal(queue.setContext('interface.menu'), false);
  assert.equal(queue.bindRuntime({
    runtime: INITIAL.runtime,
    context: 'interface.menu',
  }), false);
  assert.deepEqual(queue.drain(), []);
  queue.bindRuntime({
    runtime: { instanceId: '8', generation: '0', controlRevision: '0' },
    context: INITIAL.context,
  });
  assert.deepEqual(queue.drain(), [{
    runtime: { instanceId: '8', generation: '0', controlRevision: '0' },
    sequence: '0',
    context: INITIAL.context,
    fact: { kind: 'clear', reason: 'restart' },
  }]);
});

void test('terminal exhaustion rewinds undispatched max-minus-one input into one gap-free clear', () => {
  const queue = createRustyApplicationInputQueue(4, RUSTY_APPLICATION_INPUT_U64_MAXIMUM - 1n);
  queue.bindRuntime(INITIAL);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' }), false);
  assert.equal(queue.enqueueFact({ kind: 'wheel', x: 0, y: 1 }), true);
  assert.deepEqual(queue.drain(), [{
    runtime: INITIAL.runtime,
    sequence: '18446744073709551614',
    context: INITIAL.context,
    fact: { kind: 'clear', reason: 'ingress-overflow' },
  }]);
});

void test('input ingress rejects malformed runtime, context, and intent wire values', () => {
  const queue = createRustyApplicationInputQueue(4);
  assert.throws(() => queue.bindRuntime({
    runtime: { instanceId: '01', generation: '0', controlRevision: '0' },
    context: INITIAL.context,
  }), /canonical unsigned decimal/u);
  assert.throws(() => queue.bindRuntime({
    runtime: INITIAL.runtime,
    context: 'Gameplay.default',
  }), /lowercase product identity/u);
  queue.bindRuntime(INITIAL);
  for (const value of ['bad identity', 'bad..identity', '.leading', 'trailing-', 'unicode-é']) {
    assert.throws(
      () => queue.claim(value, { kind: 'digital', active: true }),
      /lowercase product identity/u,
    );
  }
  assert.throws(() => queue.bindRuntime({
    runtime: { instanceId: '7', generation: '2', controlRevision: '99' },
    context: INITIAL.context,
  }), /generation cannot move backward/u);
  assert.throws(() => queue.bindRuntime({
    runtime: { instanceId: '7', generation: '3', controlRevision: '10' },
    context: INITIAL.context,
  }), /control revision cannot move backward/u);
  assert.throws(() => queue.bindRuntime({
    runtime: { instanceId: '7', generation: '4', controlRevision: '11' },
    context: INITIAL.context,
  }), /control revision must advance with generation/u);
  assert.throws(() => createRustyApplicationInputIngress(
    { maximumQueue: 1_025 },
    {
      canvas: () => ({}) as HTMLCanvasElement,
      eventTarget: {} as HTMLElement,
      document: {} as Document,
      allowsGameplayInput: () => true,
      interactionMode: () => 'gameplay',
      active: () => true,
      focusGameplay: () => undefined,
      gamepads: () => [],
    },
  ), /maximumQueue must be a safe integer within \[1, 1024\]/u);
});

void test('shared host-neutral input envelope fixture has the exact public application-host wire shape', async () => {
  const values: unknown = JSON.parse(await readFile(HOST_NEUTRAL_WIRE_FIXTURE, 'utf8'));
  assert.ok(Array.isArray(values));
  assert.equal(values.length, 17);
  for (const entry of values) {
    assertRecord(entry);
  }
});

function assertRecord(entry: unknown): asserts entry is Record<string, unknown> {
  assert.ok(isRecord(entry));
  const physical = 'fact' in entry;
  assert.deepEqual(
    Object.keys(entry).sort(),
    physical
      ? ['context', 'fact', 'runtime', 'sequence']
      : ['context', 'intent', 'runtime', 'sequence', 'value'],
  );
  assertCanonicalU64(entry['sequence']);
  assertIdentity(entry['context']);
  assert.ok(isRecord(entry['runtime']));
  assert.deepEqual(Object.keys(entry['runtime']).sort(), ['controlRevision', 'generation', 'instanceId']);
  assertCanonicalU64(entry['runtime']['instanceId']);
  assertCanonicalU64(entry['runtime']['generation']);
  assertCanonicalU64(entry['runtime']['controlRevision']);
  if (physical) {
    assertFact(entry['fact']);
    return;
  }
  assertIdentity(entry['intent']);
  assert.ok(isRecord(entry['value']));
  if (entry['value']['kind'] === 'digital') {
    assert.deepEqual(Object.keys(entry['value']).sort(), ['active', 'kind']);
    assert.equal(typeof entry['value']['active'], 'boolean');
    return;
  }
  if (entry['value']['kind'] === 'product-payload') {
    assert.deepEqual(Object.keys(entry['value']).sort(), ['contract', 'data', 'kind']);
    assertIdentity(entry['value']['contract']);
    assertPlainProductPayload(entry['value']['data']);
    return;
  }
  assert.equal(entry['value']['kind'], 'axis');
  assert.deepEqual(Object.keys(entry['value']).sort(), ['kind', 'value']);
  assert.ok(typeof entry['value']['value'] === 'number'
    && Number.isFinite(entry['value']['value'])
    && entry['value']['value'] >= -1 && entry['value']['value'] <= 1);
}

function assertPlainProductPayload(value: unknown): void {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return;
  if (typeof value === 'number') {
    assert.ok(Number.isFinite(value) && (!Number.isInteger(value) || Math.abs(value) <= 9_007_199_254_740_991));
    return;
  }
  if (Array.isArray(value)) {
    value.forEach(assertPlainProductPayload);
    return;
  }
  assert.ok(isRecord(value));
  Object.values(value).forEach(assertPlainProductPayload);
}

function assertFact(value: unknown): void {
  assert.ok(isRecord(value));
  assert.equal(typeof value['kind'], 'string');
  switch (value['kind']) {
    case 'key':
      assert.deepEqual(Object.keys(value).sort(), ['code', 'edge', 'kind']);
      assert.ok(typeof value['code'] === 'string' && normalizeRustyApplicationKeyboardControl(
        `Key${value['code'].slice(4).toUpperCase()}`,
      ) === value['code']);
      assertEdge(value['edge']);
      return;
    case 'pointer-button':
      assert.deepEqual(Object.keys(value).sort(), ['button', 'edge', 'kind']);
      assert.ok(value['button'] === 'primary' || value['button'] === 'secondary' || value['button'] === 'middle');
      assertEdge(value['edge']);
      return;
    case 'pointer-delta':
    case 'wheel':
      assert.deepEqual(Object.keys(value).sort(), ['kind', 'x', 'y']);
      assertFiniteNumber(value['x']);
      assertFiniteNumber(value['y']);
      return;
    case 'controller-button':
      assert.deepEqual(Object.keys(value).sort(), ['button', 'edge', 'kind']);
      assert.ok(typeof value['button'] === 'string' && /^button-(?:[0-9]|1[0-5])$/u.test(value['button']));
      assertEdge(value['edge']);
      return;
    case 'controller-axis':
      assert.deepEqual(Object.keys(value).sort(), ['axis', 'kind', 'value']);
      assert.ok(typeof value['axis'] === 'string' && /^axis-[0-3]$/u.test(value['axis']));
      assertFiniteNumber(value['value']);
      assert.ok((value['value'] as number) >= -1 && (value['value'] as number) <= 1);
      return;
    case 'clear':
      assert.deepEqual(Object.keys(value).sort(), ['kind', 'reason']);
      assert.ok([
        'focus-loss', 'interaction-mode-loss', 'pointer-lock-loss', 'restart',
        'control-revision-change', 'dispose', 'ingress-overflow',
      ].includes(value['reason'] as string));
      return;
    default:
      assert.fail(`unknown host-neutral input fact ${String(value['kind'])}`);
  }
}

function assertCanonicalU64(value: unknown): void {
  assert.ok(typeof value === 'string' && /^(?:0|[1-9][0-9]*)$/u.test(value));
  assert.ok(BigInt(value) <= RUSTY_APPLICATION_INPUT_U64_MAXIMUM);
}

function assertIdentity(value: unknown): void {
  assert.ok(typeof value === 'string'
    && /^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)
    && new TextEncoder().encode(value).byteLength <= 128);
}

function assertFiniteNumber(value: unknown): asserts value is number {
  assert.ok(typeof value === 'number' && Number.isFinite(value));
}

function assertEdge(value: unknown): void {
  assert.ok(value === 'pressed' || value === 'released');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
