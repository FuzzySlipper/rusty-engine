import { signal, type Signal } from '@angular/core';
import type {
  AdapterDescription,
  AnimatedMeshResourceReadout,
  AssetBrowserReadout,
  AssetImportPlanReadout,
  CanonicalOwnerContent,
  LoadingBayDomainReadout,
  OwnerInspections,
  ProjectionReadout,
  ProjectionFrameKind,
  ProjectMutationAppliedResponse,
  SceneHierarchyNodeReadout,
  SceneHierarchyReadout,
  StudioProjectIdentity,
  StudioProjectReadout,
  StudioAssetImportSettings,
  StudioFileSelection,
  VoxelConversionPlan,
  VoxelConversionPreview,
  VoxelObjectAuthoringReadout,
  VoxelObjectAssetAuthoringReadout,
  VoxelObjectConversionPlan,
  VoxelObjectConversionPreview,
  VoxelObjectInstancePlaybackReadout,
  VoxelObjectInstanceReadout,
  VoxelObjectSourceInspection,
  VoxelPickReadout,
  VoxelReadout,
  VoxelHistoryRevertPreview,
  ProjectMutationReceipt,
  StoredCollision,
  StoredKinematic,
  StudioSceneAppearance,
  StudioSceneObjectDraft,
  VoxelAuthoringReadout,
} from '@rusty-engine/studio-adapter-client';
import {
  StudioAdapterOperationRejected,
} from '@rusty-engine/studio-adapter-client';
import type { StudioAdapterClient } from '@rusty-engine/studio-adapter-client';
import type {
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderMetadata,
  Transform,
} from '@rusty-engine/render-contracts';
import type {
  StudioLightingMode,
  StudioTransformOrientation,
  StudioTransformTool,
} from '@rusty-engine/studio-viewport';
import {
  deriveVoxelPickValidation,
  type VoxelEditorAction,
  type VoxelViewportPickCandidate,
} from '@rusty-engine/studio-voxel-editor/model';
import {
  buildDefaultStudioHostUserSettings,
  validateStudioHostUserSettings,
  type HttpStudioUserSettingsClient,
  type StudioHostUserSettingsArtifact,
  type StudioKeyboardBindings,
  type StudioUserSettingsSnapshot,
} from '@rusty-engine/studio-user-settings';
import {
  localTransformFromWorld,
} from './transform-tools.js';

export type StudioConnectionState =
  | { readonly kind: 'disconnected'; readonly message: string }
  | { readonly kind: 'connecting'; readonly message: string }
  | { readonly kind: 'connected'; readonly adapter: AdapterDescription; readonly message: string }
  | { readonly kind: 'unavailable'; readonly message: string };

export interface AuthoringDocumentView {
  readonly identity: StudioProjectIdentity;
  readonly canonical: CanonicalOwnerContent;
  readonly inspections: OwnerInspections;
  readonly sceneHierarchy: SceneHierarchyReadout;
  readonly assetBrowser: AssetBrowserReadout;
  readonly domain: LoadingBayDomainReadout;
  readonly voxel: Readonly<Record<string, unknown>> | null;
  readonly voxelAuthoring: VoxelAuthoringReadout;
  readonly voxelObjectAuthoring: VoxelObjectAuthoringReadout;
  readonly animatedMeshResources: readonly AnimatedMeshResourceReadout[];
}

export interface AssetWorkspaceState {
  readonly selectedAssetId: string | null;
  readonly plan: AssetImportPlanReadout | null;
  readonly message: string;
}

export interface ProjectedEntityView {
  readonly entityId: number;
  readonly label: string;
  readonly asset: string | null;
  readonly transform: Transform | null;
  readonly renderHandle: RenderHandle | null;
  readonly projected: boolean;
}

export interface LiveProjectionView {
  /** Compact complete authored frame used for remounts and presentation changes. */
  readonly frame: RenderFrameDiff;
  /** Optional retained-state patch for the current generation. */
  readonly framePatch: RenderFrameDiff | null;
  readonly readout: ProjectionReadout<ProjectionFrameKind>;
  readonly entities: readonly ProjectedEntityView[];
  readonly generation: number;
}

export interface TransformPreviewState {
  readonly entityId: number;
  readonly original: Transform;
  readonly tool: StudioTransformTool;
  readonly orientation: StudioTransformOrientation;
  readonly translation: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly scale: readonly [number, number, number];
}

export interface CreateProjectInput {
  readonly root: string;
  readonly projectFile: string;
  readonly projectId: string;
  readonly name: string;
  readonly entryScene: string;
  readonly entrySceneName: string;
}

export interface SaveProjectAsInput {
  readonly root: string;
  readonly projectFile: string;
  readonly projectId: string;
  readonly name: string;
}

export interface EditorSelectionState {
  readonly sceneNodeId: number | null;
  readonly entityId: number | null;
  readonly source: 'hierarchy' | 'renderer' | 'inspector' | null;
}

export interface StudioViewSettings {
  readonly lightingMode: StudioLightingMode;
  readonly gridVisible: boolean;
  readonly snappingEnabled: boolean;
  readonly translationSnap: number;
  readonly translationSnapAxes: readonly [number, number, number];
  readonly rotationSnapDegrees: number;
  readonly scaleSnapAxes: readonly [number, number, number];
  readonly fineMultiplier: number;
  readonly transformOrientation: StudioTransformOrientation;
  readonly theme: 'graphite' | 'highContrast';
  readonly minorColor: readonly [number, number, number, number];
  readonly majorColor: readonly [number, number, number, number];
  readonly xAxisColor: readonly [number, number, number, number];
  readonly yAxisColor: readonly [number, number, number, number];
  readonly zAxisColor: readonly [number, number, number, number];
  readonly majorLineEvery: number;
  readonly opacity: number;
  readonly fadeStart: number;
  readonly fadeEnd: number;
  readonly cameraMoveSpeed: number;
  readonly cameraBoostMultiplier: number;
  readonly invertLookY: boolean;
  readonly invertPanY: boolean;
  readonly keyboard: StudioKeyboardBindings;
}

export interface StudioUserSettingsState {
  readonly status: 'scratch' | 'loaded' | 'defaulted' | 'unsupported' | 'saving' | 'error';
  readonly projectRoot: string | null;
  readonly projectKey: string;
  readonly path: string | null;
  readonly sha256: string | null;
  readonly writesEnabled: boolean;
  readonly message: string;
}

export interface VoxelWorkspaceState {
  readonly validatedPick: VoxelPickReadout | null;
  readonly lastReadout: VoxelReadout | null;
  readonly conversion: {
    readonly plan: VoxelConversionPlan;
    readonly preview: VoxelConversionPreview;
  } | null;
  readonly objectSourceInspection: VoxelObjectSourceInspection | null;
  readonly objectConversion: {
    readonly plan: VoxelObjectConversionPlan;
    readonly preview: VoxelObjectConversionPreview;
  } | null;
  readonly objectPlayback: VoxelObjectInstancePlaybackReadout | null;
  readonly historyPreview: VoxelHistoryRevertPreview | null;
  readonly lastReceipt: ProjectMutationReceipt | null;
  readonly message: string;
}

export interface StudioWorkspaceSnapshot {
  readonly connection: StudioConnectionState;
  readonly authoringDocument: AuthoringDocumentView | null;
  readonly liveProjection: LiveProjectionView | null;
  readonly preview: TransformPreviewState | null;
  readonly selection: EditorSelectionState;
  readonly operation: 'idle' | 'opening' | 'refreshing' | 'committing' | 'asset' | 'voxel' | 'closing';
  readonly assetWorkspace: AssetWorkspaceState;
  readonly voxelWorkspace: VoxelWorkspaceState;
  readonly hierarchyFilter: string;
  readonly activeMenu: 'file' | 'edit' | 'view' | 'tools' | null;
  readonly bottomPanel: 'assets' | 'diagnostics' | 'owners' | 'output';
  readonly settingsOpen: boolean;
  readonly settings: StudioViewSettings;
  readonly userSettings: StudioUserSettingsState;
  readonly lastError: string | null;
}

type ObjectPlaybackControlAction = Extract<
  VoxelEditorAction,
  { readonly kind: 'previewObjectInstance' }
>;

interface QueuedObjectPlaybackControl {
  readonly action: ObjectPlaybackControlAction;
  readonly projectScopeGeneration: number;
  readonly objectOperationGeneration: number;
  readonly expectedProjectHash: string;
}

interface ProjectionBaseIdentity {
  readonly kind: 'project' | 'voxelObjectConversion';
  readonly generation: number;
  readonly projectScopeGeneration: number;
  readonly key: string;
}

interface CanonicalProjectProjection {
  readonly identity: ProjectionBaseIdentity;
  readonly projectHash: string;
  readonly projectScopeGeneration: number;
  readonly frame: RenderFrameDiff;
  readonly entities: readonly ProjectedEntityView[];
}

export interface StudioPlaybackTimer {
  readonly cancel: (handle: unknown) => void;
  readonly schedule: (callback: () => void, delayMilliseconds: number) => unknown;
}

interface ObjectPlaybackSchedule {
  readonly sceneId: string;
  readonly instanceId: string;
  readonly expectedProjectHash: string;
  readonly projectScopeGeneration: number;
  virtualNowMicroseconds: number;
  expectedFrameGeneration: number | null;
  timerHandle: unknown | null;
}

const DEFAULT_PLAYBACK_TIMER: StudioPlaybackTimer = {
  cancel: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
  schedule: (callback, delayMilliseconds) => setTimeout(callback, delayMilliseconds),
};

const MAX_RETAINED_OBJECT_FRAME_PATCHES = 120;

const INITIAL_ARTIFACT = buildDefaultStudioHostUserSettings('rusty-studio-project:scratch');
const INITIAL_SETTINGS = viewSettings(INITIAL_ARTIFACT);

function initialSnapshot(): StudioWorkspaceSnapshot {
  return {
    connection: { kind: 'disconnected', message: 'Studio host is not connected.' },
    authoringDocument: null,
    liveProjection: null,
    preview: null,
    selection: { sceneNodeId: null, entityId: null, source: null },
    operation: 'idle',
    assetWorkspace: {
      selectedAssetId: null,
      plan: null,
      message: 'Import or select a project asset to inspect its owner data.',
    },
    voxelWorkspace: {
      validatedPick: null,
      lastReadout: null,
      conversion: null,
      objectSourceInspection: null,
      objectConversion: null,
      objectPlayback: null,
      historyPreview: null,
      lastReceipt: null,
      message: 'Select a rendered voxel instance to begin authoring.',
    },
    hierarchyFilter: '',
    activeMenu: null,
    bottomPanel: 'diagnostics',
    settingsOpen: false,
    settings: INITIAL_SETTINGS,
    userSettings: {
      status: 'scratch',
      projectRoot: null,
      projectKey: INITIAL_ARTIFACT.projectKey,
      path: null,
      sha256: null,
      writesEnabled: false,
      message: 'Open a project to load host-user settings.',
    },
    lastError: null,
  };
}

export class StudioWorkspaceStore {
  readonly #client: StudioAdapterClient;
  readonly #settingsClient: HttpStudioUserSettingsClient | null;
  readonly #playbackTimer: StudioPlaybackTimer;
  readonly #snapshot = signal<StudioWorkspaceSnapshot>(initialSnapshot());
  readonly snapshot: Signal<StudioWorkspaceSnapshot> = this.#snapshot.asReadonly();
  #previewCommit: Promise<boolean> | null = null;
  #selectionRequest = 0;
  #settingsWriteChain: Promise<void> = Promise.resolve();
  #settingsGeneration = 0;
  #projectScopeGeneration = 0;
  #objectOperationGeneration = 0;
  #queuedObjectPlaybackControl: QueuedObjectPlaybackControl | null = null;
  #objectPlaybackSchedule: ObjectPlaybackSchedule | null = null;
  #retainedObjectFramePatches = 0;
  #projectionBaseGeneration = 0;
  #liveProjectionBase: ProjectionBaseIdentity | null = null;
  #canonicalProjectProjection: CanonicalProjectProjection | null = null;

  constructor(
    client: StudioAdapterClient,
    settingsClient: HttpStudioUserSettingsClient | null = null,
    playbackTimer: StudioPlaybackTimer = DEFAULT_PLAYBACK_TIMER,
  ) {
    this.#client = client;
    this.#settingsClient = settingsClient;
    this.#playbackTimer = playbackTimer;
  }

  async connect(): Promise<boolean> {
    const current = this.#snapshot();
    if (current.connection.kind === 'connected') return true;
    this.#patch({
      connection: { kind: 'connecting', message: 'Connecting to the project adapter…' },
      lastError: null,
    });
    try {
      const response = await this.#client.describe();
      this.#patch({
        connection: {
          kind: 'connected',
          adapter: response.adapter,
          message: `${response.adapter.adapterId} is ready. No project is open.`,
        },
      });
      return true;
    } catch (error) {
      const message = errorMessage(error);
      this.#patch({
        connection: { kind: 'unavailable', message },
        lastError: message,
      });
      return false;
    }
  }

  async openProject(root: string, projectFile: string): Promise<void> {
    if (!(await this.connect())) return;
    this.#invalidateProjectScope();
    this.#patch({ operation: 'opening', lastError: null, activeMenu: null });
    try {
      await this.#settingsWriteChain;
      const userSettings = await this.#loadUserSettings(root);
      const response = await this.#client.openProject(root, projectFile);
      this.#acceptProject(response.project, true);
      this.#acceptUserSettings(userSettings);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async createProject(input: CreateProjectInput): Promise<void> {
    if (!(await this.connect())) return;
    this.#invalidateProjectScope();
    this.#patch({ operation: 'opening', lastError: null, activeMenu: null });
    try {
      await this.#settingsWriteChain;
      const userSettings = await this.#loadUserSettings(input.root);
      const response = await this.#client.createProject(input);
      this.#acceptProject(response.project, true);
      this.#acceptUserSettings(userSettings);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async saveProjectAs(input: SaveProjectAsInput): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null) return;
    this.#invalidateProjectScope();
    this.#patch({ operation: 'committing', lastError: null, activeMenu: null });
    try {
      await this.#settingsWriteChain;
      const userSettings = await this.#loadUserSettings(input.root);
      const response = await this.#client.saveProjectAs({
        ...input,
        expectedProjectHash: document.identity.projectHash,
      });
      this.#acceptProject(response.project, true);
      this.#acceptUserSettings(userSettings);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async createScene(sceneId: string, name: string, makeEntry: boolean): Promise<void> {
    await this.#mutateProject((document) => this.#client.createScene({
      expectedProjectHash: document.identity.projectHash,
      sceneId,
      name,
      makeEntry,
    }), makeEntry);
  }

  async renameScene(sceneId: string, name: string): Promise<void> {
    await this.#mutateProject((document) => this.#client.renameScene({
      expectedProjectHash: document.identity.projectHash,
      sceneId,
      name,
    }));
  }

  async deleteScene(sceneId: string): Promise<void> {
    await this.#mutateProject((document) => this.#client.deleteScene({
      expectedProjectHash: document.identity.projectHash,
      sceneId,
    }), true);
  }

  async setEntryScene(sceneId: string): Promise<void> {
    await this.#mutateProject((document) => this.#client.setEntryScene({
      expectedProjectHash: document.identity.projectHash,
      sceneId,
    }), true);
  }

  async createSceneObject(object: StudioSceneObjectDraft): Promise<void> {
    await this.#mutateProject((document) => this.#client.createSceneObject({
      expectedProjectHash: document.identity.projectHash,
      expectedSceneRevision: document.identity.sceneRevision,
      object,
    }));
  }

  async deleteSceneObject(entityId: number): Promise<void> {
    await this.#mutateProject((document) => this.#client.deleteSceneObject({
      expectedProjectHash: document.identity.projectHash,
      expectedSceneRevision: document.identity.sceneRevision,
      entityId,
    }));
  }

  async renameSceneObject(entityId: number, name: string): Promise<void> {
    await this.#mutateProject((document) => this.#client.renameSceneObject({
      expectedProjectHash: document.identity.projectHash,
      expectedSceneRevision: document.identity.sceneRevision,
      entityId,
      name,
    }));
  }

  async reparentSceneObject(
    entityId: number,
    parentEntityId: number | null,
    childOrder: number,
  ): Promise<void> {
    await this.#mutateProject((document) => this.#client.reparentSceneObject({
      expectedProjectHash: document.identity.projectHash,
      expectedSceneRevision: document.identity.sceneRevision,
      entityId,
      parentEntityId,
      childOrder,
    }));
  }

  async setSceneObjectAppearance(
    entityId: number,
    appearance: StudioSceneAppearance,
  ): Promise<void> {
    await this.#mutateProject((document) => this.#client.setSceneObjectAppearance({
      expectedProjectHash: document.identity.projectHash,
      expectedSceneRevision: document.identity.sceneRevision,
      entityId,
      appearance,
    }));
  }

  async setEntityCollision(entityId: number, collision: StoredCollision | null): Promise<void> {
    await this.#mutateProject((document) => this.#client.setEntityCollision({
      expectedProjectHash: document.identity.projectHash,
      entityId,
      collision,
    }));
  }

  async setEntityKinematic(entityId: number, kinematic: StoredKinematic | null): Promise<void> {
    await this.#mutateProject((document) => this.#client.setEntityKinematic({
      expectedProjectHash: document.identity.projectHash,
      entityId,
      kinematic,
    }));
  }

  async refreshProject(): Promise<void> {
    if (this.#snapshot().authoringDocument === null) return;
    this.#invalidateObjectOperation();
    this.#patch({ operation: 'refreshing', lastError: null });
    try {
      const response = await this.#client.readProject();
      this.#acceptProject(response.project, false);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  selectAsset(assetId: string): void {
    if (!this.#snapshot().authoringDocument?.assetBrowser.assets.some(
      (asset) => asset.assetId === assetId,
    )) return;
    this.#patch({
      assetWorkspace: {
        ...this.#snapshot().assetWorkspace,
        selectedAssetId: assetId,
      },
    });
  }

  async prepareAssetImport(
    source: StudioFileSelection,
    settings: StudioAssetImportSettings,
  ): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null || this.#snapshot().operation !== 'idle') return;
    this.#patch({ operation: 'asset', lastError: null, activeMenu: null });
    try {
      const response = await this.#client.prepareAssetImport({
        expectedProjectHash: document.identity.projectHash,
        source,
        settings,
      });
      this.#patch({
        operation: 'idle',
        bottomPanel: 'assets',
        assetWorkspace: {
          ...this.#snapshot().assetWorkspace,
          plan: response.plan,
          message: response.plan.hasErrors
            ? 'Import plan has errors; project bytes remain unchanged.'
            : `Prepared ${response.plan.reimportKind ?? 'import'} for ${response.plan.meshAssetId ?? source.path}.`,
        },
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async prepareAssetReimport(assetId: string): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null || this.#snapshot().operation !== 'idle') return;
    this.#patch({ operation: 'asset', lastError: null });
    try {
      const response = await this.#client.prepareAssetReimport({
        expectedProjectHash: document.identity.projectHash,
        assetId,
      });
      this.#patch({
        operation: 'idle',
        bottomPanel: 'assets',
        assetWorkspace: {
          selectedAssetId: assetId,
          plan: response.plan,
          message: `Prepared ${response.plan.reimportKind ?? 'reimport'} for ${assetId}.`,
        },
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async applyAssetImport(): Promise<void> {
    const current = this.#snapshot();
    const document = current.authoringDocument;
    const plan = current.assetWorkspace.plan;
    if (document === null || plan === null || current.operation !== 'idle') return;
    this.#patch({ operation: 'asset', lastError: null });
    try {
      const response = await this.#client.applyAssetImport({
        expectedProjectHash: document.identity.projectHash,
        planId: plan.planId,
        expectedPlanHash: plan.planHash,
      });
      this.#acceptProject(response.project, false);
      const assetId = response.receipt.kind === 'assetImportApplied'
        ? response.receipt.assetId
        : plan.meshAssetId;
      this.#patch({
        assetWorkspace: {
          selectedAssetId: assetId,
          plan: null,
          message: mutationMessage(response.receipt),
        },
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async discardAssetImport(): Promise<void> {
    const plan = this.#snapshot().assetWorkspace.plan;
    if (plan === null || this.#snapshot().operation !== 'idle') return;
    this.#patch({ operation: 'asset', lastError: null });
    try {
      await this.#client.discardAssetImport({ planId: plan.planId });
      this.#patch({
        operation: 'idle',
        assetWorkspace: {
          ...this.#snapshot().assetWorkspace,
          plan: null,
          message: 'Prepared asset import discarded.',
        },
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async closeProject(): Promise<void> {
    if (this.#snapshot().connection.kind !== 'connected') return;
    this.#invalidateProjectScope();
    this.#patch({ operation: 'closing', lastError: null });
    try {
      await this.#settingsWriteChain;
      await this.#client.closeProject();
      const connection = this.#snapshot().connection;
      this.#patch({
        connection: connection.kind === 'connected'
          ? {
              ...connection,
              message: `${connection.adapter.adapterId} is ready. No project is open.`,
            }
          : connection,
        authoringDocument: null,
        liveProjection: null,
        preview: null,
        assetWorkspace: {
          selectedAssetId: null,
          plan: null,
          message: 'Import or select a project asset to inspect its owner data.',
        },
        voxelWorkspace: {
          validatedPick: null,
          lastReadout: null,
          conversion: null,
          objectSourceInspection: null,
          objectConversion: null,
          objectPlayback: null,
          historyPreview: null,
          lastReceipt: null,
          message: 'Select a rendered voxel instance to begin authoring.',
        },
        selection: { sceneNodeId: null, entityId: null, source: null },
        settings: INITIAL_SETTINGS,
        userSettings: {
          status: 'scratch',
          projectRoot: null,
          projectKey: INITIAL_ARTIFACT.projectKey,
          path: null,
          sha256: null,
          writesEnabled: false,
          message: 'Open a project to load host-user settings.',
        },
        operation: 'idle',
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  selectEntity(entityId: number | null, source: EditorSelectionState['source']): Promise<void> {
    const sceneNodeId = entityId === null
      ? null
      : (this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
          (node) => node.entityId === entityId,
        )?.nodeId ?? null);
    return this.#requestSelection({
      sceneNodeId,
      entityId,
      source: entityId === null ? null : source,
    });
  }

  selectHierarchyNode(sceneNodeId: number): Promise<void> {
    const node = this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.nodeId === sceneNodeId,
    );
    if (node === undefined) return Promise.resolve();
    return this.#requestSelection({
      sceneNodeId: node.nodeId,
      entityId: node.entityId,
      source: 'hierarchy',
    });
  }

  async #requestSelection(selection: EditorSelectionState): Promise<void> {
    const request = ++this.#selectionRequest;
    const preview = this.#snapshot().preview;
    if (preview === null || preview.entityId === selection.entityId) {
      this.#patch({ selection });
      return;
    }
    const committed = await this.commitPreview();
    if (!committed || request !== this.#selectionRequest) return;
    this.#patch({ selection, preview: null });
  }

  beginTranslationPreview(entityId: number): void {
    this.beginTransformPreview(entityId, 'translate', this.#snapshot().settings.transformOrientation);
  }

  beginTransformPreview(
    entityId: number,
    tool: StudioTransformTool,
    orientation: StudioTransformOrientation,
  ): void {
    const current = this.#snapshot();
    if (current.operation !== 'idle') return;
    const existing = current.preview;
    if (existing?.entityId === entityId) {
      this.#patch({ preview: { ...existing, tool, orientation }, lastError: null });
      return;
    }
    const node = current.authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.entityId === entityId,
    );
    if (node === undefined) return;
    const translation = node.localTransform.translation;
    this.#patch({
      selection: current.selection.entityId === entityId
        ? current.selection
        : { sceneNodeId: node.nodeId, entityId, source: 'inspector' },
      preview: {
        entityId,
        original: node.localTransform,
        tool,
        orientation,
        translation,
        rotation: node.localTransform.rotation,
        scale: node.localTransform.scale,
      },
      lastError: null,
    });
  }

  applyPreviewWorldTransform(world: Transform): void {
    const current = this.#snapshot();
    const preview = current.preview;
    if (preview === null || !validTransform(world)) return;
    const node = current.authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.entityId === preview.entityId,
    );
    if (node === undefined) return;
    const parentWorld = node.parentNodeId === null
      ? null
      : current.authoringDocument?.sceneHierarchy.nodes.find(
          (candidate) => candidate.nodeId === node.parentNodeId,
        )?.worldTransform ?? null;
    const local = localTransformFromWorld(parentWorld, world);
    if (!validTransform(local)) return;
    this.#patch({
      preview: {
        ...preview,
        translation: local.translation,
        rotation: local.rotation,
        scale: local.scale,
      },
    });
  }

  setPreviewOrientation(orientation: StudioTransformOrientation): void {
    const preview = this.#snapshot().preview;
    if (preview !== null) this.#patch({ preview: { ...preview, orientation } });
  }

  setPreviewTool(tool: StudioTransformTool, orientation: StudioTransformOrientation): void {
    const preview = this.#snapshot().preview;
    if (preview !== null) this.#patch({ preview: { ...preview, tool, orientation } });
  }

  setPreviewTranslationAxis(axis: 0 | 1 | 2, value: number): void {
    const preview = this.#snapshot().preview;
    if (preview === null) return;
    const translation: [number, number, number] = [...preview.translation];
    translation[axis] = value;
    this.#patch({ preview: { ...preview, translation } });
  }

  setPreviewRotationAxis(axis: 0 | 1 | 2 | 3, value: number): void {
    const preview = this.#snapshot().preview;
    if (preview === null) return;
    const rotation: [number, number, number, number] = [...preview.rotation];
    rotation[axis] = value;
    this.#patch({ preview: { ...preview, rotation } });
  }

  setPreviewScaleAxis(axis: 0 | 1 | 2, value: number): void {
    const preview = this.#snapshot().preview;
    if (preview === null) return;
    const scale: [number, number, number] = [...preview.scale];
    scale[axis] = value;
    this.#patch({ preview: { ...preview, scale } });
  }

  revertPreview(): void {
    this.#patch({ preview: null, lastError: null });
  }

  commitPreview(): Promise<boolean> {
    if (this.#previewCommit !== null) return this.#previewCommit;
    const pending = this.#commitPreview();
    this.#previewCommit = pending;
    void pending.finally(() => {
      if (this.#previewCommit === pending) this.#previewCommit = null;
    });
    return pending;
  }

  async #commitPreview(): Promise<boolean> {
    const current = this.#snapshot();
    const preview = current.preview;
    const document = current.authoringDocument;
    if (preview === null || document === null) return true;
    if (current.operation !== 'idle') return false;
    const candidate: Transform = {
      translation: preview.translation,
      rotation: preview.rotation,
      scale: preview.scale,
    };
    if (sameTransform(candidate, preview.original)) {
      this.#patch({ preview: null, lastError: null });
      return true;
    }
    this.#patch({ operation: 'committing', lastError: null });
    try {
      const response = await this.#client.setSceneObjectTransform({
        expectedProjectHash: document.identity.projectHash,
        expectedSceneRevision: document.identity.sceneRevision,
        entityId: preview.entityId,
        transform: candidate,
      });
      this.#acceptProject(response.project, false);
      return true;
    } catch (error) {
      this.#operationFailed(error);
      return false;
    }
  }

  async validateVoxelViewportPick(candidate: VoxelViewportPickCandidate): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null || this.#snapshot().operation !== 'idle') return;
    const instance = document.voxelAuthoring.instances.find(
      (entry) => entry.instance.instanceId === candidate.instanceId,
    );
    if (instance === undefined) {
      this.reportUiError(`Renderer named unknown voxel instance ${candidate.instanceId}.`);
      return;
    }
    const asset = document.voxelAuthoring.assets.find(
      (entry) => entry.inspection.assetId === instance.instance.voxelAssetId,
    );
    if (asset === undefined) {
      this.reportUiError(`Voxel instance ${candidate.instanceId} has no authoring asset readout.`);
      return;
    }
    const input = deriveVoxelPickValidation(candidate, instance.sceneId, instance.instance, asset);
    if (input === null) {
      this.reportUiError('Renderer voxel hint could not be converted into a finite authored-cell claim.');
      return;
    }
    this.#patch({ operation: 'voxel', lastError: null });
    try {
      const response = await this.#client.validateVoxelPick({
        expectedProjectHash: document.identity.projectHash,
        ...input,
      });
      this.#patch({
        operation: 'idle',
        voxelWorkspace: {
          ...this.#snapshot().voxelWorkspace,
          validatedPick: response.anchor,
          message: `Validated ${response.anchor.instanceId} voxel ${response.anchor.hitVoxel.join(', ')}.`,
        },
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async runVoxelAction(requestedAction: VoxelEditorAction): Promise<void> {
    const action = this.#prepareObjectPlaybackAction(requestedAction);
    const current = this.#snapshot();
    const document = current.authoringDocument;
    if (document === null) return;
    if (current.operation !== 'idle') {
      if (current.operation === 'voxel' && isObjectPlaybackControl(action)) {
        this.#queuedObjectPlaybackControl = {
          action,
          projectScopeGeneration: this.#projectScopeGeneration,
          objectOperationGeneration: this.#objectOperationGeneration,
          expectedProjectHash: document.identity.projectHash,
        };
      }
      return;
    }
    if (!objectCandidateActionIsCurrent(action, current.voxelWorkspace.objectConversion)) return;
    const expectedProjectHash = document.identity.projectHash;
    const projectScopeGeneration = this.#projectScopeGeneration;
    const objectAction = isVoxelObjectAction(action);
    const objectOperationGeneration = objectAction
      ? ++this.#objectOperationGeneration
      : this.#objectOperationGeneration;
    this.#patch({ operation: 'voxel', lastError: null });
    try {
      switch (action.kind) {
        case 'upsertMaterial':
          this.#acceptVoxelMutation(await this.#client.upsertMaterial({
            expectedProjectHash,
            assetId: action.assetId,
            definition: action.definition,
          }));
          return;
        case 'initializeAsset':
          this.#acceptVoxelMutation(await this.#client.initializeVoxelAsset({
            expectedProjectHash,
            assetId: action.assetId,
            cellSize: action.cellSize,
            chunkSize: action.chunkSize,
            origin: action.origin,
            bounds: action.bounds,
            materialPalette: action.materialPalette,
            initialMaterialSlot: action.initialMaterialSlot,
          }));
          return;
        case 'duplicateAsset':
          this.#acceptVoxelMutation(await this.#client.duplicateVoxelAsset({
            expectedProjectHash,
            sourceAssetId: action.sourceAssetId,
            expectedSourceContentHash: action.expectedSourceContentHash,
            targetAssetId: action.targetAssetId,
          }));
          return;
        case 'attachInstance':
          this.#acceptVoxelMutation(await this.#client.attachVoxelInstance({
            expectedProjectHash,
            sceneId: action.sceneId,
            instance: action.instance,
          }));
          return;
        case 'setInstanceTransform':
          this.#acceptVoxelMutation(await this.#client.setVoxelInstanceTransform({
            expectedProjectHash,
            sceneId: action.sceneId,
            instanceId: action.instanceId,
            translation: action.translation,
            rotation: action.rotation,
            scale: action.scale,
          }));
          return;
        case 'removeInstance':
          this.#acceptVoxelMutation(await this.#client.removeVoxelInstance({
            expectedProjectHash,
            sceneId: action.sceneId,
            instanceId: action.instanceId,
          }));
          return;
        case 'replacePalette':
          this.#acceptVoxelMutation(await this.#client.replaceVoxelPalette({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            expectedVoxelDataHash: action.expectedVoxelDataHash,
            replacement: action.replacement,
          }));
          return;
        case 'applyBrush':
          this.#acceptVoxelMutation(await this.#client.applyVoxelBrush({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            center: action.center,
            radius: action.radius,
            mode: action.mode,
            materialSlot: action.materialSlot,
          }));
          return;
        case 'applyPrimitive':
          this.#acceptVoxelMutation(await this.#client.applyVoxelPrimitive({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            request: action.request,
          }));
          return;
        case 'initializeTemplate':
          this.#acceptVoxelMutation(await this.#client.initializeVoxelTemplate({
            expectedProjectHash,
            assetId: action.assetId,
            cellSize: action.cellSize,
            chunkSize: action.chunkSize,
            materialPalette: action.materialPalette,
            request: action.request,
          }));
          return;
        case 'importAssetFile':
          this.#acceptVoxelMutation(await this.#client.importVoxelAssetFile({
            expectedProjectHash,
            sourcePath: action.sourcePath,
            targetAssetId: action.targetAssetId,
          }));
          return;
        case 'exportAssetFile': {
          const response = await this.#client.exportVoxelAssetFile({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            targetPath: action.targetPath,
            ...(action.expectedTargetSha256 === undefined
              ? {}
              : { expectedTargetSha256: action.expectedTargetSha256 }),
          });
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              message: `Exported ${response.assetId} to ${response.targetPath} (${response.sha256}).`,
            },
          });
          return;
        }
        case 'materializeEnvironment':
          this.#acceptVoxelMutation(await this.#client.materializeEnvironment({
            expectedProjectHash,
            expectedSceneRevision: document.identity.sceneRevision,
            sceneId: action.sceneId,
            preset: action.preset,
            seed: action.seed,
            voxelAssetId: action.voxelAssetId,
            voxelInstanceId: action.voxelInstanceId,
            voxelTranslation: action.voxelTranslation,
            playerEntityId: action.playerEntityId,
            exitEntityId: action.exitEntityId,
            wallMaterial: action.wallMaterial,
            floorMaterial: action.floorMaterial,
            accentMaterial: action.accentMaterial,
            materialPalette: action.materialPalette,
          }));
          return;
        case 'undo':
        case 'redo': {
          const input = {
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
          };
          const response = action.kind === 'undo'
            ? await this.#client.undoVoxelEdit(input)
            : await this.#client.redoVoxelEdit(input);
          this.#acceptVoxelMutation(response);
          return;
        }
        case 'revert':
          this.#acceptVoxelMutation(await this.#client.revertVoxelHistory({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            targetCursor: action.targetCursor,
          }));
          return;
        case 'queryHistory': {
          const response = await this.#client.queryVoxelHistory({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            maxEntries: action.maxEntries,
            maxDeltasPerEntry: action.maxDeltasPerEntry,
          });
          this.#acceptVoxelReadout(response.readout, 'Bounded voxel history query completed.');
          return;
        }
        case 'prepareHistoryRevert': {
          const response = await this.#client.prepareVoxelHistoryRevert({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            targetCursor: action.targetCursor,
            maxSamples: action.maxSamples,
          });
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              historyPreview: response.preview,
              message: `Prepared history move to cursor ${String(response.preview.cursorAfter)}.`,
            },
          });
          return;
        }
        case 'applyHistoryRevert':
          this.#acceptVoxelMutation(await this.#client.applyVoxelHistoryRevert({
            expectedProjectHash,
            previewId: action.previewId,
          }));
          return;
        case 'discardHistoryRevert':
          await this.#client.discardVoxelHistoryRevert({ previewId: action.previewId });
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              historyPreview: null,
              message: 'Prepared history move discarded.',
            },
          });
          return;
        case 'createAnnotation':
          this.#acceptVoxelMutation(await this.#client.createVoxelAnnotationLayer({
            expectedProjectHash,
            assetId: action.assetId,
            draft: action.draft,
          }));
          return;
        case 'editAnnotation':
          this.#acceptVoxelMutation(await this.#client.editVoxelAnnotation({
            expectedProjectHash,
            assetId: action.assetId,
            layerId: action.layerId,
            transaction: action.transaction,
          }));
          return;
        case 'queryAnnotation': {
          const response = await this.#client.queryVoxelAnnotation({
            expectedProjectHash,
            assetId: action.assetId,
            layerId: action.layerId,
            query: action.query,
          });
          this.#acceptVoxelReadout(response.readout, 'Annotation query completed.');
          return;
        }
        case 'exportAnnotation': {
          const response = await this.#client.exportVoxelAnnotation({
            expectedProjectHash,
            assetId: action.assetId,
            layerId: action.layerId,
            expectedLayerHash: action.expectedLayerHash,
          });
          this.#acceptVoxelReadout(response.readout, 'Canonical annotation export is ready.');
          return;
        }
        case 'queryModel': {
          const response = await this.#client.queryVoxelModel({
            expectedProjectHash,
            assetId: action.assetId,
            expectedAssetContentHash: action.expectedAssetContentHash,
            ...(action.window === undefined ? {} : { window: action.window }),
          });
          this.#acceptVoxelReadout(response.readout, 'Bounded voxel model query completed.');
          return;
        }
        case 'prepareConversion': {
          const response = await this.#client.prepareVoxelConversion({
            expectedProjectHash,
            sourceAssetId: action.sourceAssetId,
            source: action.source,
            targetAssetId: action.targetAssetId,
            ...(action.license === undefined ? {} : { license: action.license }),
            ...(action.meshPrimitive === undefined ? {} : { meshPrimitive: action.meshPrimitive }),
            settings: action.settings,
            maxPreviewSamples: action.maxPreviewSamples,
          });
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              conversion: { plan: response.plan, preview: response.preview },
              message: `Prepared ${String(response.preview.outputVoxelCount)}-voxel conversion.`,
            },
          });
          return;
        }
        case 'applyConversion':
          this.#acceptVoxelMutation(await this.#client.applyVoxelConversion({
            expectedProjectHash,
            planId: action.planId,
            expectedPlanHash: action.expectedPlanHash,
            expectedOutputHash: action.expectedOutputHash,
          }), true);
          return;
        case 'discardConversion':
          await this.#client.discardVoxelConversion({ planId: action.planId });
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              conversion: null,
              message: 'Prepared conversion discarded.',
            },
          });
          return;
        case 'inspectObjectSource': {
          const response = await this.#client.inspectVoxelObjectSource({
            expectedProjectHash,
            sourceKind: action.sourceKind,
            sourceAssetId: action.sourceAssetId,
            source: action.source,
            ...(action.meshPrimitive === undefined ? {} : { meshPrimitive: action.meshPrimitive }),
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              objectSourceInspection: response.inspection,
              message: `Inspected ${String(response.inspection.metadata.vertexCount)} vertices, ${String(response.inspection.metadata.triangleCount)} triangles, and ${String(response.inspection.clips.length)} clips.`,
            },
          });
          return;
        }
        case 'prepareObjectConversion': {
          const response = await this.#client.prepareVoxelObjectConversion({
            expectedProjectHash,
            sourceKind: action.sourceKind,
            sourceAssetId: action.sourceAssetId,
            source: action.source,
            targetAssetId: action.targetAssetId,
            ...(action.license === undefined ? {} : { license: action.license }),
            ...(action.meshPrimitive === undefined ? {} : { meshPrimitive: action.meshPrimitive }),
            settings: action.settings,
            clips: action.clips,
            ...(action.defaultClip === undefined ? {} : { defaultClip: action.defaultClip }),
            frame: action.frame,
            maxPreviewSamples: action.maxPreviewSamples,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          this.#acceptCompleteObjectProjection(
            response.projection,
            response.projectionReadout,
            this.#newProjectionBase(
              'voxelObjectConversion',
              `${response.plan.planId}@${response.plan.planHash}`,
            ),
          );
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              objectConversion: { plan: response.plan, preview: response.preview },
              message: `Prepared ${String(response.preview.storedFrameCount)} stored frames with ${String(response.preview.aggregateVoxelCount)} aggregate voxels.`,
            },
          });
          return;
        }
        case 'previewObjectFrame': {
          const response = await this.#client.previewVoxelObjectConversion({
            planId: action.planId,
            expectedPlanHash: action.expectedPlanHash,
            frame: action.frame,
            maxPreviewSamples: action.maxPreviewSamples,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          this.#acceptCompleteObjectProjection(
            response.projection,
            response.projectionReadout,
            this.#newProjectionBase(
              'voxelObjectConversion',
              `${action.planId}@${action.expectedPlanHash}`,
            ),
          );
          const current = this.#snapshot().voxelWorkspace.objectConversion;
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              objectConversion: current === null
                ? null
                : { plan: current.plan, preview: response.preview },
              message: `Previewing ${String(response.preview.selectedFrame.voxelCount)} voxels in the selected stored frame.`,
            },
          });
          return;
        }
        case 'applyObjectConversion': {
          const response = await this.#client.applyVoxelObjectConversion({
            expectedProjectHash,
            planId: action.planId,
            expectedPlanHash: action.expectedPlanHash,
            expectedOutputHash: action.expectedOutputHash,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          this.#acceptVoxelMutation(response, false, true);
          return;
        }
        case 'discardObjectConversion': {
          const response = await this.#client.discardVoxelObjectConversion({
            planId: action.planId,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          const canonical = this.#currentCanonicalProjectProjection();
          if (canonical === null) {
            throw new Error('Canonical project projection is unavailable after conversion discard.');
          }
          this.#acceptCompleteObjectProjection(
            response.projection,
            response.projectionReadout,
            canonical.identity,
          );
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              objectConversion: null,
              message: 'Prepared voxel-object conversion discarded.',
            },
          });
          return;
        }
        case 'attachObjectInstance': {
          const response = await this.#client.attachVoxelObjectInstance({
            expectedProjectHash,
            sceneId: action.sceneId,
            instance: action.instance,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          this.#acceptVoxelMutation(response);
          return;
        }
        case 'previewObjectInstance': {
          const response = await this.#client.previewVoxelObjectInstance({
            expectedProjectHash,
            sceneId: action.sceneId,
            instanceId: action.instanceId,
            nowMicroseconds: action.nowMicroseconds,
            command: action.command,
          });
          if (!this.#objectResponseIsCurrent(
            action,
            expectedProjectHash,
            projectScopeGeneration,
            objectOperationGeneration,
          )) return;
          const frameGeneration = this.#acceptCanonicalObjectProjection(
            response.projection,
            response.projectionReadout,
          );
          this.#patch({
            operation: 'idle',
            voxelWorkspace: {
              ...this.#snapshot().voxelWorkspace,
              objectPlayback: response.playback,
              message: response.playback.status === 'stopped'
                ? `Restored ${response.playback.instanceId} to its saved initial pose.`
                : `${response.playback.status === 'playing' ? 'Playing' : 'Previewing'} ${response.playback.instanceId} · ${response.playback.clipId ?? 'saved pose'} frame ${String(response.playback.clipFrame ?? 0)}.`,
            },
          });
          this.#acceptObjectPlaybackResponse(action, response.playback, frameGeneration);
          return;
        }
      }
    } catch (error) {
      if (objectAction && !this.#objectResponseIsCurrent(
        action,
        expectedProjectHash,
        projectScopeGeneration,
        objectOperationGeneration,
      )) return;
      if (action.kind === 'previewObjectInstance') this.#clearObjectPlaybackSchedule();
      this.#operationFailed(error);
    } finally {
      this.#drainQueuedObjectPlaybackControl();
    }
  }

  /**
   * Confirms that the shared renderer accepted a projection generation.
   * Playback waits for this acknowledgement before displaying the current
   * pose for its authored duration and requesting exactly one successor pose.
   */
  acknowledgeProjectionGeneration(generation: number): void {
    const schedule = this.#objectPlaybackSchedule;
    const current = this.#snapshot();
    if (
      schedule === null
      || schedule.expectedFrameGeneration !== generation
      || schedule.timerHandle !== null
      || current.operation !== 'idle'
      || current.liveProjection?.generation !== generation
      || schedule.projectScopeGeneration !== this.#projectScopeGeneration
      || current.authoringDocument?.identity.projectHash !== schedule.expectedProjectHash
    ) return;
    const playback = current.voxelWorkspace.objectPlayback;
    if (
      playback?.status !== 'playing'
      || playback.ended
      || playback.sceneId !== schedule.sceneId
      || playback.instanceId !== schedule.instanceId
    ) {
      this.#clearObjectPlaybackSchedule();
      return;
    }
    const stepMicroseconds = objectPlaybackStepMicroseconds(
      current.authoringDocument,
      playback,
    );
    if (stepMicroseconds === null) {
      this.#clearObjectPlaybackSchedule();
      this.reportUiError('Voxel-object playback frame has no usable authored duration.');
      return;
    }
    schedule.expectedFrameGeneration = null;
    schedule.timerHandle = this.#playbackTimer.schedule(() => {
      if (this.#objectPlaybackSchedule !== schedule) return;
      schedule.timerHandle = null;
      const latest = this.#snapshot();
      if (
        latest.operation !== 'idle'
        || schedule.projectScopeGeneration !== this.#projectScopeGeneration
        || latest.authoringDocument?.identity.projectHash !== schedule.expectedProjectHash
        || latest.voxelWorkspace.objectPlayback?.status !== 'playing'
      ) {
        this.#clearObjectPlaybackSchedule();
        return;
      }
      const nextNow = schedule.virtualNowMicroseconds + stepMicroseconds;
      if (!Number.isSafeInteger(nextNow)) {
        this.#clearObjectPlaybackSchedule();
        this.reportUiError('Voxel-object playback virtual clock exceeded the safe integer range.');
        return;
      }
      schedule.virtualNowMicroseconds = nextNow;
      void this.runVoxelAction({
        kind: 'previewObjectInstance',
        sceneId: schedule.sceneId,
        instanceId: schedule.instanceId,
        nowMicroseconds: nextNow,
        command: { kind: 'sample' },
      });
    }, Math.max(1, Math.ceil(stepMicroseconds / 1_000)));
  }

  #prepareObjectPlaybackAction(requested: VoxelEditorAction): VoxelEditorAction {
    if (requested.kind !== 'previewObjectInstance' || requested.command.kind === 'sample') {
      return requested;
    }
    const schedule = this.#objectPlaybackSchedule;
    const nowMicroseconds = requested.command.kind === 'pause'
      && schedule?.sceneId === requested.sceneId
      && schedule.instanceId === requested.instanceId
      ? schedule.virtualNowMicroseconds
      : requested.nowMicroseconds;
    this.#clearObjectPlaybackSchedule();
    return nowMicroseconds === requested.nowMicroseconds
      ? requested
      : { ...requested, nowMicroseconds };
  }

  #acceptObjectPlaybackResponse(
    action: ObjectPlaybackControlAction,
    playback: VoxelObjectInstancePlaybackReadout,
    frameGeneration: number,
  ): void {
    let schedule = this.#objectPlaybackSchedule;
    if (action.command.kind === 'play') {
      schedule = {
        sceneId: action.sceneId,
        instanceId: action.instanceId,
        expectedProjectHash: playback.projectHash,
        projectScopeGeneration: this.#projectScopeGeneration,
        virtualNowMicroseconds: action.nowMicroseconds,
        expectedFrameGeneration: null,
        timerHandle: null,
      };
      this.#objectPlaybackSchedule = schedule;
    } else if (action.command.kind !== 'sample') {
      return;
    }
    if (
      schedule === null
      || schedule.sceneId !== action.sceneId
      || schedule.instanceId !== action.instanceId
      || playback.status !== 'playing'
      || playback.ended
    ) {
      this.#clearObjectPlaybackSchedule();
      return;
    }
    schedule.virtualNowMicroseconds = action.nowMicroseconds;
    schedule.expectedFrameGeneration = frameGeneration;
  }

  #clearObjectPlaybackSchedule(): void {
    const schedule = this.#objectPlaybackSchedule;
    if (schedule?.timerHandle !== null && schedule?.timerHandle !== undefined) {
      this.#playbackTimer.cancel(schedule.timerHandle);
    }
    this.#objectPlaybackSchedule = null;
  }

  #drainQueuedObjectPlaybackControl(): void {
    const queued = this.#queuedObjectPlaybackControl;
    if (queued === null) return;
    const current = this.#snapshot();
    if (
      queued.projectScopeGeneration !== this.#projectScopeGeneration
      || queued.objectOperationGeneration !== this.#objectOperationGeneration
      || current.authoringDocument?.identity.projectHash !== queued.expectedProjectHash
    ) {
      this.#queuedObjectPlaybackControl = null;
      return;
    }
    if (current.operation !== 'idle') return;
    this.#queuedObjectPlaybackControl = null;
    void this.runVoxelAction(queued.action);
  }

  setHierarchyFilter(value: string): void {
    this.#patch({ hierarchyFilter: value });
  }

  visibleHierarchyNodes(): readonly SceneHierarchyNodeReadout[] {
    const state = this.#snapshot();
    const query = state.hierarchyFilter.trim().toLocaleLowerCase();
    const nodes = [...(state.authoringDocument?.sceneHierarchy.nodes ?? [])]
      .sort((left, right) => left.displayOrder - right.displayOrder);
    if (query.length === 0) return nodes;
    return nodes.filter((node) =>
      `${node.label} ${String(node.nodeId)} ${String(node.entityId ?? '')} ${node.nodeKind} ${node.asset ?? ''} ${node.tags.join(' ')}`
        .toLocaleLowerCase()
        .includes(query),
    );
  }

  selectedHierarchyNode(): SceneHierarchyNodeReadout | null {
    const state = this.#snapshot();
    return state.authoringDocument?.sceneHierarchy.nodes.find(
      (node) => node.nodeId === state.selection.sceneNodeId,
    ) ?? null;
  }

  selectedEntity(): ProjectedEntityView | null {
    const state = this.#snapshot();
    return state.liveProjection?.entities.find(
      (entity) => entity.entityId === state.selection.entityId,
    ) ?? null;
  }

  selectedVoxelObjectInstance(): VoxelObjectInstanceReadout | null {
    const state = this.#snapshot();
    const entityId = state.selection.entityId;
    if (entityId === null) return null;
    return state.authoringDocument?.voxelObjectAuthoring.instances.find(
      (entry) => entry.ownerEntityId === entityId,
    ) ?? null;
  }

  selectedVoxelObjectAsset(): VoxelObjectAssetAuthoringReadout | null {
    const state = this.#snapshot();
    const instance = this.selectedVoxelObjectInstance();
    if (instance === null) return null;
    return state.authoringDocument?.voxelObjectAuthoring.assets.find(
      (asset) => asset.assetId === instance.instance.voxelObjectAssetId,
    ) ?? null;
  }

  toggleMenu(menu: StudioWorkspaceSnapshot['activeMenu']): void {
    this.#patch({ activeMenu: this.#snapshot().activeMenu === menu ? null : menu });
  }

  setBottomPanel(panel: StudioWorkspaceSnapshot['bottomPanel']): void {
    this.#patch({ bottomPanel: panel });
  }

  setSettingsOpen(open: boolean): void {
    this.#patch({ settingsOpen: open, activeMenu: null });
  }

  updateSettings(update: Partial<StudioViewSettings>): void {
    const current = this.#snapshot();
    const next = {
      ...current.settings,
      ...update,
      keyboard: update.keyboard === undefined
        ? current.settings.keyboard
        : { ...current.settings.keyboard, ...update.keyboard },
    };
    try {
      validateStudioHostUserSettings(settingsArtifact(current.userSettings.projectKey, next));
    } catch (error) {
      this.reportUiError(errorMessage(error));
      return;
    }
    this.#patch({ settings: next });
    this.#queueUserSettingsWrite();
  }

  setKeyboardBinding(key: keyof StudioKeyboardBindings, code: string): void {
    const binding = code.trim();
    if (binding.length === 0) return;
    this.updateSettings({
      keyboard: { ...this.#snapshot().settings.keyboard, [key]: binding },
    });
  }

  setGridColor(
    key: 'minorColor' | 'majorColor' | 'xAxisColor' | 'yAxisColor' | 'zAxisColor',
    hex: string,
  ): void {
    const rgb = parseHexColor(hex);
    if (rgb === null) return;
    const current = this.#snapshot().settings[key];
    this.updateSettings({ [key]: [...rgb, current[3]] });
  }

  restoreSceneViewDefaults(): void {
    const current = this.#snapshot();
    const defaults = viewSettings(buildDefaultStudioHostUserSettings(current.userSettings.projectKey));
    this.updateSettings({
      lightingMode: defaults.lightingMode,
      gridVisible: defaults.gridVisible,
      snappingEnabled: defaults.snappingEnabled,
      translationSnap: defaults.translationSnap,
      translationSnapAxes: defaults.translationSnapAxes,
      rotationSnapDegrees: defaults.rotationSnapDegrees,
      scaleSnapAxes: defaults.scaleSnapAxes,
      fineMultiplier: defaults.fineMultiplier,
      transformOrientation: defaults.transformOrientation,
      minorColor: defaults.minorColor,
      majorColor: defaults.majorColor,
      xAxisColor: defaults.xAxisColor,
      yAxisColor: defaults.yAxisColor,
      zAxisColor: defaults.zAxisColor,
      majorLineEvery: defaults.majorLineEvery,
      opacity: defaults.opacity,
      fadeStart: defaults.fadeStart,
      fadeEnd: defaults.fadeEnd,
      cameraMoveSpeed: defaults.cameraMoveSpeed,
      cameraBoostMultiplier: defaults.cameraBoostMultiplier,
      invertLookY: defaults.invertLookY,
      invertPanY: defaults.invertPanY,
    });
  }

  async reloadUserSettings(): Promise<void> {
    const root = this.#snapshot().userSettings.projectRoot;
    if (root === null || this.#settingsClient === null) return;
    try {
      await this.#settingsWriteChain;
      this.#acceptUserSettings(await this.#settingsClient.load(root));
    } catch (error) {
      this.#patch({
        userSettings: {
          ...this.#snapshot().userSettings,
          status: 'error',
          writesEnabled: false,
          message: errorMessage(error),
        },
        lastError: errorMessage(error),
      });
    }
  }

  clearError(): void {
    this.#patch({ lastError: null });
  }

  reportUiError(message: string): void {
    this.#patch({ lastError: message });
  }

  #acceptVoxelMutation(
    response: ProjectMutationAppliedResponse,
    clearConversion = false,
    clearObjectConversion = false,
  ): void {
    this.#acceptProject(response.project, false);
    this.#patch({
      voxelWorkspace: {
        ...this.#snapshot().voxelWorkspace,
        validatedPick: null,
        lastReceipt: response.receipt,
        historyPreview: null,
        conversion: clearConversion ? null : this.#snapshot().voxelWorkspace.conversion,
        objectConversion: clearObjectConversion
          ? null
          : this.#snapshot().voxelWorkspace.objectConversion,
        message: mutationMessage(response.receipt),
      },
    });
  }

  #acceptCompleteObjectProjection(
    frame: RenderFrameDiff,
    readout: ProjectionReadout<ProjectionFrameKind>,
    base: ProjectionBaseIdentity,
  ): number {
    if (readout.frameKind !== 'complete') {
      throw new Error(`Complete ${base.kind} projection has an incremental readout.`);
    }
    if (frame.ops.length > 0 && isVoxelObjectFramePatch(frame)) {
      throw new Error(`Complete ${base.kind} projection contained only a retained frame patch.`);
    }
    const current = this.#snapshot();
    const ownerEntityIds = current.authoringDocument?.inspections.entityState.entityIds ?? [];
    this.#retainedObjectFramePatches = 0;
    const generation = (current.liveProjection?.generation ?? 0) + 1;
    this.#liveProjectionBase = base;
    this.#patch({
      liveProjection: {
        frame,
        framePatch: null,
        readout,
        entities: summarizeProjectionForUi(frame, ownerEntityIds, []),
        generation,
      },
    });
    return generation;
  }

  #acceptCanonicalObjectProjection(
    frame: RenderFrameDiff,
    readout: ProjectionReadout<ProjectionFrameKind>,
  ): number {
    const current = this.#snapshot();
    const canonical = this.#currentCanonicalProjectProjection();
    if (canonical === null) {
      throw new Error('Applied voxel-object playback has no current canonical project projection.');
    }
    if (!isVoxelObjectFramePatch(frame)) {
      return this.#acceptCompleteObjectProjection(frame, readout, canonical.identity);
    }

    const liveBaseIsCanonical = sameProjectionBase(this.#liveProjectionBase, canonical.identity)
      && current.liveProjection !== null;
    const base = liveBaseIsCanonical ? current.liveProjection : canonical;
    const compacted = compactVoxelObjectFramePatch(base.frame, frame);
    if (compacted === null) {
      throw new Error('Applied voxel-object frame patch does not match its canonical project base.');
    }

    let framePatch: RenderFrameDiff | null = null;
    if (liveBaseIsCanonical) {
      this.#retainedObjectFramePatches += 1;
      if (this.#retainedObjectFramePatches < MAX_RETAINED_OBJECT_FRAME_PATCHES) {
        framePatch = frame;
      } else {
        this.#retainedObjectFramePatches = 0;
      }
    } else {
      this.#retainedObjectFramePatches = 0;
    }

    const generation = (current.liveProjection?.generation ?? 0) + 1;
    this.#liveProjectionBase = canonical.identity;
    this.#patch({
      liveProjection: {
        frame: compacted,
        framePatch,
        readout,
        entities: base.entities,
        generation,
      },
    });
    return generation;
  }

  #newProjectionBase(
    kind: ProjectionBaseIdentity['kind'],
    key: string,
  ): ProjectionBaseIdentity {
    return {
      kind,
      generation: ++this.#projectionBaseGeneration,
      projectScopeGeneration: this.#projectScopeGeneration,
      key,
    };
  }

  #currentCanonicalProjectProjection(): CanonicalProjectProjection | null {
    const canonical = this.#canonicalProjectProjection;
    const projectHash = this.#snapshot().authoringDocument?.identity.projectHash;
    return canonical !== null
      && canonical.projectScopeGeneration === this.#projectScopeGeneration
      && canonical.projectHash === projectHash
      ? canonical
      : null;
  }

  #invalidateProjectScope(): void {
    this.#projectScopeGeneration += 1;
    this.#liveProjectionBase = null;
    this.#canonicalProjectProjection = null;
    this.#invalidateObjectOperation();
  }

  #invalidateObjectOperation(): void {
    this.#objectOperationGeneration += 1;
    this.#queuedObjectPlaybackControl = null;
    this.#clearObjectPlaybackSchedule();
    this.#retainedObjectFramePatches = 0;
  }

  #objectResponseIsCurrent(
    action: VoxelEditorAction,
    expectedProjectHash: string,
    projectScopeGeneration: number,
    objectOperationGeneration: number,
  ): boolean {
    const current = this.#snapshot();
    return projectScopeGeneration === this.#projectScopeGeneration
      && objectOperationGeneration === this.#objectOperationGeneration
      && current.operation === 'voxel'
      && current.authoringDocument?.identity.projectHash === expectedProjectHash
      && objectCandidateActionIsCurrent(action, current.voxelWorkspace.objectConversion);
  }

  async #mutateProject(
    run: (document: AuthoringDocumentView) => Promise<ProjectMutationAppliedResponse>,
    resetSelection = false,
  ): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null || this.#snapshot().operation !== 'idle') return;
    this.#patch({ operation: 'committing', lastError: null, activeMenu: null });
    try {
      const response = await run(document);
      this.#acceptProject(response.project, resetSelection);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  #acceptVoxelReadout(readout: VoxelReadout, message: string): void {
    this.#patch({
      operation: 'idle',
      voxelWorkspace: {
        ...this.#snapshot().voxelWorkspace,
        lastReadout: readout,
        message,
      },
    });
  }

  #acceptProject(project: StudioProjectReadout, resetSelection: boolean): void {
    this.#invalidateObjectOperation();
    const current = this.#snapshot();
    const entities = summarizeProjectionForUi(
      project.projection,
      project.inspections.entityState.entityIds,
      [],
    );
    const projectionBase = this.#newProjectionBase('project', project.identity.projectHash);
    this.#canonicalProjectProjection = {
      identity: projectionBase,
      projectHash: project.identity.projectHash,
      projectScopeGeneration: this.#projectScopeGeneration,
      frame: project.projection,
      entities,
    };
    this.#liveProjectionBase = projectionBase;
    const selection = acceptedSelection(project, current.selection, resetSelection);
    const connection = current.connection.kind === 'connected'
      ? {
          ...current.connection,
          message: `${current.connection.adapter.adapterId} · ${project.identity.name} is open.`,
        }
      : current.connection;
    this.#patch({
      connection,
      authoringDocument: {
        identity: project.identity,
        canonical: project.canonical,
        inspections: project.inspections,
        sceneHierarchy: project.sceneHierarchy,
        assetBrowser: project.assetBrowser,
        domain: project.loadingBay,
        voxel: project.voxel ?? null,
        voxelAuthoring: project.voxelAuthoring,
        voxelObjectAuthoring: project.voxelObjectAuthoring,
        animatedMeshResources: project.animatedMeshResources,
      },
      liveProjection: {
        frame: project.projection,
        framePatch: null,
        readout: project.projectionReadout,
        entities,
        generation: (current.liveProjection?.generation ?? 0) + 1,
      },
      selection,
      preview: null,
      voxelWorkspace: {
        ...current.voxelWorkspace,
        validatedPick: null,
        objectSourceInspection: resetSelection
          ? null
          : current.voxelWorkspace.objectSourceInspection,
        objectConversion: resetSelection
          ? null
          : current.voxelWorkspace.objectConversion,
        objectPlayback: null,
      },
      assetWorkspace: {
        selectedAssetId: project.assetBrowser.assets.some(
          (asset) => asset.assetId === current.assetWorkspace.selectedAssetId,
        )
          ? current.assetWorkspace.selectedAssetId
          : null,
        plan: current.assetWorkspace.plan?.expectedProjectHash === project.identity.projectHash
          ? current.assetWorkspace.plan
          : null,
        message: current.assetWorkspace.message,
      },
      operation: 'idle',
      lastError: null,
    });
  }

  async #loadUserSettings(projectRoot: string): Promise<StudioUserSettingsSnapshot> {
    if (this.#settingsClient !== null) return this.#settingsClient.load(projectRoot);
    const artifact = buildDefaultStudioHostUserSettings('rusty-studio-project:unpersisted');
    return {
      canonicalProjectRoot: projectRoot,
      projectKey: artifact.projectKey,
      path: '',
      artifact,
      sha256: null,
      writesEnabled: false,
      message: 'No host-user settings client is configured; preferences are session-only.',
    };
  }

  #acceptUserSettings(settings: StudioUserSettingsSnapshot): void {
    this.#settingsGeneration += 1;
    this.#patch({
      settings: viewSettings(settings.artifact),
      userSettings: {
        status: settings.writesEnabled
          ? settings.sha256 === null ? 'defaulted' : 'loaded'
          : 'unsupported',
        projectRoot: settings.canonicalProjectRoot,
        projectKey: settings.projectKey,
        path: settings.path.length === 0 ? null : settings.path,
        sha256: settings.sha256,
        writesEnabled: settings.writesEnabled,
        message: settings.message,
      },
    });
  }

  #queueUserSettingsWrite(): void {
    const current = this.#snapshot();
    const client = this.#settingsClient;
    const root = current.userSettings.projectRoot;
    if (client === null || root === null) {
      this.#patch({
        userSettings: {
          ...current.userSettings,
          message: 'Preferences are session-only until a project is opened through the Studio host.',
        },
      });
      return;
    }
    if (!current.userSettings.writesEnabled) {
      this.reportUiError('Host-user settings writes are disabled until the settings file is reloaded or repaired.');
      return;
    }
    const generation = this.#settingsGeneration;
    this.#settingsWriteChain = this.#settingsWriteChain.then(async () => {
      const before = this.#snapshot();
      if (generation !== this.#settingsGeneration
        || before.userSettings.projectRoot !== root
        || !before.userSettings.writesEnabled) return;
      this.#patch({
        userSettings: {
          ...before.userSettings,
          status: 'saving',
          message: 'Saving host-user settings…',
        },
      });
      const artifact = settingsArtifact(before.userSettings.projectKey, before.settings);
      const saved = await client.save(root, artifact, before.userSettings.sha256);
      if (generation !== this.#settingsGeneration) return;
      this.#patch({
        userSettings: {
          ...this.#snapshot().userSettings,
          status: 'loaded',
          path: saved.path,
          sha256: saved.sha256,
          writesEnabled: true,
          message: 'Host-user settings saved for this canonical project root.',
        },
      });
    }).catch((error: unknown) => {
      if (generation !== this.#settingsGeneration) return;
      const message = errorMessage(error);
      this.#patch({
        userSettings: {
          ...this.#snapshot().userSettings,
          status: 'error',
          writesEnabled: false,
          message,
        },
        lastError: message,
      });
    });
  }

  #operationFailed(error: unknown): void {
    const message = error instanceof StudioAdapterOperationRejected
      ? `${error.rejection.code}: ${error.rejection.message}`
      : errorMessage(error);
    this.#patch({ operation: 'idle', lastError: message });
  }

  #patch(update: Partial<StudioWorkspaceSnapshot>): void {
    this.#snapshot.update((current) => ({ ...current, ...update }));
  }
}

function objectCandidateActionIsCurrent(
  action: VoxelEditorAction,
  conversion: VoxelWorkspaceState['objectConversion'],
): boolean {
  switch (action.kind) {
    case 'previewObjectFrame':
      return conversion !== null
        && action.planId === conversion.plan.planId
        && action.expectedPlanHash === conversion.plan.planHash;
    case 'applyObjectConversion':
      return conversion !== null
        && action.planId === conversion.plan.planId
        && action.expectedPlanHash === conversion.plan.planHash
        && action.expectedOutputHash === conversion.preview.outputHash;
    case 'discardObjectConversion':
      return conversion !== null && action.planId === conversion.plan.planId;
    default:
      return true;
  }
}

function isVoxelObjectAction(action: VoxelEditorAction): boolean {
  switch (action.kind) {
    case 'inspectObjectSource':
    case 'prepareObjectConversion':
    case 'previewObjectFrame':
    case 'applyObjectConversion':
    case 'discardObjectConversion':
    case 'attachObjectInstance':
    case 'previewObjectInstance':
      return true;
    default:
      return false;
  }
}

function compactVoxelObjectFramePatch(
  base: RenderFrameDiff,
  patch: RenderFrameDiff,
): RenderFrameDiff | null {
  if (!isVoxelObjectFramePatch(patch)) return null;
  const frames = new Map<number, number>();
  for (const operation of patch.ops) {
    if (operation.op === 'setVoxelObjectFrame') frames.set(operation.handle, operation.frame);
  }
  if (frames.size === 0) return base;
  const matched = new Set<number>();
  const ops = base.ops.map((operation): RenderDiff => {
    if (operation.op !== 'createVoxelObjectInstance') return operation;
    const frame = frames.get(operation.handle);
    if (frame === undefined) return operation;
    matched.add(operation.handle);
    return {
      ...operation,
      instance: { ...operation.instance, frame },
    };
  });
  return matched.size === frames.size ? { schemaVersion: 1, ops } : null;
}

function isVoxelObjectFramePatch(frame: RenderFrameDiff): boolean {
  return frame.ops.every((operation) => operation.op === 'setVoxelObjectFrame');
}

function sameProjectionBase(
  left: ProjectionBaseIdentity | null,
  right: ProjectionBaseIdentity,
): boolean {
  return left !== null
    && left.kind === right.kind
    && left.generation === right.generation
    && left.projectScopeGeneration === right.projectScopeGeneration
    && left.key === right.key;
}

function objectPlaybackStepMicroseconds(
  document: AuthoringDocumentView,
  playback: VoxelObjectInstancePlaybackReadout,
): number | null {
  if (playback.clipId === null || playback.clipFrame === null) return null;
  const asset = document.voxelObjectAuthoring.assets.find(
    (candidate) => candidate.assetId === playback.voxelObjectAssetId,
  );
  const clip = asset?.clips.find((candidate) => candidate.clipId === playback.clipId);
  const frame = clip?.frames[playback.clipFrame];
  const authoredDuration = frame?.durationMicroseconds
    ?? (clip === undefined || clip.framesPerSecond <= 0
      ? null
      : Math.round(1_000_000 / clip.framesPerSecond));
  if (
    authoredDuration === null
    || !Number.isSafeInteger(authoredDuration)
    || authoredDuration <= 0
    || !Number.isSafeInteger(playback.rate.numerator)
    || !Number.isSafeInteger(playback.rate.denominator)
    || playback.rate.numerator <= 0
    || playback.rate.denominator <= 0
  ) return null;
  const scaled = authoredDuration * playback.rate.denominator;
  if (!Number.isSafeInteger(scaled)) return null;
  const step = Math.ceil(scaled / playback.rate.numerator);
  return Number.isSafeInteger(step) && step > 0 ? step : null;
}

function isObjectPlaybackControl(
  action: VoxelEditorAction,
): action is ObjectPlaybackControlAction {
  return action.kind === 'previewObjectInstance'
    && (action.command.kind === 'pause' || action.command.kind === 'stop');
}

function acceptedSelection(
  project: StudioProjectReadout,
  previous: EditorSelectionState,
  reset: boolean,
): EditorSelectionState {
  if (reset) return { sceneNodeId: null, entityId: null, source: null };
  const node = project.sceneHierarchy.nodes.find((candidate) =>
    previous.entityId === null
      ? candidate.nodeId === previous.sceneNodeId
      : candidate.entityId === previous.entityId,
  );
  if (node === undefined) return { sceneNodeId: null, entityId: null, source: null };
  return { sceneNodeId: node.nodeId, entityId: node.entityId, source: previous.source };
}

function mutationMessage(receipt: ProjectMutationReceipt): string {
  switch (receipt.kind) {
    case 'sceneCreated': return `Scene ${receipt.sceneId} created${receipt.madeEntry ? ' and opened' : ''}.`;
    case 'sceneRenamed': return `Scene ${receipt.sceneId} renamed.`;
    case 'sceneDeleted': return `Scene ${receipt.sceneId} deleted.`;
    case 'entrySceneSet': return `Scene ${receipt.sceneId} is now the entry scene.`;
    case 'sceneObjectCreated': return `Entity ${String(receipt.entityId)} created.`;
    case 'sceneObjectDeleted': return `Entity ${String(receipt.entityId)} and ${String(receipt.removedObjects - 1)} descendants deleted.`;
    case 'sceneObjectRenamed': return `Entity ${String(receipt.entityId)} renamed.`;
    case 'sceneObjectReparented': return `Entity ${String(receipt.entityId)} reparented.`;
    case 'sceneObjectTransformSet': return `Entity ${String(receipt.entityId)} transform stored.`;
    case 'sceneObjectAppearanceSet': return `Entity ${String(receipt.entityId)} appearance stored.`;
    case 'entityCollisionSet': return `Entity ${String(receipt.entityId)} collision ${receipt.attached ? 'attached' : 'removed'}.`;
    case 'entityKinematicSet': return `Entity ${String(receipt.entityId)} kinematic data ${receipt.attached ? 'attached' : 'removed'}.`;
    case 'materialUpserted': return `Material ${receipt.assetId} stored.`;
    case 'assetImportApplied': return `${receipt.reimportKind} installed ${receipt.assetId} from ${receipt.sourcePath}.`;
    case 'voxelAssetInitialized': return `Voxel asset ${receipt.assetId} initialized.`;
    case 'voxelAssetDuplicated': return `Duplicated ${receipt.sourceAssetId} to ${receipt.targetAssetId}.`;
    case 'voxelInstanceAttached': return `Instance ${receipt.instanceId} attached.`;
    case 'voxelInstanceTransformSet': return `Instance ${receipt.instanceId} transform stored.`;
    case 'voxelInstanceRemoved': return `Instance ${receipt.instanceId} removed.`;
    case 'voxelPaletteReplaced': return `Palette for ${receipt.assetId} replaced.`;
    case 'voxelBrushApplied': return `Brush changed ${String(receipt.changedVoxels)} voxels.`;
    case 'voxelPrimitiveApplied': return `${receipt.primitiveKind} changed ${String(receipt.changedVoxels)} voxels.`;
    case 'voxelTemplateInitialized': return `${receipt.templateKind} initialized ${receipt.assetId}.`;
    case 'voxelAssetFileImported': return `Imported ${receipt.sourcePath} as ${receipt.targetAssetId}.`;
    case 'environmentMaterialized': return `${receipt.preset} environment materialized in ${receipt.sceneId}.`;
    case 'voxelHistoryMoved': return `History moved to cursor ${String(receipt.cursorAfter)}.`;
    case 'voxelAnnotationCreated': return `Annotation layer ${receipt.layerId} created.`;
    case 'voxelAnnotationEdited': return `Annotation layer ${receipt.layerId} updated.`;
    case 'voxelConversionApplied': return `Conversion installed ${receipt.assetId}.`;
    case 'voxelObjectConversionApplied': return `Voxel object ${receipt.assetId} installed with ${String(receipt.storedFrames)} stored frames.`;
    case 'voxelObjectInstanceAttached': return `Voxel object instance ${receipt.instanceId} attached.`;
  }
}

/**
 * Produces a disposable UI list from owner IDs and shared projection metadata.
 * It is not an authored hierarchy or a retained renderer implementation.
 */
export function summarizeProjectionForUi(
  frame: RenderFrameDiff,
  ownerEntityIds: readonly number[],
  previous: readonly ProjectedEntityView[],
): readonly ProjectedEntityView[] {
  const byEntity = new Map(previous.map((entity) => [entity.entityId, entity]));
  const handleToEntity = new Map(
    previous.flatMap((entity) =>
      entity.renderHandle === null ? [] : [[entity.renderHandle, entity.entityId] as const],
    ),
  );
  for (const operation of frame.ops) {
    applyProjectionOperation(operation, byEntity, handleToEntity);
  }
  for (const entityId of ownerEntityIds) {
    if (!byEntity.has(entityId)) {
      byEntity.set(entityId, {
        entityId,
        label: `Entity ${String(entityId)}`,
        asset: null,
        transform: null,
        renderHandle: null,
        projected: false,
      });
    }
  }
  return [...byEntity.values()]
    .filter((entity) => ownerEntityIds.includes(entity.entityId))
    .sort((left, right) => left.entityId - right.entityId);
}

function applyProjectionOperation(
  operation: RenderDiff,
  entities: Map<number, ProjectedEntityView>,
  handles: Map<RenderHandle, number>,
): void {
  if (operation.op === 'destroy') {
    const entityId = handles.get(operation.handle);
    if (entityId !== undefined) {
      const previous = entities.get(entityId);
      if (previous !== undefined) {
        entities.set(entityId, {
          ...previous,
          asset: null,
          transform: null,
          renderHandle: null,
          projected: false,
        });
      }
      handles.delete(operation.handle);
    }
    return;
  }
  if (operation.op === 'update') {
    const entityId = handles.get(operation.handle);
    if (entityId === undefined) return;
    const previous = entities.get(entityId);
    if (previous === undefined) return;
    entities.set(entityId, {
      ...previous,
      label: operation.metadata?.label ?? previous.label,
      transform: operation.transform ?? previous.transform,
    });
    return;
  }

  const descriptor = projectionDescriptor(operation);
  if (descriptor === null || descriptor.metadata.sourceEntity === null) return;
  const entityId = descriptor.metadata.sourceEntity;
  entities.set(entityId, {
    entityId,
    label: descriptor.metadata.label ?? `Entity ${String(entityId)}`,
    asset: descriptor.asset,
    transform: descriptor.transform,
    renderHandle: descriptor.handle,
    projected: true,
  });
  handles.set(descriptor.handle, entityId);
}

function projectionDescriptor(operation: RenderDiff): {
  readonly handle: RenderHandle;
  readonly asset: string | null;
  readonly transform: Transform;
  readonly metadata: RenderMetadata;
} | null {
  switch (operation.op) {
    case 'create':
      return {
        handle: operation.handle,
        asset: operation.node.geometry.kind,
        transform: operation.node.transform,
        metadata: operation.node.metadata,
      };
    case 'createStaticMeshInstance':
      return {
        handle: operation.handle,
        asset: operation.instance.asset,
        transform: operation.instance.transform,
        metadata: operation.instance.metadata,
      };
    case 'createAnimatedMeshInstance':
      return {
        handle: operation.handle,
        asset: operation.instance.asset,
        transform: operation.instance.transform,
        metadata: operation.instance.metadata,
      };
    case 'createSprite':
      return {
        handle: operation.handle,
        asset: operation.sprite.asset,
        transform: operation.sprite.transform,
        metadata: operation.sprite.metadata,
      };
    default:
      return null;
  }
}

function validTransform(transform: Transform): boolean {
  return transform.translation.every(Number.isFinite)
    && transform.rotation.every(Number.isFinite)
    && Math.hypot(...transform.rotation) > Number.EPSILON
    && transform.scale.every((value) => Number.isFinite(value) && value > 0);
}

function sameTransform(left: Transform, right: Transform): boolean {
  return left.translation.every((value, axis) => value === right.translation[axis])
    && left.rotation.every((value, axis) => value === right.rotation[axis])
    && left.scale.every((value, axis) => value === right.scale[axis]);
}

function viewSettings(artifact: StudioHostUserSettingsArtifact): StudioViewSettings {
  return {
    theme: artifact.theme,
    lightingMode: artifact.sceneView.lightingMode,
    snappingEnabled: artifact.editor.snappingEnabled,
    translationSnap: artifact.editor.translationSnap,
    translationSnapAxes: [...artifact.editor.translationSnapAxes],
    rotationSnapDegrees: artifact.editor.rotationSnapDegrees,
    scaleSnapAxes: [...artifact.editor.scaleSnapAxes],
    fineMultiplier: artifact.editor.fineMultiplier,
    transformOrientation: artifact.editor.transformOrientation,
    gridVisible: artifact.sceneView.gridVisible,
    minorColor: [...artifact.sceneView.minorColor],
    majorColor: [...artifact.sceneView.majorColor],
    xAxisColor: [...artifact.sceneView.xAxisColor],
    yAxisColor: [...artifact.sceneView.yAxisColor],
    zAxisColor: [...artifact.sceneView.zAxisColor],
    majorLineEvery: artifact.sceneView.majorLineEvery,
    opacity: artifact.sceneView.opacity,
    fadeStart: artifact.sceneView.fadeStart,
    fadeEnd: artifact.sceneView.fadeEnd,
    cameraMoveSpeed: artifact.sceneView.cameraMoveSpeed,
    cameraBoostMultiplier: artifact.sceneView.cameraBoostMultiplier,
    invertLookY: artifact.sceneView.invertLookY,
    invertPanY: artifact.sceneView.invertPanY,
    keyboard: { ...artifact.keyboard },
  };
}

function settingsArtifact(
  projectKey: string,
  settings: StudioViewSettings,
): StudioHostUserSettingsArtifact {
  return {
    schemaVersion: 1,
    artifactKind: 'rusty_engine_studio_host_user_settings',
    settingsVersion: 'rusty-engine-studio-host-user-settings.v1',
    projectKey,
    theme: settings.theme,
    editor: {
      snappingEnabled: settings.snappingEnabled,
      translationSnap: settings.translationSnap,
      translationSnapAxes: [...settings.translationSnapAxes],
      rotationSnapDegrees: settings.rotationSnapDegrees,
      scaleSnapAxes: [...settings.scaleSnapAxes],
      fineMultiplier: settings.fineMultiplier,
      transformOrientation: settings.transformOrientation,
    },
    sceneView: {
      lightingMode: settings.lightingMode,
      gridVisible: settings.gridVisible,
      minorColor: [...settings.minorColor],
      majorColor: [...settings.majorColor],
      xAxisColor: [...settings.xAxisColor],
      yAxisColor: [...settings.yAxisColor],
      zAxisColor: [...settings.zAxisColor],
      majorLineEvery: settings.majorLineEvery,
      opacity: settings.opacity,
      fadeStart: settings.fadeStart,
      fadeEnd: settings.fadeEnd,
      cameraMoveSpeed: settings.cameraMoveSpeed,
      cameraBoostMultiplier: settings.cameraBoostMultiplier,
      invertLookY: settings.invertLookY,
      invertPanY: settings.invertPanY,
    },
    keyboard: { ...settings.keyboard },
  };
}

function parseHexColor(value: string): readonly [number, number, number] | null {
  const match = /^#([0-9a-f]{6})$/i.exec(value);
  if (match === null) return null;
  const encoded = Number.parseInt(match[1] as string, 16);
  return [
    ((encoded >> 16) & 0xff) / 255,
    ((encoded >> 8) & 0xff) / 255,
    (encoded & 0xff) / 255,
  ];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown Studio adapter failure';
}
