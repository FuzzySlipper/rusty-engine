import { NgComponentOutlet } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  HostBinding,
  computed,
  effect,
  inject,
  input,
  output,
  signal,
  viewChild,
  type ElementRef,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import type {
  AssetEntryReadout,
  OwnerDiagnostic,
  StoredLight,
  StudioHostStatus,
  StudioEntityComponentReference,
  StudioSceneAppearance,
} from '@rusty-engine/studio-adapter-client';
import type { EditorGridDescriptor, MeshBoundsDescriptor, Transform } from '@rusty-engine/render-contracts';
import type { StudioKeyboardBindings } from '@rusty-engine/studio-user-settings';
import {
  StudioViewportComponent,
  type StudioAnimationInspectionCapture,
  type StudioViewportFrameSubmitted,
  type StudioGroundingInspection,
  type StudioTransformOrientation,
  type StudioTransformSnapping,
  type StudioTransformTool,
  type StudioVoxelPreview,
  type StudioVoxelObjectPlacementPick,
  type VoxelViewportPickCandidate,
} from '@rusty-engine/studio-viewport';
import {
  VoxelEditorComponent,
  type VoxelEditorPreviewPresentation,
  type VoxelEditorAction,
} from '@rusty-engine/studio-voxel-editor';

import {
  admitStudioEntityInspectorContributions,
  matchStudioEntityInspectorContributions,
  studioEntityInspectorInstanceKey,
  type StudioEntityInspectorContribution,
  type StudioEntityInspectorRenderMatch,
} from './entity-inspector.js';
import { composeTransform, localTransformFromWorld } from './transform-tools.js';
import {
  activeStudioMeshResources,
  resolveStudioMeshResource,
  type StudioMeshResourceDescriptor,
} from './mesh-resources.js';
import {
  resolveStudioTextureResource,
  type StudioTextureResourceDescriptor,
} from './texture-resources.js';

import { STUDIO_WORKSPACE } from './tokens.js';
import {
  HttpStudioHostFileBrowser,
  HttpStudioHostStatusClient,
  HttpStudioRenderResourceClient,
  type StudioHostDirectoryReadout,
  type StudioHostPathRequest,
} from './transport.js';
import {
  STUDIO_VOXEL_OBJECT_INSPECTOR_HOST,
  type StudioVoxelObjectInspectorHost,
} from './voxel-object-inspector-panel.component.js';

interface HostPathDialogState {
  readonly request: StudioHostPathRequest;
  readonly readout: StudioHostDirectoryReadout | null;
  readonly filter: string;
  readonly busy: boolean;
  readonly error: string | null;
}

interface AnimatedMeshResourceDescriptor {
  readonly asset: string;
  readonly contentHash: string;
  readonly clipIds: readonly string[];
}

@Component({
  selector: 'rusty-studio-shell',
  standalone: true,
  imports: [
    FormsModule,
    NgComponentOutlet,
    StudioViewportComponent,
    VoxelEditorComponent,
  ],
  providers: [{
    provide: STUDIO_VOXEL_OBJECT_INSPECTOR_HOST,
    useFactory: createStudioVoxelObjectInspectorHost,
  }],
  templateUrl: './studio-shell.component.html',
  styleUrl: './studio-shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioShellComponent {
  readonly store = inject(STUDIO_WORKSPACE);
  readonly state = this.store.snapshot;
  readonly hostStatus = signal<StudioHostStatus | null>(null);
  readonly hostStatusError = signal<string | null>(null);
  readonly hostStatusLabel = computed(() => {
    const status = this.hostStatus();
    if (status === null) return this.hostStatusError() === null ? 'identity loading' : 'identity unavailable';
    const engine = status.engineSourceCommit?.slice(0, 8) ?? 'unmanaged';
    const consumer = status.configuredConsumer?.commit.slice(0, 8) ?? 'unmanaged';
    return `Engine ${engine} · consumer ${consumer} · protocol ${String(status.runningAdapter.protocolVersion)}`;
  });
  readonly frameSubmitted = output<StudioViewportFrameSubmitted>();
  readonly entityInspectorContributions =
    input<readonly StudioEntityInspectorContribution[]>([]);
  readonly admittedEntityInspectorContributions = computed(() =>
    admitStudioEntityInspectorContributions(this.entityInspectorContributions()));
  readonly activeEntityInspectorMatches = computed<
    readonly StudioEntityInspectorRenderMatch[]
  >(() => {
    const snapshot = this.state();
    const entityId = snapshot.selection.entityId;
    if (
      snapshot.connection.kind !== 'connected'
      || snapshot.authoringDocument === null
      || entityId === null
    ) {
      return [];
    }
    return matchStudioEntityInspectorContributions(
      this.admittedEntityInspectorContributions(),
      snapshot.authoringDocument.entityComponents,
      snapshot.connection.adapter,
      entityId,
    ).flatMap((match) => {
      const context = this.store.entityInspectorContext(match.reference);
      return context === null
        ? []
        : [{
            ...match,
            context,
            instanceKey: studioEntityInspectorInstanceKey(
              match.contribution.key,
              context,
            ),
          }];
    });
  });
  readonly voxelEditorPreview = signal<VoxelEditorPreviewPresentation | null>(null);
  readonly objectPlacementHistoryReadout = computed(() => {
    const history = this.state().voxelWorkspace.objectPlacementHistory;
    return history === null ? null : {
      state: history.state,
      instanceId: history.instance.instanceId,
      ownerEntityId: history.ownerEntityId,
    };
  });
  readonly objectPlacementResourceReadout = computed(() => {
    const resource = this.state().voxelWorkspace.objectPlacementResource;
    return resource === null ? null : {
      assetId: resource.assetId,
      objectContentHash: resource.objectContentHash,
    };
  });
  readonly selectedTransformTool = signal<StudioTransformTool | null>(null);
  readonly hostPathDialog = signal<HostPathDialogState | null>(null);
  readonly visibleHostEntries = computed(() => {
    const dialog = this.hostPathDialog();
    if (dialog?.readout === null || dialog === null) return [];
    const filter = dialog.filter.trim().toLocaleLowerCase();
    return filter.length === 0
      ? dialog.readout.entries
      : dialog.readout.entries.filter((entry) => entry.name.toLocaleLowerCase().includes(filter));
  });
  readonly voxelPreview = computed<StudioVoxelPreview | null>(() => {
    const editorPreview = this.voxelEditorPreview();
    if (editorPreview !== null) return editorPreview;
    const conversion = this.state().voxelWorkspace.conversion;
    if (conversion === null) return null;
    return {
      kind: 'conversion',
      cellSize: conversion.plan.settings.conversion.cellSize,
      samples: conversion.preview.sampleVoxels,
    };
  });
  readonly transformPreview = computed<Transform | null>(() => {
    const preview = this.state().preview;
    return preview === null ? null : {
      translation: preview.translation,
      rotation: preview.rotation,
      scale: preview.scale,
    };
  });
  readonly transformManipulatorTransform = computed<Transform | null>(() => {
    const snapshot = this.state();
    const document = snapshot.authoringDocument;
    const entityId = snapshot.selection.entityId;
    if (this.selectedTransformTool() === null || document === null || entityId === null) return null;
    const node = document.sceneHierarchy.nodes.find(
      (candidate) => candidate.entityId === entityId,
    );
    if (node === undefined) return null;
    const preview = snapshot.preview?.entityId === entityId ? snapshot.preview : null;
    if (preview === null) return node.worldTransform;
    const local: Transform = {
      translation: preview.translation,
      rotation: preview.rotation,
      scale: preview.scale,
    };
    if (node.parentNodeId === null) return local;
    const parent = document.sceneHierarchy.nodes.find(
      (candidate) => candidate.nodeId === node.parentNodeId,
    );
    return parent === undefined ? null : composeTransform(parent.worldTransform, local);
  });
  readonly selectedEntityComponentReferences = computed<
    readonly StudioEntityComponentReference[]
  >(() => {
    const snapshot = this.state();
    const entityId = snapshot.selection.entityId;
    if (entityId === null) return [];
    return (snapshot.authoringDocument?.entityComponents ?? []).filter(
      (reference) => reference.ownerEntityId === entityId,
    );
  });
  readonly matchedEntityInspectorComponentTypes = computed(() =>
    new Set(this.activeEntityInspectorMatches().map(
      (match) => match.reference.componentTypeId,
    )));
  readonly transformSnapping = computed<StudioTransformSnapping>(() => ({
    enabled: this.state().settings.snappingEnabled,
    rotationDegrees: this.state().settings.rotationSnapDegrees,
    scale: this.state().settings.scaleSnapAxes,
    translation: this.state().settings.translationSnapAxes,
  }));
  readonly groundingInspection = computed<StudioGroundingInspection | null>(() => {
    const snapshot = this.state();
    const node = this.store.selectedHierarchyNode();
    const entity = this.store.selectedEntity();
    const frame = snapshot.liveProjection?.frame;
    if (
      node?.asset === null
      || node?.asset === undefined
      || entity === null
      || entity.transform === null
      || frame === undefined
    ) {
      return null;
    }
    const definition = frame.ops.find((operation) =>
      (operation.op === 'defineStaticMesh' || operation.op === 'defineAnimatedMesh')
      && operation.asset.asset === node.asset);
    const localBounds = definition?.op === 'defineStaticMesh'
      ? definition.asset.payload.bounds
      : definition?.op === 'defineAnimatedMesh'
        ? definition.asset.bounds
        : null;
    if (localBounds === null) return null;
    const bounds = transformedBounds(localBounds, entity.transform);
    const contactPlaneY = snapshot.settings.gridVisible
      ? (this.viewportGrid()?.grid.origin[1] ?? 0)
      : 0;
    return {
      origin: entity.transform.translation,
      bounds,
      contactPlaneY,
      clearance: bounds.min[1] - contactPlaneY,
    };
  });
  readonly viewportGrid = computed<EditorGridDescriptor | null>(() => {
    const settings = this.state().settings;
    if (!settings.gridVisible) return null;
    return {
      visible: true,
      grid: {
        coordinateSystem: 'rightHandedYUp',
        origin: [0, 0, 0],
        spacing: [settings.translationSnap, settings.translationSnap, settings.translationSnap],
      },
      plane: 'xz',
      snapAnchor: 'boundary',
      style: {
        minorColor: settings.minorColor,
        majorColor: settings.majorColor,
        xAxisColor: settings.xAxisColor,
        yAxisColor: settings.yAxisColor,
        zAxisColor: settings.zAxisColor,
        majorLineEvery: settings.majorLineEvery,
        opacity: settings.opacity,
        fadeStart: settings.fadeStart,
        fadeEnd: settings.fadeEnd,
      },
    };
  });
  readonly viewportControlPreferences = computed(() => {
    const settings = this.state().settings;
    return {
      moveSpeed: settings.cameraMoveSpeed,
      boostMultiplier: settings.cameraBoostMultiplier,
      invertLookY: settings.invertLookY,
      invertPanY: settings.invertPanY,
      keyboard: { ...settings.keyboard },
    };
  });
  readonly animatedMeshManifest = computed(() => {
    const resources = this.state().authoringDocument?.animatedMeshResources ?? [];
    if (resources.length === 0) return null;
    return {
      kind: 'rusty_renderer_animated_mesh_resources.v1' as const,
      resources: resources.map(({ asset, contentHash, clipIds }) => ({
        asset,
        contentHash,
        clipIds,
      })),
    };
  });
  readonly animatedMeshResourceKey = computed(() => JSON.stringify([
    this.state().userSettings.projectRoot,
    this.state().authoringDocument?.animatedMeshResources ?? [],
  ]));
  readonly activeMeshResources = computed(() => {
    const canonical = this.state().liveProjection?.meshResources ?? [];
    const placement = this.state().voxelWorkspace.objectPlacementResource?.meshResources ?? [];
    return activeStudioMeshResources(canonical, placement);
  });
  readonly meshResourceManifest = computed(() => {
    const resources = this.activeMeshResources();
    if (resources.length === 0) return null;
    return {
      kind: 'rusty_renderer_mesh_resources.v1' as const,
      resources: resources.map(({ resource, contentHash, byteLength }) => ({
        resource,
        contentHash,
        byteLength,
      })),
    };
  });
  readonly meshResourceKey = computed(() => JSON.stringify([
    this.state().userSettings.projectRoot,
    this.activeMeshResources(),
  ]));
  readonly textureResourceManifest = computed(() => {
    const resources = this.state().liveProjection?.textureResources ?? [];
    if (resources.length === 0) return null;
    return {
      kind: 'rusty_renderer_texture_resources.v1' as const,
      resources: resources.map(({ resource, contentHash, byteLength }) => ({
        resource,
        contentHash,
        byteLength,
      })),
    };
  });
  readonly textureResourceKey = computed(() => JSON.stringify([
    this.state().userSettings.projectRoot,
    this.state().liveProjection?.textureResources ?? [],
  ]));
  readonly selectedAnimatedMeshResource = computed(() => {
    const asset = this.store.selectedHierarchyNode()?.asset;
    if (asset === null || asset === undefined) return null;
    return this.state().authoringDocument?.animatedMeshResources.find(
      (resource) => resource.asset === asset,
    ) ?? null;
  });
  readonly animationInspectionCapture = signal<StudioAnimationInspectionCapture | null>(null);
  readonly animationInspectionSample = signal<
    ReturnType<StudioViewportComponent['sampleSelectedAnimatedMesh']> | null
  >(null);
  readonly animationInspectionReadout = signal(
    'Select a retained animated mesh, then scrub or play a clip.',
  );
  readonly gridColors = [
    { key: 'minorColor', label: 'Minor lines' },
    { key: 'majorColor', label: 'Major lines' },
    { key: 'xAxisColor', label: 'X axis' },
    { key: 'yAxisColor', label: 'Y axis' },
    { key: 'zAxisColor', label: 'Z axis' },
  ] as const;
  readonly keyboardBindings = [
    { key: 'moveForward', label: 'Move forward' },
    { key: 'moveBackward', label: 'Move backward' },
    { key: 'moveLeft', label: 'Move left' },
    { key: 'moveRight', label: 'Move right' },
    { key: 'moveDown', label: 'Move down' },
    { key: 'moveUp', label: 'Move up' },
    { key: 'boost', label: 'Boost' },
  ] as const;

  projectRoot = '';
  projectFile = 'content/projects/converted-wall.project.json';
  inspectorMode: 'entity' | 'voxel' = 'voxel';
  pivotGroundingToolActive = false;
  animationInspectionToolActive = false;
  animationInspectionClip = '';
  animationInspectionTime = 0;
  animationInspectionFadeSeconds = 0.15;
  authoringDialog: 'createProject' | 'saveProjectAs' | 'scene' | 'object' | 'assetImport' | null = null;

  projectDraft = {
    root: '',
    projectFile: 'content/projects/new-project.project.json',
    projectId: 'new-project',
    name: 'New Project',
    entryScene: 'scene/main',
    entrySceneName: 'Main',
  };
  sceneDraft = { sceneId: 'scene/main', name: 'Main', makeEntry: true };
  objectDraft = {
    entityId: 1,
    name: 'New Entity',
    parentEntityId: null as number | null,
    childOrder: 0,
    kind: 'empty' as 'empty' | 'staticMesh' | 'animatedMesh' | 'light',
    asset: '',
    clip: '',
  };
  appearanceKind: 'empty' | 'staticMesh' | 'animatedMesh' | 'light' = 'empty';
  appearanceAsset = '';
  appearanceClip = '';
  appearanceVisible = true;
  lightKind: StoredLight['kind'] = 'directional';
  lightColor: [number, number, number] = [1, 1, 1];
  lightIntensity = 1;
  lightRange: number | null = null;
  lightDecay = 2;
  lightOuterAngle = Math.PI / 4;
  lightPenumbra = 0;
  lightEnabled = true;
  lightShadows = false;
  collisionEnabled = true;
  staticCollider = true;
  kinematicHalfExtents: [number, number, number] = [0.5, 0.5, 0.5];
  kinematicVelocity: [number, number, number] = [0, 0, 0];
  assetImportScope: 'project' | 'host' = 'project';
  assetImportPath = 'content/assets/studio-triangle.mesh.json';
  assetImportScale = 1;
  assetImportGenerateCollision = false;
  assetImportMaterialNamespace = '';

  readonly chooseHostPath = (request: StudioHostPathRequest): Promise<string | null> =>
    this.openHostPathDialog(request);
  readonly resolveAnimatedMeshResource = async (
    descriptor: AnimatedMeshResourceDescriptor,
  ): Promise<ArrayBuffer> => {
    const snapshot = this.state();
    const projectRoot = snapshot.userSettings.projectRoot;
    if (projectRoot === null) throw new Error('Animated resources require an open project root.');
    const resource = snapshot.authoringDocument?.animatedMeshResources.find(
      (candidate) => candidate.asset === descriptor.asset,
    );
    if (resource === undefined || resource.contentHash !== descriptor.contentHash) {
      throw new Error(`Animated resource ${descriptor.asset} is not in the current Rust readout.`);
    }
    return this.#renderResources.read(projectRoot, resource.sourcePath, resource.contentHash);
  };
  readonly resolveMeshResource = async (
    descriptor: StudioMeshResourceDescriptor,
  ): Promise<ArrayBuffer> => {
    const snapshot = this.state();
    const projectRoot = snapshot.userSettings.projectRoot;
    if (projectRoot === null) throw new Error('Mesh resources require an open project root.');
    return resolveStudioMeshResource(
      projectRoot,
      this.activeMeshResources(),
      descriptor,
      this.#renderResources.read.bind(this.#renderResources),
    );
  };
  readonly resolveTextureResource = async (
    descriptor: StudioTextureResourceDescriptor,
  ): Promise<ArrayBuffer> => {
    const snapshot = this.state();
    const projectRoot = snapshot.userSettings.projectRoot;
    if (projectRoot === null) throw new Error('Texture resources require an open project root.');
    return resolveStudioTextureResource(
      projectRoot,
      snapshot.liveProjection?.textureResources ?? [],
      descriptor,
      this.#renderResources.read.bind(this.#renderResources),
    );
  };

  readonly #hostFiles = new HttpStudioHostFileBrowser();
  readonly #hostStatus = new HttpStudioHostStatusClient();
  readonly #renderResources = new HttpStudioRenderResourceClient();
  private readonly studioViewport = viewChild<StudioViewportComponent>('studioViewport');
  private readonly voxelEditor = viewChild<VoxelEditorComponent>('voxelEditor');
  private readonly hostPathFilterElement = viewChild<ElementRef<HTMLInputElement>>('hostPathFilter');
  #hierarchySelection = Promise.resolve();
  #hostPathResolve: ((path: string | null) => void) | null = null;
  #restoreFocus: HTMLElement | null = null;

  constructor() {
    void this.#hostStatus.read().then((status) => {
      this.hostStatus.set(status);
      this.hostStatusError.set(null);
    }).catch((error: unknown) => {
      this.hostStatus.set(null);
      this.hostStatusError.set(error instanceof Error ? error.message : String(error));
    });
    effect(() => {
      const snapshot = this.state();
      const canonicalRoot = snapshot.userSettings.projectRoot;
      const relativeProjectFile = snapshot.authoringDocument?.identity.relativeProjectFile;
      if (canonicalRoot !== null) this.projectRoot = canonicalRoot;
      if (relativeProjectFile !== undefined) this.projectFile = relativeProjectFile;
    });
  }

  inspectorPanelInputs(
    match: StudioEntityInspectorRenderMatch,
  ): Record<string, unknown> {
    return {
      context: match.context,
      mutationPort: this.store.entityInspectorMutationPort,
    };
  }

  @HostBinding('class.theme-high-contrast')
  get highContrast(): boolean {
    return this.state().settings.theme === 'highContrast';
  }

  openProject(): void {
    void this.store.openProject(this.projectRoot, this.projectFile);
  }

  browseProjectRoot(): void {
    void this.chooseHostPath({
      kind: 'directory',
      title: 'Choose project root',
      initialPath: this.projectRoot || '/',
    }).then((path) => { if (path !== null) this.projectRoot = path; });
  }

  browseProjectFile(): void {
    const root = this.projectRoot || '/';
    void this.chooseHostPath({
      kind: 'file',
      title: 'Choose project file',
      initialPath: absoluteHostPath(root, this.projectFile),
      extensions: ['.project.json', '.json'],
    }).then((path) => {
      if (path === null) return;
      const relative = relativeHostPath(root, path);
      if (relative === null) {
        this.store.reportUiError('Project file must be selected inside the chosen project root.');
      } else {
        this.projectFile = relative;
      }
    });
  }

  browseProjectDraftRoot(): void {
    void this.chooseHostPath({
      kind: 'directory',
      title: 'Choose project root',
      initialPath: this.projectDraft.root || '/',
    }).then((path) => { if (path !== null) this.projectDraft.root = path; });
  }

  browseProjectDraftFile(): void {
    const root = this.projectDraft.root || '/';
    void this.chooseHostPath({
      kind: 'file',
      title: 'Choose project file',
      initialPath: absoluteHostPath(root, this.projectDraft.projectFile),
      extensions: ['.project.json', '.json'],
    }).then((path) => {
      if (path === null) return;
      const relative = relativeHostPath(root, path);
      if (relative === null) {
        this.store.reportUiError('Project file must be selected inside the chosen project root.');
      } else {
        this.projectDraft.projectFile = relative;
      }
    });
  }

  browseAssetImportSource(): void {
    void this.chooseHostPath({
      kind: 'file',
      title: 'Choose asset import source',
      initialPath: this.assetImportPath.startsWith('/') ? this.assetImportPath : this.projectRoot || '/',
      extensions: ['.json', '.glb', '.gltf'],
    }).then((path) => { if (path !== null) this.assetImportPath = path; });
  }

  setHostPathFilter(filter: string): void {
    const dialog = this.hostPathDialog();
    if (dialog !== null) this.hostPathDialog.set({ ...dialog, filter });
  }

  navigateHostDirectory(directory: string): void {
    const dialog = this.hostPathDialog();
    if (dialog === null) return;
    this.hostPathDialog.set({ ...dialog, busy: true, error: null, filter: '' });
    void this.#hostFiles.list(directory, dialog.request.extensions).then(
      (readout) => this.hostPathDialog.update((current) => current === null
        ? null
        : { ...current, readout, busy: false, error: null }),
      (error: unknown) => this.hostPathDialog.update((current) => current === null
        ? null
        : {
            ...current,
            busy: false,
            error: error instanceof Error ? error.message : 'Host directory could not be read.',
          }),
    );
  }

  activateHostEntry(path: string, kind: 'directory' | 'file'): void {
    const dialog = this.hostPathDialog();
    if (dialog === null) return;
    if (kind === 'directory') {
      this.navigateHostDirectory(path);
    } else if (dialog.request.kind === 'file') {
      this.finishHostPathDialog(path);
    }
  }

  chooseCurrentHostDirectory(): void {
    const dialog = this.hostPathDialog();
    if (dialog?.request.kind === 'directory' && dialog.readout !== null) {
      this.finishHostPathDialog(dialog.readout.directory);
    }
  }

  cancelHostPathDialog(): void {
    this.finishHostPathDialog(null);
  }

  openProjectDialog(mode: 'createProject' | 'saveProjectAs'): void {
    const identity = this.state().authoringDocument?.identity;
    this.projectDraft = mode === 'createProject'
      ? {
          root: this.projectRoot,
          projectFile: 'content/projects/new-project.project.json',
          projectId: 'new-project',
          name: 'New Project',
          entryScene: 'scene/main',
          entrySceneName: 'Main',
        }
      : {
          root: this.projectRoot,
          projectFile: identity?.relativeProjectFile ?? this.projectFile,
          projectId: identity === undefined ? 'project-copy' : `${identity.projectId}-copy`,
          name: identity === undefined ? 'Project Copy' : `${identity.name} Copy`,
          entryScene: identity?.entryScene ?? 'scene/main',
          entrySceneName: this.state().authoringDocument?.sceneHierarchy.name ?? 'Main',
        };
    this.authoringDialog = mode;
    this.store.toggleMenu(null);
  }

  submitProjectDialog(): void {
    if (this.authoringDialog === 'createProject') {
      void this.store.createProject(this.projectDraft);
    } else if (this.authoringDialog === 'saveProjectAs') {
      void this.store.saveProjectAs({
        root: this.projectDraft.root,
        projectFile: this.projectDraft.projectFile,
        projectId: this.projectDraft.projectId,
        name: this.projectDraft.name,
      });
    }
    this.projectRoot = this.projectDraft.root;
    this.projectFile = this.projectDraft.projectFile;
    this.authoringDialog = null;
  }

  openSceneDialog(): void {
    const document = this.state().authoringDocument;
    this.sceneDraft = {
      sceneId: document?.identity.entryScene ?? 'scene/main',
      name: document?.sceneHierarchy.name ?? 'Main',
      makeEntry: true,
    };
    this.authoringDialog = 'scene';
  }

  createScene(): void {
    void this.store.createScene(
      this.sceneDraft.sceneId,
      this.sceneDraft.name,
      this.sceneDraft.makeEntry,
    );
    this.authoringDialog = null;
  }

  renameScene(): void {
    void this.store.renameScene(this.sceneDraft.sceneId, this.sceneDraft.name);
    this.authoringDialog = null;
  }

  setEntryScene(): void {
    void this.store.setEntryScene(this.sceneDraft.sceneId);
    this.authoringDialog = null;
  }

  deleteScene(): void {
    void this.store.deleteScene(this.sceneDraft.sceneId);
    this.authoringDialog = null;
  }

  openObjectDialog(): void {
    const document = this.state().authoringDocument;
    const entityIds = document?.inspections.entityState.entityIds ?? [];
    const parentEntityId = this.store.selectedHierarchyNode()?.entityId ?? null;
    const childOrder = document?.sceneHierarchy.nodes.filter(
      (node) => node.parentNodeId === this.store.selectedHierarchyNode()?.nodeId,
    ).length ?? 0;
    this.objectDraft = {
      entityId: Math.max(0, ...entityIds) + 1,
      name: 'New Entity',
      parentEntityId,
      childOrder,
      kind: 'empty',
      asset: '',
      clip: '',
    };
    this.authoringDialog = 'object';
  }

  createObject(): void {
    void this.store.createSceneObject({
      entityId: this.objectDraft.entityId,
      name: this.objectDraft.name,
      parentEntityId: this.objectDraft.parentEntityId,
      childOrder: this.objectDraft.childOrder,
      transform: {
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      appearance: this.objectDraft.kind === 'staticMesh'
        ? { kind: 'staticMesh', asset: this.objectDraft.asset, visible: true }
        : this.objectDraft.kind === 'animatedMesh'
          ? {
              kind: 'animatedMesh',
              asset: this.objectDraft.asset,
              visible: true,
              clip: this.objectDraft.clip,
            }
        : this.objectDraft.kind === 'light'
          ? { kind: 'light', light: this.light() }
          : { kind: 'empty' },
      collision: null,
      kinematic: null,
    });
    this.authoringDialog = null;
  }

  refreshProject(): void {
    void this.store.refreshProject();
  }

  closeProject(): void {
    void this.store.closeProject();
  }

  openAssetImportDialog(): void {
    this.authoringDialog = 'assetImport';
    this.store.toggleMenu(null);
  }

  prepareAssetImport(): void {
    void this.store.prepareAssetImport(
      { scope: this.assetImportScope, path: this.assetImportPath },
      {
        scale: this.assetImportScale,
        generateCollision: this.assetImportGenerateCollision,
        materialNamespace: this.assetImportMaterialNamespace.trim() === ''
          ? null
          : this.assetImportMaterialNamespace.trim(),
      },
    );
    this.authoringDialog = null;
  }

  prepareAssetReimport(assetId: string): void {
    void this.store.prepareAssetReimport(assetId);
  }

  applyAssetImport(): void {
    void this.store.applyAssetImport();
  }

  discardAssetImport(): void {
    void this.store.discardAssetImport();
  }

  selectedAsset(): AssetEntryReadout | null {
    const assetId = this.state().assetWorkspace.selectedAssetId;
    return this.state().authoringDocument?.assetBrowser.assets.find(
      (asset) => asset.assetId === assetId,
    ) ?? null;
  }

  gridColorHex(
    key: 'minorColor' | 'majorColor' | 'xAxisColor' | 'yAxisColor' | 'zAxisColor',
  ): string {
    const color = this.state().settings[key];
    return `#${color.slice(0, 3).map((value) => Math.round(value * 255)
      .toString(16)
      .padStart(2, '0')).join('')}`;
  }

  captureKeyboardBinding(event: KeyboardEvent, key: keyof StudioKeyboardBindings): void {
    event.preventDefault();
    event.stopPropagation();
    if (event.code.length > 0) this.store.setKeyboardBinding(key, event.code);
  }

  updateSnapAxis(kind: 'translation' | 'scale', axis: 0 | 1 | 2, value: number): void {
    const field = kind === 'translation' ? 'translationSnapAxes' : 'scaleSnapAxes';
    const values = [...this.state().settings[field]] as [number, number, number];
    values[axis] = value;
    this.store.updateSettings({ [field]: values });
  }

  setInspectorMode(mode: 'entity' | 'voxel'): void {
    if (mode !== 'voxel') {
      this.voxelEditor()?.cancelObjectPlacement();
      this.voxelEditorPreview.set(null);
    }
    this.inspectorMode = mode;
    this.pivotGroundingToolActive = false;
    this.animationInspectionToolActive = false;
  }

  openPivotGroundingTool(): void {
    this.setInspectorMode('entity');
    this.pivotGroundingToolActive = true;
    this.store.toggleMenu(null);
  }

  openAnimationInspectionTool(): void {
    this.setInspectorMode('entity');
    this.animationInspectionToolActive = true;
    this.animationInspectionClip = this.selectedAnimatedMeshResource()?.clipIds[0] ?? '';
    this.animationInspectionTime = 0;
    this.animationInspectionCapture.set(null);
    this.animationInspectionSample.set(null);
    this.store.toggleMenu(null);
  }

  chooseAnimationInspectionClip(clip: string): void {
    this.animationInspectionClip = clip;
    this.animationInspectionTime = 0;
    this.animationInspectionCapture.set(null);
    this.animationInspectionSample.set(null);
    this.sampleAnimationInspection(0);
  }

  effectiveAnimationInspectionClip(): string {
    const clips = this.selectedAnimatedMeshResource()?.clipIds ?? [];
    return clips.includes(this.animationInspectionClip)
      ? this.animationInspectionClip
      : clips[0] ?? '';
  }

  sampleAnimationInspection(raw: number | string): void {
    const normalizedTime = Number(raw);
    if (!Number.isFinite(normalizedTime)) return;
    this.animationInspectionTime = normalizedTime;
    try {
      const clip = this.effectiveAnimationInspectionClip();
      this.animationInspectionClip = clip;
      const sample = this.studioViewport()?.sampleSelectedAnimatedMesh(
        clip,
        normalizedTime,
      );
      if (sample === undefined) throw new Error('shared Studio viewport is unavailable');
      this.animationInspectionSample.set(sample);
      const facts = sample.skinningFacts;
      this.animationInspectionReadout.set(
        `${sample.clip} ${(sample.normalizedTime * 100).toFixed(0)}% / ${sample.durationSeconds.toFixed(3)}s · ${facts.joints.length} joints · ${facts.inverseBindMatrixCount} inverse binds (${facts.inverseBindMatricesFinite ? 'finite' : 'invalid'}) · weights ${facts.weightsNormalized ? 'normalized' : 'invalid'} (${facts.invalidWeightVertexCount} invalid, max error ${facts.maximumWeightSumError.toExponential(2)}) · ${facts.interpolationModes.join('/')} interpolation · clone ${facts.instanceRootDistinctFromTemplate && facts.skeletonsIndependentFromTemplate ? 'independent' : 'invalid'} · shared geometry/material ${facts.sharedGeometryCount}/${facts.sharedMaterialCount} · ${sample.diagnostics.length} diagnostics`,
      );
    } catch (error) {
      this.store.reportUiError(error instanceof Error ? error.message : String(error));
    }
  }

  playAnimationInspection(): void {
    try {
      const clip = this.effectiveAnimationInspectionClip();
      this.animationInspectionClip = clip;
      const fadeSeconds = Math.min(2, Math.max(0, this.animationInspectionFadeSeconds));
      this.animationInspectionFadeSeconds = fadeSeconds;
      this.studioViewport()?.setSelectedAnimatedMeshPlayback({
        kind: 'play',
        clip,
        loop: 'repeat',
        speed: 1,
        weight: 1,
        restart: true,
        fadeSeconds,
      });
      this.animationInspectionReadout.set(
        `Playing ${clip} · fade ${fadeSeconds.toFixed(2)}s`,
      );
    } catch (error) {
      this.store.reportUiError(error instanceof Error ? error.message : String(error));
    }
  }

  pauseAnimationInspection(): void {
    try {
      this.studioViewport()?.setSelectedAnimatedMeshPlayback({ kind: 'pause' });
      this.animationInspectionReadout.set(`Paused ${this.animationInspectionClip}`);
    } catch (error) {
      this.store.reportUiError(error instanceof Error ? error.message : String(error));
    }
  }

  captureAnimationInspection(): void {
    try {
      const clip = this.effectiveAnimationInspectionClip();
      this.animationInspectionClip = clip;
      const capture = this.studioViewport()?.captureSelectedAnimatedMesh(
        clip,
      );
      if (capture === undefined) throw new Error('shared Studio viewport is unavailable');
      this.animationInspectionCapture.set(capture);
      const diagnosticCount = capture.samples.reduce(
        (total, sample) => total + sample.diagnostics.length,
        0,
      );
      this.animationInspectionReadout.set(
        `Captured ${capture.samples.length} labeled frames · ${diagnosticCount} diagnostics`,
      );
    } catch (error) {
      this.store.reportUiError(error instanceof Error ? error.message : String(error));
    }
  }

  selectHierarchyNode(nodeId: number, event: MouseEvent): void {
    if (event.detail > 1) return;
    this.#hierarchySelection = this.store.selectHierarchyNode(nodeId);
  }

  async focusHierarchyNode(nodeId: number): Promise<void> {
    await this.#hierarchySelection;
    if (this.state().selection.sceneNodeId !== nodeId) return;
    const node = this.state().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.nodeId === nodeId,
    );
    if (node === undefined) return;
    this.studioViewport()?.focusTarget(node.worldTransform.translation);
  }

  validateVoxelPick(candidate: VoxelViewportPickCandidate): void {
    void this.store.validateVoxelViewportPick(candidate);
  }

  runVoxelAction(action: VoxelEditorAction): void {
    void this.store.runVoxelAction(action);
  }

  setVoxelEditorPreview(preview: VoxelEditorPreviewPresentation | null): void {
    this.voxelEditorPreview.set(preview);
  }

  placeVoxelObjectAtPick(pick: StudioVoxelObjectPlacementPick): void {
    this.voxelEditor()?.applyObjectPlacementPick(pick.worldPoint);
  }

  commitVoxelObjectPlacement(): void {
    this.voxelEditor()?.placeObjectInstance();
  }

  cancelVoxelObjectPlacement(): void {
    this.voxelEditor()?.cancelObjectPlacement();
    this.voxelEditorPreview.set(null);
  }

  beginSelectedPreview(tool: StudioTransformTool = 'translate'): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      this.selectedTransformTool.set(tool);
      if (this.state().preview?.entityId === entityId) {
        this.store.setPreviewTool(tool, this.state().settings.transformOrientation);
      }
    }
  }

  selectPointerTool(): void {
    const finish = async (): Promise<void> => {
      if (this.state().preview !== null && !await this.store.commitPreview()) return;
      this.selectedTransformTool.set(null);
    };
    void finish();
  }

  beginViewportTransformDrag(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    const tool = this.selectedTransformTool();
    if (entityId === null || entityId === undefined || tool === null) return;
    this.store.beginTransformPreview(entityId, tool, this.state().settings.transformOrientation);
  }

  setTransformOrientation(orientation: StudioTransformOrientation): void {
    this.store.updateSettings({ transformOrientation: orientation });
    this.store.setPreviewOrientation(orientation);
  }

  applyTransformCandidate(transform: Transform): void {
    this.store.applyPreviewWorldTransform(transform);
  }

  finishTransformGizmoDrag(cancelled: boolean): void {
    if (!cancelled) void this.store.commitPreview();
  }

  canPreviewTranslation(): boolean {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    return entityId !== null && entityId !== undefined;
  }

  updateTranslation(axis: 0 | 1 | 2, raw: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    this.selectedTransformTool.set('translate');
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTranslationPreview(entityId);
    }
    this.store.setPreviewTranslationAxis(axis, Number(raw));
  }

  updateRotation(axis: 0 | 1 | 2 | 3, raw: string): void {
    this.selectedTransformTool.set('rotate');
    this.ensureSelectedPreview('rotate');
    this.store.setPreviewRotationAxis(axis, Number(raw));
  }

  updateScale(axis: 0 | 1 | 2, raw: string): void {
    this.selectedTransformTool.set('scale');
    this.ensureSelectedPreview('scale');
    this.store.setPreviewScaleAxis(axis, Number(raw));
  }

  translation(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.translation[axis];
    return this.store.selectedHierarchyNode()?.localTransform.translation[axis] ?? null;
  }

  rotation(axis: 0 | 1 | 2 | 3): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.rotation[axis];
    return this.store.selectedHierarchyNode()?.localTransform.rotation[axis] ?? null;
  }

  scale(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.scale[axis];
    return this.store.selectedHierarchyNode()?.localTransform.scale[axis] ?? null;
  }

  renderableTranslation(axis: 0 | 1 | 2): number | null {
    return this.store.selectedHierarchyNode()?.renderableTransform.translation[axis] ?? null;
  }

  renderableRotation(axis: 0 | 1 | 2 | 3): number | null {
    return this.store.selectedHierarchyNode()?.renderableTransform.rotation[axis] ?? null;
  }

  renderableScale(axis: 0 | 1 | 2): number | null {
    return this.store.selectedHierarchyNode()?.renderableTransform.scale[axis] ?? null;
  }

  updateRenderableTranslation(axis: 0 | 1 | 2, raw: string): void {
    this.updateRenderableTransform('translation', axis, Number(raw));
  }

  updateRenderableRotation(axis: 0 | 1 | 2 | 3, raw: string): void {
    this.updateRenderableTransform('rotation', axis, Number(raw));
  }

  updateRenderableScale(axis: 0 | 1 | 2, raw: string): void {
    this.updateRenderableTransform('scale', axis, Number(raw));
  }

  alignRenderableLowerBound(): void {
    const node = this.store.selectedHierarchyNode();
    const entity = this.store.selectedEntity();
    const inspection = this.groundingInspection();
    if (
      node?.entityId === null
      || node?.entityId === undefined
      || entity === null
      || entity.transform === null
      || inspection === null
    ) return;
    const desiredWorld: Transform = {
      ...entity.transform,
      translation: [
        entity.transform.translation[0],
        entity.transform.translation[1] - inspection.clearance,
        entity.transform.translation[2],
      ],
    };
    void this.store.setSceneObjectRenderableTransform(
      node.entityId,
      localTransformFromWorld(node.worldTransform, desiredWorld),
    );
  }

  private updateRenderableTransform(
    field: 'translation' | 'rotation' | 'scale',
    axis: 0 | 1 | 2 | 3,
    value: number,
  ): void {
    const node = this.store.selectedHierarchyNode();
    if (node?.entityId === null || node?.entityId === undefined || !Number.isFinite(value)) return;
    const translation = [...node.renderableTransform.translation] as [number, number, number];
    const rotation = [...node.renderableTransform.rotation] as [number, number, number, number];
    const scale = [...node.renderableTransform.scale] as [number, number, number];
    if (field === 'rotation') {
      rotation[axis] = value;
    } else if (field === 'translation') {
      translation[axis as 0 | 1 | 2] = value;
    } else {
      scale[axis as 0 | 1 | 2] = value;
    }
    void this.store.setSceneObjectRenderableTransform(node.entityId, {
      translation,
      rotation,
      scale,
    });
  }

  renameSelected(name: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      void this.store.renameSceneObject(entityId, name);
    }
  }

  deleteSelected(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      void this.store.deleteSceneObject(entityId);
    }
  }

  reparentSelected(parentRaw: string, orderRaw: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    const parent = parentRaw.trim() === '' ? null : Number(parentRaw);
    void this.store.reparentSceneObject(entityId, parent, Number(orderRaw));
  }

  applyAppearance(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    let appearance: StudioSceneAppearance;
    if (this.appearanceKind === 'staticMesh') {
      appearance = { kind: 'staticMesh', asset: this.appearanceAsset, visible: this.appearanceVisible };
    } else if (this.appearanceKind === 'animatedMesh') {
      appearance = {
        kind: 'animatedMesh',
        asset: this.appearanceAsset,
        visible: this.appearanceVisible,
        clip: this.appearanceClip,
      };
    } else if (this.appearanceKind === 'light') {
      appearance = { kind: 'light', light: this.light() };
    } else {
      appearance = { kind: 'empty' };
    }
    void this.store.setSceneObjectAppearance(entityId, appearance);
  }

  animatedMeshClipIds(asset: string): readonly string[] {
    return this.state().authoringDocument?.animatedMeshResources.find(
      (resource) => resource.asset === asset,
    )?.clipIds ?? [];
  }

  animatedProjectionClips(): string {
    return (this.state().liveProjection?.frame.ops ?? [])
      .flatMap((operation) => operation.op === 'createAnimatedMeshInstance'
        && operation.instance.playback?.kind === 'play'
        ? [operation.instance.playback.clip]
        : [])
      .join(',');
  }

  chooseAnimatedMeshAsset(target: 'object' | 'appearance', asset: string): void {
    const firstClip = this.animatedMeshClipIds(asset)[0] ?? '';
    if (target === 'object') {
      this.objectDraft.asset = asset;
      this.objectDraft.clip = firstClip;
      return;
    }
    this.appearanceAsset = asset;
    this.appearanceClip = firstClip;
  }

  applyCollision(attached: boolean): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    void this.store.setEntityCollision(entityId, attached ? {
      enabled: this.collisionEnabled,
      staticCollider: this.staticCollider,
    } : null);
  }

  applyKinematic(attached: boolean): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    void this.store.setEntityKinematic(entityId, attached ? {
      halfExtents: this.kinematicHalfExtents,
      velocity: this.kinematicVelocity,
    } : null);
  }

  nodeIcon(kind: string): string {
    switch (kind) {
      case 'emptyGroup': return '▾';
      case 'light': return '☀';
      case 'voxelVolume': return '▦';
      case 'entityInstance': return '▤';
      case 'marker': return '⌖';
      default: return '◇';
    }
  }

  commitPreview(): void {
    void this.store.commitPreview();
  }

  ownerDiagnosticCount(): number {
    return this.ownerDiagnostics().length;
  }

  ownerDiagnostics(): readonly OwnerDiagnostic[] {
    const inspections = this.state().authoringDocument?.inspections;
    if (inspections === undefined) return [];
    return [
      ...inspections.catalog.diagnostics.diagnostics,
      ...inspections.scene.diagnostics.diagnostics,
      ...inspections.entityState.diagnostics.diagnostics,
      ...inspections.persistence.diagnostics.diagnostics,
    ];
  }

  private ensureSelectedPreview(tool: StudioTransformTool): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTransformPreview(
        entityId,
        tool,
        this.state().settings.transformOrientation,
      );
    } else {
      this.store.setPreviewTool(tool, this.state().settings.transformOrientation);
    }
  }

  private openHostPathDialog(request: StudioHostPathRequest): Promise<string | null> {
    this.#hostPathResolve?.(null);
    this.#restoreFocus = globalThis.document?.activeElement instanceof HTMLElement
      ? globalThis.document.activeElement
      : null;
    const initialDirectory = request.kind === 'directory'
      ? request.initialPath
      : hostParent(request.initialPath);
    this.hostPathDialog.set({ request, readout: null, filter: '', busy: true, error: null });
    this.navigateHostDirectory(initialDirectory || '/');
    globalThis.setTimeout(() => this.hostPathFilterElement()?.nativeElement.focus(), 0);
    return new Promise((resolvePromise) => { this.#hostPathResolve = resolvePromise; });
  }

  private finishHostPathDialog(path: string | null): void {
    const resolvePromise = this.#hostPathResolve;
    this.#hostPathResolve = null;
    this.hostPathDialog.set(null);
    resolvePromise?.(path);
    const focus = this.#restoreFocus;
    this.#restoreFocus = null;
    queueMicrotask(() => focus?.focus());
  }

  private light(): StoredLight {
    const base = {
      color: this.lightColor,
      intensity: this.lightIntensity,
      enabled: this.lightEnabled,
      shadows: this.lightShadows,
    };
    switch (this.lightKind) {
      case 'ambient': return { kind: 'ambient', ...base };
      case 'directional': return { kind: 'directional', ...base };
      case 'point': return {
        kind: 'point', ...base, range: this.lightRange, decay: this.lightDecay,
      };
      case 'spot': return {
        kind: 'spot', ...base, range: this.lightRange, decay: this.lightDecay,
        outerAngleRadians: this.lightOuterAngle, penumbra: this.lightPenumbra,
      };
    }
  }
}

function createStudioVoxelObjectInspectorHost(): StudioVoxelObjectInspectorHost {
  const store = inject(STUDIO_WORKSPACE);
  return Object.freeze({
    read: (ownerEntityId: number) => {
      const snapshot = store.snapshot();
      const instance = snapshot.authoringDocument?.voxelObjectAuthoring.instances.find(
        (candidate) => candidate.ownerEntityId === ownerEntityId,
      ) ?? null;
      const asset = instance === null
        ? null
        : snapshot.authoringDocument?.voxelObjectAuthoring.assets.find(
            (candidate) => candidate.assetId === instance.instance.voxelObjectAssetId,
          ) ?? null;
      const playback = instance !== null
        && snapshot.voxelWorkspace.objectPlayback?.instanceId === instance.instance.instanceId
        ? snapshot.voxelWorkspace.objectPlayback
        : null;
      return {
        instance,
        asset,
        knownInstanceIds: snapshot.authoringDocument?.voxelObjectAuthoring.instances.map(
          (entry) => entry.instance.instanceId,
        ) ?? [],
        playback,
        busy: snapshot.operation !== 'idle',
      };
    },
    run: (action: VoxelEditorAction) => {
      void store.runVoxelAction(action);
    },
  });
}

function hostParent(path: string): string {
  const normalized = path.replace(/\/+$/, '');
  const separator = normalized.lastIndexOf('/');
  return separator <= 0 ? '/' : normalized.slice(0, separator);
}

function absoluteHostPath(root: string, path: string): string {
  if (path.startsWith('/')) return path;
  return `${root.replace(/\/+$/, '')}/${path.replace(/^\/+/, '')}`;
}

function relativeHostPath(root: string, path: string): string | null {
  const prefix = `${root.replace(/\/+$/, '')}/`;
  return path.startsWith(prefix) ? path.slice(prefix.length) : null;
}

function transformedBounds(
  bounds: MeshBoundsDescriptor,
  transform: Transform,
): StudioGroundingInspection['bounds'] {
  const corners: readonly (readonly [number, number, number])[] = [
    [bounds.min[0], bounds.min[1], bounds.min[2]],
    [bounds.max[0], bounds.min[1], bounds.min[2]],
    [bounds.min[0], bounds.max[1], bounds.min[2]],
    [bounds.max[0], bounds.max[1], bounds.min[2]],
    [bounds.min[0], bounds.min[1], bounds.max[2]],
    [bounds.max[0], bounds.min[1], bounds.max[2]],
    [bounds.min[0], bounds.max[1], bounds.max[2]],
    [bounds.max[0], bounds.max[1], bounds.max[2]],
  ];
  const points = corners.map((point) => transformPoint(transform, point));
  return {
    min: [
      Math.min(...points.map((point) => point[0])),
      Math.min(...points.map((point) => point[1])),
      Math.min(...points.map((point) => point[2])),
    ],
    max: [
      Math.max(...points.map((point) => point[0])),
      Math.max(...points.map((point) => point[1])),
      Math.max(...points.map((point) => point[2])),
    ],
  };
}

function transformPoint(
  transform: Transform,
  point: readonly [number, number, number],
): readonly [number, number, number] {
  const scaled: readonly [number, number, number] = [
    point[0] * transform.scale[0],
    point[1] * transform.scale[1],
    point[2] * transform.scale[2],
  ];
  const [x, y, z, w] = transform.rotation;
  const tx = 2 * (y * scaled[2] - z * scaled[1]);
  const ty = 2 * (z * scaled[0] - x * scaled[2]);
  const tz = 2 * (x * scaled[1] - y * scaled[0]);
  return [
    transform.translation[0] + scaled[0] + w * tx + (y * tz - z * ty),
    transform.translation[1] + scaled[1] + w * ty + (z * tx - x * tz),
    transform.translation[2] + scaled[2] + w * tz + (x * ty - y * tx),
  ];
}
