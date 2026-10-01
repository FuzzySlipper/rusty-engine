import assert from 'node:assert/strict';
import test from 'node:test';

import { advanceRustyApplicationFrameCursor } from './frame-view.js';

test('a viewer shows newer frames and skips one that arrives late', () => {
  const first = advanceRustyApplicationFrameCursor({ newest: 0, nextAfter: 0 }, 0, 5);
  assert.deepEqual(first, { show: true, newest: 5, nextAfter: 5 });
  const next = advanceRustyApplicationFrameCursor(first, 5, 6);
  assert.deepEqual(next, { show: true, newest: 6, nextAfter: 6 });
  // The other outstanding request answers with frame 6 too: not shown again.
  const late = advanceRustyApplicationFrameCursor(next, 5, 6);
  assert.deepEqual(late, { show: false, newest: 6, nextAfter: 7 });
});

test('a runtime that restarted its frame numbering is followed from its frame', () => {
  // The viewer waited after frame 8096 of the replaced runtime; the new
  // runtime answers with its frame 30.
  const restarted = advanceRustyApplicationFrameCursor({ newest: 8096, nextAfter: 8096 }, 8096, 30);
  assert.deepEqual(restarted, { show: true, newest: 30, nextAfter: 30 });
});
