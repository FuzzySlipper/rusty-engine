import assert from 'node:assert/strict';
import test from 'node:test';

import type { MeshResourceReadout } from '@rusty-engine/studio-adapter-client';

import {
  activeStudioMeshResources,
  resolveStudioMeshResource,
} from './mesh-resources.js';

const CANONICAL_DIGEST = '1'.repeat(64);
const PLACEMENT_DIGEST = '2'.repeat(64);

void test('placement-only packed resources mount and resolve only while admitted', async () => {
  const canonicalResource = resourceReadout(CANONICAL_DIGEST, 'canonical', 16);
  const canonical = [canonicalResource];
  const placement = resourceReadout(PLACEMENT_DIGEST, 'placement', 136);
  const canonicalBefore = structuredClone(canonical);
  const active = activeStudioMeshResources(canonical, [placement]);
  assert.deepEqual(active.map((resource) => resource.resource), [
    canonicalResource.resource,
    placement.resource,
  ]);

  const packed = packedMeshBytes(placement.byteLength);
  const reads: string[] = [];
  const resolved = await resolveStudioMeshResource(
    '/projects/loading-bay',
    active,
    placement,
    (_projectRoot, sourcePath, contentHash) => {
      reads.push(`${sourcePath}:${contentHash}`);
      return Promise.resolve(copyArrayBuffer(packed));
    },
  );
  assert.equal(resolved.byteLength, placement.byteLength);
  assert.deepEqual(reads, [`${placement.sourcePath}:${placement.contentHash}`]);

  const afterDiscard = activeStudioMeshResources(canonical, []);
  let readAfterDiscard = false;
  await assert.rejects(
    resolveStudioMeshResource(
      '/projects/loading-bay',
      afterDiscard,
      placement,
      () => {
        readAfterDiscard = true;
        return Promise.resolve(copyArrayBuffer(packed));
      },
    ),
    /is not in the current Rust readout/u,
  );
  assert.equal(readAfterDiscard, false);
  assert.deepEqual(afterDiscard, canonicalBefore);
  assert.deepEqual(canonical, canonicalBefore, 'temporary admission never mutates canonical readout');
});

void test('canonical mesh resources retain collision precedence and exact descriptor guards', async () => {
  const canonical = resourceReadout(CANONICAL_DIGEST, 'canonical', 16);
  const placementCollision = {
    ...canonical,
    byteLength: 136,
    sourcePath: '.rusty-engine-cache/render-resources/placement-collision.rmesh',
  };
  const active = activeStudioMeshResources([canonical], [placementCollision]);
  assert.deepEqual(active, [canonical]);

  await assert.rejects(
    resolveStudioMeshResource(
      '/projects/loading-bay',
      active,
      placementCollision,
      () => Promise.resolve(new ArrayBuffer(placementCollision.byteLength)),
    ),
    /is not in the current Rust readout/u,
  );
});

function resourceReadout(
  digest: string,
  label: string,
  byteLength: number,
): MeshResourceReadout {
  return {
    resource: `mesh-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    byteLength,
    sourcePath: `.rusty-engine-cache/render-resources/${label}-${digest}.rmesh`,
  };
}

function packedMeshBytes(byteLength: number): Uint8Array {
  const bytes = new Uint8Array(byteLength);
  bytes.set([0x52, 0x4d, 0x53, 0x48, 0x4c, 0x45, 0x30, 0x31]);
  const header = new DataView(bytes.buffer);
  header.setUint32(8, byteLength, true);
  header.setUint32(12, 1, true);
  return bytes;
}

function copyArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(copy).set(bytes);
  return copy;
}
