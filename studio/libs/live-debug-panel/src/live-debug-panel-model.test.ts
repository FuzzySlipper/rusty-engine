import assert from 'node:assert/strict';
import test from 'node:test';

import {
  LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES,
  appendLiveDebugTranscript,
  commandSummary,
  historyCommand,
} from './live-debug-panel-model.js';

void test('transcript retains the most recent bounded command responses', () => {
  let entries = [] as ReturnType<typeof appendLiveDebugTranscript>;
  for (let index = 0; index <= LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES; index += 1) {
    entries = appendLiveDebugTranscript(entries, {
      command: `command.${String(index)}`,
      message: `response ${String(index)}`,
      succeeded: true,
    });
  }
  assert.equal(entries.length, LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES);
  assert.equal(entries[0]?.command, 'command.1');
  assert.equal(entries.at(-1)?.command, `command.${String(LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES)}`);
});

void test('history navigation restores older commands and clears after the newest entry', () => {
  const history = ['debug.world', 'debug.entity 42'];
  assert.deepEqual(historyCommand(history, null, -1), { cursor: 1, command: 'debug.entity 42' });
  assert.deepEqual(historyCommand(history, 1, -1), { cursor: 0, command: 'debug.world' });
  assert.deepEqual(historyCommand(history, 0, 1), { cursor: 1, command: 'debug.entity 42' });
  assert.deepEqual(historyCommand(history, 1, 1), { cursor: null, command: '' });
});

void test('catalog labels retain parameter names and types for command help', () => {
  assert.equal(commandSummary({
    name: 'debug.entity',
    description: 'Reads one entity.',
    parameters: [{ name: 'entityId', type: 'u64' }],
  }), 'debug.entity entityId: u64');
});
