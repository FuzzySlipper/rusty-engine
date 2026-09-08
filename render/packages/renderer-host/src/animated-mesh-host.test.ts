import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

import type { AnimationClipPack } from '@rusty-engine/render-contracts';
import {
  loadRendererAnimatedMeshSource,
  type RendererAnimatedMeshResourceDescriptor,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimationClipPackResourceDescriptor,
} from './animated-mesh-host.js';

const FIXTURE = resolve(
  import.meta.dirname,
  '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb',
);
const BASE_ASSET = 'mesh-animation/clip-pack-budget-base';
const BASE_BYTES = fixtureBytes();
const BASE_HASH = sha256(BASE_BYTES);

void test('animated clip packs admit a set beyond the retired count cap', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const packs = Array.from(
      { length: 17 },
      (_, index) => packDescriptor(`clip-pack/count-${String(index)}`),
    );
    const source = await loadRendererAnimatedMeshSource(
      manifest(packs),
      () => Promise.resolve(BASE_BYTES.slice(0)),
    );
    for (const pack of packs) {
      assert.ok(source.getAnimationClipPackResource(asClipPack(pack)));
    }
  } finally {
    restore();
  }
});

void test('animated clip packs admit bytes beyond the retired aggregate cap', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const bytes = paddedFixture(32 * 1024 * 1024 + 1);
    const pack = packDescriptor('clip-pack/large', sha256(bytes));
    const source = await loadRendererAnimatedMeshSource(
      manifest([pack]),
      (descriptor) => Promise.resolve(
        descriptor.asset === BASE_ASSET ? BASE_BYTES.slice(0) : bytes.slice(0),
      ),
    );
    assert.ok(source.getAnimationClipPackResource(asClipPack(pack)));
  } finally {
    restore();
  }
});

void test('standalone clip-pack loading resolves packs sequentially with maximum resolver concurrency one', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const packs = [
      packDescriptor('clip-pack/sequential-0'),
      packDescriptor('clip-pack/sequential-1'),
      packDescriptor('clip-pack/sequential-2'),
    ];
    const calls: string[] = [];
    let active = 0;
    let maximumActive = 0;
    const callerSnapshot = new Uint8Array(BASE_BYTES).slice();
    const source = await loadRendererAnimatedMeshSource(
      manifest(packs),
      async (descriptor) => {
        calls.push(descriptor.asset);
        active += 1;
        maximumActive = Math.max(maximumActive, active);
        await Promise.resolve();
        active -= 1;
        return BASE_BYTES.slice(0);
      },
    );

    assert.equal(maximumActive, 1);
    assert.deepEqual(calls, [BASE_ASSET, ...packs.map((pack) => pack.asset)]);
    assert.deepEqual(new Uint8Array(BASE_BYTES), callerSnapshot);
    for (const pack of packs) {
      assert.ok(source.getAnimationClipPackResource(asClipPack(pack)));
    }
  } finally {
    restore();
  }
});

function manifest(
  clipPacks: readonly RendererAnimationClipPackResourceDescriptor[],
): RendererAnimatedMeshResourceManifest {
  return {
    kind: 'rusty_renderer_animated_mesh_resources.v1',
    resources: [{ asset: BASE_ASSET, contentHash: BASE_HASH, clipIds: [] }],
    clipPacks,
  };
}

function packDescriptor(
  asset: string,
  contentHash = BASE_HASH,
): RendererAnimationClipPackResourceDescriptor {
  return { asset, contentHash, clipIds: ['idle'] };
}

function asClipPack(descriptor: RendererAnimatedMeshResourceDescriptor): AnimationClipPack {
  return { asset: descriptor.asset } as unknown as AnimationClipPack;
}

function fixtureBytes(): ArrayBuffer {
  const bytes = readFileSync(FIXTURE);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

function paddedFixture(byteLength: number): ArrayBuffer {
  const padded = new Uint8Array(byteLength);
  padded.set(new Uint8Array(BASE_BYTES));
  return padded.buffer;
}

function sha256(data: ArrayBuffer): string {
  return `sha256:${createHash('sha256').update(new Uint8Array(data)).digest('hex')}`;
}

function installGltfNodeGlobals(): () => void {
  const globals = globalThis as unknown as { self: unknown };
  const previousSelf = globals.self;
  const previousWarn = console.warn;
  const previousError = console.error;
  globals.self = globalThis;
  console.warn = () => undefined;
  console.error = () => undefined;
  return () => {
    globals.self = previousSelf;
    console.warn = previousWarn;
    console.error = previousError;
  };
}
