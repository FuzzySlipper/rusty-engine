import assert from 'node:assert/strict';
import test from 'node:test';
import { installPlaytestInspection, type PlaytestInspectionRequest } from './playtest-inspection.js';

type Playtest = (request: PlaytestInspectionRequest) => Promise<unknown>;

test('action ids pass declared dotted intent names to the product and refuse others', async () => {
  const commands: string[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (_url: string, init?: { body?: string }) => {
    commands.push(init?.body ?? '');
    return new Response('{"id":"party.move-forward"}', { status: 200 });
  }) as typeof fetch;
  const uninstall = installPlaytestInspection(async () => {}, async () => {});
  try {
    const playtest = (globalThis as { __rustyPlaytest?: Playtest }).__rustyPlaytest!;
    assert.deepEqual(await playtest({ op: 'action', id: 'party.move-forward' }), { id: 'party.move-forward' });
    assert.deepEqual(commands, ['playtest.action party.move-forward']);
    for (const id of ['party move', 'a;b', 'x\nengine.time.advance 9', 'é', 'a'.repeat(65)]) {
      await assert.rejects(playtest({ op: 'action', id }), /invalid target\/action id/);
    }
    assert.equal(commands.length, 1);
  } finally {
    uninstall();
    globalThis.fetch = realFetch;
  }
});

test('focus takes and releases the gameplay cursor the way a click and Escape do, and reports a refusal', async () => {
  const globals = globalThis as { document?: unknown; requestAnimationFrame?: unknown };
  const realDocument = globals.document;
  const realFrame = globals.requestAnimationFrame;
  globals.document = { activeElement: { tagName: 'CANVAS' }, pointerLockElement: null };
  globals.requestAnimationFrame = (callback: () => void) => setTimeout(callback, 1);
  let captured = false;
  let grants = true;
  const calls: string[] = [];
  const uninstall = installPlaytestInspection(async () => {}, async () => {}, {
    capture: () => { calls.push('capture'); if (grants) setTimeout(() => { captured = true; }, 5); },
    release: () => { calls.push('release'); captured = false; },
    state: () => ({ interactionMode: 'gameplay', cursorMode: 'pointer-lock', pointerCaptured: captured }),
  });
  try {
    const playtest = (globalThis as { __rustyPlaytest?: Playtest }).__rustyPlaytest!;
    assert.deepEqual(await playtest({ op: 'focus' }), { focused: true, pointerLocked: false, interactionMode: 'gameplay', cursorMode: 'pointer-lock', pointerCaptured: false });
    assert.equal((await playtest({ op: 'focus', mode: 'capture' }) as { pointerCaptured: boolean }).pointerCaptured, true);
    assert.equal((await playtest({ op: 'focus', mode: 'release' }) as { pointerCaptured: boolean }).pointerCaptured, false);
    grants = false;
    const refused = await playtest({ op: 'focus', mode: 'capture' }) as { pointerCaptured: boolean; note?: string };
    assert.equal(refused.pointerCaptured, false);
    assert.match(refused.note ?? '', /did not grant/);
    await assert.rejects(playtest({ op: 'focus', mode: 'grab' }), /focus mode must be/);
    assert.deepEqual(calls, ['capture', 'release', 'capture']);
  } finally {
    uninstall();
    globals.document = realDocument;
    globals.requestAnimationFrame = realFrame;
  }
});
