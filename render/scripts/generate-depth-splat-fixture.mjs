import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';

const variants = ['quad', 'flat', 'physical', 'compressed', 'tangent'];
const sourceRoot = resolve(process.argv[2] ?? '');
const outputRoot = resolve(process.argv[3] ?? '');
if (process.argv.length < 4) {
  throw new Error('usage: node generate-depth-splat-fixture.mjs SOURCE_SUBJECT_DIR OUTPUT_FIXTURE_DIR');
}
mkdirSync(outputRoot, { recursive: true });

const imported = variants.map((variant) => importVariant(
  variant,
  join(sourceRoot, 'models', variant, 'dir-00.glb'),
));
const quad = imported[0];
if (quad.textureBytes === null) throw new Error('quad source must contain one embedded PNG texture');
for (const candidate of imported.slice(1)) {
  if (candidate.textureBytes !== null || candidate.payload.source.colors === undefined) {
    throw new Error(`${candidate.variant} must use vertex colors without a texture`);
  }
}

const resources = [
  packResource(imported.slice(0, 1), 'packedStreamsLeV2'),
  packResource(imported.slice(1), 'packedStreamsLeV3'),
];
const meshLocations = {};
for (const resource of resources) {
  const file = `mesh-${resource.contentHash.slice('sha256:'.length)}.rmsh`;
  writeFileSync(join(outputRoot, file), resource.bytes);
  meshLocations[resource.resource] = `/fixtures/render/depth-splat/${file}`;
}

const textureHash = sha256(quad.textureBytes);
const textureResource = `texture-resource/${textureHash.slice('sha256:'.length)}`;
const textureFile = `texture-${textureHash.slice('sha256:'.length)}.png`;
writeFileSync(join(outputRoot, textureFile), quad.textureBytes);
const [textureWidth, textureHeight] = pngDimensions(quad.textureBytes);

const fixture = {
  schemaVersion: 1,
  source: {
    project: 'asset-pipeline',
    task: 6977,
    run: 'depth-splat-20260815-001',
    subject: basename(sourceRoot),
    direction: 'dir-00',
    sourceGlbSha256: Object.fromEntries(imported.map((item) => [item.variant, item.sourceHash])),
  },
  meshResourceManifest: {
    kind: 'rusty_renderer_mesh_resources.v1',
    resources: resources.map(({ resource, contentHash, bytes }) => ({
      resource,
      contentHash,
      byteLength: bytes.byteLength,
    })),
  },
  textureResourceManifest: {
    kind: 'rusty_renderer_texture_resources.v1',
    resources: [{ resource: textureResource, contentHash: textureHash, byteLength: quad.textureBytes.byteLength }],
  },
  texture: {
    id: 'texture/depth-splat/spatial-wizard-dir-00',
    width: textureWidth,
    height: textureHeight,
    filter: 'nearest',
    wrap: 'clamp',
    contentHash: textureHash,
    version: 1,
    payload: {
      encoding: 'pngRgba8',
      colorSpace: 'srgb',
      contentHash: textureHash,
      byteLength: quad.textureBytes.byteLength,
      source: { kind: 'resource', resource: textureResource },
    },
  },
  materials: [
    material('material/depth-splat/quad', 'texture/depth-splat/spatial-wizard-dir-00'),
    material('material/depth-splat/colors', null),
  ],
  assets: imported.map((item) => ({
    asset: `mesh/depth-splat/spatial-wizard/${item.variant}`,
    payload: item.payload,
    materialSlots: [{
      slot: 0,
      material: item.variant === 'quad'
        ? 'material/depth-splat/quad'
        : 'material/depth-splat/colors',
    }],
    collision: { kind: 'visualOnly' },
  })),
  metrics: {
    sourceGlbBytes: imported.reduce((sum, item) => sum + item.sourceBytes, 0),
    packedMeshBytes: resources.reduce((sum, item) => sum + item.bytes.byteLength, 0),
    encodedTextureBytes: quad.textureBytes.byteLength,
    decodedTextureBytes: textureWidth * textureHeight * 4,
    uploadedMeshBytes: imported.reduce((sum, item) => sum + item.uploadedBytes, 0),
    variants: imported.map((item) => ({
      id: item.variant,
      vertices: item.payload.layout.vertexCount,
      triangles: item.payload.layout.indexCount / 3,
      sourceGlbBytes: item.sourceBytes,
      uploadedBytes: item.uploadedBytes,
    })),
  },
};

writeFileSync(
  resolve(outputRoot, '..', 'depth-splat-comparison-v1.json'),
  `${JSON.stringify(fixture, null, 2)}\n`,
);
writeFileSync(
  resolve(outputRoot, '..', '..', '..', 'render', 'browser', 'depth-splat-resource-locations.ts'),
  resourceLocationModule(meshLocations, {
    [textureResource]: `/fixtures/render/depth-splat/${textureFile}`,
  }),
);

function material(id, texture) {
  return {
    schemaVersion: 3,
    id,
    color: [1, 1, 1, 1],
    texture,
    roughness: 1,
    textureTint: [1, 1, 1, 1],
    emissionColor: [1, 1, 1],
    emissionIntensity: 0.08,
    uvStrategy: texture === null ? 'flat' : 'planar',
    alphaMode: { kind: 'mask', cutoff: 0.1 },
    doubleSided: true,
  };
}

function importVariant(variant, sourcePath) {
  const bytes = readFileSync(sourcePath);
  const { json, binary } = parseGlb(bytes);
  const scene = json.scenes?.[json.scene ?? 0];
  if (scene?.nodes?.length !== 1) throw new Error(`${variant} must contain one scene root`);
  const node = json.nodes?.[scene.nodes[0]];
  if (node?.mesh === undefined || hasTransform(node)) throw new Error(`${variant} must contain one untransformed mesh node`);
  const mesh = json.meshes?.[node.mesh];
  if (mesh?.primitives?.length !== 1) throw new Error(`${variant} must contain one mesh primitive`);
  const primitive = mesh.primitives[0];
  if (primitive.mode !== undefined && primitive.mode !== 4) throw new Error(`${variant} is not triangles`);
  const positions = accessor(json, binary, primitive.attributes.POSITION, 'VEC3');
  const normals = accessor(json, binary, primitive.attributes.NORMAL, 'VEC3');
  const uvs = primitive.attributes.TEXCOORD_0 === undefined
    ? undefined : accessor(json, binary, primitive.attributes.TEXCOORD_0, 'VEC2');
  const colors = primitive.attributes.COLOR_0 === undefined
    ? undefined : accessor(json, binary, primitive.attributes.COLOR_0, 'VEC4');
  const indices = accessor(json, binary, primitive.indices, 'SCALAR', true);
  const vertexCount = positions.length / 3;
  if (normals.length !== positions.length
    || (uvs !== undefined && uvs.length !== vertexCount * 2)
    || (colors !== undefined && colors.length !== vertexCount * 4)
    || indices.length % 3 !== 0) {
    throw new Error(`${variant} has mismatched retained streams`);
  }
  const bounds = boundsOf(positions);
  const attributes = [
    { name: 'position', components: 3, kind: 'f32' },
    { name: 'normal', components: 3, kind: 'f32' },
    ...(uvs === undefined ? [] : [{ name: 'uv', components: 2, kind: 'f32' }]),
    ...(colors === undefined ? [] : [{ name: 'color', components: 4, kind: 'f32' }]),
  ];
  const textureBytes = embeddedTexture(json, binary, primitive.material);
  return {
    variant,
    sourceBytes: bytes.byteLength,
    sourceHash: sha256(bytes),
    textureBytes,
    uploadedBytes: (positions.length + normals.length + (uvs?.length ?? 0)
      + (colors?.length ?? 0) + indices.length) * 4,
    payload: {
      layout: { vertexCount, indexCount: indices.length, indexWidth: 'u32', attributes },
      groups: [{ materialSlot: 0, start: 0, count: indices.length }],
      bounds,
      source: { kind: 'inline', positions, normals, ...(uvs === undefined ? {} : { uvs }), ...(colors === undefined ? {} : { colors }), indices },
      provenance: 'staticAsset',
    },
  };
}

function packResource(items, encoding) {
  const chunks = [];
  let length = 16;
  for (const item of items) {
    const source = item.payload.source;
    const offsets = {};
    for (const [name, values] of [
      ['positions', source.positions], ['normals', source.normals],
      ['uvs', source.uvs], ['colors', source.colors], ['indices', source.indices],
    ]) {
      if (values === undefined) continue;
      offsets[`${name}ByteOffset`] = length;
      const chunk = name === 'indices' ? u32Bytes(values) : f32Bytes(values);
      chunks.push(chunk);
      length += chunk.byteLength;
    }
    item.resourceOffsets = offsets;
  }
  const bytes = Buffer.alloc(length);
  bytes.write(encoding === 'packedStreamsLeV2' ? 'RMSHLE02' : 'RMSHLE03', 0, 'ascii');
  bytes.writeUInt32LE(length, 8);
  bytes.writeUInt32LE(items.length, 12);
  let cursor = 16;
  for (const chunk of chunks) {
    chunk.copy(bytes, cursor);
    cursor += chunk.byteLength;
  }
  const contentHash = sha256(bytes);
  const resource = `mesh-resource/${contentHash.slice('sha256:'.length)}`;
  for (const item of items) {
    item.payload.source = {
      kind: 'resource', resource, contentHash, byteLength: bytes.byteLength, encoding,
      ...item.resourceOffsets,
    };
    delete item.resourceOffsets;
  }
  return { resource, contentHash, bytes };
}

function parseGlb(bytes) {
  if (bytes.toString('ascii', 0, 4) !== 'glTF' || bytes.readUInt32LE(4) !== 2
    || bytes.readUInt32LE(8) !== bytes.byteLength) throw new Error('invalid GLB header');
  let json = null;
  let binary = null;
  for (let offset = 12; offset < bytes.byteLength;) {
    const length = bytes.readUInt32LE(offset);
    const type = bytes.readUInt32LE(offset + 4);
    const chunk = bytes.subarray(offset + 8, offset + 8 + length);
    if (type === 0x4e4f534a) json = JSON.parse(chunk.toString('utf8').replace(/[\0 ]+$/u, ''));
    if (type === 0x004e4942) binary = chunk;
    offset += 8 + length;
  }
  if (json === null || binary === null) throw new Error('GLB needs JSON and BIN chunks');
  return { json, binary };
}

function accessor(json, binary, accessorIndex, expectedType, integer = false) {
  const value = json.accessors?.[accessorIndex];
  if (value?.type !== expectedType || value.bufferView === undefined || value.sparse !== undefined) {
    throw new Error(`unsupported ${expectedType} accessor`);
  }
  const view = json.bufferViews?.[value.bufferView];
  if (view?.buffer !== 0) throw new Error('accessor must use the embedded BIN buffer');
  const components = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[value.type];
  const componentBytes = { 5121: 1, 5123: 2, 5125: 4, 5126: 4 }[value.componentType];
  if (componentBytes === undefined) throw new Error(`unsupported component type ${value.componentType}`);
  const stride = view.byteStride ?? componentBytes * components;
  const start = (view.byteOffset ?? 0) + (value.byteOffset ?? 0);
  const data = new DataView(binary.buffer, binary.byteOffset, binary.byteLength);
  const result = [];
  for (let item = 0; item < value.count; item += 1) {
    for (let component = 0; component < components; component += 1) {
      const offset = start + item * stride + component * componentBytes;
      let number = value.componentType === 5121 ? data.getUint8(offset)
        : value.componentType === 5123 ? data.getUint16(offset, true)
          : value.componentType === 5125 ? data.getUint32(offset, true)
            : data.getFloat32(offset, true);
      if (value.normalized === true) {
        number /= value.componentType === 5121 ? 255 : value.componentType === 5123 ? 65535 : 4294967295;
      }
      result.push(integer ? Math.trunc(number) : number);
    }
  }
  return result;
}

function embeddedTexture(json, binary, materialIndex) {
  const textureIndex = json.materials?.[materialIndex]?.pbrMetallicRoughness?.baseColorTexture?.index;
  if (textureIndex === undefined) return null;
  const imageIndex = json.textures?.[textureIndex]?.source;
  const image = json.images?.[imageIndex];
  if (image?.mimeType !== 'image/png' || image.bufferView === undefined) throw new Error('texture must be embedded PNG');
  const view = json.bufferViews[image.bufferView];
  return Buffer.from(binary.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength));
}

function hasTransform(node) {
  return node.matrix !== undefined || node.translation !== undefined
    || node.rotation !== undefined || node.scale !== undefined;
}

function boundsOf(positions) {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let index = 0; index < positions.length; index += 3) {
    for (let axis = 0; axis < 3; axis += 1) {
      min[axis] = Math.min(min[axis], positions[index + axis]);
      max[axis] = Math.max(max[axis], positions[index + axis]);
    }
  }
  return { min, max };
}

function f32Bytes(values) {
  const bytes = Buffer.alloc(values.length * 4);
  values.forEach((value, index) => bytes.writeFloatLE(value, index * 4));
  return bytes;
}

function u32Bytes(values) {
  const bytes = Buffer.alloc(values.length * 4);
  values.forEach((value, index) => bytes.writeUInt32LE(value, index * 4));
  return bytes;
}

function sha256(bytes) {
  return `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
}

function pngDimensions(bytes) {
  if (bytes.toString('hex', 0, 8) !== '89504e470d0a1a0a' || bytes.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error('invalid PNG texture');
  }
  return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
}

function resourceLocationModule(mesh, texture) {
  const table = (name, locations) => [
    `export const ${name} = Object.freeze({`,
    ...Object.entries(locations).map(([resource, location]) => {
      const relative = `../..${location}`;
      return `  ${JSON.stringify(resource)}: new URL(${JSON.stringify(relative)}, import.meta.url).href,`;
    }),
    '} as const);',
  ].join('\n');
  return [
    '// Generated by render/scripts/generate-depth-splat-fixture.mjs.',
    '// Static URL expressions let Vite retain the checked fixture bytes.',
    table('DEPTH_SPLAT_MESH_RESOURCE_LOCATIONS', mesh),
    '',
    table('DEPTH_SPLAT_TEXTURE_RESOURCE_LOCATIONS', texture),
    '',
  ].join('\n');
}
