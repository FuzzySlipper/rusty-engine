import type {
  Quaternion,
  StoredMaterialDefinition,
  StoredVoxelInstance,
  Vector3,
  Vector3i,
  VoxelAnnotationEditTransaction,
  VoxelAnnotationLayerDraft,
  VoxelAnnotationQuery,
  VoxelAssetAuthoringReadout,
  VoxelBounds,
  VoxelBrushMode,
  VoxelConversionSettings,
  StudioFileSelection,
  VoxelMaterialBinding,
  VoxelModelWindowRequest,
  VoxelPickFace,
  VoxelPrimitiveRequest,
  VoxelTemplateRequest,
} from '@rusty-engine/studio-adapter-client';

export interface VoxelViewportPickCandidate {
  readonly instanceId: string;
  readonly cameraOrigin: Vector3;
  readonly direction: Vector3;
  readonly worldPoint: Vector3;
  readonly worldNormal: Vector3;
  readonly maxDistance: number;
}

export interface VoxelBrushPreviewPresentation {
  readonly kind: 'brush';
  readonly worldPoint: Vector3;
  readonly cellSize: number;
  readonly radius: number;
  readonly mode: VoxelBrushMode;
}

export interface VoxelPickValidationInput {
  readonly sceneId: string;
  readonly instanceId: string;
  readonly origin: Vector3;
  readonly direction: Vector3;
  readonly maxDistance: number;
  readonly claimedVoxel: Vector3i;
  readonly claimedFace: VoxelPickFace;
}

export type VoxelEditorAction =
  | { readonly kind: 'upsertMaterial'; readonly assetId: string; readonly definition: StoredMaterialDefinition }
  | { readonly kind: 'initializeAsset'; readonly assetId: string; readonly cellSize: number; readonly chunkSize: number; readonly origin: Vector3i; readonly bounds: VoxelBounds; readonly materialPalette: readonly VoxelMaterialBinding[]; readonly initialMaterialSlot: number }
  | { readonly kind: 'duplicateAsset'; readonly sourceAssetId: string; readonly expectedSourceContentHash: string; readonly targetAssetId: string }
  | { readonly kind: 'attachInstance'; readonly sceneId: string; readonly instance: StoredVoxelInstance }
  | { readonly kind: 'setInstanceTransform'; readonly sceneId: string; readonly instanceId: string; readonly translation: Vector3; readonly rotation: Quaternion; readonly scale: Vector3 }
  | { readonly kind: 'removeInstance'; readonly sceneId: string; readonly instanceId: string }
  | { readonly kind: 'replacePalette'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly expectedVoxelDataHash: string; readonly replacement: readonly VoxelMaterialBinding[] }
  | { readonly kind: 'applyBrush'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly center: Vector3i; readonly radius: number; readonly mode: VoxelBrushMode; readonly materialSlot: number | null }
  | { readonly kind: 'applyPrimitive'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly request: VoxelPrimitiveRequest }
  | { readonly kind: 'initializeTemplate'; readonly assetId: string; readonly cellSize: number; readonly chunkSize: number; readonly materialPalette: readonly VoxelMaterialBinding[]; readonly request: VoxelTemplateRequest }
  | { readonly kind: 'importAssetFile'; readonly sourcePath: string; readonly targetAssetId: string }
  | { readonly kind: 'exportAssetFile'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly targetPath: string; readonly expectedTargetSha256?: string }
  | { readonly kind: 'materializeEnvironment'; readonly sceneId: string; readonly preset: 'tinyEnclosed'; readonly seed: number; readonly voxelAssetId: string; readonly voxelInstanceId: string; readonly voxelTranslation: Vector3; readonly playerEntityId: number; readonly exitEntityId: number; readonly wallMaterial: number; readonly floorMaterial: number; readonly accentMaterial: number; readonly materialPalette: readonly VoxelMaterialBinding[] }
  | { readonly kind: 'undo'; readonly assetId: string; readonly expectedAssetContentHash: string }
  | { readonly kind: 'redo'; readonly assetId: string; readonly expectedAssetContentHash: string }
  | { readonly kind: 'revert'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly targetCursor: number }
  | { readonly kind: 'queryHistory'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly maxEntries: number; readonly maxDeltasPerEntry: number }
  | { readonly kind: 'prepareHistoryRevert'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly targetCursor: number; readonly maxSamples: number }
  | { readonly kind: 'applyHistoryRevert'; readonly previewId: string }
  | { readonly kind: 'discardHistoryRevert'; readonly previewId: string }
  | { readonly kind: 'createAnnotation'; readonly assetId: string; readonly draft: VoxelAnnotationLayerDraft }
  | { readonly kind: 'editAnnotation'; readonly assetId: string; readonly layerId: string; readonly transaction: VoxelAnnotationEditTransaction }
  | { readonly kind: 'queryAnnotation'; readonly assetId: string; readonly layerId: string; readonly query: VoxelAnnotationQuery }
  | { readonly kind: 'exportAnnotation'; readonly assetId: string; readonly layerId: string; readonly expectedLayerHash: string }
  | { readonly kind: 'queryModel'; readonly assetId: string; readonly expectedAssetContentHash: string; readonly window?: VoxelModelWindowRequest }
  | { readonly kind: 'prepareConversion'; readonly sourceAssetId: string; readonly source: StudioFileSelection; readonly targetAssetId: string; readonly license?: StudioFileSelection; readonly meshPrimitive?: string; readonly settings: VoxelConversionSettings; readonly maxPreviewSamples: number }
  | { readonly kind: 'applyConversion'; readonly planId: string; readonly expectedPlanHash: string; readonly expectedOutputHash: string }
  | { readonly kind: 'discardConversion'; readonly planId: string };

/**
 * Converts a renderer hit into an explicitly untrusted authored-cell claim.
 * Rust re-runs transformed-instance picking before any edit may use it.
 */
export function deriveVoxelPickValidation(
  candidate: VoxelViewportPickCandidate,
  sceneId: string,
  instance: StoredVoxelInstance,
  asset: VoxelAssetAuthoringReadout,
): VoxelPickValidationInput | null {
  const point = inversePoint(candidate.worldPoint, instance);
  const normal = inverseNormal(candidate.worldNormal, instance);
  if (point === null || normal === null) return null;
  const cellSize = asset.inspection.cellSize;
  if (!Number.isFinite(cellSize) || cellSize <= 0) return null;
  const epsilon = Math.max(1e-7, cellSize * 1e-6);
  const authority: Vector3i = [
    Math.floor((point[0] - normal[0] * epsilon) / cellSize),
    Math.floor((point[1] - normal[1] * epsilon) / cellSize),
    Math.floor((point[2] - normal[2] * epsilon) / cellSize),
  ];
  const origin = asset.inspection.origin;
  const claimedVoxel: Vector3i = [
    authority[0] - origin[0],
    authority[1] - origin[1],
    authority[2] - origin[2],
  ];
  const claimedFace = dominantFace(normal);
  if (claimedFace === null) return null;
  return {
    sceneId,
    instanceId: instance.instanceId,
    origin: candidate.cameraOrigin,
    direction: candidate.direction,
    maxDistance: candidate.maxDistance,
    claimedVoxel,
    claimedFace,
  };
}

function inversePoint(point: Vector3, instance: StoredVoxelInstance): Vector3 | null {
  const translated: Vector3 = [
    point[0] - instance.translation[0],
    point[1] - instance.translation[1],
    point[2] - instance.translation[2],
  ];
  const rotated = rotateByInverse(translated, instance.rotation);
  const result: Vector3 = [
    rotated[0] / instance.scale[0],
    rotated[1] / instance.scale[1],
    rotated[2] / instance.scale[2],
  ];
  return result.every(Number.isFinite) ? result : null;
}

function inverseNormal(normal: Vector3, instance: StoredVoxelInstance): Vector3 | null {
  const rotated = rotateByInverse(normal, instance.rotation);
  return normalize([
    rotated[0] * instance.scale[0],
    rotated[1] * instance.scale[1],
    rotated[2] * instance.scale[2],
  ]);
}

function rotateByInverse(vector: Vector3, rotation: Quaternion): Vector3 {
  return rotate(vector, [-rotation[0], -rotation[1], -rotation[2], rotation[3]]);
}

function rotate(vector: Vector3, rotation: Quaternion): Vector3 {
  const [x, y, z, w] = rotation;
  const tx = 2 * (y * vector[2] - z * vector[1]);
  const ty = 2 * (z * vector[0] - x * vector[2]);
  const tz = 2 * (x * vector[1] - y * vector[0]);
  return [
    vector[0] + w * tx + (y * tz - z * ty),
    vector[1] + w * ty + (z * tx - x * tz),
    vector[2] + w * tz + (x * ty - y * tx),
  ];
}

function normalize(vector: Vector3): Vector3 | null {
  const length = Math.hypot(...vector);
  if (!Number.isFinite(length) || length <= Number.EPSILON) return null;
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function dominantFace(normal: Vector3): VoxelPickFace | null {
  const magnitudes = normal.map(Math.abs);
  const maximum = Math.max(...magnitudes);
  if (!Number.isFinite(maximum) || maximum <= Number.EPSILON) return null;
  const axis = magnitudes.indexOf(maximum);
  if (axis === 0) return normal[0] < 0 ? 'negativeX' : 'positiveX';
  if (axis === 1) return normal[1] < 0 ? 'negativeY' : 'positiveY';
  return normal[2] < 0 ? 'negativeZ' : 'positiveZ';
}
