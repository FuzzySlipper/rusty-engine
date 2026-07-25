import { signal, type Signal } from '@angular/core';
import type {
  AdapterDescription,
  AssetBrowserReadout,
  AssetImportPlanReadout,
  CanonicalOwnerContent,
  LoadingBayDomainReadout,
  OwnerInspections,
  ProjectionReadout,
  ProjectMutationAppliedResponse,
  SceneHierarchyNodeReadout,
  SceneHierarchyReadout,
  StudioProjectIdentity,
  StudioProjectReadout,
  StudioAssetImportSettings,
  StudioFileSelection,
  VoxelConversionPlan,
  VoxelConversionPreview,
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
  readonly frame: RenderFrameDiff;
  readonly readout: ProjectionReadout;
  readonly entities: readonly ProjectedEntityView[];
  readonly generation: number;
}

export interface TransformPreviewState {
  readonly entityId: number;
  readonly original: Transform;
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
  readonly gridVisible: boolean;
  readonly snappingEnabled: boolean;
  readonly translationSnap: number;
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
  readonly #snapshot = signal<StudioWorkspaceSnapshot>(initialSnapshot());
  readonly snapshot: Signal<StudioWorkspaceSnapshot> = this.#snapshot.asReadonly();
  #settingsWriteChain: Promise<void> = Promise.resolve();
  #settingsGeneration = 0;

  constructor(
    client: StudioAdapterClient,
    settingsClient: HttpStudioUserSettingsClient | null = null,
  ) {
    this.#client = client;
    this.#settingsClient = settingsClient;
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

  selectEntity(entityId: number | null, source: EditorSelectionState['source']): void {
    const sceneNodeId = entityId === null
      ? null
      : (this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
          (node) => node.entityId === entityId,
        )?.nodeId ?? null);
    this.#patch({
      selection: {
        sceneNodeId,
        entityId,
        source: entityId === null ? null : source,
      },
      preview: this.#snapshot().preview?.entityId === entityId ? this.#snapshot().preview : null,
    });
  }

  selectHierarchyNode(sceneNodeId: number): void {
    const node = this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.nodeId === sceneNodeId,
    );
    if (node === undefined) return;
    this.#patch({
      selection: {
        sceneNodeId: node.nodeId,
        entityId: node.entityId,
        source: 'hierarchy',
      },
      preview: node.entityId !== null && this.#snapshot().preview?.entityId === node.entityId
        ? this.#snapshot().preview
        : null,
    });
  }

  beginTranslationPreview(entityId: number): void {
    const node = this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.entityId === entityId,
    );
    if (node === undefined) return;
    const translation = node.localTransform.translation;
    this.#patch({
      selection: { sceneNodeId: node.nodeId, entityId, source: 'inspector' },
      preview: {
        entityId,
        original: node.localTransform,
        translation,
        rotation: node.localTransform.rotation,
        scale: node.localTransform.scale,
      },
      lastError: null,
    });
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

  cancelPreview(): void {
    this.#patch({ preview: null, lastError: null });
  }

  async commitPreview(): Promise<void> {
    const current = this.#snapshot();
    const preview = current.preview;
    const document = current.authoringDocument;
    if (preview === null || document === null) return;
    this.#patch({ operation: 'committing', lastError: null });
    try {
      const response = await this.#client.setSceneObjectTransform({
        expectedProjectHash: document.identity.projectHash,
        expectedSceneRevision: document.identity.sceneRevision,
        entityId: preview.entityId,
        transform: {
          translation: preview.translation,
          rotation: preview.rotation,
          scale: preview.scale,
        },
      });
      this.#acceptProject(response.project, false);
      this.#patch({ preview: null });
    } catch (error) {
      this.#operationFailed(error);
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

  async runVoxelAction(action: VoxelEditorAction): Promise<void> {
    const document = this.#snapshot().authoringDocument;
    if (document === null || this.#snapshot().operation !== 'idle') return;
    const expectedProjectHash = document.identity.projectHash;
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
      }
    } catch (error) {
      this.#operationFailed(error);
    }
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
      gridVisible: defaults.gridVisible,
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
  ): void {
    this.#acceptProject(response.project, false);
    this.#patch({
      voxelWorkspace: {
        ...this.#snapshot().voxelWorkspace,
        validatedPick: null,
        lastReceipt: response.receipt,
        historyPreview: null,
        conversion: clearConversion ? null : this.#snapshot().voxelWorkspace.conversion,
        message: mutationMessage(response.receipt),
      },
    });
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
    const current = this.#snapshot();
    const entities = summarizeProjectionForUi(
      project.projection,
      project.inspections.entityState.entityIds,
      [],
    );
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
      },
      liveProjection: {
        frame: project.projection,
        readout: project.projectionReadout,
        entities,
        generation: (current.liveProjection?.generation ?? 0) + 1,
      },
      selection,
      preview: null,
      voxelWorkspace: {
        ...current.voxelWorkspace,
        validatedPick: null,
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

function viewSettings(artifact: StudioHostUserSettingsArtifact): StudioViewSettings {
  return {
    theme: artifact.theme,
    snappingEnabled: artifact.editor.snappingEnabled,
    translationSnap: artifact.editor.translationSnap,
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
    },
    sceneView: {
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
