import type { Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

export interface PackedPlacementResourceEvidence {
  preparedCount: number;
  resourceReadCount: number;
}

interface PackedPlacementPayload {
  readonly payload: Record<string, unknown>;
  readonly positions: readonly number[];
  readonly normals: readonly number[];
  readonly uvs: readonly number[] | null;
  readonly indices: readonly number[];
  positionsByteOffset: number;
  normalsByteOffset: number;
  uvsByteOffset: number | null;
  indicesByteOffset: number;
}

/**
 * Turn the real adapter's inline placement response into one valid packed
 * resource so the browser test exercises Studio's ordinary host resolver.
 */
export async function installPackedPlacementResourceAdapter(
  page: Page,
  projectRoot: string,
): Promise<PackedPlacementResourceEvidence> {
  const evidence: PackedPlacementResourceEvidence = {
    preparedCount: 0,
    resourceReadCount: 0,
  };
  page.on('request', (request) => {
    if (request.url().includes('/api/studio-render-resource')
      && request.url().includes('browser-placement-')) {
      evidence.resourceReadCount += 1;
    }
  });
  await page.route('**/api/studio-adapter', async (route) => {
    const request = route.request();
    const body = request.postDataJSON() as { readonly type?: unknown } | null;
    if (body?.type !== 'prepareVoxelObjectPlacement') {
      await route.continue();
      return;
    }
    const upstream = await route.fetch();
    const packed = await packPlacementResponse(await upstream.json(), projectRoot);
    evidence.preparedCount += 1;
    await route.fulfill({ response: upstream, json: packed });
  });
  return evidence;
}

async function packPlacementResponse(
  input: unknown,
  projectRoot: string,
): Promise<Record<string, unknown>> {
  const response = record(structuredClone(input), 'placement response');
  if (response['type'] !== 'voxelObjectPlacementPrepared') {
    throw new Error('placement route received an unexpected response type');
  }
  const resourceFrame = record(response['resourceFrame'], 'placement resource frame');
  const operations = array(resourceFrame['ops'], 'placement resource operations');
  const payloads: PackedPlacementPayload[] = [];
  for (const operationInput of operations) {
    const operation = record(operationInput, 'placement resource operation');
    if (operation['op'] !== 'defineVoxelObject') continue;
    const asset = record(operation['asset'], 'placement voxel object asset');
    const meshes = array(asset['meshes'], 'placement voxel object meshes');
    for (const meshInput of meshes) {
      const mesh = record(meshInput, 'placement voxel object mesh');
      const payload = record(mesh['payload'], 'placement voxel object payload');
      const source = record(payload['source'], 'placement voxel object payload source');
      if (source['kind'] !== 'inline') {
        throw new Error('browser placement fixture expected one inline source to pack');
      }
      payloads.push({
        payload,
        positions: numberArray(source['positions'], 'placement positions'),
        normals: numberArray(source['normals'], 'placement normals'),
        uvs: Object.hasOwn(source, 'uvs')
          ? numberArray(source['uvs'], 'placement uvs')
          : null,
        indices: integerArray(source['indices'], 'placement indices'),
        positionsByteOffset: 0,
        normalsByteOffset: 0,
        uvsByteOffset: null,
        indicesByteOffset: 0,
      });
    }
  }
  if (payloads.length === 0) throw new Error('placement response had no mesh payload to pack');

  let byteLength = 16;
  for (const payload of payloads) {
    payload.positionsByteOffset = byteLength;
    byteLength += payload.positions.length * 4;
    payload.normalsByteOffset = byteLength;
    byteLength += payload.normals.length * 4;
    if (payload.uvs !== null) {
      payload.uvsByteOffset = byteLength;
      byteLength += payload.uvs.length * 4;
    }
    payload.indicesByteOffset = byteLength;
    byteLength += payload.indices.length * 4;
  }
  const bytes = Buffer.alloc(byteLength);
  Buffer.from(payloads.some(({ uvs }) => uvs !== null) ? 'RMSHLE02' : 'RMSHLE01', 'ascii')
    .copy(bytes, 0);
  bytes.writeUInt32LE(byteLength, 8);
  bytes.writeUInt32LE(1, 12);
  for (const payload of payloads) {
    payload.positions.forEach((value, index) => {
      bytes.writeFloatLE(value, payload.positionsByteOffset + index * 4);
    });
    payload.normals.forEach((value, index) => {
      bytes.writeFloatLE(value, payload.normalsByteOffset + index * 4);
    });
    payload.uvs?.forEach((value, index) => {
      bytes.writeFloatLE(value, (payload.uvsByteOffset as number) + index * 4);
    });
    payload.indices.forEach((value, index) => {
      bytes.writeUInt32LE(value, payload.indicesByteOffset + index * 4);
    });
  }

  const digest = createHash('sha256').update(bytes).digest('hex');
  const contentHash = `sha256:${digest}`;
  const resource = `mesh-resource/${digest}`;
  const sourcePath = `.rusty-engine-cache/render-resources/browser-placement-${digest}.rmesh`;
  for (const payload of payloads) {
    payload.payload['source'] = {
      kind: 'resource',
      resource,
      contentHash,
      byteLength,
      encoding: payload.uvs === null ? 'packedStreamsLeV1' : 'packedStreamsLeV2',
      positionsByteOffset: payload.positionsByteOffset,
      normalsByteOffset: payload.normalsByteOffset,
      ...(payload.uvsByteOffset === null ? {} : { uvsByteOffset: payload.uvsByteOffset }),
      indicesByteOffset: payload.indicesByteOffset,
    };
  }
  await mkdir(join(projectRoot, '.rusty-engine-cache/render-resources'), { recursive: true });
  await writeFile(join(projectRoot, sourcePath), bytes);
  response['meshResources'] = [{ resource, contentHash, byteLength, sourcePath }];
  return response;
}

function record(input: unknown, label: string): Record<string, unknown> {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new TypeError(`${label} must be an object`);
  }
  return input as Record<string, unknown>;
}

function array(input: unknown, label: string): unknown[] {
  if (!Array.isArray(input)) throw new TypeError(`${label} must be an array`);
  return input;
}

function numberArray(input: unknown, label: string): number[] {
  return array(input, label).map((value) => {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      throw new TypeError(`${label} must contain only finite numbers`);
    }
    return value;
  });
}

function integerArray(input: unknown, label: string): number[] {
  return numberArray(input, label).map((value) => {
    if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new TypeError(`${label} must contain only unsigned 32-bit integers`);
    }
    return value;
  });
}
