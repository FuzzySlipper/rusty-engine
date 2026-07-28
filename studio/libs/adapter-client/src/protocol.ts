import {
  decodeRenderFrameDiff,
  type RenderFrameDiff,
  type Transform,
} from '@rusty-engine/render-contracts';
import {
  validateProjectMutationReceipt,
  validateVoxelAuthoringReadout,
  validateVoxelConversionPlan,
  validateVoxelConversionPreview,
  validateVoxelHistoryRevertPreview,
  validateVoxelPickReadout,
  validateVoxelReadout,
  type ProjectMutationReceipt,
  type Quaternion,
  type StoredMaterialDefinition,
  type StoredVoxelInstance,
  type Vector3,
  type Vector3i,
  type VoxelAnnotationEditTransaction,
  type VoxelAnnotationLayerDraft,
  type VoxelAnnotationQuery,
  type VoxelAuthoringReadout,
  type VoxelBounds,
  type VoxelBrushMode,
  type VoxelConversionPlan,
  type VoxelConversionPreview,
  type VoxelConversionSettings,
  type VoxelMaterialBinding,
  type VoxelModelWindowRequest,
  type VoxelPickFace,
  type VoxelPickReadout,
  type VoxelPrimitiveRequest,
  type VoxelReadout,
  type VoxelTemplateRequest,
  type VoxelHistoryRevertPreview,
} from './voxel-protocol.js';
import {
  validateVoxelObjectAuthoringReadout,
  validateVoxelObjectConversionPlan,
  validateVoxelObjectConversionPreview,
  validateVoxelObjectInstancePlaybackReadout,
  validateVoxelObjectSourceInspection,
  type StoredVoxelObjectInstance,
  type VoxelObjectAuthoringReadout,
  type VoxelObjectClipConversionRequest,
  type VoxelObjectConversionPlan,
  type VoxelObjectConversionPreview,
  type VoxelObjectConversionSettings,
  type VoxelObjectFrameSelection,
  type VoxelObjectInstancePlaybackReadout,
  type VoxelObjectPlaybackCommand,
  type VoxelObjectSourceInspection,
  type VoxelObjectSourceKind,
} from './voxel-object-protocol.js';

export type * from './voxel-protocol.js';
export type * from './voxel-object-protocol.js';

export const STUDIO_ADAPTER_PROTOCOL_VERSION = 9 as const;
// Requests remain compact control-plane commands. Responses include complete
// retained-frame readouts; 64 MiB admits the checked 96x144x96 voxel-object
// corpus while retaining a finite host/browser liveness guard.
export const MAX_STUDIO_ADAPTER_REQUEST_BYTES = 256 * 1024;
export const MAX_STUDIO_ADAPTER_RESPONSE_BYTES = 64 * 1024 * 1024;

export type StudioAdapterRequest =
  | DescribeRequest
  | OpenProjectRequest
  | CreateProjectRequest
  | SaveProjectAsRequest
  | ReadProjectRequest
  | CreateSceneRequest
  | RenameSceneRequest
  | DeleteSceneRequest
  | SetEntrySceneRequest
  | CreateSceneObjectRequest
  | DeleteSceneObjectRequest
  | RenameSceneObjectRequest
  | ReparentSceneObjectRequest
  | SetSceneObjectTransformRequest
  | SetSceneObjectAppearanceRequest
  | SetEntityCollisionRequest
  | SetEntityKinematicRequest
  | SetEntityTranslationRequest
  | UpsertMaterialRequest
  | PrepareAssetImportRequest
  | PrepareAssetReimportRequest
  | ApplyAssetImportRequest
  | DiscardAssetImportRequest
  | InitializeVoxelAssetRequest
  | DuplicateVoxelAssetRequest
  | AttachVoxelInstanceRequest
  | SetVoxelInstanceTransformRequest
  | RemoveVoxelInstanceRequest
  | ReplaceVoxelPaletteRequest
  | ValidateVoxelPickRequest
  | ApplyVoxelBrushRequest
  | ApplyVoxelPrimitiveRequest
  | InitializeVoxelTemplateRequest
  | ImportVoxelAssetFileRequest
  | ExportVoxelAssetFileRequest
  | MaterializeEnvironmentRequest
  | UndoVoxelEditRequest
  | RedoVoxelEditRequest
  | RevertVoxelHistoryRequest
  | QueryVoxelHistoryRequest
  | PrepareVoxelHistoryRevertRequest
  | ApplyVoxelHistoryRevertRequest
  | DiscardVoxelHistoryRevertRequest
  | CreateVoxelAnnotationLayerRequest
  | EditVoxelAnnotationRequest
  | QueryVoxelAnnotationRequest
  | ExportVoxelAnnotationRequest
  | QueryVoxelModelRequest
  | PrepareVoxelConversionRequest
  | ApplyVoxelConversionRequest
  | DiscardVoxelConversionRequest
  | InspectVoxelObjectSourceRequest
  | PrepareVoxelObjectConversionRequest
  | PreviewVoxelObjectConversionRequest
  | ApplyVoxelObjectConversionRequest
  | DiscardVoxelObjectConversionRequest
  | AttachVoxelObjectInstanceRequest
  | PreviewVoxelObjectInstanceRequest
  | CloseProjectRequest;

interface RequestHeader {
  readonly protocolVersion: typeof STUDIO_ADAPTER_PROTOCOL_VERSION;
  readonly requestId: string;
}

export interface DescribeRequest extends RequestHeader {
  readonly type: 'describe';
}

export interface OpenProjectRequest extends RequestHeader {
  readonly type: 'openProject';
  readonly root: string;
  readonly projectFile: string;
}

export interface CreateProjectRequest extends RequestHeader {
  readonly type: 'createProject';
  readonly root: string;
  readonly projectFile: string;
  readonly projectId: string;
  readonly name: string;
  readonly entryScene: string;
  readonly entrySceneName: string;
}

export interface SaveProjectAsRequest extends RequestHeader {
  readonly type: 'saveProjectAs';
  readonly expectedProjectHash: string;
  readonly root: string;
  readonly projectFile: string;
  readonly projectId: string;
  readonly name: string;
}

export interface ReadProjectRequest extends RequestHeader {
  readonly type: 'readProject';
}

export interface CreateSceneRequest extends RequestHeader {
  readonly type: 'createScene';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly name: string;
  readonly makeEntry: boolean;
}

export interface RenameSceneRequest extends RequestHeader {
  readonly type: 'renameScene';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly name: string;
}

export interface DeleteSceneRequest extends RequestHeader {
  readonly type: 'deleteScene';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
}

export interface SetEntrySceneRequest extends RequestHeader {
  readonly type: 'setEntryScene';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
}

export interface CreateSceneObjectRequest extends RequestHeader {
  readonly type: 'createSceneObject';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly object: StudioSceneObjectDraft;
}

export interface DeleteSceneObjectRequest extends RequestHeader {
  readonly type: 'deleteSceneObject';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
}

export interface RenameSceneObjectRequest extends RequestHeader {
  readonly type: 'renameSceneObject';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly name: string;
}

export interface ReparentSceneObjectRequest extends RequestHeader {
  readonly type: 'reparentSceneObject';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly parentEntityId: number | null;
  readonly childOrder: number;
}

export interface SetSceneObjectTransformRequest extends RequestHeader {
  readonly type: 'setSceneObjectTransform';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly transform: Transform;
}

export interface SetSceneObjectAppearanceRequest extends RequestHeader {
  readonly type: 'setSceneObjectAppearance';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly appearance: StudioSceneAppearance;
}

export interface SetEntityCollisionRequest extends RequestHeader {
  readonly type: 'setEntityCollision';
  readonly expectedProjectHash: string;
  readonly entityId: number;
  readonly collision: StoredCollision | null;
}

export interface SetEntityKinematicRequest extends RequestHeader {
  readonly type: 'setEntityKinematic';
  readonly expectedProjectHash: string;
  readonly entityId: number;
  readonly kinematic: StoredKinematic | null;
}

export interface StudioSceneObjectDraft {
  readonly entityId: number;
  readonly name: string;
  readonly parentEntityId: number | null;
  readonly childOrder: number;
  readonly transform: Transform;
  readonly appearance: StudioSceneAppearance;
  readonly collision: StoredCollision | null;
  readonly kinematic: StoredKinematic | null;
}

export type StudioSceneAppearance =
  | { readonly kind: 'empty' }
  | { readonly kind: 'staticMesh'; readonly asset: string; readonly visible: boolean }
  | {
      readonly kind: 'animatedMesh';
      readonly asset: string;
      readonly visible: boolean;
      readonly clip: string;
    }
  | { readonly kind: 'light'; readonly light: StoredLight };

export type StoredLight =
  | StoredBaseLight<'ambient'>
  | StoredBaseLight<'directional'>
  | (StoredBaseLight<'point'> & {
      readonly range: number | null;
      readonly decay: number;
    })
  | (StoredBaseLight<'spot'> & {
      readonly range: number | null;
      readonly decay: number;
      readonly outerAngleRadians: number;
      readonly penumbra: number;
    });

interface StoredBaseLight<Kind extends string> {
  readonly kind: Kind;
  readonly color: readonly [number, number, number];
  readonly intensity: number;
  readonly enabled: boolean;
  readonly shadows: boolean;
}

export interface StoredCollision {
  readonly enabled: boolean;
  readonly staticCollider: boolean;
}

export interface StoredKinematic {
  readonly halfExtents: readonly [number, number, number];
  readonly velocity: readonly [number, number, number];
}

export interface SetEntityTranslationRequest extends RequestHeader {
  readonly type: 'setEntityTranslation';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly translation: readonly [number, number, number];
}

export interface UpsertMaterialRequest extends RequestHeader {
  readonly type: 'upsertMaterial';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly definition: StoredMaterialDefinition;
}

export interface PrepareAssetImportRequest extends RequestHeader {
  readonly type: 'prepareAssetImport';
  readonly expectedProjectHash: string;
  readonly source: StudioFileSelection;
  readonly settings: StudioAssetImportSettings;
}

export interface PrepareAssetReimportRequest extends RequestHeader {
  readonly type: 'prepareAssetReimport';
  readonly expectedProjectHash: string;
  readonly assetId: string;
}

export interface ApplyAssetImportRequest extends RequestHeader {
  readonly type: 'applyAssetImport';
  readonly expectedProjectHash: string;
  readonly planId: string;
  readonly expectedPlanHash: string;
}

export interface DiscardAssetImportRequest extends RequestHeader {
  readonly type: 'discardAssetImport';
  readonly planId: string;
}

export interface StudioAssetImportSettings {
  readonly scale: number;
  readonly generateCollision: boolean;
  readonly materialNamespace: string | null;
}

export interface InitializeVoxelAssetRequest extends RequestHeader {
  readonly type: 'initializeVoxelAsset';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly cellSize: number;
  readonly chunkSize: number;
  readonly origin: Vector3i;
  readonly bounds: VoxelBounds;
  readonly materialPalette: readonly VoxelMaterialBinding[];
  readonly initialMaterialSlot: number;
}

export interface DuplicateVoxelAssetRequest extends RequestHeader {
  readonly type: 'duplicateVoxelAsset';
  readonly expectedProjectHash: string;
  readonly sourceAssetId: string;
  readonly expectedSourceContentHash: string;
  readonly targetAssetId: string;
}

export interface AttachVoxelInstanceRequest extends RequestHeader {
  readonly type: 'attachVoxelInstance';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instance: StoredVoxelInstance;
}

export interface SetVoxelInstanceTransformRequest extends RequestHeader {
  readonly type: 'setVoxelInstanceTransform';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instanceId: string;
  readonly translation: Vector3;
  readonly rotation: Quaternion;
  readonly scale: Vector3;
}

export interface RemoveVoxelInstanceRequest extends RequestHeader {
  readonly type: 'removeVoxelInstance';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instanceId: string;
}

export interface ReplaceVoxelPaletteRequest extends RequestHeader {
  readonly type: 'replaceVoxelPalette';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly expectedAssetContentHash: string;
  readonly expectedVoxelDataHash: string;
  readonly replacement: readonly VoxelMaterialBinding[];
}

export interface ValidateVoxelPickRequest extends RequestHeader {
  readonly type: 'validateVoxelPick';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instanceId: string;
  readonly origin: Vector3;
  readonly direction: Vector3;
  readonly maxDistance: number;
  readonly claimedVoxel: Vector3i;
  readonly claimedFace: VoxelPickFace;
}

export interface ApplyVoxelBrushRequest extends RequestHeader {
  readonly type: 'applyVoxelBrush';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly expectedAssetContentHash: string;
  readonly center: Vector3i;
  readonly radius: number;
  readonly mode: VoxelBrushMode;
  readonly materialSlot: number | null;
}

export interface ApplyVoxelPrimitiveRequest extends VoxelHistoryRequest {
  readonly type: 'applyVoxelPrimitive';
  readonly request: VoxelPrimitiveRequest;
}

export interface InitializeVoxelTemplateRequest extends RequestHeader {
  readonly type: 'initializeVoxelTemplate';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly cellSize: number;
  readonly chunkSize: number;
  readonly materialPalette: readonly VoxelMaterialBinding[];
  readonly request: VoxelTemplateRequest;
}

export interface ImportVoxelAssetFileRequest extends RequestHeader {
  readonly type: 'importVoxelAssetFile';
  readonly expectedProjectHash: string;
  readonly sourcePath: string;
  readonly targetAssetId: string;
}

export interface ExportVoxelAssetFileRequest extends VoxelHistoryRequest {
  readonly type: 'exportVoxelAssetFile';
  readonly targetPath: string;
  readonly expectedTargetSha256?: string;
}

export interface MaterializeEnvironmentRequest extends RequestHeader {
  readonly type: 'materializeEnvironment';
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly sceneId: string;
  readonly preset: 'tinyEnclosed';
  readonly seed: number;
  readonly voxelAssetId: string;
  readonly voxelInstanceId: string;
  readonly voxelTranslation: Vector3;
  readonly playerEntityId: number;
  readonly exitEntityId: number;
  readonly wallMaterial: number;
  readonly floorMaterial: number;
  readonly accentMaterial: number;
  readonly materialPalette: readonly VoxelMaterialBinding[];
}

interface VoxelHistoryRequest extends RequestHeader {
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly expectedAssetContentHash: string;
}

export interface UndoVoxelEditRequest extends VoxelHistoryRequest {
  readonly type: 'undoVoxelEdit';
}

export interface RedoVoxelEditRequest extends VoxelHistoryRequest {
  readonly type: 'redoVoxelEdit';
}

export interface RevertVoxelHistoryRequest extends VoxelHistoryRequest {
  readonly type: 'revertVoxelHistory';
  readonly targetCursor: number;
}

export interface QueryVoxelHistoryRequest extends VoxelHistoryRequest {
  readonly type: 'queryVoxelHistory';
  readonly maxEntries: number;
  readonly maxDeltasPerEntry: number;
}

export interface PrepareVoxelHistoryRevertRequest extends VoxelHistoryRequest {
  readonly type: 'prepareVoxelHistoryRevert';
  readonly targetCursor: number;
  readonly maxSamples: number;
}

export interface ApplyVoxelHistoryRevertRequest extends RequestHeader {
  readonly type: 'applyVoxelHistoryRevert';
  readonly expectedProjectHash: string;
  readonly previewId: string;
}

export interface DiscardVoxelHistoryRevertRequest extends RequestHeader {
  readonly type: 'discardVoxelHistoryRevert';
  readonly previewId: string;
}

export interface CreateVoxelAnnotationLayerRequest extends RequestHeader {
  readonly type: 'createVoxelAnnotationLayer';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly draft: VoxelAnnotationLayerDraft;
}

export interface EditVoxelAnnotationRequest extends RequestHeader {
  readonly type: 'editVoxelAnnotation';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly layerId: string;
  readonly transaction: VoxelAnnotationEditTransaction;
}

export interface QueryVoxelAnnotationRequest extends RequestHeader {
  readonly type: 'queryVoxelAnnotation';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly layerId: string;
  readonly query: VoxelAnnotationQuery;
}

export interface ExportVoxelAnnotationRequest extends RequestHeader {
  readonly type: 'exportVoxelAnnotation';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly layerId: string;
  readonly expectedLayerHash: string;
}

export interface QueryVoxelModelRequest extends RequestHeader {
  readonly type: 'queryVoxelModel';
  readonly expectedProjectHash: string;
  readonly assetId: string;
  readonly expectedAssetContentHash: string;
  readonly window?: VoxelModelWindowRequest;
}

export interface PrepareVoxelConversionRequest extends RequestHeader {
  readonly type: 'prepareVoxelConversion';
  readonly expectedProjectHash: string;
  readonly sourceAssetId: string;
  readonly source: StudioFileSelection;
  readonly targetAssetId: string;
  readonly license?: StudioFileSelection;
  readonly meshPrimitive?: string;
  readonly settings: VoxelConversionSettings;
  readonly maxPreviewSamples: number;
}

export type StudioFileSelection =
  | { readonly scope: 'project'; readonly path: string }
  | { readonly scope: 'host'; readonly path: string };

export interface ApplyVoxelConversionRequest extends RequestHeader {
  readonly type: 'applyVoxelConversion';
  readonly expectedProjectHash: string;
  readonly planId: string;
  readonly expectedPlanHash: string;
  readonly expectedOutputHash: string;
}

export interface DiscardVoxelConversionRequest extends RequestHeader {
  readonly type: 'discardVoxelConversion';
  readonly planId: string;
}

export interface InspectVoxelObjectSourceRequest extends RequestHeader {
  readonly type: 'inspectVoxelObjectSource';
  readonly expectedProjectHash: string;
  readonly sourceKind: VoxelObjectSourceKind;
  readonly sourceAssetId: string;
  readonly source: StudioFileSelection;
  readonly meshPrimitive?: string;
}

export interface PrepareVoxelObjectConversionRequest extends RequestHeader {
  readonly type: 'prepareVoxelObjectConversion';
  readonly expectedProjectHash: string;
  readonly sourceKind: VoxelObjectSourceKind;
  readonly sourceAssetId: string;
  readonly source: StudioFileSelection;
  readonly targetAssetId: string;
  readonly license?: StudioFileSelection;
  readonly meshPrimitive?: string;
  readonly settings: VoxelObjectConversionSettings;
  readonly clips: readonly VoxelObjectClipConversionRequest[];
  readonly defaultClip?: string;
  readonly frame: VoxelObjectFrameSelection;
  readonly maxPreviewSamples: number;
}

export interface PreviewVoxelObjectConversionRequest extends RequestHeader {
  readonly type: 'previewVoxelObjectConversion';
  readonly planId: string;
  readonly expectedPlanHash: string;
  readonly frame: VoxelObjectFrameSelection;
  readonly maxPreviewSamples: number;
}

export interface ApplyVoxelObjectConversionRequest extends RequestHeader {
  readonly type: 'applyVoxelObjectConversion';
  readonly expectedProjectHash: string;
  readonly planId: string;
  readonly expectedPlanHash: string;
  readonly expectedOutputHash: string;
}

export interface DiscardVoxelObjectConversionRequest extends RequestHeader {
  readonly type: 'discardVoxelObjectConversion';
  readonly planId: string;
}

export interface AttachVoxelObjectInstanceRequest extends RequestHeader {
  readonly type: 'attachVoxelObjectInstance';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instance: StoredVoxelObjectInstance;
}

export interface PreviewVoxelObjectInstanceRequest extends RequestHeader {
  readonly type: 'previewVoxelObjectInstance';
  readonly expectedProjectHash: string;
  readonly sceneId: string;
  readonly instanceId: string;
  readonly nowMicroseconds: number;
  readonly command: VoxelObjectPlaybackCommand;
}

export interface CloseProjectRequest extends RequestHeader {
  readonly type: 'closeProject';
}

export type StudioAdapterResponse =
  | DescribedResponse
  | ProjectOpenedResponse
  | ProjectCreatedResponse
  | ProjectSavedAsResponse
  | ProjectReadResponse
  | EntityTranslationAppliedResponse
  | ProjectMutationAppliedResponse
  | VoxelPickValidatedResponse
  | VoxelReadResponse
  | VoxelConversionPreparedResponse
  | VoxelConversionDiscardedResponse
  | VoxelObjectSourceInspectedResponse
  | VoxelObjectConversionPreparedResponse
  | VoxelObjectConversionPreviewedResponse
  | VoxelObjectConversionDiscardedResponse
  | VoxelObjectInstancePreviewedResponse
  | AssetImportPreparedResponse
  | AssetImportDiscardedResponse
  | VoxelHistoryRevertPreparedResponse
  | VoxelHistoryRevertDiscardedResponse
  | VoxelAssetFileExportedResponse
  | ProjectClosedResponse
  | RejectedResponse;

export interface DescribedResponse extends ResponseHeader {
  readonly type: 'described';
  readonly adapter: AdapterDescription;
}

export interface ProjectOpenedResponse extends ResponseHeader {
  readonly type: 'projectOpened';
  readonly project: StudioProjectReadout;
}

export interface ProjectCreatedResponse extends ResponseHeader {
  readonly type: 'projectCreated';
  readonly project: StudioProjectReadout;
}

export interface ProjectSavedAsResponse extends ResponseHeader {
  readonly type: 'projectSavedAs';
  readonly project: StudioProjectReadout;
}

export interface ProjectReadResponse extends ResponseHeader {
  readonly type: 'projectRead';
  readonly project: StudioProjectReadout;
}

export interface EntityTranslationAppliedResponse extends ResponseHeader {
  readonly type: 'entityTranslationApplied';
  readonly receipt: EntityTranslationReceipt;
  readonly project: StudioProjectReadout;
}

export interface ProjectMutationAppliedResponse extends ResponseHeader {
  readonly type: 'projectMutationApplied';
  readonly receipt: ProjectMutationReceipt;
  readonly project: StudioProjectReadout;
}

export interface VoxelPickValidatedResponse extends ResponseHeader {
  readonly type: 'voxelPickValidated';
  readonly anchor: VoxelPickReadout;
}

export interface VoxelReadResponse extends ResponseHeader {
  readonly type: 'voxelRead';
  readonly readout: VoxelReadout;
}

export interface VoxelConversionPreparedResponse extends ResponseHeader {
  readonly type: 'voxelConversionPrepared';
  readonly plan: VoxelConversionPlan;
  readonly preview: VoxelConversionPreview;
}

export interface VoxelConversionDiscardedResponse extends ResponseHeader {
  readonly type: 'voxelConversionDiscarded';
  readonly planId: string;
}

export interface VoxelObjectSourceInspectedResponse extends ResponseHeader {
  readonly type: 'voxelObjectSourceInspected';
  readonly inspection: VoxelObjectSourceInspection;
}

export interface VoxelObjectConversionPreparedResponse extends ResponseHeader {
  readonly type: 'voxelObjectConversionPrepared';
  readonly plan: VoxelObjectConversionPlan;
  readonly preview: VoxelObjectConversionPreview;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
  readonly meshResources?: readonly MeshResourceReadout[];
}

export interface VoxelObjectConversionPreviewedResponse extends ResponseHeader {
  readonly type: 'voxelObjectConversionPreviewed';
  readonly preview: VoxelObjectConversionPreview;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
  readonly meshResources?: readonly MeshResourceReadout[];
}

export interface VoxelObjectConversionDiscardedResponse extends ResponseHeader {
  readonly type: 'voxelObjectConversionDiscarded';
  readonly planId: string;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
  readonly meshResources?: readonly MeshResourceReadout[];
}

export interface VoxelObjectInstancePreviewedResponse extends ResponseHeader {
  readonly type: 'voxelObjectInstancePreviewed';
  readonly playback: VoxelObjectInstancePlaybackReadout;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout<ProjectionFrameKind>;
  readonly meshResources?: readonly MeshResourceReadout[];
}

export interface AssetImportPreparedResponse extends ResponseHeader {
  readonly type: 'assetImportPrepared';
  readonly plan: AssetImportPlanReadout;
}

export interface AssetImportDiscardedResponse extends ResponseHeader {
  readonly type: 'assetImportDiscarded';
  readonly planId: string;
}

export interface VoxelHistoryRevertPreparedResponse extends ResponseHeader {
  readonly type: 'voxelHistoryRevertPrepared';
  readonly preview: VoxelHistoryRevertPreview;
}

export interface VoxelHistoryRevertDiscardedResponse extends ResponseHeader {
  readonly type: 'voxelHistoryRevertDiscarded';
  readonly previewId: string;
}

export interface VoxelAssetFileExportedResponse extends ResponseHeader {
  readonly type: 'voxelAssetFileExported';
  readonly assetId: string;
  readonly targetPath: string;
  readonly byteCount: number;
  readonly sha256: string;
  readonly replacedExisting: boolean;
}

export interface ProjectClosedResponse extends ResponseHeader {
  readonly type: 'projectClosed';
}

export interface RejectedResponse {
  readonly type: 'rejected';
  readonly protocolVersion: typeof STUDIO_ADAPTER_PROTOCOL_VERSION;
  readonly requestId?: string;
  readonly error: AdapterRejection;
}

interface ResponseHeader {
  readonly protocolVersion: typeof STUDIO_ADAPTER_PROTOCOL_VERSION;
  readonly requestId: string;
}

export const STUDIO_ADAPTER_OPERATIONS = [
  'describe',
  'openProject',
  'createProject',
  'saveProjectAs',
  'readProject',
  'createScene',
  'renameScene',
  'deleteScene',
  'setEntryScene',
  'createSceneObject',
  'deleteSceneObject',
  'renameSceneObject',
  'reparentSceneObject',
  'setSceneObjectTransform',
  'setSceneObjectAppearance',
  'setEntityCollision',
  'setEntityKinematic',
  'setEntityTranslation',
  'upsertMaterial',
  'prepareAssetImport',
  'prepareAssetReimport',
  'applyAssetImport',
  'discardAssetImport',
  'initializeVoxelAsset',
  'duplicateVoxelAsset',
  'attachVoxelInstance',
  'setVoxelInstanceTransform',
  'removeVoxelInstance',
  'replaceVoxelPalette',
  'validateVoxelPick',
  'applyVoxelBrush',
  'applyVoxelPrimitive',
  'initializeVoxelTemplate',
  'importVoxelAssetFile',
  'exportVoxelAssetFile',
  'materializeEnvironment',
  'undoVoxelEdit',
  'redoVoxelEdit',
  'revertVoxelHistory',
  'queryVoxelHistory',
  'prepareVoxelHistoryRevert',
  'applyVoxelHistoryRevert',
  'discardVoxelHistoryRevert',
  'createVoxelAnnotationLayer',
  'editVoxelAnnotation',
  'queryVoxelAnnotation',
  'exportVoxelAnnotation',
  'queryVoxelModel',
  'prepareVoxelConversion',
  'applyVoxelConversion',
  'discardVoxelConversion',
  'inspectVoxelObjectSource',
  'prepareVoxelObjectConversion',
  'previewVoxelObjectConversion',
  'applyVoxelObjectConversion',
  'discardVoxelObjectConversion',
  'attachVoxelObjectInstance',
  'previewVoxelObjectInstance',
  'closeProject',
] as const;

export interface AdapterDescription {
  readonly adapterId: string;
  readonly adapterVersion: number;
  readonly protocolVersion: typeof STUDIO_ADAPTER_PROTOCOL_VERSION;
  readonly projectKind: string;
  readonly projectSchemaVersion: number;
  readonly operations: typeof STUDIO_ADAPTER_OPERATIONS;
}

export interface StudioProjectReadout {
  readonly identity: StudioProjectIdentity;
  readonly canonical: CanonicalOwnerContent;
  readonly inspections: OwnerInspections;
  readonly sceneHierarchy: SceneHierarchyReadout;
  readonly assetBrowser: AssetBrowserReadout;
  readonly voxel?: Readonly<Record<string, unknown>>;
  readonly voxelAuthoring: VoxelAuthoringReadout;
  readonly voxelObjectAuthoring: VoxelObjectAuthoringReadout;
  readonly animatedMeshResources: readonly AnimatedMeshResourceReadout[];
  /** Optional protocol-9 extension. Its manifest and packed byte encoding are
   * independently versioned, so existing inline adapters remain valid. */
  readonly meshResources?: readonly MeshResourceReadout[];
  readonly loadingBay: LoadingBayDomainReadout;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
}

export interface MeshResourceReadout {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
  readonly sourcePath: string;
}

export interface AnimatedMeshResourceReadout {
  readonly asset: string;
  readonly contentHash: string;
  readonly clipIds: readonly string[];
  readonly sourcePath: string;
}

export interface AssetBrowserReadout {
  readonly assets: readonly AssetEntryReadout[];
  readonly lockEntries: readonly AssetLockEntryReadout[];
}

export interface AssetEntryReadout {
  readonly assetId: string;
  readonly kind: string;
  readonly version: number;
  readonly hash: string | null;
  readonly sourcePath: string | null;
  readonly label: string | null;
  readonly dependencies: readonly string[];
  readonly dependents: readonly string[];
  readonly material: boolean;
  readonly importedMesh: boolean;
  readonly import: AssetImportReadout | null;
}

export interface AssetImportReadout {
  readonly source: StudioFileSelection;
  readonly sourceHash: string;
  readonly sourceByteCount: number;
  readonly importerVersion: number;
  readonly generatedAssetIds: readonly string[];
  readonly status: 'unchanged' | 'contentChanged' | 'movedFile' | 'unavailable' | 'metadataInvalid';
}

export interface AssetLockEntryReadout {
  readonly assetId: string;
  readonly kind: string;
  readonly version: number;
  readonly hash: string | null;
  readonly dependencies: readonly string[];
}

export interface AssetImportPlanReadout {
  readonly planId: string;
  readonly planHash: string;
  readonly expectedProjectHash: string;
  readonly source: StudioFileSelection;
  readonly sourceHash: string;
  readonly sourceByteCount: number;
  readonly meshAssetId: string | null;
  readonly reimportKind: 'noop' | 'visualUpdate' | 'structuralReload' | null;
  readonly hasErrors: boolean;
  readonly diagnostics: readonly AssetImportDiagnosticReadout[];
  readonly generatedArtifacts: readonly AssetImportArtifactReadout[];
  readonly generatedAssetIds: readonly string[];
  readonly settings: StudioAssetImportSettings;
}

export interface AssetImportDiagnosticReadout {
  readonly severity: 'warning' | 'error';
  readonly code: string;
  readonly locus: string;
  readonly message: string;
  readonly remedy: string;
}

export interface AssetImportArtifactReadout {
  readonly relativePath: string;
  readonly byteCount: number;
}

export interface StudioProjectIdentity {
  readonly projectId: string;
  readonly name: string;
  readonly entryScene: string;
  readonly sourceSchemaVersion: number;
  readonly currentSchemaVersion: number;
  readonly projectHash: string;
  readonly sceneRevision: number;
  readonly relativeProjectFile: string;
}

export interface CanonicalOwnerContent {
  readonly projectJson: string;
  readonly assetCatalogJson: string;
  readonly authoredSceneJson: string;
  readonly entityStateJson: string;
  readonly contentManifestJson: string;
}

export interface OwnerInspections {
  readonly catalog: CatalogInspection;
  readonly scene: SceneInspection;
  readonly entityState: EntityStateInspection;
  readonly persistence: PersistenceInspection;
}

export interface SceneHierarchyReadout {
  readonly sceneId: number;
  readonly revision: number;
  readonly name: string | null;
  readonly rootNodeIds: readonly number[];
  readonly nodes: readonly SceneHierarchyNodeReadout[];
}

export interface SceneHierarchyNodeReadout {
  readonly nodeId: number;
  readonly parentNodeId: number | null;
  readonly childOrder: number;
  readonly displayOrder: number;
  readonly depth: number;
  readonly nodeKind: 'emptyGroup' | 'staticMesh' | 'animatedMesh' | 'sprite' | 'voxelVolume' | 'light' | 'marker' | 'entityInstance' | 'bootstrap';
  readonly label: string;
  readonly tags: readonly string[];
  readonly asset: string | null;
  readonly entityId: number | null;
  readonly localTransform: Transform;
  readonly worldTransform: Transform;
}

export interface NamedCount {
  readonly name: string;
  readonly count: number;
}

export interface DiagnosticSet {
  readonly diagnostics: readonly OwnerDiagnostic[];
}

export interface OwnerDiagnostic {
  readonly domain: 'assetCatalog' | 'entityState' | 'scene' | 'voxelState' | 'persistence' | 'import';
  readonly severity: 'info' | 'warning' | 'error' | 'fatal';
  readonly code: string;
  readonly location: DiagnosticLocation;
  readonly message: string;
  readonly remedy?: {
    readonly action: 'inspect' | 'provideAsset' | 'fixReference' | 'breakCycle' | 'regenerate' | 'restoreArtifact' | 'refreshCache';
    readonly detail: string;
  };
}

export interface DiagnosticLocation {
  readonly path?: string;
  readonly assetId?: string;
  readonly entityId?: number;
  readonly sceneNodeId?: number;
  readonly chunk?: readonly [number, number, number];
}

export interface CatalogInspection {
  readonly entryCount: number;
  readonly dependencyCount: number;
  readonly kinds: readonly NamedCount[];
  readonly lock?: {
    readonly entryCount: number;
    readonly findingCount: number;
  };
  readonly diagnostics: DiagnosticSet;
}

export interface SceneInspection {
  readonly sceneId: number;
  readonly revision: number;
  readonly schemaVersion: number;
  readonly name: string | null;
  readonly nodeCount: number;
  readonly rootCount: number;
  readonly dependencyCount: number;
  readonly nodeKinds: readonly NamedCount[];
  readonly diagnostics: DiagnosticSet;
}

export interface EntityStateInspection {
  readonly schemaVersion: number;
  readonly revision: number;
  readonly entityCount: number;
  readonly lifecycle: readonly NamedCount[];
  readonly sources: readonly NamedCount[];
  readonly capabilities: readonly NamedCount[];
  readonly relationships: readonly NamedCount[];
  readonly entityIds: readonly number[];
  readonly diagnostics: DiagnosticSet;
}

export interface PersistenceInspection {
  readonly schemaVersion: number;
  readonly artifactCount: number;
  readonly requiredArtifactCount: number;
  readonly declaredByteCount: number;
  readonly classes: readonly NamedCount[];
  readonly roles: readonly NamedCount[];
  readonly loadSteps: readonly {
    readonly stage: string;
    readonly path: string;
  }[];
  readonly diagnostics: DiagnosticSet;
}

export interface LoadingBayDomainReadout {
  readonly sceneName: string;
  readonly entityCount: number;
  readonly doorCount: number;
  readonly switchCount: number;
  readonly enemyCount: number;
  readonly encounterCount: number;
  readonly extractionBeaconCount: number;
  readonly navigatorCount: number;
  readonly playerControllerCount: number;
  readonly weaponCount: number;
  readonly voxelEnvironment: string;
}

export type ProjectionFrameKind = 'complete' | 'incremental';

export interface ProjectionReadout<FrameKind extends ProjectionFrameKind = 'complete'> {
  readonly frameKind: FrameKind;
  readonly sourceRevision: number;
  readonly retainedEntities: number;
  readonly retainedLights: number;
  readonly retainedVoxelInstances: number;
  readonly retainedVoxelChunks: number;
  readonly diagnostics: readonly ProjectionDiagnosticReadout[];
}

export interface ProjectionDiagnosticReadout {
  readonly code: string;
  readonly entityId: number;
  readonly asset: string;
  readonly assetKind?: string;
}

export interface EntityTranslationReceipt {
  readonly entityId: number;
  readonly translation: readonly [number, number, number];
  readonly projectHashBefore: string;
  readonly projectHashAfter: string;
  readonly sceneRevisionBefore: number;
  readonly sceneRevisionAfter: number;
  readonly contentCandidateHash: string;
}

export interface AdapterRejection {
  readonly code: string;
  readonly path?: string;
  readonly message: string;
}

export class StudioAdapterDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'StudioAdapterDecodeError';
  }
}

export function decodeStudioAdapterResponse(input: unknown): StudioAdapterResponse {
  const base = looseRecord(input, '$');
  const type = text(base['type'], '$.type');
  switch (type) {
    case 'described': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'adapter']);
      responseHeader(value);
      adapterDescription(value['adapter'], '$.adapter');
      return input as DescribedResponse;
    }
    case 'projectOpened':
    case 'projectCreated':
    case 'projectSavedAs':
    case 'projectRead': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'project']);
      responseHeader(value);
      projectReadout(value['project'], '$.project');
      return input as ProjectOpenedResponse | ProjectCreatedResponse | ProjectSavedAsResponse | ProjectReadResponse;
    }
    case 'entityTranslationApplied': {
      const value = record(input, '$', [
        'type',
        'protocolVersion',
        'requestId',
        'receipt',
        'project',
      ]);
      responseHeader(value);
      translationReceipt(value['receipt'], '$.receipt');
      projectReadout(value['project'], '$.project');
      return input as EntityTranslationAppliedResponse;
    }
    case 'projectMutationApplied': {
      const value = record(input, '$', [
        'type',
        'protocolVersion',
        'requestId',
        'receipt',
        'project',
      ]);
      responseHeader(value);
      voxelContract('$.receipt', () => validateProjectMutationReceipt(value['receipt'], '$.receipt'));
      projectReadout(value['project'], '$.project');
      return input as ProjectMutationAppliedResponse;
    }
    case 'voxelPickValidated': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'anchor']);
      responseHeader(value);
      voxelContract('$.anchor', () => validateVoxelPickReadout(value['anchor'], '$.anchor'));
      return input as VoxelPickValidatedResponse;
    }
    case 'voxelRead': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'readout']);
      responseHeader(value);
      voxelContract('$.readout', () => validateVoxelReadout(value['readout'], '$.readout'));
      return input as VoxelReadResponse;
    }
    case 'voxelConversionPrepared': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'plan', 'preview',
      ]);
      responseHeader(value);
      voxelContract('$.plan', () => validateVoxelConversionPlan(value['plan'], '$.plan'));
      voxelContract('$.preview', () => validateVoxelConversionPreview(value['preview'], '$.preview'));
      return input as VoxelConversionPreparedResponse;
    }
    case 'voxelConversionDiscarded': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'planId',
      ]);
      responseHeader(value);
      text(value['planId'], '$.planId');
      return input as VoxelConversionDiscardedResponse;
    }
    case 'voxelObjectSourceInspected': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'inspection',
      ]);
      responseHeader(value);
      voxelContract('$.inspection', () =>
        validateVoxelObjectSourceInspection(value['inspection'], '$.inspection'));
      return input as VoxelObjectSourceInspectedResponse;
    }
    case 'voxelObjectConversionPrepared': {
      const value = record(
        input,
        '$',
        [
          'type', 'protocolVersion', 'requestId', 'plan', 'preview', 'projection',
          'projectionReadout',
        ],
        ['meshResources'],
      );
      responseHeader(value);
      voxelContract('$.plan', () =>
        validateVoxelObjectConversionPlan(value['plan'], '$.plan'));
      voxelContract('$.preview', () =>
        validateVoxelObjectConversionPreview(value['preview'], '$.preview'));
      completeProjection(value, '$');
      optional(value['meshResources'], '$.meshResources', meshResources);
      return input as VoxelObjectConversionPreparedResponse;
    }
    case 'voxelObjectConversionPreviewed': {
      const value = record(
        input,
        '$',
        [
          'type', 'protocolVersion', 'requestId', 'preview', 'projection',
          'projectionReadout',
        ],
        ['meshResources'],
      );
      responseHeader(value);
      voxelContract('$.preview', () =>
        validateVoxelObjectConversionPreview(value['preview'], '$.preview'));
      completeProjection(value, '$');
      optional(value['meshResources'], '$.meshResources', meshResources);
      return input as VoxelObjectConversionPreviewedResponse;
    }
    case 'voxelObjectConversionDiscarded': {
      const value = record(
        input,
        '$',
        [
          'type', 'protocolVersion', 'requestId', 'planId', 'projection',
          'projectionReadout',
        ],
        ['meshResources'],
      );
      responseHeader(value);
      text(value['planId'], '$.planId');
      completeProjection(value, '$');
      optional(value['meshResources'], '$.meshResources', meshResources);
      return input as VoxelObjectConversionDiscardedResponse;
    }
    case 'voxelObjectInstancePreviewed': {
      const value = record(
        input,
        '$',
        [
          'type', 'protocolVersion', 'requestId', 'playback', 'projection',
          'projectionReadout',
        ],
        ['meshResources'],
      );
      responseHeader(value);
      voxelContract('$.playback', () =>
        validateVoxelObjectInstancePlaybackReadout(value['playback'], '$.playback'));
      objectInstanceProjection(value, '$');
      optional(value['meshResources'], '$.meshResources', meshResources);
      return input as VoxelObjectInstancePreviewedResponse;
    }
    case 'assetImportPrepared': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'plan',
      ]);
      responseHeader(value);
      assetImportPlan(value['plan'], '$.plan');
      return input as AssetImportPreparedResponse;
    }
    case 'assetImportDiscarded': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'planId',
      ]);
      responseHeader(value);
      text(value['planId'], '$.planId');
      return input as AssetImportDiscardedResponse;
    }
    case 'voxelHistoryRevertPrepared': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'preview',
      ]);
      responseHeader(value);
      voxelContract('$.preview', () =>
        validateVoxelHistoryRevertPreview(value['preview'], '$.preview'));
      return input as VoxelHistoryRevertPreparedResponse;
    }
    case 'voxelHistoryRevertDiscarded': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'previewId',
      ]);
      responseHeader(value);
      text(value['previewId'], '$.previewId');
      return input as VoxelHistoryRevertDiscardedResponse;
    }
    case 'voxelAssetFileExported': {
      const value = record(input, '$', [
        'type', 'protocolVersion', 'requestId', 'assetId', 'targetPath',
        'byteCount', 'sha256', 'replacedExisting',
      ]);
      responseHeader(value);
      text(value['assetId'], '$.assetId');
      text(value['targetPath'], '$.targetPath');
      integer(value['byteCount'], '$.byteCount');
      text(value['sha256'], '$.sha256');
      truth(value['replacedExisting'], '$.replacedExisting');
      return input as VoxelAssetFileExportedResponse;
    }
    case 'projectClosed': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId']);
      responseHeader(value);
      return input as ProjectClosedResponse;
    }
    case 'rejected': {
      const value = record(
        input,
        '$',
        ['type', 'protocolVersion', 'error'],
        ['requestId'],
      );
      protocolVersion(value['protocolVersion'], '$.protocolVersion');
      optional(value['requestId'], '$.requestId', text);
      rejection(value['error'], '$.error');
      return input as RejectedResponse;
    }
    default:
      fail('$.type', `${JSON.stringify(type)} is not a closed Studio adapter response`);
  }
}

function responseHeader(value: Readonly<Record<string, unknown>>): void {
  protocolVersion(value['protocolVersion'], '$.protocolVersion');
  text(value['requestId'], '$.requestId');
}

function completeProjection(value: Readonly<Record<string, unknown>>, path: string): void {
  rendererProjection(value, path);
  projectionReadout(value['projectionReadout'], `${path}.projectionReadout`, ['complete']);
}

function objectInstanceProjection(
  value: Readonly<Record<string, unknown>>,
  path: string,
): void {
  rendererProjection(value, path);
  projectionReadout(
    value['projectionReadout'],
    `${path}.projectionReadout`,
    ['complete', 'incremental'],
  );
}

function rendererProjection(value: Readonly<Record<string, unknown>>, path: string): void {
  try {
    decodeRenderFrameDiff(value['projection']);
  } catch (error) {
    fail(
      `${path}.projection`,
      error instanceof Error ? error.message : 'renderer contract rejected the frame',
    );
  }
}

function voxelContract(path: string, validate: () => void): void {
  try {
    validate();
  } catch (error) {
    fail(path, error instanceof Error ? error.message : 'voxel contract is malformed');
  }
}

function protocolVersion(input: unknown, path: string): void {
  if (input !== STUDIO_ADAPTER_PROTOCOL_VERSION) {
    fail(path, `must equal ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)}`);
  }
}

function adapterDescription(input: unknown, path: string): void {
  const value = record(input, path, [
    'adapterId',
    'adapterVersion',
    'protocolVersion',
    'projectKind',
    'projectSchemaVersion',
    'operations',
  ]);
  text(value['adapterId'], `${path}.adapterId`);
  integer(value['adapterVersion'], `${path}.adapterVersion`);
  protocolVersion(value['protocolVersion'], `${path}.protocolVersion`);
  text(value['projectKind'], `${path}.projectKind`);
  integer(value['projectSchemaVersion'], `${path}.projectSchemaVersion`);
  const operations = list(value['operations'], `${path}.operations`).map((entry, index) =>
    text(entry, `${path}.operations[${String(index)}]`),
  );
  const expected = STUDIO_ADAPTER_OPERATIONS;
  if (operations.length !== expected.length || operations.some((entry, index) => entry !== expected[index])) {
    fail(`${path}.operations`, 'must name the protocol 9 operation set in order');
  }
}

function projectReadout(input: unknown, path: string): void {
  const value = record(
    input,
    path,
    [
      'identity',
      'canonical',
      'inspections',
      'sceneHierarchy',
      'assetBrowser',
      'voxelAuthoring',
      'voxelObjectAuthoring',
      'animatedMeshResources',
      'loadingBay',
      'projection',
      'projectionReadout',
    ],
    ['voxel', 'meshResources'],
  );
  projectIdentity(value['identity'], `${path}.identity`);
  canonicalOwnerContent(value['canonical'], `${path}.canonical`);
  ownerInspections(value['inspections'], `${path}.inspections`);
  sceneHierarchy(value['sceneHierarchy'], `${path}.sceneHierarchy`);
  assetBrowser(value['assetBrowser'], `${path}.assetBrowser`);
  optional(value['voxel'], `${path}.voxel`, looseRecord);
  voxelContract(`${path}.voxelAuthoring`, () =>
    validateVoxelAuthoringReadout(value['voxelAuthoring'], `${path}.voxelAuthoring`));
  voxelContract(`${path}.voxelObjectAuthoring`, () =>
    validateVoxelObjectAuthoringReadout(
      value['voxelObjectAuthoring'],
      `${path}.voxelObjectAuthoring`,
    ));
  animatedMeshResources(value['animatedMeshResources'], `${path}.animatedMeshResources`);
  optional(value['meshResources'], `${path}.meshResources`, meshResources);
  loadingBayReadout(value['loadingBay'], `${path}.loadingBay`);
  try {
    decodeRenderFrameDiff(value['projection']);
  } catch (error) {
    fail(
      `${path}.projection`,
      error instanceof Error ? error.message : 'renderer contract rejected the frame',
    );
  }
  projectionReadout(value['projectionReadout'], `${path}.projectionReadout`);
}

function meshResources(input: unknown, path: string): void {
  const identities = new Set<string>();
  let aggregateBytes = 0;
  list(input, path).forEach((entry, index) => {
    const entryPath = `${path}[${String(index)}]`;
    const resource = record(entry, entryPath, [
      'resource', 'contentHash', 'byteLength', 'sourcePath',
    ]);
    const identity = text(resource['resource'], `${entryPath}.resource`);
    const contentHash = text(resource['contentHash'], `${entryPath}.contentHash`);
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(contentHash)?.[1];
    if (digest === undefined || identity !== `mesh-resource/${digest}`) {
      fail(entryPath, 'must declare one content-addressed mesh resource identity');
    }
    if (identities.has(identity)) fail(`${entryPath}.resource`, 'is duplicated');
    identities.add(identity);
    const byteLength = integer(resource['byteLength'], `${entryPath}.byteLength`);
    if (byteLength < 16 || byteLength > 64 * 1024 * 1024) {
      fail(`${entryPath}.byteLength`, 'must be between 16 bytes and 64 MiB');
    }
    aggregateBytes += byteLength;
    if (aggregateBytes > 256 * 1024 * 1024) {
      fail(path, 'exceeds the aggregate mesh resource byte bound');
    }
    text(resource['sourcePath'], `${entryPath}.sourcePath`);
  });
}

function animatedMeshResources(input: unknown, path: string): void {
  list(input, path).forEach((entry, index) => {
    const entryPath = `${path}[${String(index)}]`;
    const resource = record(entry, entryPath, [
      'asset', 'contentHash', 'clipIds', 'sourcePath',
    ]);
    text(resource['asset'], `${entryPath}.asset`);
    text(resource['contentHash'], `${entryPath}.contentHash`);
    stringList(resource['clipIds'], `${entryPath}.clipIds`);
    text(resource['sourcePath'], `${entryPath}.sourcePath`);
  });
}

function assetBrowser(input: unknown, path: string): void {
  const value = record(input, path, ['assets', 'lockEntries']);
  list(value['assets'], `${path}.assets`).forEach((entry, index) => {
    const entryPath = `${path}.assets[${String(index)}]`;
    const asset = record(entry, entryPath, [
      'assetId', 'kind', 'version', 'hash', 'sourcePath', 'label', 'dependencies',
      'dependents', 'material', 'importedMesh', 'import',
    ]);
    text(asset['assetId'], `${entryPath}.assetId`);
    text(asset['kind'], `${entryPath}.kind`);
    integer(asset['version'], `${entryPath}.version`);
    nullable(asset['hash'], `${entryPath}.hash`, text);
    nullable(asset['sourcePath'], `${entryPath}.sourcePath`, text);
    nullable(asset['label'], `${entryPath}.label`, text);
    stringList(asset['dependencies'], `${entryPath}.dependencies`);
    stringList(asset['dependents'], `${entryPath}.dependents`);
    truth(asset['material'], `${entryPath}.material`);
    truth(asset['importedMesh'], `${entryPath}.importedMesh`);
    nullable(asset['import'], `${entryPath}.import`, assetImportReadout);
  });
  list(value['lockEntries'], `${path}.lockEntries`).forEach((entry, index) => {
    const entryPath = `${path}.lockEntries[${String(index)}]`;
    const lock = record(entry, entryPath, [
      'assetId', 'kind', 'version', 'hash', 'dependencies',
    ]);
    text(lock['assetId'], `${entryPath}.assetId`);
    text(lock['kind'], `${entryPath}.kind`);
    integer(lock['version'], `${entryPath}.version`);
    nullable(lock['hash'], `${entryPath}.hash`, text);
    stringList(lock['dependencies'], `${entryPath}.dependencies`);
  });
}

function assetImportReadout(input: unknown, path: string): void {
  const value = record(input, path, [
    'source', 'sourceHash', 'sourceByteCount', 'importerVersion', 'generatedAssetIds', 'status',
  ]);
  fileSelection(value['source'], `${path}.source`);
  text(value['sourceHash'], `${path}.sourceHash`);
  integer(value['sourceByteCount'], `${path}.sourceByteCount`);
  integer(value['importerVersion'], `${path}.importerVersion`);
  stringList(value['generatedAssetIds'], `${path}.generatedAssetIds`);
  choice(value['status'], `${path}.status`, [
    'unchanged', 'contentChanged', 'movedFile', 'unavailable', 'metadataInvalid',
  ]);
}

function assetImportPlan(input: unknown, path: string): void {
  const value = record(input, path, [
    'planId', 'planHash', 'expectedProjectHash', 'source', 'sourceHash', 'sourceByteCount',
    'meshAssetId', 'reimportKind', 'hasErrors', 'diagnostics', 'generatedArtifacts',
    'generatedAssetIds', 'settings',
  ]);
  for (const field of ['planId', 'planHash', 'expectedProjectHash', 'sourceHash']) {
    text(value[field], `${path}.${field}`);
  }
  fileSelection(value['source'], `${path}.source`);
  integer(value['sourceByteCount'], `${path}.sourceByteCount`);
  nullable(value['meshAssetId'], `${path}.meshAssetId`, text);
  nullable(value['reimportKind'], `${path}.reimportKind`, (entry, entryPath) => {
    choice(entry, entryPath, ['noop', 'visualUpdate', 'structuralReload']);
  });
  truth(value['hasErrors'], `${path}.hasErrors`);
  list(value['diagnostics'], `${path}.diagnostics`).forEach((entry, index) => {
    const entryPath = `${path}.diagnostics[${String(index)}]`;
    const diagnostic = record(entry, entryPath, [
      'severity', 'code', 'locus', 'message', 'remedy',
    ]);
    choice(diagnostic['severity'], `${entryPath}.severity`, ['warning', 'error']);
    for (const field of ['code', 'locus', 'message', 'remedy']) {
      text(diagnostic[field], `${entryPath}.${field}`);
    }
  });
  list(value['generatedArtifacts'], `${path}.generatedArtifacts`).forEach((entry, index) => {
    const entryPath = `${path}.generatedArtifacts[${String(index)}]`;
    const artifact = record(entry, entryPath, ['relativePath', 'byteCount']);
    text(artifact['relativePath'], `${entryPath}.relativePath`);
    integer(artifact['byteCount'], `${entryPath}.byteCount`);
  });
  stringList(value['generatedAssetIds'], `${path}.generatedAssetIds`);
  assetImportSettings(value['settings'], `${path}.settings`);
}

function fileSelection(input: unknown, path: string): void {
  const value = record(input, path, ['scope', 'path']);
  choice(value['scope'], `${path}.scope`, ['project', 'host']);
  text(value['path'], `${path}.path`);
}

function assetImportSettings(input: unknown, path: string): void {
  const value = record(input, path, ['scale', 'generateCollision', 'materialNamespace']);
  finiteNumber(value['scale'], `${path}.scale`);
  truth(value['generateCollision'], `${path}.generateCollision`);
  nullable(value['materialNamespace'], `${path}.materialNamespace`, text);
}

function sceneHierarchy(input: unknown, path: string): void {
  const value = record(input, path, ['sceneId', 'revision', 'name', 'rootNodeIds', 'nodes']);
  integer(value['sceneId'], `${path}.sceneId`);
  integer(value['revision'], `${path}.revision`);
  nullable(value['name'], `${path}.name`, text);
  list(value['rootNodeIds'], `${path}.rootNodeIds`).forEach((entry, index) => {
    integer(entry, `${path}.rootNodeIds[${String(index)}]`);
  });
  list(value['nodes'], `${path}.nodes`).forEach((entry, index) => {
    hierarchyNode(entry, `${path}.nodes[${String(index)}]`);
  });
}

function hierarchyNode(input: unknown, path: string): void {
  const value = record(input, path, [
    'nodeId',
    'parentNodeId',
    'childOrder',
    'displayOrder',
    'depth',
    'nodeKind',
    'label',
    'tags',
    'asset',
    'entityId',
    'localTransform',
    'worldTransform',
  ]);
  for (const field of ['nodeId', 'childOrder', 'displayOrder', 'depth']) {
    integer(value[field], `${path}.${field}`);
  }
  nullable(value['parentNodeId'], `${path}.parentNodeId`, integer);
  choice(value['nodeKind'], `${path}.nodeKind`, [
    'emptyGroup',
    'staticMesh',
    'animatedMesh',
    'sprite',
    'voxelVolume',
    'light',
    'marker',
    'entityInstance',
    'bootstrap',
  ]);
  text(value['label'], `${path}.label`);
  list(value['tags'], `${path}.tags`).forEach((entry, index) => {
    text(entry, `${path}.tags[${String(index)}]`);
  });
  nullable(value['asset'], `${path}.asset`, text);
  nullable(value['entityId'], `${path}.entityId`, integer);
  transform(value['localTransform'], `${path}.localTransform`);
  transform(value['worldTransform'], `${path}.worldTransform`);
}

function transform(input: unknown, path: string): void {
  const value = record(input, path, ['translation', 'rotation', 'scale']);
  vector3(value['translation'], `${path}.translation`);
  vector4(value['rotation'], `${path}.rotation`);
  vector3(value['scale'], `${path}.scale`);
}

function projectIdentity(input: unknown, path: string): void {
  const value = record(input, path, [
    'projectId',
    'name',
    'entryScene',
    'sourceSchemaVersion',
    'currentSchemaVersion',
    'projectHash',
    'sceneRevision',
    'relativeProjectFile',
  ]);
  for (const field of ['projectId', 'name', 'entryScene', 'projectHash', 'relativeProjectFile']) {
    text(value[field], `${path}.${field}`);
  }
  for (const field of ['sourceSchemaVersion', 'currentSchemaVersion', 'sceneRevision']) {
    integer(value[field], `${path}.${field}`);
  }
}

function canonicalOwnerContent(input: unknown, path: string): void {
  const fields = [
    'projectJson',
    'assetCatalogJson',
    'authoredSceneJson',
    'entityStateJson',
    'contentManifestJson',
  ];
  const value = record(input, path, fields);
  for (const field of fields) text(value[field], `${path}.${field}`);
}

function ownerInspections(input: unknown, path: string): void {
  const value = record(input, path, ['catalog', 'scene', 'entityState', 'persistence']);
  catalogInspection(value['catalog'], `${path}.catalog`);
  sceneInspection(value['scene'], `${path}.scene`);
  entityStateInspection(value['entityState'], `${path}.entityState`);
  persistenceInspection(value['persistence'], `${path}.persistence`);
}

function catalogInspection(input: unknown, path: string): void {
  const value = record(
    input,
    path,
    ['entryCount', 'dependencyCount', 'kinds', 'diagnostics'],
    ['lock'],
  );
  integer(value['entryCount'], `${path}.entryCount`);
  integer(value['dependencyCount'], `${path}.dependencyCount`);
  namedCounts(value['kinds'], `${path}.kinds`);
  optional(value['lock'], `${path}.lock`, (entry, entryPath) => {
    const lock = record(entry, entryPath, ['entryCount', 'findingCount']);
    integer(lock['entryCount'], `${entryPath}.entryCount`);
    integer(lock['findingCount'], `${entryPath}.findingCount`);
  });
  diagnosticSet(value['diagnostics'], `${path}.diagnostics`);
}

function sceneInspection(input: unknown, path: string): void {
  const value = record(input, path, [
    'sceneId',
    'revision',
    'schemaVersion',
    'name',
    'nodeCount',
    'rootCount',
    'dependencyCount',
    'nodeKinds',
    'diagnostics',
  ]);
  for (const field of ['sceneId', 'revision', 'schemaVersion', 'nodeCount', 'rootCount', 'dependencyCount']) {
    integer(value[field], `${path}.${field}`);
  }
  nullable(value['name'], `${path}.name`, text);
  namedCounts(value['nodeKinds'], `${path}.nodeKinds`);
  diagnosticSet(value['diagnostics'], `${path}.diagnostics`);
}

function entityStateInspection(input: unknown, path: string): void {
  const value = record(input, path, [
    'schemaVersion',
    'revision',
    'entityCount',
    'lifecycle',
    'sources',
    'capabilities',
    'relationships',
    'entityIds',
    'diagnostics',
  ]);
  for (const field of ['schemaVersion', 'revision', 'entityCount']) {
    integer(value[field], `${path}.${field}`);
  }
  for (const field of ['lifecycle', 'sources', 'capabilities', 'relationships']) {
    namedCounts(value[field], `${path}.${field}`);
  }
  list(value['entityIds'], `${path}.entityIds`).forEach((entry, index) => {
    integer(entry, `${path}.entityIds[${String(index)}]`);
  });
  diagnosticSet(value['diagnostics'], `${path}.diagnostics`);
}

function persistenceInspection(input: unknown, path: string): void {
  const value = record(input, path, [
    'schemaVersion',
    'artifactCount',
    'requiredArtifactCount',
    'declaredByteCount',
    'classes',
    'roles',
    'loadSteps',
    'diagnostics',
  ]);
  for (const field of ['schemaVersion', 'artifactCount', 'requiredArtifactCount', 'declaredByteCount']) {
    integer(value[field], `${path}.${field}`);
  }
  namedCounts(value['classes'], `${path}.classes`);
  namedCounts(value['roles'], `${path}.roles`);
  list(value['loadSteps'], `${path}.loadSteps`).forEach((entry, index) => {
    const entryPath = `${path}.loadSteps[${String(index)}]`;
    const step = record(entry, entryPath, ['stage', 'path']);
    text(step['stage'], `${entryPath}.stage`);
    text(step['path'], `${entryPath}.path`);
  });
  diagnosticSet(value['diagnostics'], `${path}.diagnostics`);
}

function namedCounts(input: unknown, path: string): void {
  list(input, path).forEach((entry, index) => {
    const entryPath = `${path}[${String(index)}]`;
    const count = record(entry, entryPath, ['name', 'count']);
    text(count['name'], `${entryPath}.name`);
    integer(count['count'], `${entryPath}.count`);
  });
}

function stringList(input: unknown, path: string): void {
  list(input, path).forEach((entry, index) => {
    text(entry, `${path}[${String(index)}]`);
  });
}

function diagnosticSet(input: unknown, path: string): void {
  const value = record(input, path, ['diagnostics']);
  list(value['diagnostics'], `${path}.diagnostics`).forEach((entry, index) => {
    ownerDiagnostic(entry, `${path}.diagnostics[${String(index)}]`);
  });
}

function ownerDiagnostic(input: unknown, path: string): void {
  const value = record(
    input,
    path,
    ['domain', 'severity', 'code', 'location', 'message'],
    ['remedy'],
  );
  choice(value['domain'], `${path}.domain`, [
    'assetCatalog', 'entityState', 'scene', 'voxelState', 'persistence', 'import',
  ]);
  choice(value['severity'], `${path}.severity`, ['info', 'warning', 'error', 'fatal']);
  text(value['code'], `${path}.code`);
  text(value['message'], `${path}.message`);
  diagnosticLocation(value['location'], `${path}.location`);
  optional(value['remedy'], `${path}.remedy`, (entry, entryPath) => {
    const remedy = record(entry, entryPath, ['action', 'detail']);
    choice(remedy['action'], `${entryPath}.action`, [
      'inspect', 'provideAsset', 'fixReference', 'breakCycle', 'regenerate',
      'restoreArtifact', 'refreshCache',
    ]);
    text(remedy['detail'], `${entryPath}.detail`);
  });
}

function diagnosticLocation(input: unknown, path: string): void {
  const value = record(
    input,
    path,
    [],
    ['path', 'assetId', 'entityId', 'sceneNodeId', 'chunk'],
  );
  optional(value['path'], `${path}.path`, text);
  optional(value['assetId'], `${path}.assetId`, text);
  optional(value['entityId'], `${path}.entityId`, integer);
  optional(value['sceneNodeId'], `${path}.sceneNodeId`, integer);
  optional(value['chunk'], `${path}.chunk`, (entry, entryPath) => {
    const chunk = list(entry, entryPath);
    if (chunk.length !== 3) fail(entryPath, 'must have exactly 3 entries');
    chunk.forEach((coordinate, index) => signedInteger(coordinate, `${entryPath}[${String(index)}]`));
  });
}

function loadingBayReadout(input: unknown, path: string): void {
  const textFields = ['sceneName', 'voxelEnvironment'];
  const countFields = [
    'entityCount',
    'doorCount',
    'switchCount',
    'enemyCount',
    'encounterCount',
    'extractionBeaconCount',
    'navigatorCount',
    'playerControllerCount',
    'weaponCount',
  ];
  const value = record(input, path, [...textFields, ...countFields]);
  for (const field of textFields) text(value[field], `${path}.${field}`);
  for (const field of countFields) integer(value[field], `${path}.${field}`);
}

function projectionReadout(
  input: unknown,
  path: string,
  frameKinds: readonly ProjectionFrameKind[] = ['complete'],
): void {
  const value = record(input, path, [
    'frameKind',
    'sourceRevision',
    'retainedEntities',
    'retainedLights',
    'retainedVoxelInstances',
    'retainedVoxelChunks',
    'diagnostics',
  ]);
  choice(value['frameKind'], `${path}.frameKind`, frameKinds);
  integer(value['sourceRevision'], `${path}.sourceRevision`);
  integer(value['retainedEntities'], `${path}.retainedEntities`);
  integer(value['retainedLights'], `${path}.retainedLights`);
  integer(value['retainedVoxelInstances'], `${path}.retainedVoxelInstances`);
  integer(value['retainedVoxelChunks'], `${path}.retainedVoxelChunks`);
  list(value['diagnostics'], `${path}.diagnostics`).forEach((entry, index) => {
    const itemPath = `${path}.diagnostics[${String(index)}]`;
    const item = record(entry, itemPath, ['code', 'entityId', 'asset'], ['assetKind']);
    text(item['code'], `${itemPath}.code`);
    integer(item['entityId'], `${itemPath}.entityId`);
    text(item['asset'], `${itemPath}.asset`);
    optional(item['assetKind'], `${itemPath}.assetKind`, text);
  });
}

function translationReceipt(input: unknown, path: string): void {
  const value = record(input, path, [
    'entityId',
    'translation',
    'projectHashBefore',
    'projectHashAfter',
    'sceneRevisionBefore',
    'sceneRevisionAfter',
    'contentCandidateHash',
  ]);
  integer(value['entityId'], `${path}.entityId`);
  vector3(value['translation'], `${path}.translation`);
  for (const field of ['projectHashBefore', 'projectHashAfter', 'contentCandidateHash']) {
    text(value[field], `${path}.${field}`);
  }
  integer(value['sceneRevisionBefore'], `${path}.sceneRevisionBefore`);
  integer(value['sceneRevisionAfter'], `${path}.sceneRevisionAfter`);
}

function rejection(input: unknown, path: string): void {
  const value = record(input, path, ['code', 'message'], ['path']);
  text(value['code'], `${path}.code`);
  text(value['message'], `${path}.message`);
  optional(value['path'], `${path}.path`, text);
}

function vector3(input: unknown, path: string): readonly [number, number, number] {
  const value = list(input, path);
  if (value.length !== 3) fail(path, 'must have exactly 3 entries');
  return [
    finiteNumber(value[0], `${path}[0]`),
    finiteNumber(value[1], `${path}[1]`),
    finiteNumber(value[2], `${path}[2]`),
  ];
}

function vector4(input: unknown, path: string): readonly [number, number, number, number] {
  const value = list(input, path);
  if (value.length !== 4) fail(path, 'must have exactly 4 entries');
  return [
    finiteNumber(value[0], `${path}[0]`),
    finiteNumber(value[1], `${path}[1]`),
    finiteNumber(value[2], `${path}[2]`),
    finiteNumber(value[3], `${path}[3]`),
  ];
}

function record(
  input: unknown,
  path: string,
  required: readonly string[],
  optionalFields: readonly string[] = [],
): Readonly<Record<string, unknown>> {
  const value = looseRecord(input, path);
  for (const field of required) {
    if (!Object.hasOwn(value, field)) fail(`${path}.${field}`, 'is required');
  }
  const allowed = new Set([...required, ...optionalFields]);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) fail(`${path}.${field}`, 'is unknown');
  }
  return value;
}

function looseRecord(input: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  return input as Readonly<Record<string, unknown>>;
}

function list(input: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(input)) fail(path, 'must be an array');
  return input;
}

function text(input: unknown, path: string): string {
  if (typeof input !== 'string') fail(path, 'must be a string');
  return input;
}

function integer(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isSafeInteger(input) || input < 0) {
    fail(path, 'must be a nonnegative safe integer');
  }
  return input;
}

function truth(input: unknown, path: string): boolean {
  if (typeof input !== 'boolean') fail(path, 'must be a boolean');
  return input;
}

function signedInteger(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isSafeInteger(input)) {
    fail(path, 'must be a safe integer');
  }
  return input;
}

function choice(input: unknown, path: string, choices: readonly string[]): string {
  const value = text(input, path);
  if (!choices.includes(value)) {
    fail(path, `must be one of ${choices.join(', ')}`);
  }
  return value;
}

function finiteNumber(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    fail(path, 'must be a finite number');
  }
  return input;
}

function optional(
  input: unknown,
  path: string,
  validate: (input: unknown, path: string) => unknown,
): void {
  if (input !== undefined) validate(input, path);
}

function nullable(
  input: unknown,
  path: string,
  validate: (input: unknown, path: string) => unknown,
): void {
  if (input !== null) validate(input, path);
}

function fail(path: string, message: string): never {
  throw new StudioAdapterDecodeError(`${path}: ${message}`);
}
