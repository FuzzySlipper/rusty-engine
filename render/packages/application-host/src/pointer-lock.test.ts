import assert from 'node:assert/strict';
import { test } from 'node:test';

import { requestPointerLockWhile } from './pointer-lock.js';

function createDeferredLockCanvas(): {
  canvas: HTMLCanvasElement;
  document: { pointerLockElement: unknown; exits: number };
  grant: () => Promise<void>;
  reject: () => Promise<void>;
} {
  const document = {
    pointerLockElement: null as unknown,
    exits: 0,
    exitPointerLock(): void {
      this.exits += 1;
      this.pointerLockElement = null;
    },
  };
  let settle: { resolve: () => void; reject: (cause: unknown) => void } | null = null;
  const canvas = {
    ownerDocument: document,
    requestPointerLock: () => new Promise<void>((resolve, reject) => {
      settle = { resolve, reject };
    }),
  } as unknown as HTMLCanvasElement;
  return {
    canvas,
    document,
    grant: async () => {
      document.pointerLockElement = canvas;
      settle?.resolve();
      await new Promise((resolve) => setImmediate(resolve));
    },
    reject: async () => {
      settle?.reject(new Error('denied'));
      await new Promise((resolve) => setImmediate(resolve));
    },
  };
}

void test('a lock granted after gameplay is no longer wanted is released', async () => {
  const { canvas, document, grant } = createDeferredLockCanvas();
  let gameplay = true;
  requestPointerLockWhile(canvas, () => gameplay);
  gameplay = false;
  await grant();
  assert.equal(document.pointerLockElement, null);
  assert.equal(document.exits, 1);
});

void test('a lock granted while gameplay is still wanted is kept', async () => {
  const { canvas, document, grant } = createDeferredLockCanvas();
  requestPointerLockWhile(canvas, () => true);
  await grant();
  assert.equal(document.pointerLockElement, canvas);
  assert.equal(document.exits, 0);
});

void test('a rejected or throwing lock request is absorbed', async () => {
  const { canvas, document, reject } = createDeferredLockCanvas();
  requestPointerLockWhile(canvas, () => false);
  await reject();
  assert.equal(document.exits, 0);

  const throwing = {
    requestPointerLock: () => {
      throw new Error('no gesture');
    },
  } as unknown as HTMLCanvasElement;
  assert.doesNotThrow(() => requestPointerLockWhile(throwing, () => true));
});
