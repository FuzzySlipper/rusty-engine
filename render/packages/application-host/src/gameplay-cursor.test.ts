import assert from 'node:assert/strict';
import { test } from 'node:test';

import type { ProductHostCursorMode } from './generated/contracts.js';
import { createGameplayCursor, DESKTOP_CONFINEMENT_END } from './gameplay-cursor.js';

interface FakeDesktopCursor {
  refused: boolean;
  isConfined: boolean;
  confine(): boolean;
  release(): void;
  confined(): boolean;
}

/** A canvas at (10, 20) sized 200x100 in a layer at (0, 0); lock requests are granted on `grant`. */
function createPage(desktop?: FakeDesktopCursor) {
  const listeners = new Map<string, Set<(event: Event) => void>>();
  const emit = (type: string, event: Event = { type } as Event): void => {
    for (const listener of [...(listeners.get(type) ?? [])]) listener(event);
  };
  const element = {
    dataset: {} as Record<string, string>,
    style: { cssText: '', display: '', transform: '' },
    innerHTML: '',
    removed: false,
    setAttribute: () => undefined,
    remove() { this.removed = true; },
  };
  const document = {
    pointerLockElement: null as unknown,
    exits: 0,
    defaultView: { __rustyDesktopCursor: desktop },
    createElement: () => element,
    addEventListener: (type: string, listener: (event: Event) => void) => {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)?.add(listener);
    },
    removeEventListener: (type: string, listener: (event: Event) => void) => {
      listeners.get(type)?.delete(listener);
    },
    exitPointerLock(): void {
      this.exits += 1;
      this.pointerLockElement = null;
      emit('pointerlockchange');
    },
  };
  let requests = 0;
  const canvas = {
    ownerDocument: document,
    getBoundingClientRect: () => ({ left: 10, top: 20, right: 210, bottom: 120, width: 200, height: 100 }),
    requestPointerLock: () => {
      requests += 1;
      return Promise.resolve();
    },
  } as unknown as HTMLCanvasElement;
  const layer = {
    append: () => undefined,
    getBoundingClientRect: () => ({ left: 0, top: 0, right: 220, bottom: 140, width: 220, height: 140 }),
  } as unknown as HTMLElement;
  return {
    canvas,
    layer,
    document,
    element,
    emit,
    requests: () => requests,
    grant: () => {
      document.pointerLockElement = canvas;
      emit('pointerlockchange');
    },
  };
}

void test('a browser confines with a drawn cursor that starts at the click and stays on the canvas', () => {
  const page = createPage();
  const cursor = createGameplayCursor({
    canvas: page.canvas,
    layer: page.layer,
    mode: () => 'confined',
    wanted: () => true,
    onConfinementLost: () => assert.fail('no native confinement'),
  });
  cursor.capture({ x: 150, y: 80 });
  assert.equal(page.requests(), 1);
  assert.equal(cursor.captured(), false);
  assert.equal(cursor.software(), null);
  page.grant();
  assert.equal(cursor.captured(), true);
  assert.equal(page.element.style.display, 'block');
  assert.equal(page.element.style.transform, 'translate(150px,80px)');
  cursor.software()?.move(500, -500);
  assert.deepEqual(cursor.software()?.point(), { x: 209, y: 20 });
  assert.equal(page.element.style.transform, 'translate(209px,20px)');
  cursor.release();
  assert.equal(page.document.exits, 1);
  assert.equal(cursor.captured(), false);
  assert.equal(page.element.style.display, 'none');
  cursor.dispose();
  assert.equal(page.element.removed, true);
});

void test('switching a held lock to confined draws the cursor at the canvas centre without a new request', () => {
  const page = createPage();
  let mode: ProductHostCursorMode = 'pointer-lock';
  const cursor = createGameplayCursor({
    canvas: page.canvas,
    layer: page.layer,
    mode: () => mode,
    wanted: () => true,
    onConfinementLost: () => undefined,
  });
  cursor.capture();
  page.grant();
  assert.equal(page.element.style.display, 'none');
  assert.equal(cursor.software(), null);
  mode = 'confined';
  cursor.capture();
  assert.equal(page.requests(), 1);
  assert.deepEqual(cursor.software()?.point(), { x: 110, y: 70 });
  mode = 'pointer-lock';
  cursor.capture();
  assert.equal(page.element.style.display, 'none');
  assert.equal(cursor.captured(), true);
});

void test('the desktop shell confines the real cursor, and a refusal falls back to the drawn one', () => {
  const desktop: FakeDesktopCursor = {
    refused: false,
    isConfined: false,
    confine() {
      if (this.refused) return false;
      this.isConfined = true;
      return true;
    },
    release() { this.isConfined = false; },
    confined() { return this.isConfined; },
  };
  const page = createPage(desktop);
  let lost = 0;
  const cursor = createGameplayCursor({
    canvas: page.canvas,
    layer: page.layer,
    mode: () => 'confined',
    wanted: () => true,
    onConfinementLost: () => { lost += 1; },
  });
  cursor.capture({ x: 50, y: 50 });
  assert.equal(desktop.isConfined, true);
  assert.equal(cursor.captured(), true);
  assert.equal(cursor.software(), null);
  assert.equal(page.requests(), 0);

  // Escape or focus loss in the shell.
  desktop.isConfined = false;
  page.emit(DESKTOP_CONFINEMENT_END, { detail: { refused: false } } as unknown as Event);
  assert.equal(lost, 1);
  assert.equal(cursor.captured(), false);

  cursor.capture({ x: 50, y: 50 });
  desktop.isConfined = false;
  desktop.refused = true;
  page.emit(DESKTOP_CONFINEMENT_END, { detail: { refused: true } } as unknown as Event);
  assert.equal(lost, 1);
  assert.equal(page.requests(), 1);
  page.grant();
  assert.deepEqual(cursor.software()?.point(), { x: 110, y: 70 });
});
