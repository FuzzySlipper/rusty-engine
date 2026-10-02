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
