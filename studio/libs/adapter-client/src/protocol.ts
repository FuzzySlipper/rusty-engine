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
  type VoxelReadout,
} from './voxel-protocol.js';

export type * from './voxel-protocol.js';

export const STUDIO_ADAPTER_PROTOCOL_VERSION = 3 as const;
export const MAX_STUDIO_ADAPTER_REQUEST_BYTES = 256 * 1024;
export const MAX_STUDIO_ADAPTER_RESPONSE_BYTES = 32 * 1024 * 1024;

export type StudioAdapterRequest =
  | DescribeRequest
  | OpenProjectRequest
  | ReadProjectRequest
  | SetEntityTranslationRequest
  | UpsertMaterialRequest
  | InitializeVoxelAssetRequest
  | DuplicateVoxelAssetRequest
  | AttachVoxelInstanceRequest
  | SetVoxelInstanceTransformRequest
  | RemoveVoxelInstanceRequest
  | ReplaceVoxelPaletteRequest
  | ValidateVoxelPickRequest
  | ApplyVoxelBrushRequest
  | UndoVoxelEditRequest
  | RedoVoxelEditRequest
  | RevertVoxelHistoryRequest
  | CreateVoxelAnnotationLayerRequest
  | EditVoxelAnnotationRequest
  | QueryVoxelAnnotationRequest
  | ExportVoxelAnnotationRequest
  | QueryVoxelModelRequest
  | PrepareVoxelConversionRequest
  | ApplyVoxelConversionRequest
  | DiscardVoxelConversionRequest
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

export interface ReadProjectRequest extends RequestHeader {
  readonly type: 'readProject';
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
  readonly sourcePath: string;
  readonly targetAssetId: string;
  readonly licensePath?: string;
  readonly settings: VoxelConversionSettings;
  readonly maxPreviewSamples: number;
}

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

export interface CloseProjectRequest extends RequestHeader {
  readonly type: 'closeProject';
}

export type StudioAdapterResponse =
  | DescribedResponse
  | ProjectOpenedResponse
  | ProjectReadResponse
  | EntityTranslationAppliedResponse
  | ProjectMutationAppliedResponse
  | VoxelPickValidatedResponse
  | VoxelReadResponse
  | VoxelConversionPreparedResponse
  | VoxelConversionDiscardedResponse
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
  'readProject',
  'setEntityTranslation',
  'upsertMaterial',
  'initializeVoxelAsset',
  'duplicateVoxelAsset',
  'attachVoxelInstance',
  'setVoxelInstanceTransform',
  'removeVoxelInstance',
  'replaceVoxelPalette',
  'validateVoxelPick',
  'applyVoxelBrush',
  'undoVoxelEdit',
  'redoVoxelEdit',
  'revertVoxelHistory',
  'createVoxelAnnotationLayer',
  'editVoxelAnnotation',
  'queryVoxelAnnotation',
  'exportVoxelAnnotation',
  'queryVoxelModel',
  'prepareVoxelConversion',
  'applyVoxelConversion',
  'discardVoxelConversion',
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
  readonly voxel?: Readonly<Record<string, unknown>>;
  readonly voxelAuthoring: VoxelAuthoringReadout;
  readonly loadingBay: LoadingBayDomainReadout;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
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
  readonly nodeKind: 'emptyGroup' | 'staticMesh' | 'sprite' | 'voxelVolume' | 'light' | 'marker' | 'entityInstance' | 'bootstrap';
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

export interface ProjectionReadout {
  readonly frameKind: 'complete';
  readonly sourceRevision: number;
  readonly retainedEntities: number;
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
    case 'projectRead': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'project']);
      responseHeader(value);
      projectReadout(value['project'], '$.project');
      return input as ProjectOpenedResponse | ProjectReadResponse;
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
    fail(`${path}.operations`, 'must name the protocol 3 operation set in order');
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
      'voxelAuthoring',
      'loadingBay',
      'projection',
      'projectionReadout',
    ],
    ['voxel'],
  );
  projectIdentity(value['identity'], `${path}.identity`);
  canonicalOwnerContent(value['canonical'], `${path}.canonical`);
  ownerInspections(value['inspections'], `${path}.inspections`);
  sceneHierarchy(value['sceneHierarchy'], `${path}.sceneHierarchy`);
  optional(value['voxel'], `${path}.voxel`, looseRecord);
  voxelContract(`${path}.voxelAuthoring`, () =>
    validateVoxelAuthoringReadout(value['voxelAuthoring'], `${path}.voxelAuthoring`));
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

function projectionReadout(input: unknown, path: string): void {
  const value = record(input, path, [
    'frameKind',
    'sourceRevision',
    'retainedEntities',
    'retainedVoxelInstances',
    'retainedVoxelChunks',
    'diagnostics',
  ]);
  choice(value['frameKind'], `${path}.frameKind`, ['complete']);
  integer(value['sourceRevision'], `${path}.sourceRevision`);
  integer(value['retainedEntities'], `${path}.retainedEntities`);
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
