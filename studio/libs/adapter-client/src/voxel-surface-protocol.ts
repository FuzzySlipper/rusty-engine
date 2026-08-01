import type { StoredMaterialDefinition } from './voxel-protocol.js';

export type VoxelSurfaceTextureFilter = 'nearest' | 'linear';
export type VoxelSurfaceAlphaMode =
  | { readonly kind: 'opaque' }
  | { readonly kind: 'mask'; readonly cutoff: number }
  | { readonly kind: 'blend' };

export interface VoxelAtlasRegionDraft {
  readonly id: string;
  readonly contentMin: readonly [number, number];
  readonly contentExtent: readonly [number, number];
  readonly padding: {
    readonly left: number;
    readonly right: number;
    readonly bottom: number;
    readonly top: number;
  };
  readonly inset: 'halfTexel';
}

export type VoxelSurfaceMappingDraft =
  | {
      readonly kind: 'repeat';
      readonly tileScaleCells: readonly [number, number];
      readonly tileOriginCells: readonly [number, number];
    }
  | {
      readonly kind: 'atlas';
      readonly atlasAssetId: string;
      readonly expectedAtlasContentHash: string | null;
      readonly regions: readonly VoxelAtlasRegionDraft[];
      readonly regionId: string;
      readonly tileScaleCells: readonly [number, number];
      readonly tileOriginCells: readonly [number, number];
    };

export interface VoxelSurfaceMaterialDraft {
  readonly materialAssetId: string;
  readonly expectedMaterialContentHash: string | null;
  readonly definition: StoredMaterialDefinition;
  readonly alphaMode: VoxelSurfaceAlphaMode;
  readonly mapping: VoxelSurfaceMappingDraft;
}

export interface VoxelSurfaceAssignmentDraft {
  readonly sceneId: string;
  readonly instanceId: string;
  readonly materialSlot: number;
}

export interface VoxelSurfaceTextureReadout {
  readonly textureAssetId: string;
  readonly version: number;
  readonly contentHash: string;
  readonly sourcePath: string;
  readonly width: number;
  readonly height: number;
  readonly encodedByteLength: number;
  readonly filter: VoxelSurfaceTextureFilter;
  readonly wrap: 'repeat' | 'clamp';
}

export interface VoxelSurfaceAtlasReadout {
  readonly atlasAssetId: string;
  readonly version: number;
  readonly contentHash: string;
  readonly textureAssetId: string;
  readonly textureVersion: number;
  readonly textureContentHash: string;
  readonly regions: readonly VoxelAtlasRegionDraft[];
}

export type VoxelSurfaceMappingReadout =
  | {
      readonly kind: 'repeat';
      readonly tileScaleCells: readonly [number, number];
      readonly tileOriginCells: readonly [number, number];
    }
  | {
      readonly kind: 'atlas';
      readonly atlasAssetId: string;
      readonly atlasVersion: number;
      readonly atlasContentHash: string;
      readonly regionId: string;
      readonly tileScaleCells: readonly [number, number];
      readonly tileOriginCells: readonly [number, number];
    };

export interface VoxelSurfaceMaterialReadout {
  readonly materialAssetId: string;
  readonly version: number;
  readonly contentHash: string;
  readonly definition: StoredMaterialDefinition;
  readonly textureAssetId: string;
  readonly textureVersion: number;
  readonly textureContentHash: string;
  readonly alphaMode: VoxelSurfaceAlphaMode;
  readonly mapping: VoxelSurfaceMappingReadout;
  readonly assignments: readonly VoxelSurfaceAssignmentDraft[];
}

export interface VoxelSurfaceAuthoringReadout {
  readonly textures: readonly VoxelSurfaceTextureReadout[];
  readonly atlases: readonly VoxelSurfaceAtlasReadout[];
  readonly materials: readonly VoxelSurfaceMaterialReadout[];
}

export function validateVoxelSurfaceAuthoringReadout(input: unknown, path: string): void {
  const value = closed(input, path, ['textures', 'atlases', 'materials']);
  array(value['textures'], `${path}.textures`).forEach((entry, index) => {
    const itemPath = `${path}.textures[${String(index)}]`;
    const item = closed(entry, itemPath, [
      'textureAssetId', 'version', 'contentHash', 'sourcePath', 'width', 'height',
      'encodedByteLength', 'filter', 'wrap',
    ]);
    texts(item, itemPath, ['textureAssetId', 'contentHash', 'sourcePath']);
    integers(item, itemPath, ['version', 'width', 'height', 'encodedByteLength']);
    oneOf(item['filter'], `${itemPath}.filter`, ['nearest', 'linear']);
    oneOf(item['wrap'], `${itemPath}.wrap`, ['repeat', 'clamp']);
  });
  array(value['atlases'], `${path}.atlases`).forEach((entry, index) => {
    const itemPath = `${path}.atlases[${String(index)}]`;
    const item = closed(entry, itemPath, [
      'atlasAssetId', 'version', 'contentHash', 'textureAssetId', 'textureVersion',
      'textureContentHash', 'regions',
    ]);
    texts(item, itemPath, [
      'atlasAssetId', 'contentHash', 'textureAssetId', 'textureContentHash',
    ]);
    integers(item, itemPath, ['version', 'textureVersion']);
    regions(item['regions'], `${itemPath}.regions`);
  });
  array(value['materials'], `${path}.materials`).forEach((entry, index) => {
    const itemPath = `${path}.materials[${String(index)}]`;
    const item = closed(entry, itemPath, [
      'materialAssetId', 'version', 'contentHash', 'definition', 'textureAssetId',
      'textureVersion', 'textureContentHash', 'alphaMode', 'mapping', 'assignments',
    ]);
    texts(item, itemPath, [
      'materialAssetId', 'contentHash', 'textureAssetId', 'textureContentHash',
    ]);
    integers(item, itemPath, ['version', 'textureVersion']);
    materialDefinition(item['definition'], `${itemPath}.definition`);
    alphaMode(item['alphaMode'], `${itemPath}.alphaMode`);
    mappingReadout(item['mapping'], `${itemPath}.mapping`);
    array(item['assignments'], `${itemPath}.assignments`).forEach((assignment, assignmentIndex) => {
      const assignmentPath = `${itemPath}.assignments[${String(assignmentIndex)}]`;
      const assignmentValue = closed(assignment, assignmentPath, [
        'sceneId', 'instanceId', 'materialSlot',
      ]);
      texts(assignmentValue, assignmentPath, ['sceneId', 'instanceId']);
      integers(assignmentValue, assignmentPath, ['materialSlot']);
    });
  });
}

function mappingReadout(input: unknown, path: string): void {
  const value = record(input, path);
  const kind = text(value['kind'], `${path}.kind`);
  if (kind === 'repeat') {
    exactKeys(value, path, ['kind', 'tileScaleCells', 'tileOriginCells']);
  } else if (kind === 'atlas') {
    exactKeys(value, path, [
      'kind', 'atlasAssetId', 'atlasVersion', 'atlasContentHash', 'regionId',
      'tileScaleCells', 'tileOriginCells',
    ]);
    text(value['atlasAssetId'], `${path}.atlasAssetId`);
    safeInteger(value['atlasVersion'], `${path}.atlasVersion`);
    text(value['atlasContentHash'], `${path}.atlasContentHash`);
    text(value['regionId'], `${path}.regionId`);
  } else {
    fail(`${path}.kind`, 'must be repeat or atlas');
  }
  vector2(value['tileScaleCells'], `${path}.tileScaleCells`);
  vector2(value['tileOriginCells'], `${path}.tileOriginCells`);
}

function mapping(input: unknown, path: string): void {
  const value = record(input, path);
  const kind = text(value['kind'], `${path}.kind`);
  if (kind === 'repeat') {
    exactKeys(value, path, ['kind', 'tileScaleCells', 'tileOriginCells']);
  } else if (kind === 'atlas') {
    exactKeys(value, path, [
      'kind', 'atlasAssetId', 'expectedAtlasContentHash', 'regions', 'regionId',
      'tileScaleCells', 'tileOriginCells',
    ]);
    text(value['atlasAssetId'], `${path}.atlasAssetId`);
    nullableText(value['expectedAtlasContentHash'], `${path}.expectedAtlasContentHash`);
    regions(value['regions'], `${path}.regions`);
    text(value['regionId'], `${path}.regionId`);
  } else {
    fail(`${path}.kind`, 'must be repeat or atlas');
  }
  vector2(value['tileScaleCells'], `${path}.tileScaleCells`);
  vector2(value['tileOriginCells'], `${path}.tileOriginCells`);
}

function regions(input: unknown, path: string): void {
  array(input, path).forEach((entry, index) => {
    const itemPath = `${path}[${String(index)}]`;
    const item = closed(entry, itemPath, [
      'id', 'contentMin', 'contentExtent', 'padding', 'inset',
    ]);
    text(item['id'], `${itemPath}.id`);
    vector2(item['contentMin'], `${itemPath}.contentMin`, true);
    vector2(item['contentExtent'], `${itemPath}.contentExtent`, true);
    const padding = closed(item['padding'], `${itemPath}.padding`, [
      'left', 'right', 'bottom', 'top',
    ]);
    integers(padding, `${itemPath}.padding`, ['left', 'right', 'bottom', 'top']);
    if (item['inset'] !== 'halfTexel') fail(`${itemPath}.inset`, 'must equal halfTexel');
  });
}

function alphaMode(input: unknown, path: string): void {
  const value = record(input, path);
  const kind = text(value['kind'], `${path}.kind`);
  if (kind === 'mask') {
    exactKeys(value, path, ['kind', 'cutoff']);
    finite(value['cutoff'], `${path}.cutoff`);
  } else if (kind === 'opaque' || kind === 'blend') {
    exactKeys(value, path, ['kind']);
  } else {
    fail(`${path}.kind`, 'must be opaque, mask, or blend');
  }
}

function materialDefinition(input: unknown, path: string): void {
  const value = closed(input, path, ['authority', 'style']);
  const authority = closed(value['authority'], `${path}.authority`, [
    'solid', 'collidable', 'occludes', 'structuralClass',
  ]);
  ['solid', 'collidable', 'occludes'].forEach((field) => {
    if (typeof authority[field] !== 'boolean') fail(`${path}.authority.${field}`, 'must be boolean');
  });
  oneOf(authority['structuralClass'], `${path}.authority.structuralClass`, [
    'decorative', 'solid', 'structural',
  ]);
  const style = closed(value['style'], `${path}.style`, [
    'color', 'texture', 'textureTint', 'emissionColor', 'roughness', 'emissive', 'uvStrategy',
  ]);
  vector(style['color'], `${path}.style.color`, 4);
  if (style['texture'] !== null) {
    const reference = closed(style['texture'], `${path}.style.texture`, ['id', 'version', 'hash']);
    text(reference['id'], `${path}.style.texture.id`);
    record(reference['version'], `${path}.style.texture.version`);
    nullableText(reference['hash'], `${path}.style.texture.hash`);
  }
  vector(style['textureTint'], `${path}.style.textureTint`, 4);
  vector(style['emissionColor'], `${path}.style.emissionColor`, 4);
  finite(style['roughness'], `${path}.style.roughness`);
  finite(style['emissive'], `${path}.style.emissive`);
  oneOf(style['uvStrategy'], `${path}.style.uvStrategy`, ['flat', 'planar', 'atlas']);
}

function vector2(input: unknown, path: string, integer = false): void {
  const values = array(input, path);
  if (values.length !== 2) fail(path, 'must have two entries');
  values.forEach((value, index) => {
    if (integer) safeInteger(value, `${path}[${String(index)}]`);
    else finite(value, `${path}[${String(index)}]`);
  });
}

function vector(input: unknown, path: string, length: number): void {
  const values = array(input, path);
  if (values.length !== length) fail(path, `must have ${String(length)} entries`);
  values.forEach((value, index) => finite(value, `${path}[${String(index)}]`));
}

function integers(value: Record<string, unknown>, path: string, fields: readonly string[]): void {
  fields.forEach((field) => safeInteger(value[field], `${path}.${field}`));
}

function texts(value: Record<string, unknown>, path: string, fields: readonly string[]): void {
  fields.forEach((field) => text(value[field], `${path}.${field}`));
}

function oneOf(input: unknown, path: string, values: readonly string[]): void {
  if (typeof input !== 'string' || !values.includes(input)) {
    fail(path, `must be one of ${values.join(', ')}`);
  }
}

function nullableText(input: unknown, path: string): void {
  if (input !== null) text(input, path);
}

function safeInteger(input: unknown, path: string): number {
  if (!Number.isSafeInteger(input)) fail(path, 'must be a safe integer');
  return input as number;
}

function finite(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) fail(path, 'must be finite');
  return input;
}

function text(input: unknown, path: string): string {
  if (typeof input !== 'string' || input === '') fail(path, 'must be non-empty text');
  return input;
}

function array(input: unknown, path: string): unknown[] {
  if (!Array.isArray(input)) fail(path, 'must be an array');
  return input;
}

function record(input: unknown, path: string): Record<string, unknown> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  return input as Record<string, unknown>;
}

function closed(
  input: unknown,
  path: string,
  fields: readonly string[],
): Record<string, unknown> {
  const value = record(input, path);
  exactKeys(value, path, fields);
  return value;
}

function exactKeys(value: Record<string, unknown>, path: string, fields: readonly string[]): void {
  const expected = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!expected.has(field)) fail(`${path}.${field}`, 'is not allowed');
  }
  for (const field of fields) {
    if (!(field in value)) fail(`${path}.${field}`, 'is required');
  }
}

function fail(path: string, message: string): never {
  throw new TypeError(`${path}: ${message}`);
}
