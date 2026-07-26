import assert from 'node:assert/strict';
import test from 'node:test';

import { rendererResourceContentHash } from './resource-content-hash.js';

const vectors = [
  {
    text: '',
    sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
  },
  {
    text: 'abc',
    sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
  },
  {
    text: 'The quick brown fox jumps over the lazy dog',
    sha256: 'd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592',
  },
] as const;

void test('SHA-256 renderer identities work without secure-context Web Crypto', async () => {
  const cryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  Object.defineProperty(globalThis, 'crypto', { configurable: true, value: undefined });
  try {
    for (const vector of vectors) {
      const bytes = new TextEncoder().encode(vector.text).buffer;
      assert.equal(await rendererResourceContentHash(bytes, vector.sha256), vector.sha256);
      assert.equal(
        await rendererResourceContentHash(bytes, `sha256:${vector.sha256}`),
        `sha256:${vector.sha256}`,
      );
    }
  } finally {
    if (cryptoDescriptor === undefined) delete (globalThis as { crypto?: unknown }).crypto;
    else Object.defineProperty(globalThis, 'crypto', cryptoDescriptor);
  }
});

void test('legacy FNV identities and unsupported hash shapes remain explicit', async () => {
  const bytes = new Uint8Array([1, 2, 3]).buffer;
  assert.equal(await rendererResourceContentHash(bytes, 'd0aa6218672cf5ab'), 'd0aa6218672cf5ab');
  await assert.rejects(
    rendererResourceContentHash(bytes, 'sha256:not-a-hash'),
    /unsupported renderer resource content hash/u,
  );
});
