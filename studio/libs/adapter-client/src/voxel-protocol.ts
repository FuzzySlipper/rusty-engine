import type { Transform } from '@rusty-engine/render-contracts';

export type Vector3 = readonly [number, number, number];
export type Vector3i = readonly [number, number, number];
export type Quaternion = readonly [number, number, number, number];

export interface VoxelBounds {
  readonly min: Vector3i;
  readonly max: Vector3i;
}

export interface StoredAssetReference {
  readonly id: string;
  readonly version: Readonly<Record<string, unknown>>;
  readonly hash: string | null;
}

export interface StoredMaterialDefinition {
  readonly authority: {
    readonly solid: boolean;
    readonly collidable: boolean;
    readonly occludes: boolean;
    readonly structuralClass: 'decorative' | 'solid' | 'structural';
  };
  readonly style: {
    readonly color: readonly [number, number, number, number];
    readonly texture: StoredAssetReference | null;
    readonly textureTint: readonly [number, number, number, number];
    readonly emissionColor: readonly [number, number, number, number];
    readonly roughness: number;
    readonly emissive: number;
    readonly uvStrategy: 'flat' | 'planar' | 'atlas';
  };
}

export interface VoxelMaterialBinding {
  readonly materialSlot: number;
  readonly materialAssetId: string;
  readonly displayName?: string;
}

export interface StoredVoxelInstance {
  readonly instanceId: string;
  readonly voxelAssetId: string;
  readonly translation: Vector3;
  readonly rotation: Quaternion;
  readonly scale: Vector3;
}

export interface VoxelAssetInspection {
  readonly assetId: string;
  readonly schemaVersion: number;
  readonly cellSize: number;
  readonly chunkSize: number;
  readonly origin: Vector3i;
  readonly boundsMin: Vector3i;
  readonly boundsMax: Vector3i;
  readonly representedVoxelCount: number;
  readonly sparseRunCount: number;
  readonly materialCounts: readonly {
    readonly materialSlot: number;
    readonly voxelCount: number;
  }[];
  readonly voxelDataHash: string;
  readonly contentHash: string;
  readonly provenanceKind: string;
  readonly provenanceSource: string;
  readonly state?: {
    readonly voxelSize: number;
    readonly chunkSize: number;
    readonly sourceRevision: number;
    readonly collisionRevision: number;
    readonly navigationRevision: number;
    readonly meshRevision: number;
    readonly projectionsCoherent: boolean;
    readonly authorityHash: string;
    readonly solidVoxelCount: number;
    readonly residentChunkCount: number;
    readonly colliderChunkCount: number;
    readonly meshChunkCount: number;
    readonly navigationCellCount: number;
    readonly navigationHash: string;
    readonly chunks: readonly {
      readonly chunk: Vector3i;
      readonly contentHash: string;
      readonly materialVoxelCount: number;
      readonly hasCollider: boolean;
      readonly vertices: number;
      readonly indices: number;
      readonly quads: number;
      readonly facesCulled: number;
      readonly materialGroupCount: number;
    }[];
    readonly diagnostics: Readonly<Record<string, unknown>>;
  };
  readonly diagnostics: Readonly<Record<string, unknown>>;
}

export interface VoxelHistoryReadout {
  readonly persisted: boolean;
  readonly entryCount: number;
  readonly cursor: number;
  readonly undoDepth: number;
  readonly redoDepth: number;
  readonly authorityHash: string;
  readonly historyHash: string;
}

export interface VoxelAnnotationSummaryReadout {
  readonly layerId: string;
  readonly canonicalLayerHash: string;
  readonly membershipDataHash: string;
  readonly regionCount: number;
  readonly assignedCellCount: number;
}

export interface VoxelAssetAuthoringReadout {
  readonly inspection: VoxelAssetInspection;
  readonly palette: readonly VoxelMaterialBinding[];
  readonly history: VoxelHistoryReadout;
  readonly annotations: readonly VoxelAnnotationSummaryReadout[];
}

export interface VoxelInstanceReadout {
  readonly sceneId: string;
  readonly instance: StoredVoxelInstance;
}

export interface MaterialAssetReadout {
  readonly assetId: string;
  readonly definition: StoredMaterialDefinition;
}

export interface VoxelAuthoringReadout {
  readonly assets: readonly VoxelAssetAuthoringReadout[];
  readonly instances: readonly VoxelInstanceReadout[];
  readonly materials: readonly MaterialAssetReadout[];
}

export type VoxelBrushMode = 'paint' | 'erase';
export type VoxelBoxFill = 'filled' | 'shell' | 'edges';
export type VoxelPrimitive =
  | { readonly kind: 'block'; readonly address: Vector3i }
  | {
    readonly kind: 'box';
    readonly start: Vector3i;
    readonly end: Vector3i;
    readonly fill: VoxelBoxFill;
  }
  | {
    readonly kind: 'line';
    readonly start: Vector3i;
    readonly end: Vector3i;
    readonly radius: number;
  };
export type VoxelPrimitiveMaterial =
  | { readonly kind: 'set'; readonly materialSlot: number }
  | { readonly kind: 'clear' };
export interface VoxelPrimitiveRequest {
  readonly primitive: VoxelPrimitive;
  readonly material: VoxelPrimitiveMaterial;
}
export interface VoxelTemplateRequest {
  readonly template: 'house';
  readonly origin: Vector3i;
  readonly materialSlot: number;
}
export type VoxelPickFace =
  | 'negativeX'
  | 'positiveX'
  | 'negativeY'
  | 'positiveY'
  | 'negativeZ'
  | 'positiveZ';

export interface VoxelPickReadout {
  readonly sceneId: string;
  readonly instanceId: string;
  readonly assetId: string;
  readonly hitVoxel: Vector3i;
  readonly hitFace: VoxelPickFace;
  readonly placeVoxel: Vector3i;
  readonly authorityHitVoxel: Vector3i;
  readonly authorityPlaceVoxel: Vector3i;
  readonly instanceLocalPoint: Vector3;
  readonly worldPoint: Vector3;
  readonly worldDistance: number;
  readonly hitPreviewTransform: Transform;
  readonly placePreviewTransform: Transform;
}

export type VoxelAnnotationKind =
  | 'selection'
  | 'room'
  | 'portal'
  | 'spawnArea'
  | 'cover'
  | 'hazard'
  | 'navigationHint'
  | 'custom';

export interface VoxelAnnotationSparseRun {
  readonly start: Vector3i;
  readonly length: number;
}

export interface VoxelAnnotationRegion {
  readonly regionId: string;
  readonly label: string;
  readonly kind: VoxelAnnotationKind;
  readonly tags: readonly string[];
  readonly parentRegionId?: string;
  readonly bounds: VoxelBounds;
  readonly selection: { readonly sparseRuns: readonly VoxelAnnotationSparseRun[] };
}

export interface VoxelAnnotationLayerDraft {
  readonly layerId: string;
  readonly targetVoxelAssetId: string;
  readonly targetVoxelDataHash: string;
  readonly targetBounds: VoxelBounds;
  readonly regions: readonly VoxelAnnotationRegion[];
  readonly provenance: readonly {
    readonly kind: 'authored' | 'importedReference' | 'runtimeExport' | 'generated';
    readonly uri: string;
    readonly contentHash: string;
  }[];
}

export type VoxelAnnotationEditCommand =
  | { readonly kind: 'upsertRegion'; readonly region: VoxelAnnotationRegion }
  | { readonly kind: 'removeRegion'; readonly regionId: string }
  | { readonly kind: 'addRuns'; readonly regionId: string; readonly sparseRuns: readonly VoxelAnnotationSparseRun[] }
  | { readonly kind: 'removeRuns'; readonly regionId: string; readonly sparseRuns: readonly VoxelAnnotationSparseRun[] }
  | { readonly kind: 'replaceSelection'; readonly regionId: string; readonly selection: { readonly sparseRuns: readonly VoxelAnnotationSparseRun[] } }
  | { readonly kind: 'setParent'; readonly regionId: string; readonly parentRegionId: string | null }
  | { readonly kind: 'setTags'; readonly regionId: string; readonly tags: readonly string[] }
  | { readonly kind: 'setLabel'; readonly regionId: string; readonly label: string }
  | { readonly kind: 'setKind'; readonly regionId: string; readonly annotationKind: VoxelAnnotationKind }
  | { readonly kind: 'setBounds'; readonly regionId: string; readonly bounds: VoxelBounds };

export interface VoxelAnnotationEditTransaction {
  readonly expectedLayerHash: string;
  readonly commands: readonly VoxelAnnotationEditCommand[];
}

export type VoxelAnnotationQueryMode =
  | { readonly kind: 'cell'; readonly coordinate: Vector3i }
  | { readonly kind: 'bounds'; readonly bounds: VoxelBounds }
  | { readonly kind: 'region'; readonly regionId: string }
  | { readonly kind: 'layerSummary' };

export interface VoxelAnnotationQuery {
  readonly expectedLayerHash?: string;
  readonly mode: VoxelAnnotationQueryMode;
  readonly maxResults: number;
}

export interface VoxelModelWindowRequest {
  readonly expectedContentHash: string;
  readonly bounds: VoxelBounds;
  readonly includeEmpty: boolean;
  readonly materialFilter: readonly number[];
  readonly maxSamples: number;
}

export interface VoxelModelInfoReadout {
  readonly assetId: string;
  readonly contentHash: string;
  readonly voxelDataHash: string;
  readonly bounds: VoxelBounds;
  readonly voxelCount: number;
  readonly sparseRunCount: number;
  readonly materialCounts: readonly {
    readonly materialSlot: number;
    readonly voxelCount: number;
  }[];
}

export interface VoxelModelWindowReadout {
  readonly assetId: string;
  readonly contentHash: string;
  readonly requestedBounds: VoxelBounds;
  readonly modelBounds: VoxelBounds;
  readonly scannedCellCount: number;
  readonly samples: readonly {
    readonly coordinate: Vector3i;
    readonly materialSlot: number | null;
  }[];
  readonly samplesTruncated: boolean;
}

export interface VoxelEditDelta {
  readonly address: Vector3i;
  readonly beforeMaterial: number | null;
  readonly afterMaterial: number | null;
}

export interface VoxelHistoryEntryReadout {
  readonly transactionId: number;
  readonly parentTransactionId: number | null;
  readonly beforeHash: string;
  readonly afterHash: string;
  readonly changedVoxels: number;
  readonly deltasTruncated: boolean;
  readonly deltas: readonly VoxelEditDelta[];
}

export interface VoxelHistoryQueryReadout {
  readonly kind: 'history';
  readonly assetId: string;
  readonly cursor: number;
  readonly undoDepth: number;
  readonly redoDepth: number;
  readonly entryCount: number;
  readonly entriesTruncated: boolean;
  readonly entries: readonly VoxelHistoryEntryReadout[];
}

export interface VoxelHistoryRevertPreview {
  readonly previewId: string;
  readonly assetId: string;
  readonly expectedProjectHash: string;
  readonly expectedAssetContentHash: string;
  readonly cursorBefore: number;
  readonly cursorAfter: number;
  readonly undoDepthAfter: number;
  readonly redoDepthAfter: number;
  readonly revisionBefore: number;
  readonly revisionAfter: number;
  readonly changedVoxels: number;
  readonly bounds: VoxelBounds | null;
  readonly materialDeltas: readonly {
    readonly beforeMaterial: number | null;
    readonly afterMaterial: number | null;
    readonly changedVoxels: number;
  }[];
  readonly samples: readonly VoxelEditDelta[];
  readonly samplesTruncated: boolean;
  readonly includedTransactionIds: readonly number[];
}

export type VoxelReadout =
  | {
    readonly kind: 'model';
    readonly info: VoxelModelInfoReadout;
    readonly window?: VoxelModelWindowReadout;
  }
  | {
    readonly kind: 'annotationQuery';
    readonly layerHash: string;
    readonly totalLayerRegions: number;
    readonly truncated: boolean;
    readonly matchedRegions: readonly (Omit<VoxelAnnotationRegion, 'selection'> & {
      readonly assignedCellCount: number;
    })[];
  }
  | {
    readonly kind: 'annotationExport';
    readonly layerId: string;
    readonly canonicalJson: string;
    readonly canonicalLayerHash: string;
    readonly membershipDataHash: string;
  }
  | VoxelHistoryQueryReadout;

export interface TextureSourceRef {
  readonly textureAssetId: string;
  readonly assetVersion: number;
  readonly contentHash: string;
  readonly width: number;
  readonly height: number;
  readonly colorSpace: 'linear' | 'srgb';
  readonly channelLayout: 'palette_index_u16';
}

export interface TextureSampleAsset {
  readonly texture: TextureSourceRef;
  readonly texelMaterials: readonly number[];
}

export interface TextureUvAttributeRef {
  readonly attributeName: string;
  readonly sourceHash: string;
}

export interface TextureMaterialBinding {
  readonly sourceMaterialSlot: number;
  readonly texture: TextureSourceRef;
  readonly uvAttribute: TextureUvAttributeRef;
  readonly sampleUv: readonly [number, number];
  readonly samplingPolicy: 'nearest_texel';
  readonly wrapPolicy: 'clamp_to_edge';
  readonly materialMode: 'sample_palette_index';
}

export interface VoxelConversionSettings {
  readonly conversion: {
    readonly resolution: Vector3i;
    readonly cellSize: number;
    readonly chunkSize: number;
    readonly origin: Vector3i;
    readonly fitPolicy: 'contain' | 'cover' | 'stretch';
    readonly originPolicy: 'sourceOrigin' | 'targetMin' | 'centered';
    readonly mode: 'surface' | 'solid';
    readonly materialPalette: readonly VoxelMaterialBinding[];
    readonly materialMap: readonly {
      readonly sourceMaterialSlot: number;
      readonly sourceMaterialName?: string;
      readonly voxelMaterialSlot: number;
    }[];
    readonly maxOutputVoxels: number;
  };
  readonly transform: readonly number[];
  readonly materialPolicy: {
    readonly textureAssets: readonly TextureSampleAsset[];
    readonly textureBindings: readonly TextureMaterialBinding[];
    readonly defaultVoxelMaterial?: number;
  };
}

export interface VoxelConversionPlan {
  readonly planId: string;
  readonly source: {
    readonly assetId: string;
    readonly assetVersion: number;
    readonly sourceSha256: string;
    readonly meshPrimitive?: string;
  };
  readonly targetAssetId: string;
  readonly sourcePath: string;
  readonly licensePath?: string;
  readonly settings: VoxelConversionSettings;
  readonly planner: string;
  readonly expectedSourceSha256: string;
  readonly settingsSha256: string;
  readonly expectedOutputContentHash: string;
  readonly planHash: string;
  readonly estimatedOutputVoxels: number;
  readonly estimatedBounds: VoxelBounds;
}

export interface VoxelConversionPreview {
  readonly planId: string;
  readonly planHash: string;
  readonly outputHash: string;
  readonly outputVoxelCount: number;
  readonly outputBounds: VoxelBounds;
  readonly sampleVoxels: readonly {
    readonly coordinate: Vector3i;
    readonly materialSlot: number;
  }[];
  readonly samplesTruncated: boolean;
}

export type ProjectMutationReceipt =
  | { readonly kind: 'sceneCreated'; readonly sceneId: string; readonly madeEntry: boolean }
  | { readonly kind: 'sceneRenamed'; readonly sceneId: string }
  | { readonly kind: 'sceneDeleted'; readonly sceneId: string }
  | { readonly kind: 'entrySceneSet'; readonly sceneId: string }
  | { readonly kind: 'sceneObjectCreated'; readonly entityId: number }
  | { readonly kind: 'sceneObjectDeleted'; readonly entityId: number; readonly removedObjects: number }
  | { readonly kind: 'sceneObjectRenamed'; readonly entityId: number }
  | { readonly kind: 'sceneObjectReparented'; readonly entityId: number }
  | { readonly kind: 'sceneObjectTransformSet'; readonly entityId: number }
  | { readonly kind: 'sceneObjectAppearanceSet'; readonly entityId: number }
  | { readonly kind: 'entityCollisionSet'; readonly entityId: number; readonly attached: boolean }
  | { readonly kind: 'entityKinematicSet'; readonly entityId: number; readonly attached: boolean }
  | { readonly kind: 'materialUpserted'; readonly assetId: string }
  | { readonly kind: 'assetImportApplied'; readonly planId: string; readonly planHash: string; readonly assetId: string; readonly sourcePath: string; readonly reimportKind: string; readonly generatedAssetIds: readonly string[] }
  | { readonly kind: 'voxelAssetInitialized'; readonly assetId: string; readonly contentHash: string }
  | { readonly kind: 'voxelAssetDuplicated'; readonly sourceAssetId: string; readonly targetAssetId: string; readonly contentHash: string }
  | { readonly kind: 'voxelInstanceAttached'; readonly sceneId: string; readonly instanceId: string }
  | { readonly kind: 'voxelInstanceTransformSet'; readonly sceneId: string; readonly instanceId: string }
  | { readonly kind: 'voxelInstanceRemoved'; readonly sceneId: string; readonly instanceId: string }
  | { readonly kind: 'voxelPaletteReplaced'; readonly assetId: string; readonly contentHashBefore: string; readonly contentHashAfter: string; readonly voxelDataHash: string; readonly materialCountBefore: number; readonly materialCountAfter: number }
  | { readonly kind: 'voxelBrushApplied'; readonly assetId: string; readonly contentHashBefore: string; readonly contentHashAfter: string; readonly changedVoxels: number; readonly sourceRevision: number; readonly historyCursor: number; readonly undoDepth: number; readonly redoDepth: number }
  | { readonly kind: 'voxelPrimitiveApplied'; readonly assetId: string; readonly primitiveKind: string; readonly contentHashBefore: string; readonly contentHashAfter: string; readonly changedVoxels: number; readonly sourceRevision: number; readonly historyCursor: number; readonly undoDepth: number; readonly redoDepth: number }
  | { readonly kind: 'voxelTemplateInitialized'; readonly assetId: string; readonly templateKind: string; readonly contentHash: string; readonly changedVoxels: number; readonly historyCursor: number }
  | { readonly kind: 'voxelAssetFileImported'; readonly sourcePath: string; readonly sourceSha256: string; readonly sourceByteCount: number; readonly sourceAssetId: string; readonly targetAssetId: string; readonly contentHash: string }
  | { readonly kind: 'environmentMaterialized'; readonly sceneId: string; readonly preset: string; readonly seed: number; readonly assetId: string; readonly instanceId: string; readonly contentHash: string; readonly voxelCount: number; readonly playerEntityId: number; readonly playerTranslation: Vector3; readonly exitEntityId: number; readonly exitTranslation: Vector3; readonly generatorId: string; readonly generatorVersion: number; readonly settingsSha256: string; readonly voxelDataSha256: string }
  | { readonly kind: 'voxelHistoryMoved'; readonly assetId: string; readonly contentHashBefore: string; readonly contentHashAfter: string; readonly cursorBefore: number; readonly cursorAfter: number; readonly undoDepth: number; readonly redoDepth: number; readonly changedVoxels: number }
  | { readonly kind: 'voxelAnnotationCreated'; readonly assetId: string; readonly layerId: string; readonly layerHash: string }
  | { readonly kind: 'voxelAnnotationEdited'; readonly assetId: string; readonly layerId: string; readonly layerHashBefore: string; readonly layerHashAfter: string; readonly affectedRegionIds: readonly string[] }
  | { readonly kind: 'voxelConversionApplied'; readonly planId: string; readonly planHash: string; readonly assetId: string; readonly outputHash: string; readonly outputVoxels: number };

export function validateVoxelAuthoringReadout(input: unknown, path: string): void {
  const value = closedObject(input, path, ['assets', 'instances', 'materials']);
  const assets = array(value['assets'], `${path}.assets`);
  const instances = array(value['instances'], `${path}.instances`);
  const materials = array(value['materials'], `${path}.materials`);
  assets.forEach((entry, index) => validateVoxelAsset(entry, `${path}.assets[${String(index)}]`));
  instances.forEach((entry, index) => {
    const itemPath = `${path}.instances[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['sceneId', 'instance']);
    string(item['sceneId'], `${itemPath}.sceneId`);
    validateVoxelInstance(item['instance'], `${itemPath}.instance`);
  });
  materials.forEach((entry, index) => {
    const itemPath = `${path}.materials[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['assetId', 'definition']);
    string(item['assetId'], `${itemPath}.assetId`);
    validateMaterialDefinition(item['definition'], `${itemPath}.definition`);
  });
}

export function validateProjectMutationReceipt(input: unknown, path: string): void {
  const value = object(input, path);
  const kind = string(value['kind'], `${path}.kind`);
  const receipt = (required: readonly string[]): Readonly<Record<string, unknown>> =>
    closedObject(input, path, ['kind', ...required]);
  switch (kind) {
    case 'sceneCreated': {
      const entry = receipt(['sceneId', 'madeEntry']);
      string(entry['sceneId'], `${path}.sceneId`);
      boolean(entry['madeEntry'], `${path}.madeEntry`);
      return;
    }
    case 'sceneRenamed':
    case 'sceneDeleted':
    case 'entrySceneSet':
      string(receipt(['sceneId'])['sceneId'], `${path}.sceneId`);
      return;
    case 'sceneObjectCreated':
    case 'sceneObjectRenamed':
    case 'sceneObjectReparented':
    case 'sceneObjectTransformSet':
    case 'sceneObjectAppearanceSet':
      number(receipt(['entityId'])['entityId'], `${path}.entityId`);
      return;
    case 'sceneObjectDeleted': {
      const entry = receipt(['entityId', 'removedObjects']);
      numbers(entry, path, ['entityId', 'removedObjects']);
      return;
    }
    case 'entityCollisionSet':
    case 'entityKinematicSet': {
      const entry = receipt(['entityId', 'attached']);
      number(entry['entityId'], `${path}.entityId`);
      boolean(entry['attached'], `${path}.attached`);
      return;
    }
    case 'materialUpserted':
      string(receipt(['assetId'])['assetId'], `${path}.assetId`);
      return;
    case 'assetImportApplied': {
      const entry = receipt([
        'planId', 'planHash', 'assetId', 'sourcePath', 'reimportKind', 'generatedAssetIds',
      ]);
      strings(entry, path, ['planId', 'planHash', 'assetId', 'sourcePath', 'reimportKind']);
      array(entry['generatedAssetIds'], `${path}.generatedAssetIds`).forEach((item, index) =>
        string(item, `${path}.generatedAssetIds[${String(index)}]`));
      return;
    }
    case 'voxelAssetInitialized': {
      const entry = receipt(['assetId', 'contentHash']);
      strings(entry, path, ['assetId', 'contentHash']);
      return;
    }
    case 'voxelAssetDuplicated': {
      const entry = receipt(['sourceAssetId', 'targetAssetId', 'contentHash']);
      strings(entry, path, ['sourceAssetId', 'targetAssetId', 'contentHash']);
      return;
    }
    case 'voxelInstanceAttached':
    case 'voxelInstanceTransformSet':
    case 'voxelInstanceRemoved': {
      const entry = receipt(['sceneId', 'instanceId']);
      strings(entry, path, ['sceneId', 'instanceId']);
      return;
    }
    case 'voxelPaletteReplaced': {
      const entry = receipt([
        'assetId', 'contentHashBefore', 'contentHashAfter', 'voxelDataHash',
        'materialCountBefore', 'materialCountAfter',
      ]);
      strings(entry, path, ['assetId', 'contentHashBefore', 'contentHashAfter', 'voxelDataHash']);
      numbers(entry, path, ['materialCountBefore', 'materialCountAfter']);
      return;
    }
    case 'voxelBrushApplied': {
      const entry = receipt([
        'assetId', 'contentHashBefore', 'contentHashAfter', 'changedVoxels',
        'sourceRevision', 'historyCursor', 'undoDepth', 'redoDepth',
      ]);
      strings(entry, path, ['assetId', 'contentHashBefore', 'contentHashAfter']);
      numbers(entry, path, ['changedVoxels', 'sourceRevision', 'historyCursor', 'undoDepth', 'redoDepth']);
      return;
    }
    case 'voxelPrimitiveApplied': {
      const entry = receipt([
        'assetId', 'primitiveKind', 'contentHashBefore', 'contentHashAfter', 'changedVoxels',
        'sourceRevision', 'historyCursor', 'undoDepth', 'redoDepth',
      ]);
      strings(entry, path, ['assetId', 'primitiveKind', 'contentHashBefore', 'contentHashAfter']);
      numbers(entry, path, ['changedVoxels', 'sourceRevision', 'historyCursor', 'undoDepth', 'redoDepth']);
      return;
    }
    case 'voxelTemplateInitialized': {
      const entry = receipt([
        'assetId', 'templateKind', 'contentHash', 'changedVoxels', 'historyCursor',
      ]);
      strings(entry, path, ['assetId', 'templateKind', 'contentHash']);
      numbers(entry, path, ['changedVoxels', 'historyCursor']);
      return;
    }
    case 'voxelAssetFileImported': {
      const entry = receipt([
        'sourcePath', 'sourceSha256', 'sourceByteCount', 'sourceAssetId',
        'targetAssetId', 'contentHash',
      ]);
      strings(entry, path, [
        'sourcePath', 'sourceSha256', 'sourceAssetId', 'targetAssetId', 'contentHash',
      ]);
      number(entry['sourceByteCount'], `${path}.sourceByteCount`);
      return;
    }
    case 'environmentMaterialized': {
      const entry = receipt([
        'sceneId', 'preset', 'seed', 'assetId', 'instanceId', 'contentHash', 'voxelCount',
        'playerEntityId', 'playerTranslation', 'exitEntityId', 'exitTranslation',
        'generatorId', 'generatorVersion', 'settingsSha256', 'voxelDataSha256',
      ]);
      strings(entry, path, [
        'sceneId', 'preset', 'assetId', 'instanceId', 'contentHash', 'generatorId',
        'settingsSha256', 'voxelDataSha256',
      ]);
      numbers(entry, path, [
        'seed', 'voxelCount', 'playerEntityId', 'exitEntityId', 'generatorVersion',
      ]);
      vector3(entry['playerTranslation'], `${path}.playerTranslation`);
      vector3(entry['exitTranslation'], `${path}.exitTranslation`);
      return;
    }
    case 'voxelHistoryMoved': {
      const entry = receipt([
        'assetId', 'contentHashBefore', 'contentHashAfter', 'cursorBefore',
        'cursorAfter', 'undoDepth', 'redoDepth', 'changedVoxels',
      ]);
      strings(entry, path, ['assetId', 'contentHashBefore', 'contentHashAfter']);
      numbers(entry, path, ['cursorBefore', 'cursorAfter', 'undoDepth', 'redoDepth', 'changedVoxels']);
      return;
    }
    case 'voxelAnnotationCreated': {
      const entry = receipt(['assetId', 'layerId', 'layerHash']);
      strings(entry, path, ['assetId', 'layerId', 'layerHash']);
      return;
    }
    case 'voxelAnnotationEdited': {
      const entry = receipt([
        'assetId', 'layerId', 'layerHashBefore', 'layerHashAfter', 'affectedRegionIds',
      ]);
      strings(entry, path, ['assetId', 'layerId', 'layerHashBefore', 'layerHashAfter']);
      array(entry['affectedRegionIds'], `${path}.affectedRegionIds`).forEach((item, index) =>
        string(item, `${path}.affectedRegionIds[${String(index)}]`));
      return;
    }
    case 'voxelConversionApplied': {
      const entry = receipt(['planId', 'planHash', 'assetId', 'outputHash', 'outputVoxels']);
      strings(entry, path, ['planId', 'planHash', 'assetId', 'outputHash']);
      number(entry['outputVoxels'], `${path}.outputVoxels`);
      return;
    }
    default:
      throw new TypeError(`${path}.kind is not a closed mutation receipt`);
  }
}

export function validateVoxelPickReadout(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'sceneId', 'instanceId', 'assetId', 'hitVoxel', 'hitFace', 'placeVoxel',
    'authorityHitVoxel', 'authorityPlaceVoxel', 'instanceLocalPoint', 'worldPoint',
    'worldDistance', 'hitPreviewTransform', 'placePreviewTransform',
  ]);
  for (const field of ['sceneId', 'instanceId', 'assetId', 'hitFace']) {
    string(value[field], `${path}.${field}`);
  }
  for (const field of [
    'hitVoxel', 'placeVoxel', 'authorityHitVoxel', 'authorityPlaceVoxel',
    'instanceLocalPoint', 'worldPoint',
  ]) vector3(value[field], `${path}.${field}`);
  number(value['worldDistance'], `${path}.worldDistance`);
  for (const field of ['hitPreviewTransform', 'placePreviewTransform']) {
    const transform = closedObject(value[field], `${path}.${field}`, [
      'translation', 'rotation', 'scale',
    ]);
    vector3(transform['translation'], `${path}.${field}.translation`);
    vector4(transform['rotation'], `${path}.${field}.rotation`);
    vector3(transform['scale'], `${path}.${field}.scale`);
  }
  enumValue(value['hitFace'], `${path}.hitFace`, [
    'negativeX', 'positiveX', 'negativeY', 'positiveY', 'negativeZ', 'positiveZ',
  ]);
}

export function validateVoxelReadout(input: unknown, path: string): void {
  const value = object(input, path);
  const kind = string(value['kind'], `${path}.kind`);
  switch (kind) {
    case 'model': {
      const entry = closedObject(input, path, ['kind', 'info'], ['window']);
      validateModelInfo(entry['info'], `${path}.info`);
      if (entry['window'] !== undefined) validateModelWindow(entry['window'], `${path}.window`);
      return;
    }
    case 'annotationQuery': {
      const entry = closedObject(input, path, [
        'kind', 'layerHash', 'totalLayerRegions', 'truncated', 'matchedRegions',
      ]);
      string(entry['layerHash'], `${path}.layerHash`);
      number(entry['totalLayerRegions'], `${path}.totalLayerRegions`);
      boolean(entry['truncated'], `${path}.truncated`);
      array(entry['matchedRegions'], `${path}.matchedRegions`).forEach((region, index) =>
        validateAnnotationRegionSummary(region, `${path}.matchedRegions[${String(index)}]`));
      return;
    }
    case 'annotationExport': {
      const entry = closedObject(input, path, [
        'kind', 'layerId', 'canonicalJson', 'canonicalLayerHash', 'membershipDataHash',
      ]);
      strings(entry, path, ['layerId', 'canonicalJson', 'canonicalLayerHash', 'membershipDataHash']);
      return;
    }
    case 'history': {
      const entry = closedObject(input, path, [
        'kind', 'assetId', 'cursor', 'undoDepth', 'redoDepth', 'entryCount',
        'entriesTruncated', 'entries',
      ]);
      string(entry['assetId'], `${path}.assetId`);
      numbers(entry, path, ['cursor', 'undoDepth', 'redoDepth', 'entryCount']);
      boolean(entry['entriesTruncated'], `${path}.entriesTruncated`);
      array(entry['entries'], `${path}.entries`).forEach((historyEntry, index) =>
        validateHistoryEntry(historyEntry, `${path}.entries[${String(index)}]`));
      return;
    }
    default:
      throw new TypeError(`${path}.kind is not a closed voxel readout`);
  }
}

export function validateVoxelConversionPlan(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'planId', 'source', 'targetAssetId', 'sourcePath', 'settings', 'planner',
    'expectedSourceSha256', 'settingsSha256', 'expectedOutputContentHash',
    'planHash', 'estimatedOutputVoxels', 'estimatedBounds',
  ], ['licensePath']);
  for (const field of [
    'planId', 'targetAssetId', 'sourcePath', 'planner', 'expectedSourceSha256',
    'settingsSha256', 'expectedOutputContentHash', 'planHash',
  ]) string(value[field], `${path}.${field}`);
  if (value['licensePath'] !== undefined) string(value['licensePath'], `${path}.licensePath`);
  const source = closedObject(value['source'], `${path}.source`, [
    'assetId', 'assetVersion', 'sourceSha256',
  ], ['meshPrimitive']);
  strings(source, `${path}.source`, ['assetId', 'sourceSha256']);
  if (source['meshPrimitive'] !== undefined) {
    string(source['meshPrimitive'], `${path}.source.meshPrimitive`);
  }
  number(source['assetVersion'], `${path}.source.assetVersion`);
  number(value['estimatedOutputVoxels'], `${path}.estimatedOutputVoxels`);
  validateConversionSettings(value['settings'], `${path}.settings`);
  validateBounds(value['estimatedBounds'], `${path}.estimatedBounds`);
}

export function validateVoxelConversionPreview(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'planId', 'planHash', 'outputHash', 'outputVoxelCount', 'outputBounds',
    'sampleVoxels', 'samplesTruncated',
  ]);
  for (const field of ['planId', 'planHash', 'outputHash']) {
    string(value[field], `${path}.${field}`);
  }
  number(value['outputVoxelCount'], `${path}.outputVoxelCount`);
  validateBounds(value['outputBounds'], `${path}.outputBounds`);
  array(value['sampleVoxels'], `${path}.sampleVoxels`).forEach((sample, index) => {
    const itemPath = `${path}.sampleVoxels[${String(index)}]`;
    const item = closedObject(sample, itemPath, ['coordinate', 'materialSlot']);
    vector3(item['coordinate'], `${itemPath}.coordinate`);
    number(item['materialSlot'], `${itemPath}.materialSlot`);
  });
  boolean(value['samplesTruncated'], `${path}.samplesTruncated`);
}

export function validateVoxelHistoryRevertPreview(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'previewId', 'assetId', 'expectedProjectHash', 'expectedAssetContentHash',
    'cursorBefore', 'cursorAfter', 'undoDepthAfter', 'redoDepthAfter',
    'revisionBefore', 'revisionAfter', 'changedVoxels', 'bounds', 'materialDeltas',
    'samples', 'samplesTruncated', 'includedTransactionIds',
  ]);
  strings(value, path, [
    'previewId', 'assetId', 'expectedProjectHash', 'expectedAssetContentHash',
  ]);
  numbers(value, path, [
    'cursorBefore', 'cursorAfter', 'undoDepthAfter', 'redoDepthAfter',
    'revisionBefore', 'revisionAfter', 'changedVoxels',
  ]);
  if (value['bounds'] !== null) validateBounds(value['bounds'], `${path}.bounds`);
  array(value['materialDeltas'], `${path}.materialDeltas`).forEach((delta, index) => {
    const itemPath = `${path}.materialDeltas[${String(index)}]`;
    const item = closedObject(delta, itemPath, [
      'beforeMaterial', 'afterMaterial', 'changedVoxels',
    ]);
    nullableNumber(item['beforeMaterial'], `${itemPath}.beforeMaterial`);
    nullableNumber(item['afterMaterial'], `${itemPath}.afterMaterial`);
    number(item['changedVoxels'], `${itemPath}.changedVoxels`);
  });
  array(value['samples'], `${path}.samples`).forEach((delta, index) =>
    validateVoxelEditDelta(delta, `${path}.samples[${String(index)}]`));
  boolean(value['samplesTruncated'], `${path}.samplesTruncated`);
  array(value['includedTransactionIds'], `${path}.includedTransactionIds`).forEach((id, index) =>
    number(id, `${path}.includedTransactionIds[${String(index)}]`));
}

function validateVoxelAsset(input: unknown, path: string): void {
  const value = closedObject(input, path, ['inspection', 'palette', 'history', 'annotations']);
  const inspectionPath = `${path}.inspection`;
  const inspection = closedObject(value['inspection'], inspectionPath, [
    'assetId', 'schemaVersion', 'cellSize', 'chunkSize', 'origin', 'boundsMin',
    'boundsMax', 'representedVoxelCount', 'sparseRunCount', 'materialCounts',
    'voxelDataHash', 'contentHash', 'provenanceKind', 'provenanceSource', 'diagnostics',
  ], ['state']);
  for (const field of ['assetId', 'voxelDataHash', 'contentHash', 'provenanceKind', 'provenanceSource']) {
    string(inspection[field], `${inspectionPath}.${field}`);
  }
  for (const field of ['schemaVersion', 'cellSize', 'chunkSize', 'representedVoxelCount', 'sparseRunCount']) {
    number(inspection[field], `${inspectionPath}.${field}`);
  }
  vector3(inspection['origin'], `${inspectionPath}.origin`);
  vector3(inspection['boundsMin'], `${inspectionPath}.boundsMin`);
  vector3(inspection['boundsMax'], `${inspectionPath}.boundsMax`);
  validateMaterialCounts(inspection['materialCounts'], `${inspectionPath}.materialCounts`);
  object(inspection['diagnostics'], `${inspectionPath}.diagnostics`);
  if (inspection['state'] !== undefined) validateVoxelState(inspection['state'], `${inspectionPath}.state`);
  array(value['palette'], `${path}.palette`).forEach((binding, index) =>
    validateMaterialBinding(binding, `${path}.palette[${String(index)}]`));
  const history = closedObject(value['history'], `${path}.history`, [
    'persisted', 'entryCount', 'cursor', 'undoDepth', 'redoDepth', 'authorityHash', 'historyHash',
  ]);
  for (const field of ['entryCount', 'cursor', 'undoDepth', 'redoDepth']) {
    number(history[field], `${path}.history.${field}`);
  }
  boolean(history['persisted'], `${path}.history.persisted`);
  strings(history, `${path}.history`, ['authorityHash', 'historyHash']);
  array(value['annotations'], `${path}.annotations`).forEach((annotation, index) =>
    validateAnnotationSummary(annotation, `${path}.annotations[${String(index)}]`));
}

function validateVoxelInstance(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'instanceId', 'voxelAssetId', 'translation', 'rotation', 'scale',
  ]);
  string(value['instanceId'], `${path}.instanceId`);
  string(value['voxelAssetId'], `${path}.voxelAssetId`);
  vector3(value['translation'], `${path}.translation`);
  vector4(value['rotation'], `${path}.rotation`);
  vector3(value['scale'], `${path}.scale`);
}

function validateMaterialBinding(input: unknown, path: string): void {
  const value = closedObject(input, path, ['materialSlot', 'materialAssetId'], ['displayName']);
  number(value['materialSlot'], `${path}.materialSlot`);
  string(value['materialAssetId'], `${path}.materialAssetId`);
  if (value['displayName'] !== undefined) string(value['displayName'], `${path}.displayName`);
}

function validateMaterialDefinition(input: unknown, path: string): void {
  const value = closedObject(input, path, ['authority', 'style']);
  const authority = closedObject(value['authority'], `${path}.authority`, [
    'solid', 'collidable', 'occludes', 'structuralClass',
  ]);
  for (const field of ['solid', 'collidable', 'occludes']) boolean(authority[field], `${path}.authority.${field}`);
  enumValue(authority['structuralClass'], `${path}.authority.structuralClass`, [
    'decorative', 'solid', 'structural',
  ]);
  const style = closedObject(value['style'], `${path}.style`, [
    'color', 'texture', 'textureTint', 'emissionColor', 'roughness', 'emissive', 'uvStrategy',
  ]);
  vector4(style['color'], `${path}.style.color`);
  if (style['texture'] !== null) validateAssetReference(style['texture'], `${path}.style.texture`);
  vector4(style['textureTint'], `${path}.style.textureTint`);
  vector4(style['emissionColor'], `${path}.style.emissionColor`);
  number(style['roughness'], `${path}.style.roughness`);
  number(style['emissive'], `${path}.style.emissive`);
  enumValue(style['uvStrategy'], `${path}.style.uvStrategy`, ['flat', 'planar', 'atlas']);
}

function validateAssetReference(input: unknown, path: string): void {
  const value = closedObject(input, path, ['id', 'version', 'hash']);
  string(value['id'], `${path}.id`);
  object(value['version'], `${path}.version`);
  if (value['hash'] !== null) string(value['hash'], `${path}.hash`);
}

function validateMaterialCounts(input: unknown, path: string): void {
  array(input, path).forEach((entry, index) => {
    const itemPath = `${path}[${String(index)}]`;
    const item = closedObject(entry, itemPath, ['materialSlot', 'voxelCount']);
    numbers(item, itemPath, ['materialSlot', 'voxelCount']);
  });
}

function validateVoxelState(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'voxelSize', 'chunkSize', 'sourceRevision', 'collisionRevision', 'navigationRevision',
    'meshRevision', 'projectionsCoherent', 'authorityHash', 'solidVoxelCount',
    'residentChunkCount', 'colliderChunkCount', 'meshChunkCount', 'navigationCellCount',
    'navigationHash', 'chunks', 'diagnostics',
  ]);
  numbers(value, path, [
    'voxelSize', 'chunkSize', 'sourceRevision', 'collisionRevision',
    'navigationRevision', 'meshRevision',
  ]);
  boolean(value['projectionsCoherent'], `${path}.projectionsCoherent`);
  strings(value, path, ['authorityHash', 'navigationHash']);
  numbers(value, path, [
    'solidVoxelCount', 'residentChunkCount', 'colliderChunkCount',
    'meshChunkCount', 'navigationCellCount',
  ]);
  array(value['chunks'], `${path}.chunks`).forEach((chunk, index) => {
    const itemPath = `${path}.chunks[${String(index)}]`;
    const item = closedObject(chunk, itemPath, [
      'chunk', 'contentHash', 'materialVoxelCount', 'hasCollider', 'vertices',
      'indices', 'quads', 'facesCulled', 'materialGroupCount',
    ]);
    vector3(item['chunk'], `${itemPath}.chunk`);
    string(item['contentHash'], `${itemPath}.contentHash`);
    boolean(item['hasCollider'], `${itemPath}.hasCollider`);
    numbers(item, itemPath, [
      'materialVoxelCount', 'vertices', 'indices', 'quads',
      'facesCulled', 'materialGroupCount',
    ]);
  });
  object(value['diagnostics'], `${path}.diagnostics`);
}

function validateAnnotationSummary(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'layerId', 'canonicalLayerHash', 'membershipDataHash', 'regionCount', 'assignedCellCount',
  ]);
  strings(value, path, ['layerId', 'canonicalLayerHash', 'membershipDataHash']);
  numbers(value, path, ['regionCount', 'assignedCellCount']);
}

function validateAnnotationRegionSummary(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'regionId', 'label', 'kind', 'tags', 'bounds', 'assignedCellCount',
  ], ['parentRegionId']);
  strings(value, path, ['regionId', 'label']);
  enumValue(value['kind'], `${path}.kind`, [
    'selection', 'room', 'portal', 'spawnArea', 'cover', 'hazard', 'navigationHint', 'custom',
  ]);
  array(value['tags'], `${path}.tags`).forEach((tag, index) =>
    string(tag, `${path}.tags[${String(index)}]`));
  if (value['parentRegionId'] !== undefined) string(value['parentRegionId'], `${path}.parentRegionId`);
  validateBounds(value['bounds'], `${path}.bounds`);
  number(value['assignedCellCount'], `${path}.assignedCellCount`);
}

function validateModelInfo(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'assetId', 'contentHash', 'voxelDataHash', 'bounds', 'voxelCount',
    'sparseRunCount', 'materialCounts',
  ]);
  strings(value, path, ['assetId', 'contentHash', 'voxelDataHash']);
  validateBounds(value['bounds'], `${path}.bounds`);
  numbers(value, path, ['voxelCount', 'sparseRunCount']);
  validateMaterialCounts(value['materialCounts'], `${path}.materialCounts`);
}

function validateModelWindow(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'assetId', 'contentHash', 'requestedBounds', 'modelBounds', 'scannedCellCount',
    'samples', 'samplesTruncated',
  ]);
  strings(value, path, ['assetId', 'contentHash']);
  validateBounds(value['requestedBounds'], `${path}.requestedBounds`);
  validateBounds(value['modelBounds'], `${path}.modelBounds`);
  number(value['scannedCellCount'], `${path}.scannedCellCount`);
  array(value['samples'], `${path}.samples`).forEach((sample, index) => {
    const itemPath = `${path}.samples[${String(index)}]`;
    const item = closedObject(sample, itemPath, ['coordinate', 'materialSlot']);
    vector3(item['coordinate'], `${itemPath}.coordinate`);
    if (item['materialSlot'] !== null) number(item['materialSlot'], `${itemPath}.materialSlot`);
  });
  boolean(value['samplesTruncated'], `${path}.samplesTruncated`);
}

function validateHistoryEntry(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'transactionId', 'parentTransactionId', 'beforeHash', 'afterHash',
    'changedVoxels', 'deltasTruncated', 'deltas',
  ]);
  number(value['transactionId'], `${path}.transactionId`);
  nullableNumber(value['parentTransactionId'], `${path}.parentTransactionId`);
  strings(value, path, ['beforeHash', 'afterHash']);
  number(value['changedVoxels'], `${path}.changedVoxels`);
  boolean(value['deltasTruncated'], `${path}.deltasTruncated`);
  array(value['deltas'], `${path}.deltas`).forEach((delta, index) =>
    validateVoxelEditDelta(delta, `${path}.deltas[${String(index)}]`));
}

function validateVoxelEditDelta(input: unknown, path: string): void {
  const value = closedObject(input, path, ['address', 'beforeMaterial', 'afterMaterial']);
  vector3(value['address'], `${path}.address`);
  nullableNumber(value['beforeMaterial'], `${path}.beforeMaterial`);
  nullableNumber(value['afterMaterial'], `${path}.afterMaterial`);
}

function validateConversionSettings(input: unknown, path: string): void {
  const value = closedObject(input, path, ['conversion', 'transform', 'materialPolicy']);
  const conversion = closedObject(value['conversion'], `${path}.conversion`, [
    'resolution', 'cellSize', 'chunkSize', 'origin', 'fitPolicy', 'originPolicy',
    'mode', 'materialPalette', 'materialMap', 'maxOutputVoxels',
  ]);
  vector3(conversion['resolution'], `${path}.conversion.resolution`);
  number(conversion['cellSize'], `${path}.conversion.cellSize`);
  number(conversion['chunkSize'], `${path}.conversion.chunkSize`);
  vector3(conversion['origin'], `${path}.conversion.origin`);
  enumValue(conversion['fitPolicy'], `${path}.conversion.fitPolicy`, ['contain', 'cover', 'stretch']);
  enumValue(conversion['originPolicy'], `${path}.conversion.originPolicy`, [
    'sourceOrigin', 'targetMin', 'centered',
  ]);
  enumValue(conversion['mode'], `${path}.conversion.mode`, ['surface', 'solid']);
  array(conversion['materialPalette'], `${path}.conversion.materialPalette`).forEach((binding, index) =>
    validateMaterialBinding(binding, `${path}.conversion.materialPalette[${String(index)}]`));
  array(conversion['materialMap'], `${path}.conversion.materialMap`).forEach((mapping, index) => {
    const itemPath = `${path}.conversion.materialMap[${String(index)}]`;
    const item = closedObject(mapping, itemPath, [
      'sourceMaterialSlot', 'voxelMaterialSlot',
    ], ['sourceMaterialName']);
    numbers(item, itemPath, ['sourceMaterialSlot', 'voxelMaterialSlot']);
    if (item['sourceMaterialName'] !== undefined) string(item['sourceMaterialName'], `${itemPath}.sourceMaterialName`);
  });
  number(conversion['maxOutputVoxels'], `${path}.conversion.maxOutputVoxels`);
  const transform = array(value['transform'], `${path}.transform`);
  if (transform.length !== 16) throw new TypeError(`${path}.transform must have 16 entries`);
  transform.forEach((entry, index) => number(entry, `${path}.transform[${String(index)}]`));
  const policy = closedObject(value['materialPolicy'], `${path}.materialPolicy`, [
    'textureAssets', 'textureBindings',
  ], ['defaultVoxelMaterial']);
  array(policy['textureAssets'], `${path}.materialPolicy.textureAssets`).forEach((entry, index) =>
    validateTextureSampleAsset(entry, `${path}.materialPolicy.textureAssets[${String(index)}]`));
  array(policy['textureBindings'], `${path}.materialPolicy.textureBindings`).forEach((entry, index) =>
    validateTextureMaterialBinding(entry, `${path}.materialPolicy.textureBindings[${String(index)}]`));
  if (policy['defaultVoxelMaterial'] !== undefined) {
    number(policy['defaultVoxelMaterial'], `${path}.materialPolicy.defaultVoxelMaterial`);
  }
}

function validateTextureSourceRef(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'textureAssetId', 'assetVersion', 'contentHash', 'width', 'height',
    'colorSpace', 'channelLayout',
  ]);
  strings(value, path, ['textureAssetId', 'contentHash']);
  numbers(value, path, ['assetVersion', 'width', 'height']);
  enumValue(value['colorSpace'], `${path}.colorSpace`, ['linear', 'srgb']);
  enumValue(value['channelLayout'], `${path}.channelLayout`, ['palette_index_u16']);
}

function validateTextureSampleAsset(input: unknown, path: string): void {
  const value = closedObject(input, path, ['texture', 'texelMaterials']);
  validateTextureSourceRef(value['texture'], `${path}.texture`);
  array(value['texelMaterials'], `${path}.texelMaterials`).forEach((material, index) =>
    number(material, `${path}.texelMaterials[${String(index)}]`));
}

function validateTextureMaterialBinding(input: unknown, path: string): void {
  const value = closedObject(input, path, [
    'sourceMaterialSlot', 'texture', 'uvAttribute', 'sampleUv', 'samplingPolicy',
    'wrapPolicy', 'materialMode',
  ]);
  number(value['sourceMaterialSlot'], `${path}.sourceMaterialSlot`);
  validateTextureSourceRef(value['texture'], `${path}.texture`);
  const uv = closedObject(value['uvAttribute'], `${path}.uvAttribute`, [
    'attributeName', 'sourceHash',
  ]);
  strings(uv, `${path}.uvAttribute`, ['attributeName', 'sourceHash']);
  const sampleUv = array(value['sampleUv'], `${path}.sampleUv`);
  if (sampleUv.length !== 2) throw new TypeError(`${path}.sampleUv must have 2 entries`);
  sampleUv.forEach((coordinate, index) =>
    number(coordinate, `${path}.sampleUv[${String(index)}]`));
  enumValue(value['samplingPolicy'], `${path}.samplingPolicy`, ['nearest_texel']);
  enumValue(value['wrapPolicy'], `${path}.wrapPolicy`, ['clamp_to_edge']);
  enumValue(value['materialMode'], `${path}.materialMode`, ['sample_palette_index']);
}

function validateBounds(input: unknown, path: string): void {
  const value = closedObject(input, path, ['min', 'max']);
  vector3(value['min'], `${path}.min`);
  vector3(value['max'], `${path}.max`);
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

function number(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    throw new TypeError(`${path} must be a finite number`);
  }
  return input;
}

function nullableNumber(input: unknown, path: string): void {
  if (input !== null) number(input, path);
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

function numbers(
  value: Readonly<Record<string, unknown>>,
  path: string,
  fields: readonly string[],
): void {
  for (const field of fields) number(value[field], `${path}.${field}`);
}

function enumValue(input: unknown, path: string, allowed: readonly string[]): void {
  const value = string(input, path);
  if (!allowed.includes(value)) throw new TypeError(`${path} is not a closed value`);
}

function vector3(input: unknown, path: string): void {
  const values = array(input, path);
  if (values.length !== 3) throw new TypeError(`${path} must have 3 entries`);
  values.forEach((entry, index) => number(entry, `${path}[${String(index)}]`));
}

function vector4(input: unknown, path: string): void {
  const values = array(input, path);
  if (values.length !== 4) throw new TypeError(`${path} must have 4 entries`);
  values.forEach((entry, index) => number(entry, `${path}[${String(index)}]`));
}
