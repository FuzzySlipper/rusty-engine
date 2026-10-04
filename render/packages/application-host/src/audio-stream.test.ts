import assert from 'node:assert/strict';
import test from 'node:test';

import { samples } from './audio-stream.js';

test('streamed PCM decodes little-endian 16-bit samples', () => {
  const bytes = new Uint8Array([0xff, 0x7f, 0x01, 0x80, 0x00, 0x00, 0x00, 0x80]);
  assert.deepEqual([...samples(bytes)], [32767 / 32768, -32767 / 32768, 0, -1]);
  // A chunk may start anywhere in the response's underlying buffer.
  assert.deepEqual([...samples(new Uint8Array([9, 0x00, 0x40]).subarray(1))], [0.5]);
});
