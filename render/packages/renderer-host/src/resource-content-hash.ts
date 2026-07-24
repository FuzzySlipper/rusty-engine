export async function rendererResourceContentHash(
  data: ArrayBuffer,
  expected: string,
): Promise<string> {
  if (/^[0-9a-f]{16}$/u.test(expected)) {
    return fnv1a64Hex(data);
  }
  const prefixed = expected.startsWith('sha256:');
  if (!/^(?:sha256:)?[0-9a-f]{64}$/u.test(expected)) {
    throw new Error(`unsupported renderer resource content hash ${expected}`);
  }
  if (globalThis.crypto?.subtle === undefined) {
    throw new Error('Web Crypto SHA-256 is unavailable');
  }
  const digest = await globalThis.crypto.subtle.digest('SHA-256', data);
  const hex = [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('');
  return prefixed ? `sha256:${hex}` : hex;
}

function fnv1a64Hex(data: ArrayBuffer): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new Uint8Array(data)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}
