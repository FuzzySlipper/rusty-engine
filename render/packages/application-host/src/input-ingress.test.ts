import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  createRustyApplicationInputIngress,
  createRustyApplicationInputQueue,
  normalizeRustyApplicationKeyboardControl,
} from './input-ingress.js';

const U64_MAXIMUM = 18_446_744_073_709_551_615n;

const INITIAL = {
  runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
  context: 'gameplay.default',
} as const;

void test('input ingress normalizes exactly the Engine keyboard catalog', () => {
  assert.equal(normalizeRustyApplicationKeyboardControl('KeyW'), 'key-w');
  assert.equal(normalizeRustyApplicationKeyboardControl('Digit7'), 'digit-7');
  assert.equal(normalizeRustyApplicationKeyboardControl('ShiftLeft'), 'shift-left');
  assert.equal(normalizeRustyApplicationKeyboardControl('ControlRight'), 'control-right');
  assert.equal(normalizeRustyApplicationKeyboardControl('ArrowUp'), 'arrow-up');
  assert.equal(normalizeRustyApplicationKeyboardControl('ArrowDown'), 'arrow-down');
  assert.equal(normalizeRustyApplicationKeyboardControl('ArrowLeft'), 'arrow-left');
  assert.equal(normalizeRustyApplicationKeyboardControl('ArrowRight'), 'arrow-right');
  assert.equal(normalizeRustyApplicationKeyboardControl('Tab'), null);
  assert.equal(normalizeRustyApplicationKeyboardControl('KeyAA'), null);
});

void test('unlocked gameplay reports the cursor position, never pointer deltas, even if another caller holds pointer lock', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    pointerLockElement: canvas,
    defaultView: createListenerTarget(),
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [],
    usesPointerLock: () => false,
  });

  documentTarget.emit('pointermove', { movementX: 48, movementY: -12, clientX: 50, clientY: 25 } as PointerEvent);
  // The same position again is no new fact.
  documentTarget.emit('pointermove', { movementX: 0, movementY: 0, clientX: 50, clientY: 25 } as PointerEvent);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'pointer-position', x: 0.25, y: 0.75 },
  ]);
  ingress.dispose();
});

void test('a click carries its cursor position while the pointer is unlocked, and none while locked', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  let locked: HTMLCanvasElement | null = null;
  let usesPointerLock = false;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    get pointerLockElement() { return locked; },
    defaultView: createListenerTarget(),
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [],
    usesPointerLock: () => usesPointerLock,
  });
  eventTarget.emit('pointerdown', { button: 0, clientX: 150, clientY: 80 } as PointerEvent);
  documentTarget.emit('pointerup', { button: 0, clientX: 150, clientY: 80 } as PointerEvent);
  usesPointerLock = true;
  locked = canvas;
  eventTarget.emit('pointerdown', { button: 0, clientX: 10, clientY: 10 } as PointerEvent);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'pointer-button', button: 'primary', edge: 'pressed', position: { x: 0.75, y: 0.2 } },
    { kind: 'pointer-button', button: 'primary', edge: 'released', position: { x: 0.75, y: 0.2 } },
    { kind: 'pointer-button', button: 'primary', edge: 'pressed' },
  ]);
  ingress.dispose();
});

void test('the press that takes pointer lock reaches the product neither pressed nor released', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  let locked: HTMLCanvasElement | null = null;
  let focusRequests = 0;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    get pointerLockElement() { return locked; },
    defaultView: createListenerTarget(),
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    // Granted at once, as the desktop shell grants it.
    focusGameplay: () => { focusRequests += 1; locked = canvas; },
    gamepads: () => [],
    usesPointerLock: () => true,
  });
  for (const button of [0, 2]) {
    locked = null;
    eventTarget.emit('pointerdown', { button, clientX: 150, clientY: 80 } as PointerEvent);
    documentTarget.emit('pointerup', { button } as PointerEvent);
  }
  assert.equal(focusRequests, 2);
  assert.deepEqual(ingress.drain(), []);
  eventTarget.emit('pointerdown', { button: 0 } as PointerEvent);
  documentTarget.emit('pointerup', { button: 0 } as PointerEvent);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'pointer-button', button: 'primary', edge: 'pressed' },
    { kind: 'pointer-button', button: 'primary', edge: 'released' },
  ]);
  ingress.dispose();
});

void test('locked pointer movement reaches the Engine without per-event clipping', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    pointerLockElement: canvas,
    defaultView: createListenerTarget(),
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [],
    usesPointerLock: () => true,
  });

  documentTarget.emit('pointermove', { movementX: 480, movementY: -300 } as PointerEvent);
  documentTarget.emit('pointermove', { movementX: Number.NaN, movementY: Infinity } as PointerEvent);
  assert.deepEqual(ingress.drain().map((entry) => ('fact' in entry ? entry.fact : undefined)), [
    { kind: 'pointer-delta', x: 480, y: -300 },
  ]);
  ingress.dispose();
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

void test('Engine-published runtime cursor starts physical and direct UI input after lifecycle clear', () => {
  const queue = createRustyApplicationInputQueue(8);
  assert.equal(queue.bindRuntime({ ...INITIAL, nextSequence: '1' }), true);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' }), false);
  assert.equal(queue.claim('move.forward', { kind: 'digital', active: true }), false);
  assert.deepEqual(queue.drain().map((entry) => entry.sequence), ['1', '2']);
});

void test('same-binding Engine cursor synchronization drops stale input and preserves concurrent entries', () => {
  const queue = createRustyApplicationInputQueue(8);
  queue.bindRuntime(INITIAL);
  queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'pressed' });
  assert.equal(queue.bindRuntime({ ...INITIAL, nextSequence: '2' }), false);
  assert.equal(queue.enqueueFact({ kind: 'key', code: 'key-w', edge: 'released' }), false);
  assert.deepEqual(queue.drain().map((entry) => entry.sequence), ['2']);

  const concurrent = createRustyApplicationInputQueue(8);
  concurrent.bindRuntime(INITIAL);
  concurrent.enqueueFact({ kind: 'key', code: 'key-a', edge: 'pressed' });
  concurrent.enqueueFact({ kind: 'key', code: 'key-a', edge: 'released' });
  concurrent.enqueueFact({ kind: 'wheel', x: 0, y: 1 });
  assert.equal(concurrent.bindRuntime({ ...INITIAL, nextSequence: '2' }), false);
  assert.deepEqual(concurrent.drain().map((entry) => entry.sequence), ['2']);
  assert.equal(concurrent.bindRuntime({ ...INITIAL, nextSequence: '1' }), false);
  assert.equal(concurrent.enqueueFact({ kind: 'key', code: 'key-d', edge: 'pressed' }), false);
  assert.equal(concurrent.drain()[0]?.sequence, '3');
});

void test('a claimed product payload is a copy the product cannot change afterwards', () => {
  const queue = createRustyApplicationInputQueue(8);
  queue.bindRuntime(INITIAL);
  const data = { sourceSlot: 3, targetSlot: 5, selected: true };
  queue.claim('inventory.drop', {
    kind: 'product-payload',
    contract: 'example.inventory.drop.v1',
    data,
  });
  data.targetSlot = 9;
  const [entry] = queue.drain();
  assert.ok(entry !== undefined && 'value' in entry);
  assert.deepEqual(entry.value, {
    kind: 'product-payload',
    contract: 'example.inventory.drop.v1',
    data: { sourceSlot: 3, targetSlot: 5, selected: true },
  });
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

void test('managed interface events preserve a claimed product payload while releasing physical input', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    pointerLockElement: null,
    defaultView: createListenerTarget(),
  } as unknown as Document;
  let acceptsGameplayInput = true;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => acceptsGameplayInput,
    interactionMode: () => acceptsGameplayInput ? 'gameplay' : 'interface',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [],
  });

  documentTarget.emit('keydown', { code: 'KeyW' } as KeyboardEvent);
  ingress.claim('inventory.drop', {
    kind: 'product-payload',
    contract: 'example.inventory.drop.v1',
    data: { sourceSlot: 3, targetSlot: 5 },
  });
  acceptsGameplayInput = false;
  documentTarget.emit('pointermove', {} as PointerEvent);
  documentTarget.emit('pointermove', {} as PointerEvent);
  eventTarget.emit('pointerdown', { button: 0 } as PointerEvent);

  assert.deepEqual(ingress.drain(), [
    {
      runtime: INITIAL.runtime,
      sequence: '0',
      context: INITIAL.context,
      fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
    },
    {
      runtime: INITIAL.runtime,
      sequence: '1',
      context: INITIAL.context,
      intent: 'inventory.drop',
      value: {
        kind: 'product-payload',
        contract: 'example.inventory.drop.v1',
        data: { sourceSlot: 3, targetSlot: 5 },
      },
    },
    {
      runtime: INITIAL.runtime,
      sequence: '2',
      context: INITIAL.context,
      fact: { kind: 'clear', reason: 'interaction-mode-loss' },
    },
  ]);
  ingress.dispose();
});

void test('focus, context, and restart invalidation discard queued direct product payloads', () => {
  const queue = createRustyApplicationInputQueue(8);
  queue.bindRuntime(INITIAL);
  const claim = (): void => {
    queue.claim('inventory.drop', {
      kind: 'product-payload',
      contract: 'example.inventory.drop.v1',
      data: { sourceSlot: 3, targetSlot: 5 },
    });
  };

  claim();
  queue.clear('focus-loss');
  assert.deepEqual(queue.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'focus-loss' },
  ]);

  claim();
  queue.setContext('interface.menu');
  assert.deepEqual(queue.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'interaction-mode-loss' },
  ]);

  claim();
  queue.bindRuntime({
    runtime: { instanceId: '7', generation: '4', controlRevision: '12' },
    context: 'interface.menu',
  });
  assert.deepEqual(queue.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'restart' },
  ]);
});

void test('input ingress rebaselines held keyboard and pointer state without replaying an uncertain batch', () => {
  const eventTarget = createListenerTarget();
  const documentTarget = createListenerTarget();
  const windowTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  const document = {
    ...documentTarget,
    activeElement: canvas,
    pointerLockElement: canvas,
    defaultView: windowTarget,
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [],
  });
  documentTarget.emit('keydown', { code: 'KeyW' } as KeyboardEvent);
  eventTarget.emit('pointerdown', { button: 0 } as PointerEvent);
  // This is the batch whose result became ambiguous. It is intentionally not
  // kept for a second send.
  ingress.drain();
  ingress.rebaselineRuntime({
    runtime: { instanceId: '7', generation: '3', controlRevision: '12' },
    context: INITIAL.context,
    nextSequence: '1',
  });
  assert.deepEqual(ingress.drain(), [
    {
      runtime: { instanceId: '7', generation: '3', controlRevision: '12' },
      sequence: '1',
      context: INITIAL.context,
      fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
    },
    {
      runtime: { instanceId: '7', generation: '3', controlRevision: '12' },
      sequence: '2',
      context: INITIAL.context,
      fact: { kind: 'pointer-button', button: 'primary', edge: 'pressed' },
    },
  ]);
  ingress.dispose();
});

void test('controller pressure survives subthreshold changes, neutral, rebaseline and disconnect', () => {
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  const document = {
    ...createListenerTarget(), activeElement: canvas, pointerLockElement: null,
    defaultView: createListenerTarget(),
  } as unknown as Document;
  let pressure = 0.25;
  let connected = true;
  const ingress = createRustyApplicationInputIngress({ binding: INITIAL, selectedController: { index: 0 } }, {
    canvas: () => canvas, eventTarget: createListenerTarget() as unknown as HTMLElement, document,
    allowsGameplayInput: () => true, interactionMode: () => 'gameplay', active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [{ connected, axes: [0, 0, 0, 0],
      buttons: Array.from({ length: 16 }, (_, index) => ({
        value: index === 7 ? pressure : 0, pressed: index === 7 && pressure > 0.5,
      })),
    } as unknown as Gamepad],
  });
  ingress.drain();
  for (const value of [0.25, 0.4]) {
    pressure = value;
    ingress.sampleController();
    assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
      { kind: 'controller-button-value', button: 'button-7', value },
    ]);
  }
  pressure = 0.75;
  ingress.sampleController();
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-button-value', button: 'button-7', value: 0.75 },
    { kind: 'controller-button', button: 'button-7', edge: 'pressed' },
  ]);
  ingress.sampleController();
  assert.deepEqual(ingress.drain(), []);
  pressure = 0;
  ingress.sampleController();
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-button-value', button: 'button-7', value: 0 },
    { kind: 'controller-button', button: 'button-7', edge: 'released' },
  ]);
  pressure = 0.25;
  ingress.rebaselineRuntime({ ...INITIAL,
    runtime: { ...INITIAL.runtime, controlRevision: '12' }, nextSequence: '1',
  });
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-button-value', button: 'button-7', value: 0.25 },
  ]);
  connected = false;
  ingress.sampleController();
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'interaction-mode-loss' },
  ]);
  ingress.dispose();
});

void test('selected controller disconnect neutralizes held stick, trigger pressure, and button edges', () => {
  const eventTarget = createListenerTarget();
  const windowTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  let connected = true;
  const axes = [0.6, -0.25, 0, 0];
  const buttons = Array.from({ length: 16 }, (_, index) => ({
    value: index === 0 ? 1 : index === 7 ? 0.8 : 0,
    pressed: index === 0 || index === 7,
  }));
  const gamepad = {
    index: 0,
    get connected() { return connected; },
    axes,
    buttons,
  } as unknown as Gamepad;
  const document = {
    ...createListenerTarget(),
    activeElement: canvas,
    pointerLockElement: null,
    defaultView: windowTarget,
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({
    binding: INITIAL,
    selectedController: { index: 0 },
  }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => [gamepad],
  });

  assert.equal(ingress.sampleController(), 6);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-axis', axis: 'axis-0', value: 0.6 },
    { kind: 'controller-axis', axis: 'axis-1', value: -0.25 },
    { kind: 'controller-button-value', button: 'button-0', value: 1 },
    { kind: 'controller-button', button: 'button-0', edge: 'pressed' },
    { kind: 'controller-button-value', button: 'button-7', value: 0.8 },
    { kind: 'controller-button', button: 'button-7', edge: 'pressed' },
  ]);

  connected = false;
  windowTarget.emit('gamepaddisconnected', { gamepad } as unknown as GamepadEvent);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'interaction-mode-loss' },
  ]);
  assert.equal(ingress.sampleController(), 0);
  assert.deepEqual(ingress.drain(), []);

  connected = true;
  axes[0] = 0;
  axes[1] = 0;
  buttons[0]!.value = 0;
  buttons[0]!.pressed = false;
  buttons[7]!.value = 0;
  buttons[7]!.pressed = false;
  assert.equal(ingress.sampleController(), 0);
  assert.deepEqual(ingress.drain(), []);

  axes[0] = 0.6;
  axes[1] = -0.25;
  buttons[0]!.value = 1;
  buttons[0]!.pressed = true;
  buttons[7]!.value = 0.8;
  buttons[7]!.pressed = true;
  assert.equal(ingress.sampleController(), 6);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-axis', axis: 'axis-0', value: 0.6 },
    { kind: 'controller-axis', axis: 'axis-1', value: -0.25 },
    { kind: 'controller-button-value', button: 'button-0', value: 1 },
    { kind: 'controller-button', button: 'button-0', edge: 'pressed' },
    { kind: 'controller-button-value', button: 'button-7', value: 0.8 },
    { kind: 'controller-button', button: 'button-7', edge: 'pressed' },
  ]);
  ingress.dispose();
});

void test('controller sampling clears on focus loss, skips gamepad reads while unfocused, and rebaselines on refocus', () => {
  const eventTarget = createListenerTarget();
  const windowTarget = createListenerTarget();
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100 }) } as unknown as HTMLCanvasElement;
  let focused = true;
  let gamepadReads = 0;
  let wakeups = 0;
  const axes = [0.4, 0, 0, 0];
  const buttons = Array.from({ length: 16 }, (_, index) => ({
    value: index === 7 ? 0.6 : 0,
    pressed: index === 7,
  }));
  const gamepad = {
    index: 0,
    connected: true,
    axes,
    buttons,
  } as unknown as Gamepad;
  const document = {
    ...createListenerTarget(),
    get activeElement() { return focused ? canvas : null; },
    pointerLockElement: null,
    defaultView: windowTarget,
  } as unknown as Document;
  const ingress = createRustyApplicationInputIngress({
    binding: INITIAL,
    selectedController: { index: 0 },
    onAvailable: () => { wakeups += 1; },
  }, {
    canvas: () => canvas,
    eventTarget: eventTarget as unknown as HTMLElement,
    document,
    allowsGameplayInput: () => true,
    interactionMode: () => 'gameplay',
    active: () => true,
    focusGameplay: () => undefined,
    gamepads: () => {
      gamepadReads += 1;
      return [gamepad];
    },
  });

  assert.equal(ingress.sampleController(), 3);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-axis', axis: 'axis-0', value: 0.4 },
    { kind: 'controller-button-value', button: 'button-7', value: 0.6 },
    { kind: 'controller-button', button: 'button-7', edge: 'pressed' },
  ]);

  focused = false;
  windowTarget.emit('blur', {} as Event);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'focus-loss' },
  ]);
  assert.equal(gamepadReads, 1);

  axes[0] = 0.8;
  buttons[7]!.value = 0.9;
  assert.equal(ingress.sampleController(), 0);
  assert.equal(gamepadReads, 1);
  assert.ok(ingress.drain().every((entry) => 'fact' in entry && entry.fact.kind === 'clear'));
  const blockedWakeups = wakeups;
  // The host's input pump samples again after it sends a clear. Repeated
  // blocked samples must not schedule another request or read the device.
  for (let index = 0; index < 10; index += 1) {
    assert.equal(ingress.sampleController(), 0);
    assert.deepEqual(ingress.drain(), []);
  }
  assert.equal(wakeups, blockedWakeups);
  assert.equal(gamepadReads, 1);

  focused = true;
  assert.equal(ingress.sampleController(), 3);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'controller-axis', axis: 'axis-0', value: 0.8 },
    { kind: 'controller-button-value', button: 'button-7', value: 0.9 },
    { kind: 'controller-button', button: 'button-7', edge: 'pressed' },
  ]);
  // A subsequent focus loss still clears held input, even without a DOM blur.
  focused = false;
  assert.equal(ingress.sampleController(), 0);
  assert.deepEqual(ingress.drain().map((entry) => 'fact' in entry ? entry.fact : entry), [
    { kind: 'clear', reason: 'interaction-mode-loss' },
  ]);
  assert.equal(ingress.sampleController(), 0);
  assert.deepEqual(ingress.drain(), []);
  ingress.dispose();
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
  const queue = createRustyApplicationInputQueue(4, U64_MAXIMUM);
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
  const queue = createRustyApplicationInputQueue(4, U64_MAXIMUM - 1n);
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

void test('input ingress rejects a runtime binding that moves backward, and an unbounded queue', () => {
  const queue = createRustyApplicationInputQueue(4);
  queue.bindRuntime(INITIAL);
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

function createListenerTarget(): {
  readonly addEventListener: (type: string, listener: (event: Event) => void) => void;
  readonly removeEventListener: (type: string, listener: (event: Event) => void) => void;
  readonly emit: (type: string, event: Event) => void;
} {
  const listeners = new Map<string, (event: Event) => void>();
  return {
    addEventListener: (type, listener) => { listeners.set(type, listener); },
    removeEventListener: (type, listener) => {
      if (listeners.get(type) === listener) listeners.delete(type);
    },
    emit: (type, event) => { listeners.get(type)?.(event); },
  };
}
