import { bytesToHex } from '@noble/hashes/utils.js';
import { sha256 } from '@noble/hashes/sha2.js';
import { unzlibSync } from 'fflate';

import type { TextureDescriptor } from '@rusty-engine/render-contracts';

export interface DecodedPngTexture {
  readonly pixels: Uint8Array;
  readonly width: number;
  readonly height: number;
}

export class PngTextureError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'PngTextureError';
  }
}

export function decodeAdmittedPngTexture(
  descriptor: TextureDescriptor,
  bytes: Uint8Array,
): DecodedPngTexture {
  const payload = descriptor.payload;
  if (payload === undefined) throw new PngTextureError('texture has no retained payload');
  if (bytes.byteLength !== payload.byteLength) {
    throw new PngTextureError(`encoded byte length ${String(bytes.byteLength)} does not match ${String(payload.byteLength)}`);
  }
  const actualHash = `sha256:${bytesToHex(sha256(bytes))}`;
  if (actualHash !== payload.contentHash || descriptor.contentHash !== actualHash) {
    throw new PngTextureError(`content hash mismatch: expected ${payload.contentHash}, received ${actualHash}`);
  }
  return decodePngRgba8(bytes, descriptor.width, descriptor.height);
}

function decodePngRgba8(bytes: Uint8Array, width: number, height: number): DecodedPngTexture {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.byteLength < 45 || signature.some((value, index) => bytes[index] !== value)) {
    throw new PngTextureError('invalid PNG signature or truncated stream');
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const idat: Uint8Array[] = [];
  let offset = 8;
  let sawHeader = false;
  let sawEnd = false;
  while (offset < bytes.byteLength) {
    if (offset + 12 > bytes.byteLength) throw new PngTextureError('truncated PNG chunk');
    const length = view.getUint32(offset, false);
    const typeOffset = offset + 4;
    const dataOffset = typeOffset + 4;
    const dataEnd = dataOffset + length;
    const chunkEnd = dataEnd + 4;
    if (!Number.isSafeInteger(chunkEnd) || chunkEnd > bytes.byteLength) {
      throw new PngTextureError('PNG chunk exceeds encoded bytes');
    }
    const type = String.fromCharCode(...bytes.subarray(typeOffset, dataOffset));
    const expectedCrc = view.getUint32(dataEnd, false);
    if (crc32(bytes.subarray(typeOffset, dataEnd)) !== expectedCrc) {
      throw new PngTextureError(`PNG ${type} CRC mismatch`);
    }
    if (type === 'IHDR') {
      if (sawHeader || offset !== 8 || length !== 13) throw new PngTextureError('invalid PNG IHDR');
      const actualWidth = view.getUint32(dataOffset, false);
      const actualHeight = view.getUint32(dataOffset + 4, false);
      if (actualWidth !== width || actualHeight !== height) {
        throw new PngTextureError('PNG dimensions do not match the descriptor');
      }
      if (bytes[dataOffset + 8] !== 8 || bytes[dataOffset + 9] !== 6
        || bytes[dataOffset + 10] !== 0 || bytes[dataOffset + 11] !== 0
        || bytes[dataOffset + 12] !== 0) {
        throw new PngTextureError('only non-interlaced RGBA8 PNG is supported');
      }
      sawHeader = true;
    } else if (type === 'IDAT') {
      if (!sawHeader || sawEnd) throw new PngTextureError('PNG IDAT ordering is invalid');
      idat.push(bytes.slice(dataOffset, dataEnd));
    } else if (type === 'IEND') {
      if (!sawHeader || idat.length === 0 || sawEnd || length !== 0 || chunkEnd !== bytes.byteLength) {
        throw new PngTextureError('invalid PNG IEND');
      }
      sawEnd = true;
    } else if ((bytes[typeOffset] as number) >= 65 && (bytes[typeOffset] as number) <= 90) {
      throw new PngTextureError(`unsupported critical PNG chunk ${type}`);
    }
    offset = chunkEnd;
  }
  if (!sawHeader || !sawEnd || idat.length === 0) throw new PngTextureError('incomplete PNG stream');

  const compressedLength = idat.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const compressed = new Uint8Array(compressedLength);
  let cursor = 0;
  for (const chunk of idat) {
    compressed.set(chunk, cursor);
    cursor += chunk.byteLength;
  }
  let filtered: Uint8Array;
  try {
    filtered = unzlibSync(compressed);
  } catch (cause) {
    throw new PngTextureError(`PNG deflate stream is invalid: ${cause instanceof Error ? cause.message : String(cause)}`);
  }
  const rowBytes = width * 4;
  const expectedFiltered = height * (rowBytes + 1);
  if (filtered.byteLength !== expectedFiltered) {
    throw new PngTextureError(`decoded PNG length ${String(filtered.byteLength)} does not match ${String(expectedFiltered)}`);
  }
  const pixels = new Uint8Array(width * height * 4);
  for (let row = 0; row < height; row++) {
    const filterOffset = row * (rowBytes + 1);
    const filter = filtered[filterOffset] as number;
    if (filter > 4) throw new PngTextureError(`unsupported PNG row filter ${String(filter)}`);
    const sourceOffset = filterOffset + 1;
    const targetOffset = row * rowBytes;
    for (let column = 0; column < rowBytes; column++) {
      const raw = filtered[sourceOffset + column] as number;
      const left = column >= 4 ? pixels[targetOffset + column - 4] as number : 0;
      const above = row > 0 ? pixels[targetOffset + column - rowBytes] as number : 0;
      const upperLeft = row > 0 && column >= 4
        ? pixels[targetOffset + column - rowBytes - 4] as number
        : 0;
      const predictor = filter === 0 ? 0
        : filter === 1 ? left
          : filter === 2 ? above
            : filter === 3 ? Math.floor((left + above) / 2)
              : paeth(left, above, upperLeft);
      pixels[targetOffset + column] = (raw + predictor) & 0xff;
    }
  }
  return { pixels, width, height };
}

function paeth(left: number, above: number, upperLeft: number): number {
  const estimate = left + above - upperLeft;
  const leftDistance = Math.abs(estimate - left);
  const aboveDistance = Math.abs(estimate - above);
  const upperLeftDistance = Math.abs(estimate - upperLeft);
  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) return left;
  return aboveDistance <= upperLeftDistance ? above : upperLeft;
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffff_ffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc & 1) === 0 ? crc >>> 1 : (crc >>> 1) ^ 0xedb8_8320;
    }
  }
  return (crc ^ 0xffff_ffff) >>> 0;
}
