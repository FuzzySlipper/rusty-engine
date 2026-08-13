import { MAX_RENDER_LIGHT_INTENSITY, type RenderFrameDiff } from './render.js';
import type { PresentationFrameDiff } from './presentation.js';

const JSON_SAFE_INTEGER_MAX = 9_007_199_254_740_991;

export class ContractDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ContractDecodeError';
  }
}

export function decodeRenderFrameDiff(input: unknown): RenderFrameDiff {
  const frame = recordOptional(input, '$', ['schemaVersion', 'ops'], ['publication']);
  if (frame['schemaVersion'] !== 1) {
    fail('$.schemaVersion', 'must equal 1');
  }
  const ops = list(frame['ops'], '$.ops');
  if (frame['publication'] !== undefined) {
    const publication = record(
      frame['publication'],
      '$.publication',
      ['stream', 'baseRevision', 'revision', 'operationCount'],
    );
    const stream = text(publication['stream'], '$.publication.stream');
    if (stream.trim().length === 0 || stream.length > 256) {
      fail('$.publication.stream', 'must contain 1..=256 characters');
    }
    const baseRevision = integer(
      publication['baseRevision'], '$.publication.baseRevision', 0, JSON_SAFE_INTEGER_MAX,
    );
    const revision = integer(
      publication['revision'], '$.publication.revision', 0, JSON_SAFE_INTEGER_MAX,
    );
    if (revision !== baseRevision + 1) {
      fail('$.publication.revision', 'must immediately follow baseRevision');
    }
    integer(publication['operationCount'], '$.publication.operationCount', 0, 4_294_967_295);
    if (publication['operationCount'] !== ops.length) {
      fail('$.publication.operationCount', `must equal ops length ${String(ops.length)}`);
    }
  }
  ops.forEach((operation, index) => renderDiff(operation, `$.ops[${String(index)}]`));
  return input as RenderFrameDiff;
}

export function decodePresentationFrameDiff(input: unknown): PresentationFrameDiff {
  const frame = record(input, '$', ['schemaVersion', 'ops']);
  if (frame['schemaVersion'] !== 1) {
    fail('$.schemaVersion', 'must equal 1');
  }
  const ops = list(frame['ops'], '$.ops');
  ops.forEach((operation, index) => {
    const path = `$.ops[${String(index)}]`;
    const value = record(operation, path, ['domain', 'meta', 'op']);
    const meta = record(value['meta'], `${path}.meta`, ['sequence']);
    integer(meta['sequence'], `${path}.meta.sequence`, 0, 4_294_967_295);
    if (meta['sequence'] !== index) {
      fail(`${path}.meta.sequence`, `must equal ordered index ${String(index)}`);
    }
    const domain = enumeration(
      value['domain'],
      `${path}.domain`,
      ['audio', 'billboard', 'particle', 'telemetryOverlay', 'animation'] as const,
    );
    presentationOperation(domain, value['op'], `${path}.op`);
  });
  return input as PresentationFrameDiff;
}

function renderDiff(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const op = text(base['op'], `${path}.op`);
  switch (op) {
    case 'create': {
      const value = record(input, path, ['op', 'handle', 'parent', 'node']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      renderNode(value['node'], `${path}.node`);
      return;
    }
    case 'update': {
      const value = record(input, path, [
        'op', 'handle', 'transform', 'material', 'visible', 'metadata',
      ]);
      handle(value['handle'], `${path}.handle`);
      nullable(value['transform'], `${path}.transform`, transform);
      nullable(value['material'], `${path}.material`, material);
      nullable(value['visible'], `${path}.visible`, booleanValue);
      nullable(value['metadata'], `${path}.metadata`, metadata);
      return;
    }
    case 'destroy': {
      const value = record(input, path, ['op', 'handle']);
      handle(value['handle'], `${path}.handle`);
      return;
    }
    case 'replaceMeshPayload': {
      const value = record(input, path, ['op', 'handle', 'payload']);
      handle(value['handle'], `${path}.handle`);
      meshPayload(value['payload'], `${path}.payload`);
      return;
    }
    case 'createLight': {
      const value = record(input, path, ['op', 'handle', 'parent', 'light']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      light(value['light'], `${path}.light`);
      return;
    }
    case 'updateLight': {
      const value = record(input, path, ['op', 'handle', 'light']);
      handle(value['handle'], `${path}.handle`);
      light(value['light'], `${path}.light`);
      return;
    }
    case 'defineMaterial': {
      const value = record(input, path, ['op', 'material']);
      renderMaterial(value['material'], `${path}.material`);
      return;
    }
    case 'setMaterialInstanceParameters': {
      const value = record(input, path, ['op', 'handle', 'slot', 'parameters']);
      handle(value['handle'], `${path}.handle`);
      integer(value['slot'], `${path}.slot`, 0, 65_535);
      nullable(value['parameters'], `${path}.parameters`, materialParameters);
      return;
    }
    case 'defineTexture': {
      const value = record(input, path, ['op', 'texture']);
      texture(value['texture'], `${path}.texture`);
      return;
    }
    case 'defineSpriteAtlas': {
      const value = record(input, path, ['op', 'atlas']);
      spriteAtlas(value['atlas'], `${path}.atlas`);
      return;
    }
    case 'defineStaticMesh': {
      const value = record(input, path, ['op', 'asset']);
      staticMesh(value['asset'], `${path}.asset`);
      return;
    }
    case 'defineAnimatedMesh': {
      const value = record(input, path, ['op', 'asset']);
      animatedMesh(value['asset'], `${path}.asset`);
      return;
    }
    case 'defineVoxelObject': {
      const value = record(input, path, ['op', 'asset']);
      voxelObject(value['asset'], `${path}.asset`);
      return;
    }
    case 'releaseVoxelObject': {
      const value = record(input, path, ['op', 'asset']);
      nonEmptyText(value['asset'], `${path}.asset`);
      return;
    }
    case 'createStaticMeshInstance': {
      const value = record(input, path, ['op', 'handle', 'parent', 'instance']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      staticMeshInstance(value['instance'], `${path}.instance`);
      return;
    }
    case 'createAnimatedMeshInstance': {
      const value = record(input, path, ['op', 'handle', 'parent', 'instance']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      animatedMeshInstance(value['instance'], `${path}.instance`);
      return;
    }
    case 'setAnimatedMeshPlayback': {
      const value = record(input, path, ['op', 'handle', 'playback']);
      handle(value['handle'], `${path}.handle`);
      playback(value['playback'], `${path}.playback`);
      return;
    }
    case 'createVoxelObjectInstance': {
      const value = record(input, path, ['op', 'handle', 'parent', 'instance']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      voxelObjectInstance(value['instance'], `${path}.instance`);
      return;
    }
    case 'setVoxelObjectFrame': {
      const value = record(input, path, ['op', 'handle', 'frame']);
      handle(value['handle'], `${path}.handle`);
      integer(value['frame'], `${path}.frame`, 0, 4_294_967_295);
      return;
    }
    case 'createSprite': {
      const value = record(input, path, ['op', 'handle', 'parent', 'sprite']);
      handle(value['handle'], `${path}.handle`);
      nullableHandle(value['parent'], `${path}.parent`);
      sprite(value['sprite'], `${path}.sprite`);
      return;
    }
    case 'updateSprite': {
      const value = record(input, path, [
        'op', 'handle', 'frame', 'tint', 'renderOrder', 'visible',
      ]);
      handle(value['handle'], `${path}.handle`);
      nullable(value['frame'], `${path}.frame`, nonNegativeInteger);
      nullable(value['tint'], `${path}.tint`, color4);
      nullable(value['renderOrder'], `${path}.renderOrder`, integerValue);
      nullable(value['visible'], `${path}.visible`, booleanValue);
      return;
    }
    default:
      fail(`${path}.op`, `unsupported operation ${JSON.stringify(op)}`);
  }
}

function renderNode(input: unknown, path: string): void {
  const value = record(input, path, [
    'geometry', 'material', 'transform', 'visible', 'layer', 'metadata',
  ]);
  geometry(value['geometry'], `${path}.geometry`);
  material(value['material'], `${path}.material`);
  transform(value['transform'], `${path}.transform`);
  booleanValue(value['visible'], `${path}.visible`);
  enumeration(
    value['layer'],
    `${path}.layer`,
    ['scene', 'debug', 'ui', 'viewmodel'] as const,
  );
  metadata(value['metadata'], `${path}.metadata`);
}

function geometry(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(
    base['kind'],
    `${path}.kind`,
    ['group', 'cube', 'sphere', 'quad', 'point', 'line'] as const,
  );
  if (kind === 'line') {
    const value = record(input, path, ['kind', 'a', 'b']);
    vec3(value['a'], `${path}.a`);
    vec3(value['b'], `${path}.b`);
  } else {
    record(input, path, ['kind']);
  }
}

function material(input: unknown, path: string): void {
  const value = record(input, path, ['color', 'wireframe']);
  color4(value['color'], `${path}.color`);
  booleanValue(value['wireframe'], `${path}.wireframe`);
}

function transform(input: unknown, path: string): void {
  const value = record(input, path, ['translation', 'rotation', 'scale']);
  vec3(value['translation'], `${path}.translation`);
  const rotation = tuple(value['rotation'], `${path}.rotation`, 4);
  rotation.forEach((item, index) => finite(item, `${path}.rotation[${String(index)}]`));
  if (rotation.every((item) => item === 0)) {
    fail(`${path}.rotation`, 'must be non-zero');
  }
  vec3(value['scale'], `${path}.scale`);
}

function metadata(input: unknown, path: string): void {
  const value = record(input, path, ['sourceEntity', 'sourceSceneNode', 'tags', 'label']);
  nullable(value['sourceEntity'], `${path}.sourceEntity`, safeInteger);
  nullable(value['sourceSceneNode'], `${path}.sourceSceneNode`, safeInteger);
  const tags = list(value['tags'], `${path}.tags`);
  let previous: string | undefined;
  tags.forEach((item, index) => {
    const tag = nonEmptyText(item, `${path}.tags[${String(index)}]`);
    if (previous !== undefined && previous >= tag) {
      fail(`${path}.tags`, 'must be strictly sorted and unique');
    }
    previous = tag;
  });
  nullable(value['label'], `${path}.label`, nonEmptyText);
}

function meshPayload(input: unknown, path: string): void {
  const value = record(input, path, ['layout', 'groups', 'bounds', 'source', 'provenance']);
  const layout = record(value['layout'], `${path}.layout`, [
    'vertexCount', 'indexCount', 'indexWidth', 'attributes',
  ]);
  const vertexCount = integer(layout['vertexCount'], `${path}.layout.vertexCount`, 0, 4_294_967_295);
  const indexCount = integer(layout['indexCount'], `${path}.layout.indexCount`, 0, 4_294_967_295);
  enumeration(layout['indexWidth'], `${path}.layout.indexWidth`, ['u32'] as const);
  const attributes = list(layout['attributes'], `${path}.layout.attributes`);
  const names = new Set<string>();
  attributes.forEach((item, index) => {
    const attributePath = `${path}.layout.attributes[${String(index)}]`;
    const attribute = record(item, attributePath, ['name', 'components', 'kind']);
    const name = enumeration(
      attribute['name'],
      `${attributePath}.name`,
      ['position', 'normal', 'uv', 'color'] as const,
    );
    if (names.has(name)) fail(`${attributePath}.name`, 'is duplicated');
    names.add(name);
    const expected = name === 'uv' ? 2 : name === 'color' ? 4 : 3;
    if (attribute['components'] !== expected) {
      fail(`${attributePath}.components`, `must equal ${String(expected)}`);
    }
    enumeration(attribute['kind'], `${attributePath}.kind`, ['f32'] as const);
  });
  if (!names.has('position') || !names.has('normal')) {
    fail(`${path}.layout.attributes`, 'must declare position and normal');
  }
  bounds(value['bounds'], `${path}.bounds`);
  enumeration(
    value['provenance'],
    `${path}.provenance`,
    ['voxelChunk', 'voxelObject', 'staticAsset', 'generated', 'debug'] as const,
  );
  const sourceBase = looseRecord(value['source'], `${path}.source`);
  const sourceKind = enumeration(
    sourceBase['kind'],
    `${path}.source.kind`,
    ['inline', 'sharedBuffer', 'resource'] as const,
  );
  if (sourceKind === 'inline') {
    const source = recordOptional(value['source'], `${path}.source`,
      ['kind', 'positions', 'normals', 'indices'], ['uvs']);
    numberList(source['positions'], `${path}.source.positions`, vertexCount * 3, false);
    numberList(source['normals'], `${path}.source.normals`, vertexCount * 3, false);
    if (names.has('uv') !== Object.hasOwn(source, 'uvs')) {
      fail(`${path}.source.uvs`, 'must be present exactly when the uv attribute is declared');
    }
    if (Object.hasOwn(source, 'uvs')) {
      const uvs = numberList(source['uvs'], `${path}.source.uvs`, vertexCount * 2, false);
      if ((value['provenance'] === 'voxelChunk' || value['provenance'] === 'voxelObject')
        && uvs.some((coordinate) => Math.abs(coordinate) > 16_777_216)) {
        fail(`${path}.source.uvs`, 'voxel tile coordinate exceeds the exact f32 integer range');
      }
    }
    const indices = numberList(source['indices'], `${path}.source.indices`, indexCount, true);
    indices.forEach((item, index) => {
      if (item >= vertexCount) {
        fail(`${path}.source.indices[${String(index)}]`, 'is outside vertex range');
      }
    });
  } else if (sourceKind === 'sharedBuffer') {
    const source = recordOptional(value['source'], `${path}.source`,
      ['kind', 'buffer', 'positionsByteOffset', 'normalsByteOffset', 'indicesByteOffset'],
      ['uvsByteOffset']);
    safeInteger(source['buffer'], `${path}.source.buffer`);
    nonNegativeInteger(source['positionsByteOffset'], `${path}.source.positionsByteOffset`);
    nonNegativeInteger(source['normalsByteOffset'], `${path}.source.normalsByteOffset`);
    if (names.has('uv') !== Object.hasOwn(source, 'uvsByteOffset')) {
      fail(`${path}.source.uvsByteOffset`, 'must be present exactly when the uv attribute is declared');
    }
    if (Object.hasOwn(source, 'uvsByteOffset')) {
      nonNegativeInteger(source['uvsByteOffset'], `${path}.source.uvsByteOffset`);
    }
    nonNegativeInteger(source['indicesByteOffset'], `${path}.source.indicesByteOffset`);
  } else {
    const source = recordOptional(value['source'], `${path}.source`, [
      'kind', 'resource', 'contentHash', 'byteLength', 'encoding',
      'positionsByteOffset', 'normalsByteOffset', 'indicesByteOffset',
    ], ['uvsByteOffset']);
    const resource = nonEmptyText(source['resource'], `${path}.source.resource`);
    const contentHash = nonEmptyText(source['contentHash'], `${path}.source.contentHash`);
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(contentHash)?.[1];
    if (digest === undefined) fail(`${path}.source.contentHash`, 'must be a lowercase SHA-256 identity');
    if (resource !== `mesh-resource/${digest}`) {
      fail(`${path}.source.resource`, 'must be the content-addressed mesh-resource identity');
    }
    const byteLength = integer(
      source['byteLength'], `${path}.source.byteLength`, 16, 64 * 1024 * 1024,
    );
    const encoding = enumeration(source['encoding'], `${path}.source.encoding`,
      ['packedStreamsLeV1', 'packedStreamsLeV2'] as const);
    if (names.has('uv') !== Object.hasOwn(source, 'uvsByteOffset')
      || (encoding === 'packedStreamsLeV1' && Object.hasOwn(source, 'uvsByteOffset'))
      || (encoding === 'packedStreamsLeV2' && !Object.hasOwn(source, 'uvsByteOffset'))) {
      fail(`${path}.source`, 'mesh resource encoding and uv stream must agree');
    }
    const positionsByteOffset = integer(
      source['positionsByteOffset'], `${path}.source.positionsByteOffset`, 16, 4_294_967_295,
    );
    const normalsByteOffset = integer(
      source['normalsByteOffset'], `${path}.source.normalsByteOffset`, 16, 4_294_967_295,
    );
    const uvsByteOffset = Object.hasOwn(source, 'uvsByteOffset')
      ? integer(source['uvsByteOffset'], `${path}.source.uvsByteOffset`, 16, 4_294_967_295)
      : undefined;
    const indicesByteOffset = integer(
      source['indicesByteOffset'], `${path}.source.indicesByteOffset`, 16, 4_294_967_295,
    );
    for (const [name, offset] of [
      ['positionsByteOffset', positionsByteOffset],
      ['normalsByteOffset', normalsByteOffset],
      ...(uvsByteOffset === undefined ? [] : [['uvsByteOffset', uvsByteOffset] as const]),
      ['indicesByteOffset', indicesByteOffset],
    ] as const) {
      if (offset % 4 !== 0) fail(`${path}.source.${name}`, 'must be four-byte aligned');
    }
    const positionsEnd = positionsByteOffset + vertexCount * 3 * 4;
    const normalsEnd = normalsByteOffset + vertexCount * 3 * 4;
    const uvsEnd = uvsByteOffset === undefined ? normalsEnd : uvsByteOffset + vertexCount * 2 * 4;
    const indicesEnd = indicesByteOffset + indexCount * 4;
    if (positionsEnd > byteLength || normalsEnd > byteLength || uvsEnd > byteLength
      || indicesEnd > byteLength) {
      fail(`${path}.source`, 'declares a mesh stream outside the resource byte length');
    }
    if (positionsEnd > normalsByteOffset
      || (uvsByteOffset === undefined ? normalsEnd : uvsEnd) > indicesByteOffset
      || (uvsByteOffset !== undefined && normalsEnd > uvsByteOffset)) {
      fail(`${path}.source`, 'mesh resource streams must not overlap');
    }
  }
  const groups = list(value['groups'], `${path}.groups`);
  let cursor = 0;
  groups.forEach((item, index) => {
    const groupPath = `${path}.groups[${String(index)}]`;
    const group = record(item, groupPath, ['materialSlot', 'start', 'count']);
    integer(group['materialSlot'], `${groupPath}.materialSlot`, 0, 65_535);
    const start = nonNegativeInteger(group['start'], `${groupPath}.start`);
    const count = nonNegativeInteger(group['count'], `${groupPath}.count`);
    if (start !== cursor) fail(`${groupPath}.start`, `must tile from ${String(cursor)}`);
    cursor += count;
    if (cursor > indexCount) fail(groupPath, 'extends beyond index count');
  });
  if (cursor !== indexCount) fail(`${path}.groups`, 'must cover the complete index buffer');
}

function bounds(input: unknown, path: string): void {
  const value = record(input, path, ['min', 'max']);
  const min = vec3(value['min'], `${path}.min`);
  const max = vec3(value['max'], `${path}.max`);
  min.forEach((item, index) => {
    if (item > max[index]!) fail(path, 'minimum exceeds maximum');
  });
}

function materialSlot(input: unknown, path: string): number {
  const value = record(input, path, ['slot', 'material']);
  const slot = integer(value['slot'], `${path}.slot`, 0, 65_535);
  nonEmptyText(value['material'], `${path}.material`);
  return slot;
}

function materialSlots(input: unknown, path: string): Set<number> {
  const slots = new Set<number>();
  list(input, path).forEach((item, index) => {
    const slot = materialSlot(item, `${path}[${String(index)}]`);
    if (slots.has(slot)) fail(`${path}[${String(index)}].slot`, 'is duplicated');
    slots.add(slot);
  });
  return slots;
}

function staticMesh(input: unknown, path: string): void {
  const value = record(input, path, ['asset', 'payload', 'materialSlots', 'collision']);
  nonEmptyText(value['asset'], `${path}.asset`);
  meshPayload(value['payload'], `${path}.payload`);
  const slots = materialSlots(value['materialSlots'], `${path}.materialSlots`);
  const payload = looseRecord(value['payload'], `${path}.payload`);
  list(payload['groups'], `${path}.payload.groups`).forEach((item, index) => {
    const group = looseRecord(item, `${path}.payload.groups[${String(index)}]`);
    if (!slots.has(group['materialSlot'] as number)) {
      fail(`${path}.payload.groups[${String(index)}].materialSlot`, 'is not bound');
    }
  });
  const collisionBase = looseRecord(value['collision'], `${path}.collision`);
  const kind = enumeration(
    collisionBase['kind'],
    `${path}.collision.kind`,
    ['visualOnly', 'proxy', 'aabbFallback', 'trimesh'] as const,
  );
  if (kind === 'proxy') {
    const collision = record(value['collision'], `${path}.collision`, ['kind', 'proxyAsset']);
    nonEmptyText(collision['proxyAsset'], `${path}.collision.proxyAsset`);
  } else {
    record(value['collision'], `${path}.collision`, ['kind']);
  }
}

function staticMeshInstance(input: unknown, path: string): void {
  const value = record(input, path, ['asset', 'transform', 'visible', 'materialOverrides', 'metadata']);
  nonEmptyText(value['asset'], `${path}.asset`);
  transform(value['transform'], `${path}.transform`);
  booleanValue(value['visible'], `${path}.visible`);
  materialSlots(value['materialOverrides'], `${path}.materialOverrides`);
  metadata(value['metadata'], `${path}.metadata`);
}

function animatedMesh(input: unknown, path: string): void {
  const value = record(input, path, [
    'asset', 'runtimeFormat', 'contentHash', 'clips', 'defaultClip', 'materialSlots', 'bounds',
  ]);
  nonEmptyText(value['asset'], `${path}.asset`);
  enumeration(value['runtimeFormat'], `${path}.runtimeFormat`, ['glb'] as const);
  nullable(value['contentHash'], `${path}.contentHash`, nonEmptyText);
  const clips = new Set<string>();
  list(value['clips'], `${path}.clips`).forEach((item, index) => {
    const clipPath = `${path}.clips[${String(index)}]`;
    const clip = record(item, clipPath, ['id', 'name', 'durationSeconds']);
    const id = nonEmptyText(clip['id'], `${clipPath}.id`);
    if (clips.has(id)) fail(`${clipPath}.id`, 'is duplicated');
    clips.add(id);
    nullable(clip['name'], `${clipPath}.name`, nonEmptyText);
    nullable(clip['durationSeconds'], `${clipPath}.durationSeconds`, positiveFinite);
  });
  if (value['defaultClip'] !== null) {
    const defaultClip = nonEmptyText(value['defaultClip'], `${path}.defaultClip`);
    if (!clips.has(defaultClip)) fail(`${path}.defaultClip`, 'is not declared');
  }
  materialSlots(value['materialSlots'], `${path}.materialSlots`);
  bounds(value['bounds'], `${path}.bounds`);
}

function animatedMeshInstance(input: unknown, path: string): void {
  const value = record(input, path, [
    'asset', 'transform', 'visible', 'materialOverrides', 'playback', 'metadata',
  ]);
  nonEmptyText(value['asset'], `${path}.asset`);
  transform(value['transform'], `${path}.transform`);
  booleanValue(value['visible'], `${path}.visible`);
  materialSlots(value['materialOverrides'], `${path}.materialOverrides`);
  nullable(value['playback'], `${path}.playback`, playback);
  metadata(value['metadata'], `${path}.metadata`);
}

function voxelObject(input: unknown, path: string): void {
  const value = record(input, path, ['asset', 'contentHash', 'meshes', 'frames', 'materialSlots']);
  nonEmptyText(value['asset'], `${path}.asset`);
  nonEmptyText(value['contentHash'], `${path}.contentHash`);
  const slots = materialSlots(value['materialSlots'], `${path}.materialSlots`);
  const meshes = list(value['meshes'], `${path}.meshes`);
  if (meshes.length === 0 || meshes.length > 8_193) {
    fail(`${path}.meshes`, 'must contain 1..=8193 entries');
  }
  let totalVertices = 0;
  let totalIndices = 0;
  meshes.forEach((item, index) => {
    const meshPath = `${path}.meshes[${String(index)}]`;
    const mesh = record(item, meshPath, ['payload']);
    meshPayload(mesh['payload'], `${meshPath}.payload`);
    const payload = looseRecord(mesh['payload'], `${meshPath}.payload`);
    const layout = looseRecord(payload['layout'], `${meshPath}.payload.layout`);
    totalVertices += layout['vertexCount'] as number;
    totalIndices += layout['indexCount'] as number;
    list(payload['groups'], `${meshPath}.payload.groups`).forEach((groupItem, groupIndex) => {
      const group = looseRecord(groupItem, `${meshPath}.payload.groups[${String(groupIndex)}]`);
      if (!slots.has(group['materialSlot'] as number)) {
        fail(`${meshPath}.payload.groups[${String(groupIndex)}].materialSlot`, 'is not bound');
      }
    });
  });
  if (totalVertices > 8_000_000 || totalIndices > 12_000_000) {
    fail(`${path}.meshes`, 'exceeds aggregate vertex/index work limits');
  }
  const frames = list(value['frames'], `${path}.frames`);
  if (frames.length === 0 || frames.length > 8_193) {
    fail(`${path}.frames`, 'must contain 1..=8193 entries');
  }
  const ids = new Set<string>();
  frames.forEach((item, index) => {
    const framePath = `${path}.frames[${String(index)}]`;
    const frame = record(item, framePath, ['id', 'mesh']);
    const id = nonEmptyText(frame['id'], `${framePath}.id`);
    if (ids.has(id)) fail(`${framePath}.id`, 'is duplicated');
    ids.add(id);
    integer(frame['mesh'], `${framePath}.mesh`, 0, meshes.length - 1);
  });
}

function voxelObjectInstance(input: unknown, path: string): void {
  const value = record(input, path, [
    'asset', 'frame', 'transform', 'visible', 'materialOverrides', 'metadata',
  ]);
  nonEmptyText(value['asset'], `${path}.asset`);
  integer(value['frame'], `${path}.frame`, 0, 4_294_967_295);
  transform(value['transform'], `${path}.transform`);
  booleanValue(value['visible'], `${path}.visible`);
  materialSlots(value['materialOverrides'], `${path}.materialOverrides`);
  metadata(value['metadata'], `${path}.metadata`);
}

function playback(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['play', 'stop', 'pause', 'resume'] as const);
  if (kind === 'play') {
    const value = record(input, path, ['kind', 'clip', 'loop', 'speed', 'weight', 'restart', 'fadeSeconds']);
    nonEmptyText(value['clip'], `${path}.clip`);
    enumeration(value['loop'], `${path}.loop`, ['once', 'repeat', 'pingPong'] as const);
    positiveFinite(value['speed'], `${path}.speed`);
    range(value['weight'], `${path}.weight`, 0, 1);
    booleanValue(value['restart'], `${path}.restart`);
    nullable(value['fadeSeconds'], `${path}.fadeSeconds`, nonNegativeFinite);
  } else if (kind === 'stop') {
    const value = record(input, path, ['kind', 'fadeSeconds']);
    nullable(value['fadeSeconds'], `${path}.fadeSeconds`, nonNegativeFinite);
  } else {
    record(input, path, ['kind']);
  }
}

function renderMaterial(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'schemaVersion', 'id', 'color', 'texture', 'roughness', 'textureTint',
    'emissionColor', 'emissionIntensity', 'uvStrategy',
  ], ['voxelSurface']);
  integer(value['schemaVersion'], `${path}.schemaVersion`, 1, 4_294_967_295);
  nonEmptyText(value['id'], `${path}.id`);
  color4(value['color'], `${path}.color`);
  nullable(value['texture'], `${path}.texture`, nonEmptyText);
  range(value['roughness'], `${path}.roughness`, 0, 1);
  color4(value['textureTint'], `${path}.textureTint`);
  color3(value['emissionColor'], `${path}.emissionColor`);
  nonNegativeFinite(value['emissionIntensity'], `${path}.emissionIntensity`);
  enumeration(value['uvStrategy'], `${path}.uvStrategy`, ['flat', 'planar', 'atlas'] as const);
  if (Object.hasOwn(value, 'voxelSurface')) {
    voxelSurface(value['voxelSurface'], `${path}.voxelSurface`, value['texture']);
  }
}

function voxelSurface(input: unknown, path: string, materialTexture: unknown): void {
  const value = record(input, path, ['schemaVersion', 'filter', 'wrap', 'alphaMode', 'mapping']);
  integer(value['schemaVersion'], `${path}.schemaVersion`, 1, 1);
  const filter = enumeration(value['filter'], `${path}.filter`, ['nearest', 'linear'] as const);
  const wrap = enumeration(value['wrap'], `${path}.wrap`, ['clamp', 'repeat'] as const);
  const alpha = looseRecord(value['alphaMode'], `${path}.alphaMode`);
  const alphaKind = enumeration(alpha['kind'], `${path}.alphaMode.kind`, ['opaque', 'mask', 'blend'] as const);
  if (alphaKind === 'mask') {
    const mask = record(value['alphaMode'], `${path}.alphaMode`, ['kind', 'cutoff']);
    range(mask['cutoff'], `${path}.alphaMode.cutoff`, 0, 1);
  } else {
    record(value['alphaMode'], `${path}.alphaMode`, ['kind']);
  }

  const mapping = looseRecord(value['mapping'], `${path}.mapping`);
  const kind = enumeration(mapping['kind'], `${path}.mapping.kind`, ['repeat', 'atlas'] as const);
  const common = kind === 'repeat'
    ? record(value['mapping'], `${path}.mapping`, [
        'kind', 'texture', 'textureVersion', 'textureContentHash', 'tileScaleCells', 'tileOriginCells',
      ])
    : record(value['mapping'], `${path}.mapping`, [
        'kind', 'atlas', 'atlasVersion', 'atlasContentHash', 'texture', 'textureVersion',
        'textureContentHash', 'region', 'tileScaleCells', 'tileOriginCells',
      ]);
  const texture = nonEmptyText(common['texture'], `${path}.mapping.texture`);
  if (materialTexture !== texture) fail(`${path}.mapping.texture`, 'must match material texture');
  integer(common['textureVersion'], `${path}.mapping.textureVersion`, 1, 4_294_967_295);
  nonEmptyText(common['textureContentHash'], `${path}.mapping.textureContentHash`);
  boundedTilePair(common['tileScaleCells'], `${path}.mapping.tileScaleCells`, 1 / 256, 4_096);
  boundedTilePair(common['tileOriginCells'], `${path}.mapping.tileOriginCells`, -16_777_216, 16_777_216);
  if (kind === 'repeat') {
    if (wrap !== 'repeat') fail(`${path}.wrap`, 'repeat mapping requires repeat wrap');
    return;
  }
  if (wrap !== 'clamp') fail(`${path}.wrap`, 'atlas mapping requires clamp wrap');
  nonEmptyText(common['atlas'], `${path}.mapping.atlas`);
  integer(common['atlasVersion'], `${path}.mapping.atlasVersion`, 1, 4_294_967_295);
  nonEmptyText(common['atlasContentHash'], `${path}.mapping.atlasContentHash`);
  const region = record(common['region'], `${path}.mapping.region`, [
    'id', 'contentMin', 'contentExtent', 'padding', 'inset',
  ]);
  nonEmptyText(region['id'], `${path}.mapping.region.id`);
  integerPair(region['contentMin'], `${path}.mapping.region.contentMin`, 0, 4_294_967_295);
  integerPair(region['contentExtent'], `${path}.mapping.region.contentExtent`, 1, 4_294_967_295);
  enumeration(region['inset'], `${path}.mapping.region.inset`, ['halfTexel'] as const);
  const padding = record(region['padding'], `${path}.mapping.region.padding`, [
    'left', 'right', 'bottom', 'top',
  ]);
  for (const side of ['left', 'right', 'bottom', 'top'] as const) {
    const minimum = filter === 'linear' ? 1 : 0;
    integer(padding[side], `${path}.mapping.region.padding.${side}`, minimum, 32);
  }
}

function boundedTilePair(input: unknown, path: string, minimum: number, maximum: number): void {
  const values = tuple(input, path, 2);
  range(values[0], `${path}[0]`, minimum, maximum);
  range(values[1], `${path}[1]`, minimum, maximum);
}

function integerPair(input: unknown, path: string, minimum: number, maximum: number): void {
  const values = tuple(input, path, 2);
  integer(values[0], `${path}[0]`, minimum, maximum);
  integer(values[1], `${path}[1]`, minimum, maximum);
}

function materialParameters(input: unknown, path: string): void {
  const value = record(input, path, ['textureTint', 'emissionColor', 'emissionIntensity']);
  color4(value['textureTint'], `${path}.textureTint`);
  color3(value['emissionColor'], `${path}.emissionColor`);
  nonNegativeFinite(value['emissionIntensity'], `${path}.emissionIntensity`);
}

function texture(input: unknown, path: string): void {
  const value = recordOptional(input, path,
    ['id', 'width', 'height', 'filter', 'wrap', 'contentHash', 'version'], ['payload']);
  nonEmptyText(value['id'], `${path}.id`);
  const width = integer(value['width'], `${path}.width`, 1, 4_096);
  const height = integer(value['height'], `${path}.height`, 1, 4_096);
  if (width * height > 16_777_216) fail(path, 'texture texel quota exceeded');
  enumeration(value['filter'], `${path}.filter`, ['nearest', 'linear'] as const);
  enumeration(value['wrap'], `${path}.wrap`, ['clamp', 'repeat'] as const);
  nullable(value['contentHash'], `${path}.contentHash`, nonEmptyText);
  integer(value['version'], `${path}.version`, 1, 4_294_967_295);
  if (Object.hasOwn(value, 'payload')) {
    const payload = record(value['payload'], `${path}.payload`, [
      'encoding', 'colorSpace', 'contentHash', 'byteLength', 'source',
    ]);
    enumeration(payload['encoding'], `${path}.payload.encoding`, ['pngRgba8'] as const);
    enumeration(payload['colorSpace'], `${path}.payload.colorSpace`, ['srgb', 'linear'] as const);
    const contentHash = nonEmptyText(payload['contentHash'], `${path}.payload.contentHash`);
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(contentHash)?.[1];
    if (digest === undefined || value['contentHash'] !== contentHash) {
      fail(`${path}.payload.contentHash`, 'must be the canonical texture content hash');
    }
    const byteLength = integer(payload['byteLength'], `${path}.payload.byteLength`, 1, 16 * 1024 * 1024);
    const source = looseRecord(payload['source'], `${path}.payload.source`);
    const kind = enumeration(source['kind'], `${path}.payload.source.kind`, ['inline', 'resource'] as const);
    if (kind === 'inline') {
      const exact = record(source, `${path}.payload.source`, ['kind', 'encodedBytes']);
      const bytes = numberList(exact['encodedBytes'], `${path}.payload.source.encodedBytes`, byteLength, true);
      bytes.forEach((byte, index) => {
        if (byte > 255) fail(`${path}.payload.source.encodedBytes[${String(index)}]`, 'must be a byte');
      });
    } else {
      const exact = record(source, `${path}.payload.source`, ['kind', 'resource']);
      if (exact['resource'] !== `texture-resource/${digest}`) {
        fail(`${path}.payload.source.resource`, 'must match the content hash');
      }
    }
  }
}

function spriteAtlas(input: unknown, path: string): void {
  const value = record(input, path, ['id', 'texture', 'frames']);
  nonEmptyText(value['id'], `${path}.id`);
  nonEmptyText(value['texture'], `${path}.texture`);
  const frames = list(value['frames'], `${path}.frames`);
  if (frames.length === 0) fail(`${path}.frames`, 'must not be empty');
  const ids = new Set<number>();
  frames.forEach((item, index) => {
    const framePath = `${path}.frames[${String(index)}]`;
    const frame = recordOptional(item, framePath, ['frame', 'uvMin', 'uvMax'], ['size']);
    const id = nonNegativeInteger(frame['frame'], `${framePath}.frame`);
    if (ids.has(id)) fail(`${framePath}.frame`, 'is duplicated');
    ids.add(id);
    const min = rangedTuple(frame['uvMin'], `${framePath}.uvMin`, 2, 0, 1);
    const max = rangedTuple(frame['uvMax'], `${framePath}.uvMax`, 2, 0, 1);
    if (max[0]! <= min[0]! || max[1]! <= min[1]!) fail(framePath, 'UV rectangle is degenerate');
    if (frame['size'] !== undefined) {
      const size = tuple(frame['size'], `${framePath}.size`, 2);
      size.forEach((item, sizeIndex) => positiveFinite(item, `${framePath}.size[${String(sizeIndex)}]`));
    }
  });
}

function sprite(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'asset', 'frame', 'pivot', 'size', 'sizeMode', 'billboard', 'tint', 'renderOrder',
    'depth', 'shading', 'visible', 'transform', 'attachment', 'metadata',
  ], ['material']);
  nonEmptyText(value['asset'], `${path}.asset`);
  nonNegativeInteger(value['frame'], `${path}.frame`);
  rangedTuple(value['pivot'], `${path}.pivot`, 2, 0, 1);
  const size = tuple(value['size'], `${path}.size`, 2);
  size.forEach((item, index) => positiveFinite(item, `${path}.size[${String(index)}]`));
  enumeration(value['sizeMode'], `${path}.sizeMode`, ['world', 'pixel'] as const);
  enumeration(value['billboard'], `${path}.billboard`, ['none', 'spherical', 'cylindrical'] as const);
  color4(value['tint'], `${path}.tint`);
  integerValue(value['renderOrder'], `${path}.renderOrder`);
  enumeration(value['depth'], `${path}.depth`, ['default', 'depthTestOff', 'depthWriteOff'] as const);
  enumeration(value['shading'], `${path}.shading`, ['unlit', 'lit', 'shadowed', 'custom'] as const);
  if (value['material'] !== undefined) spriteMaterial(value['material'], `${path}.material`);
  booleanValue(value['visible'], `${path}.visible`);
  transform(value['transform'], `${path}.transform`);
  const attachment = record(value['attachment'], `${path}.attachment`, [
    'sourceEntity', 'sourceSceneNode', 'attachmentPoint',
  ]);
  nullable(attachment['sourceEntity'], `${path}.attachment.sourceEntity`, safeInteger);
  nullable(attachment['sourceSceneNode'], `${path}.attachment.sourceSceneNode`, safeInteger);
  nullable(attachment['attachmentPoint'], `${path}.attachment.attachmentPoint`, nonEmptyText);
  metadata(value['metadata'], `${path}.metadata`);
}

function spriteMaterial(input: unknown, path: string): void {
  const value = record(input, path, [
    'lighting', 'normalTexture', 'depthTexture', 'normalStrength', 'normalBias',
    'alpha', 'shadow',
  ]);
  const lighting = enumeration(value['lighting'], `${path}.lighting`, [
    'unlit', 'authoredNormal', 'authoredDepth', 'derivedGradient', 'synthetic',
  ] as const);
  nullable(value['normalTexture'], `${path}.normalTexture`, nonEmptyText);
  nullable(value['depthTexture'], `${path}.depthTexture`, nonEmptyText);
  range(value['normalStrength'], `${path}.normalStrength`, 0, 4);
  range(value['normalBias'], `${path}.normalBias`, -1, 1);
  const alpha = looseRecord(value['alpha'], `${path}.alpha`);
  const alphaKind = enumeration(alpha['kind'], `${path}.alpha.kind`, ['opaque', 'mask', 'blend'] as const);
  if (alphaKind === 'mask') {
    const mask = record(value['alpha'], `${path}.alpha`, ['kind', 'cutoff']);
    range(mask['cutoff'], `${path}.alpha.cutoff`, 0, 1);
  } else {
    record(value['alpha'], `${path}.alpha`, ['kind']);
  }
  enumeration(value['shadow'], `${path}.shadow`, ['none', 'cast', 'receive', 'castAndReceive'] as const);
  const normalPresent = value['normalTexture'] !== null;
  const depthPresent = value['depthTexture'] !== null;
  const referencesMatch = lighting === 'authoredNormal'
    ? normalPresent && !depthPresent
    : lighting === 'authoredDepth'
      ? !normalPresent && depthPresent
      : !normalPresent && !depthPresent;
  if (!referencesMatch) fail(path, 'texture references must match the selected lighting mode');
}

function light(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['ambient', 'directional', 'point', 'spot'] as const);
  const common = ['kind', 'color', 'intensity', 'enabled', 'shadowIntent'];
  const keys = kind === 'ambient'
    ? common
    : kind === 'directional'
      ? [...common, 'direction']
      : kind === 'point'
        ? [...common, 'position', 'range', 'decay']
        : [...common, 'position', 'direction', 'range', 'decay', 'outerAngleRadians', 'penumbra'];
  const value = record(input, path, keys);
  color3(value['color'], `${path}.color`);
  range(value['intensity'], `${path}.intensity`, 0, MAX_RENDER_LIGHT_INTENSITY);
  booleanValue(value['enabled'], `${path}.enabled`);
  enumeration(value['shadowIntent'], `${path}.shadowIntent`, ['disabled', 'requested'] as const);
  if (kind === 'directional' || kind === 'spot') direction(value['direction'], `${path}.direction`);
  if (kind === 'point' || kind === 'spot') {
    vec3(value['position'], `${path}.position`);
    nullable(value['range'], `${path}.range`, positiveFinite);
    nonNegativeFinite(value['decay'], `${path}.decay`);
  }
  if (kind === 'spot') {
    range(value['outerAngleRadians'], `${path}.outerAngleRadians`, Number.MIN_VALUE, Math.PI / 2);
    range(value['penumbra'], `${path}.penumbra`, 0, 1);
  }
}

function direction(input: unknown, path: string): void {
  const values = vec3(input, path);
  if (values.every((item) => item === 0)) fail(path, 'must be non-zero');
}

// Presentation validation intentionally checks its strict wire shape and safe
// identities. Rust owns the richer semantic validation before producing a frame.
function presentationOperation(domain: string, input: unknown, path: string): void {
  const value = looseRecord(input, path);
  const op = text(value['op'], `${path}.op`);
  if (domain === 'audio') return audioOperation(op, input, path);
  if (domain === 'billboard') return billboardOperation(op, input, path);
  if (domain === 'particle') return particleOperation(op, input, path);
  if (domain === 'telemetryOverlay') return telemetryOperation(op, input, path);
  animationOperation(op, input, path);
}

function audioOperation(op: string, input: unknown, path: string): void {
  if (op === 'emit') {
    const value = record(input, path, ['op', 'signalId', 'descriptor']);
    nonEmptyText(value['signalId'], `${path}.signalId`);
    audioDescriptor(value['descriptor'], `${path}.descriptor`);
  } else if (op === 'create') {
    const value = record(input, path, ['op', 'handle', 'descriptor']);
    handle(value['handle'], `${path}.handle`);
    audioDescriptor(value['descriptor'], `${path}.descriptor`);
  } else if (op === 'update') {
    const value = record(input, path, ['op', 'handle', 'patch']);
    handle(value['handle'], `${path}.handle`);
    const patch = record(value['patch'], `${path}.patch`, [
      'volume', 'pitch', 'looping', 'spatialBlend', 'attenuation', 'pan', 'emitter',
    ]);
    nullable(patch['volume'], `${path}.patch.volume`, (item, itemPath) => range(item, itemPath, 0, 1));
    nullable(patch['pitch'], `${path}.patch.pitch`, (item, itemPath) => range(item, itemPath, 0.25, 4));
    nullable(patch['looping'], `${path}.patch.looping`, booleanValue);
    nullable(patch['spatialBlend'], `${path}.patch.spatialBlend`, (item, itemPath) => range(item, itemPath, 0, 1));
    nullable(patch['attenuation'], `${path}.patch.attenuation`, positiveFinite);
    nullable(patch['pan'], `${path}.patch.pan`, (item, itemPath) => range(item, itemPath, -1, 1));
    nullable(patch['emitter'], `${path}.patch.emitter`, (item, itemPath) => anchored(item, itemPath, true));
  } else if (op === 'destroy') {
    const value = record(input, path, ['op', 'handle']);
    handle(value['handle'], `${path}.handle`);
  } else fail(`${path}.op`, 'is unsupported for audio');
}

function audioDescriptor(input: unknown, path: string): void {
  const value = record(input, path, [
    'clip', 'bus', 'volume', 'pitch', 'looping', 'spatialBlend', 'attenuation', 'pan', 'emitter',
  ]);
  const clip = record(value['clip'], `${path}.clip`, ['asset', 'contentHash']);
  nonEmptyText(clip['asset'], `${path}.clip.asset`);
  nonEmptyText(clip['contentHash'], `${path}.clip.contentHash`);
  enumeration(value['bus'], `${path}.bus`, ['sfx', 'ambient', 'ui'] as const);
  range(value['volume'], `${path}.volume`, 0, 1);
  range(value['pitch'], `${path}.pitch`, 0.25, 4);
  range(value['spatialBlend'], `${path}.spatialBlend`, 0, 1);
  positiveFinite(value['attenuation'], `${path}.attenuation`);
  range(value['pan'], `${path}.pan`, -1, 1);
  booleanValue(value['looping'], `${path}.looping`);
  anchored(value['emitter'], `${path}.emitter`, true);
}

function billboardOperation(op: string, input: unknown, path: string): void {
  retainedPresentationOperation(op, input, path, billboardDescriptor, billboardPatch);
}

function billboardDescriptor(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'anchor', 'content', 'font', 'heightPixels', 'color', 'background', 'maxDistance', 'layer', 'visible',
  ], ['layout']);
  anchored(value['anchor'], `${path}.anchor`, false);
  billboardContent(value['content'], `${path}.content`);
  billboardFont(value['font'], `${path}.font`);
  range(value['heightPixels'], `${path}.heightPixels`, 8, 256);
  color4(value['color'], `${path}.color`);
  color4(value['background'], `${path}.background`);
  range(value['maxDistance'], `${path}.maxDistance`, 1.1920928955078125e-7, 10_000);
  enumeration(value['layer'], `${path}.layer`, ['alwaysOnTop', 'depthTested', 'occluded'] as const);
  booleanValue(value['visible'], `${path}.visible`);
  if (value['layout'] !== undefined) {
    billboardLayoutPolicy(value['layout'], `${path}.layout`);
  }
  const content = looseRecord(value['content'], `${path}.content`);
  if (content['kind'] === 'structured') {
    if (value['layout'] === undefined) {
      fail(`${path}.layout`, 'is required for structured content');
    }
  } else if (value['layout'] !== undefined) {
    fail(`${path}.layout`, 'is only valid for structured content');
  }
}

function billboardPatch(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'anchor', 'content', 'font', 'heightPixels', 'color', 'background', 'maxDistance', 'layer', 'visible',
  ], ['layout']);
  nullable(value['anchor'], `${path}.anchor`, (item, itemPath) => anchored(item, itemPath, false));
  nullable(value['content'], `${path}.content`, billboardContent);
  nullable(value['font'], `${path}.font`, billboardFont);
  nullable(value['heightPixels'], `${path}.heightPixels`, (item, itemPath) => range(item, itemPath, 8, 256));
  nullable(value['color'], `${path}.color`, color4);
  nullable(value['background'], `${path}.background`, color4);
  nullable(value['maxDistance'], `${path}.maxDistance`, (item, itemPath) => range(item, itemPath, 1.1920928955078125e-7, 10_000));
  nullable(value['layer'], `${path}.layer`, (item, itemPath) => enumeration(item, itemPath, ['alwaysOnTop', 'depthTested', 'occluded'] as const));
  nullable(value['visible'], `${path}.visible`, booleanValue);
  if (value['layout'] !== undefined) {
    billboardLayoutPolicy(value['layout'], `${path}.layout`);
  }
}

function billboardContent(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['text', 'value', 'icon', 'structured'] as const);
  if (kind === 'text') {
    const value = record(input, path, ['kind', 'localizationKey', 'fallbackText', 'arguments']);
    billboardKey(value['localizationKey'], `${path}.localizationKey`);
    billboardText(value['fallbackText'], `${path}.fallbackText`);
    const names = new Set<string>();
    const argumentsList = list(value['arguments'], `${path}.arguments`);
    if (argumentsList.length > 8) fail(`${path}.arguments`, 'must contain at most 8 entries');
    argumentsList.forEach((item, index) => {
      const argumentPath = `${path}.arguments[${String(index)}]`;
      const argument = record(item, argumentPath, ['name', 'value']);
      const name = billboardKey(argument['name'], `${argumentPath}.name`);
      billboardText(argument['value'], `${argumentPath}.value`);
      if (names.has(name)) fail(`${argumentPath}.name`, 'is duplicated');
      names.add(name);
    });
  } else if (kind === 'value') {
    const value = record(input, path, [
      'kind', 'labelKey', 'fallbackLabel', 'value', 'unitKey', 'fallbackUnit',
    ]);
    billboardKey(value['labelKey'], `${path}.labelKey`);
    billboardText(value['fallbackLabel'], `${path}.fallbackLabel`);
    billboardText(value['value'], `${path}.value`);
    nullable(value['unitKey'], `${path}.unitKey`, billboardKey);
    nullable(value['fallbackUnit'], `${path}.fallbackUnit`, billboardText);
  } else if (kind === 'icon') {
    const value = record(input, path, ['kind', 'texture', 'altKey', 'fallbackAlt']);
    billboardTexture(value['texture'], `${path}.texture`);
    billboardKey(value['altKey'], `${path}.altKey`);
    billboardText(value['fallbackAlt'], `${path}.fallbackAlt`);
  } else {
    const value = record(input, path, ['kind', 'indicator']);
    billboardIndicator(value['indicator'], `${path}.indicator`);
  }
}

function billboardFont(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['system', 'asset'] as const);
  if (kind === 'system') {
    const value = record(input, path, ['kind', 'family']);
    billboardText(value['family'], `${path}.family`);
  } else {
    const value = record(input, path, ['kind', 'asset', 'contentHash', 'family']);
    billboardKey(value['asset'], `${path}.asset`);
    billboardText(value['contentHash'], `${path}.contentHash`);
    billboardText(value['family'], `${path}.family`);
  }
}

function billboardLocalizedText(input: unknown, path: string): void {
  const value = record(input, path, ['localizationKey', 'fallbackText']);
  billboardKey(value['localizationKey'], `${path}.localizationKey`);
  billboardText(value['fallbackText'], `${path}.fallbackText`);
}

function billboardMeter(input: unknown, path: string): void {
  const value = record(input, path, [
    'id', 'accessibleLabel', 'current', 'min', 'max', 'preview', 'fillDirection', 'segments',
    'fill', 'previewFill', 'back', 'border',
  ]);
  billboardKey(value['id'], `${path}.id`);
  billboardLocalizedText(value['accessibleLabel'], `${path}.accessibleLabel`);
  const current = finite(value['current'], `${path}.current`);
  const minimum = finite(value['min'], `${path}.min`);
  const maximum = finite(value['max'], `${path}.max`);
  const maxMagnitude = 1_000_000_000_000;
  if (Math.abs(current) > maxMagnitude) fail(`${path}.current`, 'exceeds the meter magnitude bound');
  if (Math.abs(minimum) > maxMagnitude) fail(`${path}.min`, 'exceeds the meter magnitude bound');
  if (Math.abs(maximum) > maxMagnitude) fail(`${path}.max`, 'exceeds the meter magnitude bound');
  if (minimum >= maximum) fail(`${path}.min`, 'must be less than max');
  if (current < minimum || current > maximum) fail(`${path}.current`, 'must be inside min..=max');
  nullable(value['preview'], `${path}.preview`, (item, itemPath) => {
    const preview = finite(item, itemPath);
    if (Math.abs(preview) > maxMagnitude) fail(itemPath, 'exceeds the meter magnitude bound');
    if (preview < minimum || preview > maximum) fail(itemPath, 'must be inside min..=max');
  });
  enumeration(
    value['fillDirection'],
    `${path}.fillDirection`,
    ['leftToRight', 'rightToLeft', 'bottomToTop', 'topToBottom'] as const,
  );
  integer(value['segments'], `${path}.segments`, 1, 32);
  color4(value['fill'], `${path}.fill`);
  color4(value['previewFill'], `${path}.previewFill`);
  color4(value['back'], `${path}.back`);
  color4(value['border'], `${path}.border`);
}

function billboardStatusCue(input: unknown, path: string): void {
  const value = record(input, path, ['id', 'label', 'icon']);
  billboardKey(value['id'], `${path}.id`);
  billboardLocalizedText(value['label'], `${path}.label`);
  nullable(value['icon'], `${path}.icon`, billboardTexture);
}

function billboardStyle(input: unknown, path: string): void {
  const value = record(input, path, ['opacity', 'backing', 'border', 'radiusPixels']);
  range(value['opacity'], `${path}.opacity`, 0, 1);
  color4(value['backing'], `${path}.backing`);
  color4(value['border'], `${path}.border`);
  range(value['radiusPixels'], `${path}.radiusPixels`, 0, 128);
}

function billboardIndicator(input: unknown, path: string): void {
  const value = record(input, path, [
    'label', 'icon', 'accessibleLabel', 'meters', 'statusCues', 'widthPixels', 'spacingPixels',
    'alignment', 'style',
  ]);
  nullable(value['label'], `${path}.label`, billboardLocalizedText);
  nullable(value['icon'], `${path}.icon`, billboardTexture);
  billboardLocalizedText(value['accessibleLabel'], `${path}.accessibleLabel`);
  const meters = list(value['meters'], `${path}.meters`);
  if (meters.length > 4) fail(`${path}.meters`, 'must contain at most 4 entries');
  const meterIds = new Set<string>();
  meters.forEach((item, index) => {
    const meterPath = `${path}.meters[${String(index)}]`;
    billboardMeter(item, meterPath);
    const id = billboardRecordId(item, meterPath);
    if (meterIds.has(id)) fail(`${meterPath}.id`, 'is duplicated');
    meterIds.add(id);
  });
  const statusCues = list(value['statusCues'], `${path}.statusCues`);
  if (statusCues.length > 8) fail(`${path}.statusCues`, 'must contain at most 8 entries');
  const statusIds = new Set<string>();
  statusCues.forEach((item, index) => {
    const cuePath = `${path}.statusCues[${String(index)}]`;
    billboardStatusCue(item, cuePath);
    const id = billboardRecordId(item, cuePath);
    if (statusIds.has(id)) fail(`${cuePath}.id`, 'is duplicated');
    statusIds.add(id);
  });
  range(value['widthPixels'], `${path}.widthPixels`, 1, 2_048);
  range(value['spacingPixels'], `${path}.spacingPixels`, 0, 128);
  enumeration(value['alignment'], `${path}.alignment`, ['start', 'center', 'end'] as const);
  billboardStyle(value['style'], `${path}.style`);
}

function billboardLayoutPolicy(input: unknown, path: string): void {
  const value = record(input, path, ['priority', 'sizing', 'safeArea', 'edgeBehavior', 'overlapBehavior']);
  integer(value['priority'], `${path}.priority`, -2_147_483_648, 2_147_483_647);
  billboardLayoutSizing(value['sizing'], `${path}.sizing`);
  const safeArea = record(value['safeArea'], `${path}.safeArea`, [
    'topPixels', 'rightPixels', 'bottomPixels', 'leftPixels',
  ]);
  range(safeArea['topPixels'], `${path}.safeArea.topPixels`, 0, 4_096);
  range(safeArea['rightPixels'], `${path}.safeArea.rightPixels`, 0, 4_096);
  range(safeArea['bottomPixels'], `${path}.safeArea.bottomPixels`, 0, 4_096);
  range(safeArea['leftPixels'], `${path}.safeArea.leftPixels`, 0, 4_096);
  enumeration(value['edgeBehavior'], `${path}.edgeBehavior`, ['clamp', 'cull'] as const);
  enumeration(value['overlapBehavior'], `${path}.overlapBehavior`, ['stack', 'suppress'] as const);
}

function billboardLayoutSizing(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['constantPixels', 'distanceScaled'] as const);
  if (kind === 'constantPixels') {
    record(input, path, ['kind']);
    return;
  }
  const value = record(input, path, ['kind', 'referenceDistance', 'minScale', 'maxScale']);
  range(value['referenceDistance'], `${path}.referenceDistance`, 1.1920928955078125e-7, 10_000);
  range(value['minScale'], `${path}.minScale`, 1.1920928955078125e-7, 16);
  const maximum = range(value['maxScale'], `${path}.maxScale`, 1.1920928955078125e-7, 16);
  if ((value['minScale'] as number) > maximum) fail(`${path}.minScale`, 'must be less than or equal to maxScale');
}

function billboardTexture(input: unknown, path: string): void {
  const value = record(input, path, ['asset', 'contentHash']);
  billboardKey(value['asset'], `${path}.asset`);
  billboardText(value['contentHash'], `${path}.contentHash`);
}

function billboardRecordId(input: unknown, path: string): string {
  return billboardKey(looseRecord(input, path)['id'], `${path}.id`);
}

function billboardKey(input: unknown, path: string): string {
  return boundedBillboardText(input, path, 128);
}

function billboardText(input: unknown, path: string): string {
  return boundedBillboardText(input, path, 256);
}

function boundedBillboardText(input: unknown, path: string, maximumBytes: number): string {
  const value = nonEmptyText(input, path);
  if (new TextEncoder().encode(value).byteLength > maximumBytes) {
    fail(path, `must contain at most ${String(maximumBytes)} UTF-8 bytes`);
  }
  return value;
}

function particleOperation(op: string, input: unknown, path: string): void {
  if (op === 'emit') {
    const value = record(input, path, ['op', 'signalId', 'descriptor']);
    nonEmptyText(value['signalId'], `${path}.signalId`);
    particleDescriptor(value['descriptor'], `${path}.descriptor`);
  } else {
    retainedPresentationOperation(op, input, path, particleDescriptor, particlePatch);
  }
}

function particleDescriptor(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'anchor', 'ratePerSecond', 'burstCount', 'lifetimeSeconds', 'velocityMin',
    'velocityMax', 'acceleration', 'sizeCurve', 'colorCurve', 'flipbookFramesPerSecond',
    'seed', 'maxParticles', 'visible',
  ], ['visual', 'sprite', 'collision']);
  anchored(value['anchor'], `${path}.anchor`, false);
  const visual = value['visual'];
  const sprite = value['sprite'];
  if ((visual === undefined) === (sprite === undefined)) {
    fail(path, 'must contain exactly one of visual or legacy sprite');
  }
  if (visual !== undefined) particleVisual(visual, `${path}.visual`);
  if (sprite !== undefined) particleSprite(sprite, `${path}.sprite`);
  range(value['ratePerSecond'], `${path}.ratePerSecond`, 0, 10_000);
  range(value['flipbookFramesPerSecond'], `${path}.flipbookFramesPerSecond`, 0, 120);
  nonNegativeInteger(value['burstCount'], `${path}.burstCount`);
  rangedTuple(value['lifetimeSeconds'], `${path}.lifetimeSeconds`, 2, 0, Number.MAX_VALUE);
  vec3(value['velocityMin'], `${path}.velocityMin`);
  vec3(value['velocityMax'], `${path}.velocityMax`);
  vec3(value['acceleration'], `${path}.acceleration`);
  particleScalarCurve(value['sizeCurve'], `${path}.sizeCurve`);
  particleColorCurve(value['colorCurve'], `${path}.colorCurve`);
  safeInteger(value['seed'], `${path}.seed`);
  nonNegativeInteger(value['maxParticles'], `${path}.maxParticles`);
  booleanValue(value['visible'], `${path}.visible`);
  if (value['collision'] !== undefined) {
    particleCollision(value['collision'], `${path}.collision`);
  }
}

function particlePatch(input: unknown, path: string): void {
  const value = recordOptional(input, path, [
    'anchor', 'sprite', 'ratePerSecond', 'burstCount', 'lifetimeSeconds', 'velocityMin',
    'velocityMax', 'acceleration', 'sizeCurve', 'colorCurve', 'flipbookFramesPerSecond',
    'maxParticles', 'visible',
  ], ['visual', 'collision']);
  nullable(value['anchor'], `${path}.anchor`, (item, itemPath) => anchored(item, itemPath, false));
  if (value['visual'] !== undefined) {
    nullable(value['visual'], `${path}.visual`, particleVisual);
  }
  nullable(value['sprite'], `${path}.sprite`, particleSprite);
  nullable(value['ratePerSecond'], `${path}.ratePerSecond`, nonNegativeFinite);
  nullable(value['burstCount'], `${path}.burstCount`, nonNegativeInteger);
  nullable(value['lifetimeSeconds'], `${path}.lifetimeSeconds`, (item, itemPath) => rangedTuple(item, itemPath, 2, 0, 60));
  nullable(value['velocityMin'], `${path}.velocityMin`, vec3);
  nullable(value['velocityMax'], `${path}.velocityMax`, vec3);
  nullable(value['acceleration'], `${path}.acceleration`, vec3);
  nullable(value['sizeCurve'], `${path}.sizeCurve`, particleScalarCurve);
  nullable(value['colorCurve'], `${path}.colorCurve`, particleColorCurve);
  nullable(value['flipbookFramesPerSecond'], `${path}.flipbookFramesPerSecond`, (item, itemPath) => range(item, itemPath, 0, 120));
  nullable(value['maxParticles'], `${path}.maxParticles`, nonNegativeInteger);
  nullable(value['visible'], `${path}.visible`, booleanValue);
  if (value['collision'] !== undefined) {
    nullable(value['collision'], `${path}.collision`, particleCollision);
  }
}

function particleVisual(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['billboard', 'cube'] as const);
  if (kind === 'billboard') {
    const value = record(input, path, ['kind', 'sprite']);
    particleSprite(value['sprite'], `${path}.sprite`);
  } else {
    record(input, path, ['kind']);
  }
}

function particleCollision(input: unknown, path: string): void {
  const value = record(input, path, [
    'radius', 'restitution', 'friction', 'maximumImpacts', 'sleepSpeed',
    'limitBehavior', 'volumes',
  ]);
  range(value['radius'], `${path}.radius`, 0, 2);
  range(value['restitution'], `${path}.restitution`, 0, 1);
  range(value['friction'], `${path}.friction`, 0, 1);
  integer(value['maximumImpacts'], `${path}.maximumImpacts`, 1, 32);
  range(value['sleepSpeed'], `${path}.sleepSpeed`, 0, 100);
  enumeration(value['limitBehavior'], `${path}.limitBehavior`, ['sleep', 'kill'] as const);
  const volumes = list(value['volumes'], `${path}.volumes`);
  if (volumes.length < 1 || volumes.length > 16) {
    fail(`${path}.volumes`, 'must contain 1..=16 volumes');
  }
  volumes.forEach((volume, index) => {
    particleCollisionVolume(volume, `${path}.volumes[${String(index)}]`);
  });
}

function particleCollisionVolume(input: unknown, path: string): void {
  const base = looseRecord(input, path);
  const kind = enumeration(base['kind'], `${path}.kind`, ['plane', 'aabb'] as const);
  if (kind === 'plane') {
    const value = record(input, path, ['kind', 'normal', 'offset']);
    const normal = vec3(value['normal'], `${path}.normal`);
    finite(value['offset'], `${path}.offset`);
    const lengthSquared = normal.reduce((sum, component) => sum + component * component, 0);
    if (lengthSquared < 0.999 || lengthSquared > 1.001) {
      fail(`${path}.normal`, 'must be normalized');
    }
    return;
  }
  const value = record(input, path, ['kind', 'minimum', 'maximum']);
  const minimum = vec3(value['minimum'], `${path}.minimum`);
  const maximum = vec3(value['maximum'], `${path}.maximum`);
  minimum.forEach((low, index) => {
    if (low >= maximum[index]!) fail(`${path}.maximum[${String(index)}]`, 'must exceed minimum');
  });
}

function particleSprite(input: unknown, path: string): void {
  const value = record(input, path, ['asset', 'contentHash', 'frameCount']);
  nonEmptyText(value['asset'], `${path}.asset`);
  nonEmptyText(value['contentHash'], `${path}.contentHash`);
  integer(value['frameCount'], `${path}.frameCount`, 1, 65_535);
}

function particleScalarCurve(input: unknown, path: string): void {
  const keys = list(input, path);
  if (keys.length < 2 || keys.length > 8) fail(path, 'must contain 2 to 8 keys');
  let previous = -1;
  keys.forEach((item, index) => {
    const keyPath = `${path}[${String(index)}]`;
    const key = record(item, keyPath, ['age', 'value']);
    const age = range(key['age'], `${keyPath}.age`, 0, 1);
    nonNegativeFinite(key['value'], `${keyPath}.value`);
    if (age <= previous) fail(`${keyPath}.age`, 'must be strictly increasing');
    previous = age;
  });
  if ((looseRecord(keys[0], `${path}[0]`)['age']) !== 0
    || (looseRecord(keys[keys.length - 1], `${path}[${String(keys.length - 1)}]`)['age']) !== 1) {
    fail(path, 'must start at age 0 and end at age 1');
  }
}

function particleColorCurve(input: unknown, path: string): void {
  const keys = list(input, path);
  if (keys.length < 2 || keys.length > 8) fail(path, 'must contain 2 to 8 keys');
  let previous = -1;
  keys.forEach((item, index) => {
    const keyPath = `${path}[${String(index)}]`;
    const key = record(item, keyPath, ['age', 'color']);
    const age = range(key['age'], `${keyPath}.age`, 0, 1);
    color4(key['color'], `${keyPath}.color`);
    if (age <= previous) fail(`${keyPath}.age`, 'must be strictly increasing');
    previous = age;
  });
  if ((looseRecord(keys[0], `${path}[0]`)['age']) !== 0
    || (looseRecord(keys[keys.length - 1], `${path}[${String(keys.length - 1)}]`)['age']) !== 1) {
    fail(path, 'must start at age 0 and end at age 1');
  }
}

function telemetryOperation(op: string, input: unknown, path: string): void {
  retainedPresentationOperation(op, input, path, telemetryDescriptor, telemetryPatch);
}

function telemetryDescriptor(input: unknown, path: string): void {
  const value = record(input, path, ['title', 'corner', 'refreshIntervalMs', 'maxFrameTimeSamples', 'visible']);
  nonEmptyText(value['title'], `${path}.title`);
  enumeration(value['corner'], `${path}.corner`, ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'] as const);
  integer(value['refreshIntervalMs'], `${path}.refreshIntervalMs`, 100, 5_000);
  integer(value['maxFrameTimeSamples'], `${path}.maxFrameTimeSamples`, 1, 240);
  booleanValue(value['visible'], `${path}.visible`);
}

function telemetryPatch(input: unknown, path: string): void {
  const value = record(input, path, ['title', 'corner', 'refreshIntervalMs', 'maxFrameTimeSamples', 'visible']);
  nullable(value['title'], `${path}.title`, nonEmptyText);
  nullable(value['corner'], `${path}.corner`, (item, itemPath) => enumeration(item, itemPath, ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'] as const));
  nullable(value['refreshIntervalMs'], `${path}.refreshIntervalMs`, (item, itemPath) => integer(item, itemPath, 100, 5_000));
  nullable(value['maxFrameTimeSamples'], `${path}.maxFrameTimeSamples`, (item, itemPath) => integer(item, itemPath, 1, 240));
  nullable(value['visible'], `${path}.visible`, booleanValue);
}

function animationOperation(op: string, input: unknown, path: string): void {
  if (op === 'create') {
    const value = record(input, path, ['op', 'handle', 'descriptor']);
    handle(value['handle'], `${path}.handle`);
    const descriptor = record(value['descriptor'], `${path}.descriptor`, [
      'target', 'asset', 'contentHash', 'tickDurationMillis', 'controller',
    ]);
    handle(descriptor['target'], `${path}.descriptor.target`);
    nonEmptyText(descriptor['asset'], `${path}.descriptor.asset`);
    nonEmptyText(descriptor['contentHash'], `${path}.descriptor.contentHash`);
    nonNegativeInteger(descriptor['tickDurationMillis'], `${path}.descriptor.tickDurationMillis`);
    animationController(descriptor['controller'], `${path}.descriptor.controller`);
  } else if (op === 'update') {
    const value = record(input, path, ['op', 'handle', 'controller']);
    handle(value['handle'], `${path}.handle`);
    animationController(value['controller'], `${path}.controller`);
  } else if (op === 'destroy') {
    const value = record(input, path, ['op', 'handle']);
    handle(value['handle'], `${path}.handle`);
  } else fail(`${path}.op`, 'is unsupported for animation');
}

function animationController(input: unknown, path: string): void {
  const value = record(input, path, [
    'entity', 'graphId', 'graphVersion', 'stateId', 'revision', 'controllerTick',
    'motion', 'transition', 'transitionFact',
  ]);
  safeInteger(value['entity'], `${path}.entity`);
  nonEmptyText(value['graphId'], `${path}.graphId`);
  nonNegativeInteger(value['graphVersion'], `${path}.graphVersion`);
  nonEmptyText(value['stateId'], `${path}.stateId`);
  safeInteger(value['revision'], `${path}.revision`);
  safeInteger(value['controllerTick'], `${path}.controllerTick`);
  animationMotion(value['motion'], `${path}.motion`);
  nullable(value['transition'], `${path}.transition`, (candidate, candidatePath) => {
    const transition = record(candidate, candidatePath, [
      'transitionId', 'fromStateId', 'toStateId', 'elapsedTicks', 'durationTicks', 'targetMotion',
    ]);
    nonEmptyText(transition['transitionId'], `${candidatePath}.transitionId`);
    nonEmptyText(transition['fromStateId'], `${candidatePath}.fromStateId`);
    nonEmptyText(transition['toStateId'], `${candidatePath}.toStateId`);
    nonNegativeInteger(transition['elapsedTicks'], `${candidatePath}.elapsedTicks`);
    nonNegativeInteger(transition['durationTicks'], `${candidatePath}.durationTicks`);
    animationMotion(transition['targetMotion'], `${candidatePath}.targetMotion`);
  });
  nullable(value['transitionFact'], `${path}.transitionFact`, (candidate, candidatePath) => {
    const fact = record(candidate, candidatePath, [
      'controllerTick', 'transitionId', 'fromStateId', 'toStateId', 'moment', 'durationTicks',
    ]);
    safeInteger(fact['controllerTick'], `${candidatePath}.controllerTick`);
    nonEmptyText(fact['transitionId'], `${candidatePath}.transitionId`);
    nonEmptyText(fact['fromStateId'], `${candidatePath}.fromStateId`);
    nonEmptyText(fact['toStateId'], `${candidatePath}.toStateId`);
    enumeration(fact['moment'], `${candidatePath}.moment`, ['started', 'completed'] as const);
    nonNegativeInteger(fact['durationTicks'], `${candidatePath}.durationTicks`);
  });
}

function animationMotion(input: unknown, path: string): void {
  const value = record(input, path, ['clipA', 'clipB', 'blendWeightMilli', 'speedMilli']);
  nonEmptyText(value['clipA'], `${path}.clipA`);
  nullable(value['clipB'], `${path}.clipB`, nonEmptyText);
  integerValue(value['blendWeightMilli'], `${path}.blendWeightMilli`);
  integerValue(value['speedMilli'], `${path}.speedMilli`);
}

function retainedPresentationOperation(
  op: string,
  input: unknown,
  path: string,
  descriptor: (value: unknown, valuePath: string) => void,
  patch: (value: unknown, valuePath: string) => void,
): void {
  if (op === 'create') {
    const value = record(input, path, ['op', 'handle', 'descriptor']);
    handle(value['handle'], `${path}.handle`);
    descriptor(value['descriptor'], `${path}.descriptor`);
  } else if (op === 'update') {
    const value = record(input, path, ['op', 'handle', 'patch']);
    handle(value['handle'], `${path}.handle`);
    patch(value['patch'], `${path}.patch`);
  } else if (op === 'destroy') {
    const value = record(input, path, ['op', 'handle']);
    handle(value['handle'], `${path}.handle`);
  } else fail(`${path}.op`, 'is unsupported for retained presentation');
}

function anchored(input: unknown, path: string, allowGlobal: boolean): void {
  const base = looseRecord(input, path);
  const kinds = allowGlobal ? ['global2d', 'world3d', 'entityAttached'] as const : ['world', 'entityAttached'] as const;
  const kind = enumeration(base['kind'], `${path}.kind`, kinds);
  if (kind === 'global2d') record(input, path, ['kind']);
  else if (kind === 'world' || kind === 'world3d') {
    const value = record(input, path, ['kind', 'position']);
    vec3(value['position'], `${path}.position`);
  } else {
    const value = record(input, path, ['kind', 'entity', 'offset']);
    safeInteger(value['entity'], `${path}.entity`);
    vec3(value['offset'], `${path}.offset`);
  }
}

function record(input: unknown, path: string, keys: readonly string[]): Record<string, unknown> {
  const value = looseRecord(input, path);
  const expected = new Set(keys);
  Object.keys(value).forEach((key) => {
    if (!expected.has(key)) fail(`${path}.${key}`, 'is unknown');
  });
  keys.forEach((key) => {
    if (!Object.hasOwn(value, key)) fail(`${path}.${key}`, 'is required');
  });
  return value;
}

function recordOptional(
  input: unknown,
  path: string,
  required: readonly string[],
  optional: readonly string[],
): Record<string, unknown> {
  const value = looseRecord(input, path);
  const expected = new Set([...required, ...optional]);
  Object.keys(value).forEach((key) => {
    if (!expected.has(key)) fail(`${path}.${key}`, 'is unknown');
  });
  required.forEach((key) => {
    if (!Object.hasOwn(value, key)) fail(`${path}.${key}`, 'is required');
  });
  return value;
}

function looseRecord(input: unknown, path: string): Record<string, unknown> {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  return input as Record<string, unknown>;
}

function list(input: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(input)) fail(path, 'must be an array');
  return input;
}

function tuple(input: unknown, path: string, length: number): readonly unknown[] {
  const values = list(input, path);
  if (values.length !== length) fail(path, `must contain ${String(length)} values`);
  return values;
}

function vec3(input: unknown, path: string): readonly number[] {
  const values = tuple(input, path, 3);
  return values.map((item, index) => finite(item, `${path}[${String(index)}]`));
}

function color3(input: unknown, path: string): void {
  rangedTuple(input, path, 3, 0, 1);
}

function color4(input: unknown, path: string): void {
  rangedTuple(input, path, 4, 0, 1);
}

function rangedTuple(input: unknown, path: string, length: number, min: number, max: number): readonly number[] {
  return tuple(input, path, length).map((item, index) => range(item, `${path}[${String(index)}]`, min, max));
}

function numberList(input: unknown, path: string, length: number, integers: boolean): readonly number[] {
  const values = list(input, path);
  if (values.length !== length) fail(path, `must contain ${String(length)} values`);
  return values.map((item, index) => integers
    ? nonNegativeInteger(item, `${path}[${String(index)}]`)
    : finite(item, `${path}[${String(index)}]`));
}

function nullable(input: unknown, path: string, validate: (value: unknown, valuePath: string) => unknown): void {
  if (input !== null) validate(input, path);
}

function nullableHandle(input: unknown, path: string): void {
  nullable(input, path, handle);
}

function handle(input: unknown, path: string): number {
  return safeInteger(input, path);
}

function safeInteger(input: unknown, path: string): number {
  return integer(input, path, 0, JSON_SAFE_INTEGER_MAX);
}

function nonNegativeInteger(input: unknown, path: string): number {
  return integer(input, path, 0, Number.MAX_SAFE_INTEGER);
}

function integerValue(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isSafeInteger(input)) fail(path, 'must be a safe integer');
  return input;
}

function integer(input: unknown, path: string, min: number, max: number): number {
  const value = integerValue(input, path);
  if (value < min || value > max) fail(path, `must be in ${String(min)}..=${String(max)}`);
  return value;
}

function finite(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) fail(path, 'must be finite');
  return input;
}

function positiveFinite(input: unknown, path: string): number {
  const value = finite(input, path);
  if (value <= 0) fail(path, 'must be positive');
  return value;
}

function nonNegativeFinite(input: unknown, path: string): number {
  const value = finite(input, path);
  if (value < 0) fail(path, 'must be non-negative');
  return value;
}

function range(input: unknown, path: string, min: number, max: number): number {
  const value = finite(input, path);
  if (value < min || value > max) fail(path, `must be in ${String(min)}..=${String(max)}`);
  return value;
}

function text(input: unknown, path: string): string {
  if (typeof input !== 'string') fail(path, 'must be a string');
  return input;
}

function nonEmptyText(input: unknown, path: string): string {
  const value = text(input, path);
  if (value.trim() === '') fail(path, 'must be non-empty');
  return value;
}

function booleanValue(input: unknown, path: string): boolean {
  if (typeof input !== 'boolean') fail(path, 'must be a boolean');
  return input;
}

function enumeration<const T extends string>(
  input: unknown,
  path: string,
  values: readonly T[],
): T {
  const value = text(input, path);
  if (!values.includes(value as T)) fail(path, `must be one of ${values.join(', ')}`);
  return value as T;
}

function fail(path: string, message: string): never {
  throw new ContractDecodeError(`${path} ${message}`);
}
