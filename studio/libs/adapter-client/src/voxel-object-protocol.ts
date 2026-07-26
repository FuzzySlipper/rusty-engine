import type {
  Quaternion,
  Vector3,
  VoxelBounds,
  VoxelConversionSettings,
  VoxelMaterialBinding,
} from './voxel-protocol.js';
import { validateVoxelConversionSettings } from './voxel-protocol.js';

export type VoxelObjectSourceKind = 'static' | 'animated';

export interface MeshSourceRef {
  readonly assetId: string;
  readonly assetVersion: number;
  readonly sourceSha256: string;
  readonly meshPrimitive?: string;
}

export interface MeshSourceBounds {
  readonly min: Vector3;
  readonly max: Vector3;
}

export interface MeshSourceMaterialSlot {
  readonly sourceMaterialSlot: number;
  readonly sourceMaterialName?: string;
}

export interface MeshSourceGroup {
  readonly groupId: string;
  readonly label?: string;
  readonly sourceMaterialSlot: number;
  readonly sourceNodeIndex: number;
  readonly sourceMeshIndex: number;
  readonly sourcePrimitiveIndex: number;
  readonly indexStart: number;
  readonly indexCount: number;
  readonly bounds: MeshSourceBounds;
}

export interface MeshSourceNode {
  readonly nodeId: string;
  readonly label?: string;
  readonly sourceNodeIndex: number;
  readonly parentSourceNodeIndex?: number;
  readonly childSourceNodeIndices: readonly number[];
  readonly sourceMeshIndex?: number;
  readonly localTransform: readonly number[];
  readonly modelTransform: readonly number[];
}

export interface MeshSourceTextureCoordinates {
  readonly attributeName: string;
  readonly sourceSetIndex: number;
  readonly sourceHash: string;
  readonly vertexCount: number;
  readonly missingVertexCount: number;
}

export interface MeshSourceMetadata {
  readonly sourceSceneIndex: number;
  readonly sourceSceneName?: string;
  readonly sourceBounds: MeshSourceBounds;
  readonly vertexCount: number;
  readonly triangleCount: number;
  readonly groups: readonly MeshSourceGroup[];
  readonly materialSlots: readonly MeshSourceMaterialSlot[];
  readonly nodes: readonly MeshSourceNode[];
  readonly textureCoordinates: readonly MeshSourceTextureCoordinates[];
}

export type VoxelObjectAnimationProperty =
  | 'translation'
  | 'rotation'
  | 'scale'
  | 'morphWeights';

export interface VoxelObjectSourceClipReadout {
  readonly sourceAnimationIndex: number;
  readonly name: string;
  readonly durationMicroseconds: number;
  readonly channelCount: number;
  readonly targetNodeIndices: readonly number[];
  readonly properties: readonly VoxelObjectAnimationProperty[];
}

export interface VoxelObjectSourceDiagnostic {
  readonly severity: 'info' | 'warning' | 'error';
  readonly code: string;
  readonly path: string;
  readonly message: string;
}

export interface VoxelObjectSourceInspection {
  readonly sourceKind: VoxelObjectSourceKind;
  readonly source: MeshSourceRef;
  readonly sourcePath: string;
  readonly sourceByteCount: number;
  readonly metadata: MeshSourceMetadata;
  readonly clips: readonly VoxelObjectSourceClipReadout[];
  readonly diagnostics: readonly VoxelObjectSourceDiagnostic[];
}

export type VoxelObjectAnimationAnchorPolicy =
  | { readonly kind: 'preserveSourceSpace' }
  | { readonly kind: 'lockNodeToBindPose'; readonly sourceNodeIndex: number };

export type VoxelObjectAnimationEndPolicy = 'includeClipEnd' | 'excludeLoopSeam';

export interface VoxelObjectConversionSettings {
  readonly mesh: VoxelConversionSettings;
  readonly pivot: Vector3;
  readonly anchorPolicy: VoxelObjectAnimationAnchorPolicy;
}

export interface VoxelObjectClipConversionRequest {
  readonly sourceClipName: string;
  readonly outputClipId: string;
  readonly outputName?: string;
  readonly sampleRateHz: number;
  readonly startMicroseconds: number;
  readonly endMicroseconds?: number;
  readonly endPolicy: VoxelObjectAnimationEndPolicy;
}

export interface VoxelObjectClipPlanSummary {
  readonly outputClipId: string;
  readonly sourceClipName: string;
  readonly sourceAnimationIndex: number;
  readonly startMicroseconds: number;
  readonly endMicroseconds: number;
  readonly sampleRateHz: number;
  readonly sampledFrameCount: number;
  readonly storedFrameCount: number;
  readonly durationMicroseconds: number;
}

export interface VoxelObjectConversionPlan {
  readonly planId: string;
  readonly source: MeshSourceRef;
  readonly sourcePath: string;
  readonly targetAssetId: string;
  readonly licensePath?: string;
  readonly settings: VoxelObjectConversionSettings;
  readonly clips: readonly VoxelObjectClipConversionRequest[];
  readonly defaultClip?: string;
  readonly planner: string;
  readonly expectedSourceSha256: string;
  readonly settingsSha256: string;
  readonly expectedOutputContentHash: string;
  readonly planHash: string;
  readonly estimatedSampledFrames: number;
  readonly estimatedStoredFrames: number;
  readonly estimatedAggregateVoxels: number;
  readonly estimatedArtifactBytes: number;
  readonly estimatedBounds: VoxelBounds;
  readonly clipSummaries: readonly VoxelObjectClipPlanSummary[];
}

export type VoxelObjectFrameSelection =
  | { readonly kind: 'default' }
  | { readonly kind: 'clip'; readonly clipId: string; readonly frameIndex: number };

export interface VoxelObjectConvertedFrameReadout {
  readonly storedFrameIndex: number;
  readonly sourceTimestampsMicroseconds: readonly number[];
  readonly durationMicroseconds: number;
  readonly bounds: VoxelBounds;
  readonly voxelCount: number;
  readonly sparseRunCount: number;
  readonly voxelDataHash: string;
}

export interface VoxelObjectConvertedClipReadout {
  readonly outputClipId: string;
  readonly sourceClipName: string;
  readonly sourceAnimationIndex: number;
  readonly startMicroseconds: number;
  readonly endMicroseconds: number;
  readonly sampleRateHz: number;
  readonly endPolicy: VoxelObjectAnimationEndPolicy;
  readonly sampledFrameCount: number;
  readonly storedFrameCount: number;
  readonly durationMicroseconds: number;
  readonly frames: readonly VoxelObjectConvertedFrameReadout[];
}

export interface VoxelObjectSelectedFramePreview {
  readonly selection: VoxelObjectFrameSelection;
  readonly bounds: VoxelBounds;
  readonly voxelCount: number;
  readonly sparseRunCount: number;
  readonly voxelDataHash: string;
  readonly durationMicroseconds: number | null;
  readonly sourceTimestampsMicroseconds: readonly number[];
  readonly sampleVoxels: readonly {
    readonly coordinate: readonly [number, number, number];
    readonly materialSlot: number;
  }[];
  readonly samplesTruncated: boolean;
}

export interface VoxelObjectConversionPreview {
  readonly planId: string;
  readonly planHash: string;
  readonly outputHash: string;
  readonly sampledFrameCount: number;
  readonly storedFrameCount: number;
  readonly aggregateVoxelCount: number;
  readonly artifactBytes: number;
  readonly unionBounds: VoxelBounds;
  readonly clips: readonly VoxelObjectConvertedClipReadout[];
  readonly selectedFrame: VoxelObjectSelectedFramePreview;
}

export interface VoxelObjectMaterialMapping {
  readonly sourceMaterialSlot: number;
  readonly sourceMaterialName?: string;
  readonly voxelMaterialSlot: number;
}

export interface VoxelObjectFrameAuthoringReadout {
  readonly bounds: VoxelBounds;
  readonly voxelDataHash: string;
  readonly voxelCount: number;
  readonly sparseRunCount: number;
  readonly durationMicroseconds: number | null;
}

export interface VoxelObjectClipAuthoringReadout {
  readonly clipId: string;
  readonly name: string | null;
  readonly framesPerSecond: number;
  readonly frames: readonly VoxelObjectFrameAuthoringReadout[];
}

export interface VoxelObjectSourceClipProvenance {
  readonly outputClipId: string;
  readonly sourceClipName: string;
  readonly sourceAnimationIndex: number;
  readonly startMicroseconds: number;
  readonly endMicroseconds: number;
  readonly sampleRateHz: number;
  readonly includedClipEnd: boolean;
}

export interface VoxelObjectAssetAuthoringReadout {
  readonly assetId: string;
  readonly contentHash: string;
  readonly grid: {
    readonly coordinateSystem: 'rightHandedYUp';
    readonly cellSize: number;
    readonly chunkSize: number;
    readonly pivot: Vector3;
  };
  readonly bounds: VoxelBounds;
  readonly defaultFrame: VoxelObjectFrameAuthoringReadout;
  readonly clips: readonly VoxelObjectClipAuthoringReadout[];
  readonly defaultClip: string | null;
  readonly materialPalette: readonly VoxelMaterialBinding[];
  readonly materialMap: readonly VoxelObjectMaterialMapping[];
  readonly provenance: {
    readonly kind: 'authored' | 'convertedStaticMesh' | 'convertedAnimatedMesh';
    readonly sourcePath: string;
    readonly sourceSha256: string;
    readonly sourceByteCount: number;
    readonly converter: string;
    readonly settingsSha256: string;
    readonly licensePath: string | null;
    readonly sourceClips: readonly VoxelObjectSourceClipProvenance[];
  };
}

export interface StoredVoxelObjectMaterialOverride {
  readonly materialSlot: number;
  readonly materialAssetId: string;
}

export interface StoredVoxelObjectInstance {
  readonly instanceId: string;
  readonly voxelObjectAssetId: string;
  readonly frame: VoxelObjectFrameSelection;
  readonly translation: Vector3;
  readonly rotation: Quaternion;
  readonly scale: Vector3;
  readonly materialOverrides: readonly StoredVoxelObjectMaterialOverride[];
}

export interface VoxelObjectInstanceReadout {
  readonly sceneId: string;
  readonly instance: StoredVoxelObjectInstance;
}

export interface VoxelObjectAuthoringReadout {
  readonly assets: readonly VoxelObjectAssetAuthoringReadout[];
  readonly instances: readonly VoxelObjectInstanceReadout[];
}

export function validateVoxelObjectSourceInspection(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'sourceKind', 'source', 'sourcePath', 'sourceByteCount', 'metadata', 'clips', 'diagnostics',
  ]);
  enumValue(value['sourceKind'], `${path}.sourceKind`, ['static', 'animated']);
  validateMeshSourceRef(value['source'], `${path}.source`);
  string(value['sourcePath'], `${path}.sourcePath`);
  integer(value['sourceByteCount'], `${path}.sourceByteCount`);
  validateMeshSourceMetadata(value['metadata'], `${path}.metadata`);
  array(value['clips'], `${path}.clips`).forEach((entry, index) => {
    const itemPath = `${path}.clips[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'sourceAnimationIndex', 'name', 'durationMicroseconds', 'channelCount',
      'targetNodeIndices', 'properties',
    ]);
    integers(item, itemPath, [
      'sourceAnimationIndex', 'durationMicroseconds', 'channelCount',
    ]);
    string(item['name'], `${itemPath}.name`);
    integerArray(item['targetNodeIndices'], `${itemPath}.targetNodeIndices`);
    array(item['properties'], `${itemPath}.properties`).forEach((property, propertyIndex) =>
      enumValue(property, `${itemPath}.properties[${String(propertyIndex)}]`, [
        'translation', 'rotation', 'scale', 'morphWeights',
      ]));
  });
  array(value['diagnostics'], `${path}.diagnostics`).forEach((entry, index) => {
    const itemPath = `${path}.diagnostics[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['severity', 'code', 'path', 'message']);
    enumValue(item['severity'], `${itemPath}.severity`, ['info', 'warning', 'error']);
    strings(item, itemPath, ['code', 'path', 'message']);
  });
}

export function validateVoxelObjectConversionPlan(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'planId', 'source', 'sourcePath', 'targetAssetId', 'settings', 'clips', 'planner',
    'expectedSourceSha256', 'settingsSha256', 'expectedOutputContentHash', 'planHash',
    'estimatedSampledFrames', 'estimatedStoredFrames', 'estimatedAggregateVoxels',
    'estimatedArtifactBytes', 'estimatedBounds', 'clipSummaries',
  ], ['licensePath', 'defaultClip']);
  strings(value, path, [
    'planId', 'sourcePath', 'targetAssetId', 'planner', 'expectedSourceSha256',
    'settingsSha256', 'expectedOutputContentHash', 'planHash',
  ]);
  optionalString(value, path, 'licensePath');
  optionalString(value, path, 'defaultClip');
  validateMeshSourceRef(value['source'], `${path}.source`);
  validateVoxelObjectConversionSettings(value['settings'], `${path}.settings`);
  array(value['clips'], `${path}.clips`).forEach((entry, index) =>
    validateClipRequest(entry, `${path}.clips[${String(index)}]`));
  integers(value, path, [
    'estimatedSampledFrames', 'estimatedStoredFrames', 'estimatedAggregateVoxels',
    'estimatedArtifactBytes',
  ]);
  validateBounds(value['estimatedBounds'], `${path}.estimatedBounds`);
  array(value['clipSummaries'], `${path}.clipSummaries`).forEach((entry, index) =>
    validateClipSummary(entry, `${path}.clipSummaries[${String(index)}]`));
}

export function validateVoxelObjectConversionPreview(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'planId', 'planHash', 'outputHash', 'sampledFrameCount', 'storedFrameCount',
    'aggregateVoxelCount', 'artifactBytes', 'unionBounds', 'clips', 'selectedFrame',
  ]);
  strings(value, path, ['planId', 'planHash', 'outputHash']);
  integers(value, path, [
    'sampledFrameCount', 'storedFrameCount', 'aggregateVoxelCount', 'artifactBytes',
  ]);
  validateBounds(value['unionBounds'], `${path}.unionBounds`);
  array(value['clips'], `${path}.clips`).forEach((entry, index) =>
    validateConvertedClip(entry, `${path}.clips[${String(index)}]`));
  validateSelectedFrame(value['selectedFrame'], `${path}.selectedFrame`);
}

export function validateVoxelObjectAuthoringReadout(input: unknown, path: string): void {
  const value = closedObject(input, path, ['assets', 'instances']);
  array(value['assets'], `${path}.assets`).forEach((entry, index) =>
    validateAssetAuthoring(entry, `${path}.assets[${String(index)}]`));
  array(value['instances'], `${path}.instances`).forEach((entry, index) => {
    const itemPath = `${path}.instances[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['sceneId', 'instance']);
    string(item['sceneId'], `${itemPath}.sceneId`);
    validateStoredVoxelObjectInstance(item['instance'], `${itemPath}.instance`);
  });
}

export function validateStoredVoxelObjectInstance(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'instanceId', 'voxelObjectAssetId', 'frame', 'translation', 'rotation', 'scale',
    'materialOverrides',
  ]);
  strings(value, path, ['instanceId', 'voxelObjectAssetId']);
  validateVoxelObjectFrameSelection(value['frame'], `${path}.frame`);
  vector(value['translation'], `${path}.translation`, 3);
  vector(value['rotation'], `${path}.rotation`, 4);
  vector(value['scale'], `${path}.scale`, 3);
  array(value['materialOverrides'], `${path}.materialOverrides`).forEach((entry, index) => {
    const itemPath = `${path}.materialOverrides[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['materialSlot', 'materialAssetId']);
    integer(item['materialSlot'], `${itemPath}.materialSlot`);
    string(item['materialAssetId'], `${itemPath}.materialAssetId`);
  });
}

export function validateVoxelObjectFrameSelection(input: unknown, path: string): void {
  const base = object(input, path);
  const kind = string(base['kind'], `${path}.kind`);
  if (kind === 'default') {
    closedObject(input, path, ['kind']);
    return;
  }
  if (kind === 'clip') {
    const value = closedObject(input, path, ['kind', 'clipId', 'frameIndex']);
    string(value['clipId'], `${path}.clipId`);
    integer(value['frameIndex'], `${path}.frameIndex`);
    return;
  }
  throw new TypeError(`${path}.kind is not a closed voxel-object frame selection`);
}

function validateMeshSourceRef(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'assetId', 'assetVersion', 'sourceSha256',
  ], ['meshPrimitive']);
  strings(value, path, ['assetId', 'sourceSha256']);
  integer(value['assetVersion'], `${path}.assetVersion`);
  optionalString(value, path, 'meshPrimitive');
}

function validateMeshSourceMetadata(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'sourceSceneIndex', 'sourceBounds', 'vertexCount', 'triangleCount', 'groups',
    'materialSlots', 'nodes', 'textureCoordinates',
  ], ['sourceSceneName']);
  integers(value, path, ['sourceSceneIndex', 'vertexCount', 'triangleCount']);
  optionalString(value, path, 'sourceSceneName');
  validateMeshBounds(value['sourceBounds'], `${path}.sourceBounds`);
  array(value['groups'], `${path}.groups`).forEach((entry, index) => {
    const itemPath = `${path}.groups[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'groupId', 'sourceMaterialSlot', 'sourceNodeIndex', 'sourceMeshIndex',
      'sourcePrimitiveIndex', 'indexStart', 'indexCount', 'bounds',
    ], ['label']);
    string(item['groupId'], `${itemPath}.groupId`);
    optionalString(item, itemPath, 'label');
    integers(item, itemPath, [
      'sourceMaterialSlot', 'sourceNodeIndex', 'sourceMeshIndex',
      'sourcePrimitiveIndex', 'indexStart', 'indexCount',
    ]);
    validateMeshBounds(item['bounds'], `${itemPath}.bounds`);
  });
  array(value['materialSlots'], `${path}.materialSlots`).forEach((entry, index) => {
    const itemPath = `${path}.materialSlots[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['sourceMaterialSlot'], ['sourceMaterialName']);
    integer(item['sourceMaterialSlot'], `${itemPath}.sourceMaterialSlot`);
    optionalString(item, itemPath, 'sourceMaterialName');
  });
  array(value['nodes'], `${path}.nodes`).forEach((entry, index) => {
    const itemPath = `${path}.nodes[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'nodeId', 'sourceNodeIndex', 'childSourceNodeIndices', 'localTransform', 'modelTransform',
    ], ['label', 'parentSourceNodeIndex', 'sourceMeshIndex']);
    string(item['nodeId'], `${itemPath}.nodeId`);
    optionalString(item, itemPath, 'label');
    integer(item['sourceNodeIndex'], `${itemPath}.sourceNodeIndex`);
    optionalInteger(item, itemPath, 'parentSourceNodeIndex');
    optionalInteger(item, itemPath, 'sourceMeshIndex');
    integerArray(item['childSourceNodeIndices'], `${itemPath}.childSourceNodeIndices`);
    vector(item['localTransform'], `${itemPath}.localTransform`, 16);
    vector(item['modelTransform'], `${itemPath}.modelTransform`, 16);
  });
  array(value['textureCoordinates'], `${path}.textureCoordinates`).forEach((entry, index) => {
    const itemPath = `${path}.textureCoordinates[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'attributeName', 'sourceSetIndex', 'sourceHash', 'vertexCount', 'missingVertexCount',
    ]);
    strings(item, itemPath, ['attributeName', 'sourceHash']);
    integers(item, itemPath, ['sourceSetIndex', 'vertexCount', 'missingVertexCount']);
  });
}

function validateVoxelObjectConversionSettings(input: unknown, path: string): void {
  const value = closedObject(input, path, ['mesh', 'pivot', 'anchorPolicy']);
  validateVoxelConversionSettings(value['mesh'], `${path}.mesh`);
  vector(value['pivot'], `${path}.pivot`, 3);
  const anchor = object(value['anchorPolicy'], `${path}.anchorPolicy`);
  const kind = string(anchor['kind'], `${path}.anchorPolicy.kind`);
  if (kind === 'preserveSourceSpace') {
    closedObject(anchor, `${path}.anchorPolicy`, ['kind']);
  } else if (kind === 'lockNodeToBindPose') {
    const locked = closedObject(anchor, `${path}.anchorPolicy`, ['kind', 'sourceNodeIndex']);
    integer(locked['sourceNodeIndex'], `${path}.anchorPolicy.sourceNodeIndex`);
  } else {
    throw new TypeError(`${path}.anchorPolicy.kind is not a closed value`);
  }
}

function validateClipRequest(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'sourceClipName', 'outputClipId', 'sampleRateHz', 'startMicroseconds', 'endPolicy',
  ], ['outputName', 'endMicroseconds']);
  strings(value, path, ['sourceClipName', 'outputClipId']);
  optionalString(value, path, 'outputName');
  optionalInteger(value, path, 'endMicroseconds');
  integers(value, path, ['sampleRateHz', 'startMicroseconds']);
  enumValue(value['endPolicy'], `${path}.endPolicy`, ['includeClipEnd', 'excludeLoopSeam']);
}

function validateClipSummary(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'outputClipId', 'sourceClipName', 'sourceAnimationIndex', 'startMicroseconds',
    'endMicroseconds', 'sampleRateHz', 'sampledFrameCount', 'storedFrameCount',
    'durationMicroseconds',
  ]);
  strings(value, path, ['outputClipId', 'sourceClipName']);
  integers(value, path, [
    'sourceAnimationIndex', 'startMicroseconds', 'endMicroseconds', 'sampleRateHz',
    'sampledFrameCount', 'storedFrameCount', 'durationMicroseconds',
  ]);
}

function validateConvertedFrame(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'storedFrameIndex', 'sourceTimestampsMicroseconds', 'durationMicroseconds', 'bounds',
    'voxelCount', 'sparseRunCount', 'voxelDataHash',
  ]);
  integers(value, path, ['storedFrameIndex', 'durationMicroseconds', 'voxelCount', 'sparseRunCount']);
  integerArray(value['sourceTimestampsMicroseconds'], `${path}.sourceTimestampsMicroseconds`);
  validateBounds(value['bounds'], `${path}.bounds`);
  string(value['voxelDataHash'], `${path}.voxelDataHash`);
}

function validateConvertedClip(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'outputClipId', 'sourceClipName', 'sourceAnimationIndex', 'startMicroseconds',
    'endMicroseconds', 'sampleRateHz', 'endPolicy', 'sampledFrameCount',
    'storedFrameCount', 'durationMicroseconds', 'frames',
  ]);
  strings(value, path, ['outputClipId', 'sourceClipName']);
  integers(value, path, [
    'sourceAnimationIndex', 'startMicroseconds', 'endMicroseconds', 'sampleRateHz',
    'sampledFrameCount', 'storedFrameCount', 'durationMicroseconds',
  ]);
  enumValue(value['endPolicy'], `${path}.endPolicy`, ['includeClipEnd', 'excludeLoopSeam']);
  array(value['frames'], `${path}.frames`).forEach((entry, index) =>
    validateConvertedFrame(entry, `${path}.frames[${String(index)}]`));
}

function validateSelectedFrame(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'selection', 'bounds', 'voxelCount', 'sparseRunCount', 'voxelDataHash',
    'durationMicroseconds', 'sourceTimestampsMicroseconds', 'sampleVoxels',
    'samplesTruncated',
  ]);
  validateVoxelObjectFrameSelection(value['selection'], `${path}.selection`);
  validateBounds(value['bounds'], `${path}.bounds`);
  integers(value, path, ['voxelCount', 'sparseRunCount']);
  string(value['voxelDataHash'], `${path}.voxelDataHash`);
  nullableInteger(value['durationMicroseconds'], `${path}.durationMicroseconds`);
  integerArray(value['sourceTimestampsMicroseconds'], `${path}.sourceTimestampsMicroseconds`);
  array(value['sampleVoxels'], `${path}.sampleVoxels`).forEach((entry, index) => {
    const itemPath = `${path}.sampleVoxels[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['coordinate', 'materialSlot']);
    vector(item['coordinate'], `${itemPath}.coordinate`, 3);
    integer(item['materialSlot'], `${itemPath}.materialSlot`);
  });
  boolean(value['samplesTruncated'], `${path}.samplesTruncated`);
}

function validateAssetAuthoring(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'assetId', 'contentHash', 'grid', 'bounds', 'defaultFrame', 'clips', 'defaultClip',
    'materialPalette', 'materialMap', 'provenance',
  ]);
  strings(value, path, ['assetId', 'contentHash']);
  const grid = closedObject(value['grid'], `${path}.grid`, [
    'coordinateSystem', 'cellSize', 'chunkSize', 'pivot',
  ]);
  enumValue(grid['coordinateSystem'], `${path}.grid.coordinateSystem`, ['rightHandedYUp']);
  number(grid['cellSize'], `${path}.grid.cellSize`);
  integer(grid['chunkSize'], `${path}.grid.chunkSize`);
  vector(grid['pivot'], `${path}.grid.pivot`, 3);
  validateBounds(value['bounds'], `${path}.bounds`);
  validateFrameAuthoring(value['defaultFrame'], `${path}.defaultFrame`);
  array(value['clips'], `${path}.clips`).forEach((entry, index) => {
    const itemPath = `${path}.clips[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['clipId', 'name', 'framesPerSecond', 'frames']);
    string(item['clipId'], `${itemPath}.clipId`);
    nullableString(item['name'], `${itemPath}.name`);
    number(item['framesPerSecond'], `${itemPath}.framesPerSecond`);
    array(item['frames'], `${itemPath}.frames`).forEach((frame, frameIndex) =>
      validateFrameAuthoring(frame, `${itemPath}.frames[${String(frameIndex)}]`));
  });
  nullableString(value['defaultClip'], `${path}.defaultClip`);
  array(value['materialPalette'], `${path}.materialPalette`).forEach((entry, index) =>
    validateMaterialBinding(entry, `${path}.materialPalette[${String(index)}]`));
  array(value['materialMap'], `${path}.materialMap`).forEach((entry, index) => {
    const itemPath = `${path}.materialMap[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'sourceMaterialSlot', 'voxelMaterialSlot',
    ], ['sourceMaterialName']);
    integers(item, itemPath, ['sourceMaterialSlot', 'voxelMaterialSlot']);
    optionalString(item, itemPath, 'sourceMaterialName');
  });
  const provenance = closedObject(value['provenance'], `${path}.provenance`, [
    'kind', 'sourcePath', 'sourceSha256', 'sourceByteCount', 'converter',
    'settingsSha256', 'licensePath', 'sourceClips',
  ]);
  enumValue(provenance['kind'], `${path}.provenance.kind`, [
    'authored', 'convertedStaticMesh', 'convertedAnimatedMesh',
  ]);
  strings(provenance, `${path}.provenance`, [
    'sourcePath', 'sourceSha256', 'converter', 'settingsSha256',
  ]);
  integer(provenance['sourceByteCount'], `${path}.provenance.sourceByteCount`);
  nullableString(provenance['licensePath'], `${path}.provenance.licensePath`);
  array(provenance['sourceClips'], `${path}.provenance.sourceClips`).forEach((entry, index) => {
    const itemPath = `${path}.provenance.sourceClips[${String(index)}]`;
    const item = closedObject(entry, itemPath, [
      'outputClipId', 'sourceClipName', 'sourceAnimationIndex', 'startMicroseconds',
      'endMicroseconds', 'sampleRateHz', 'includedClipEnd',
    ]);
    strings(item, itemPath, ['outputClipId', 'sourceClipName']);
    integers(item, itemPath, [
      'sourceAnimationIndex', 'startMicroseconds', 'endMicroseconds', 'sampleRateHz',
    ]);
    boolean(item['includedClipEnd'], `${itemPath}.includedClipEnd`);
  });
}

function validateFrameAuthoring(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'bounds', 'voxelDataHash', 'voxelCount', 'sparseRunCount', 'durationMicroseconds',
  ]);
  validateBounds(value['bounds'], `${path}.bounds`);
  string(value['voxelDataHash'], `${path}.voxelDataHash`);
  integers(value, path, ['voxelCount', 'sparseRunCount']);
  nullableInteger(value['durationMicroseconds'], `${path}.durationMicroseconds`);
}

function validateMaterialBinding(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'materialSlot', 'materialAssetId',
  ], ['displayName']);
  integer(value['materialSlot'], `${path}.materialSlot`);
  string(value['materialAssetId'], `${path}.materialAssetId`);
  optionalString(value, path, 'displayName');
}

function validateBounds(input: unknown, path: string): void {
  const value = closedObject(input, path, ['min', 'max']);
  vector(value['min'], `${path}.min`, 3);
  vector(value['max'], `${path}.max`, 3);
}

function validateMeshBounds(input: unknown, path: string): void {
  const value = closedObject(input, path, ['min', 'max']);
  vector(value['min'], `${path}.min`, 3);
  vector(value['max'], `${path}.max`, 3);
}

function object(input: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw new TypeError(`${path} must be an object`);
  }
  return input as Readonly<Record<string, unknown>>;
}

function closedObject(
  input: unknown,
  path: string,
  required: readonly string[],
  optional: readonly string[] = [],
): Readonly<Record<string, unknown>> {
  const value = object(input, path);
  const allowed = new Set([...required, ...optional]);
  for (const field of required) {
    if (!Object.hasOwn(value, field)) throw new TypeError(`${path}.${field} is required`);
  }
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) throw new TypeError(`${path}.${field} is unknown`);
  }
  return value;
}

function array(input: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(input)) throw new TypeError(`${path} must be an array`);
  return input;
}

function string(input: unknown, path: string): string {
  if (typeof input !== 'string') throw new TypeError(`${path} must be text`);
  return input;
}

function nullableString(input: unknown, path: string): void {
  if (input !== null) string(input, path);
}

function optionalString(
  value: Readonly<Record<string, unknown>>,
  path: string,
  field: string,
): void {
  if (value[field] !== undefined) string(value[field], `${path}.${field}`);
}

function number(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    throw new TypeError(`${path} must be a finite number`);
  }
  return input;
}

function integer(input: unknown, path: string): number {
  const value = number(input, path);
  if (!Number.isInteger(value) || value < 0) throw new TypeError(`${path} must be a non-negative integer`);
  return value;
}

function nullableInteger(input: unknown, path: string): void {
  if (input !== null) integer(input, path);
}

function optionalInteger(
  value: Readonly<Record<string, unknown>>,
  path: string,
  field: string,
): void {
  if (value[field] !== undefined) integer(value[field], `${path}.${field}`);
}

function boolean(input: unknown, path: string): boolean {
  if (typeof input !== 'boolean') throw new TypeError(`${path} must be boolean`);
  return input;
}

function strings(
  value: Readonly<Record<string, unknown>>,
  path: string,
  fields: readonly string[],
): void {
  for (const field of fields) string(value[field], `${path}.${field}`);
}

function integers(
  value: Readonly<Record<string, unknown>>,
  path: string,
  fields: readonly string[],
): void {
  for (const field of fields) integer(value[field], `${path}.${field}`);
}

function enumValue(input: unknown, path: string, allowed: readonly string[]): void {
  const value = string(input, path);
  if (!allowed.includes(value)) throw new TypeError(`${path} is not a closed value`);
}

function vector(input: unknown, path: string, length: number): void {
  const values = array(input, path);
  if (values.length !== length) throw new TypeError(`${path} must have ${String(length)} entries`);
  values.forEach((entry, index) => number(entry, `${path}[${String(index)}]`));
}

function integerArray(input: unknown, path: string): void {
  array(input, path).forEach((entry, index) => integer(entry, `${path}[${String(index)}]`));
}
