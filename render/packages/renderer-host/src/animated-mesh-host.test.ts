import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

import type { AnimationClipPack } from '@rusty-engine/render-contracts';
import {
  loadRendererAnimatedMeshSource,
  RendererHostError,
  RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_COUNT,
  RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES,
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

void test('animated clip-pack count admits the exact boundary and rejects one over before resolving', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const packs = Array.from(
      { length: RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_COUNT },
      (_, index) => packDescriptor(`clip-pack/count-${String(index)}`),
    );
    const source = await loadRendererAnimatedMeshSource(
      manifest(packs),
      () => Promise.resolve(BASE_BYTES.slice(0)),
    );
    for (const pack of packs) {
      assert.ok(source.getAnimationClipPackResource(asClipPack(pack)));
    }

    const overPacks = [
      ...packs,
      packDescriptor('clip-pack/count-one-over'),
    ];
    let resolverCalls = 0;
    await assert.rejects(
      loadRendererAnimatedMeshSource(
        manifest(overPacks),
        () => {
          resolverCalls += 1;
          return Promise.resolve(BASE_BYTES.slice(0));
        },
      ),
      (error: unknown) => error instanceof RendererHostError
        && error.diagnostics[0]?.code === 'animated_mesh_clip_pack_budget_exceeded',
    );
    assert.equal(resolverCalls, 0, 'count rejection happens before any resource is resolved');
  } finally {
    restore();
  }
});

void test('animated clip-pack combined bytes admit the exact finite bound and reject one over without publishing a partial source', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const exactBytes = paddedFixture(RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES);
    const exactPack = packDescriptor('clip-pack/bytes-exact', sha256(exactBytes));
    const exactSource = await loadRendererAnimatedMeshSource(
      manifest([exactPack]),
      (descriptor) => Promise.resolve(
        descriptor.asset === BASE_ASSET ? BASE_BYTES.slice(0) : exactBytes.slice(0),
      ),
    );
    assert.ok(exactSource.getAnimationClipPackResource(asClipPack(exactPack)));

    const overBytes = paddedFixture(RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES + 1);
    const overPack = packDescriptor('clip-pack/bytes-one-over');
    const callerBytes = new Uint8Array(overBytes).slice();
    const unpublished = Symbol('unpublished');
    let published: unknown = unpublished;
    try {
      published = await loadRendererAnimatedMeshSource(
        manifest([overPack]),
        (descriptor) => Promise.resolve(
          descriptor.asset === BASE_ASSET ? BASE_BYTES.slice(0) : overBytes,
        ),
      );
      assert.fail('one-over clip-pack bytes must reject');
    } catch (error: unknown) {
      assert.ok(
        error instanceof RendererHostError
          && error.diagnostics[0]?.code === 'animated_mesh_clip_pack_budget_exceeded',
      );
    }
    assert.equal(published, unpublished, 'a previously resolved candidate is not published on rejection');
    assert.deepEqual(new Uint8Array(overBytes), callerBytes, 'resolver-owned bytes remain untouched');
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
