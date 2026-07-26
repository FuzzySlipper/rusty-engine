import {
  ChangeDetectionStrategy,
  Component,
  HostBinding,
  computed,
  effect,
  inject,
  signal,
  viewChild,
  type ElementRef,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import type {
  AssetEntryReadout,
  OwnerDiagnostic,
  StoredLight,
  StudioSceneAppearance,
} from '@rusty-engine/studio-adapter-client';
import type { EditorGridDescriptor, Transform } from '@rusty-engine/render-contracts';
import type { StudioKeyboardBindings } from '@rusty-engine/studio-user-settings';
import {
  StudioViewportComponent,
  type StudioTransformGizmoDelta,
  type StudioTransformAxis,
  type StudioTransformOrientation,
  type StudioTransformTool,
  type StudioVoxelPreview,
  type VoxelViewportPickCandidate,
} from '@rusty-engine/studio-viewport';
import {
  VoxelEditorComponent,
  type VoxelBrushPreviewPresentation,
  type VoxelEditorAction,
} from '@rusty-engine/studio-voxel-editor';

import { STUDIO_WORKSPACE } from './tokens.js';
import {
  HttpStudioHostFileBrowser,
  HttpStudioRenderResourceClient,
  type StudioHostDirectoryReadout,
  type StudioHostPathRequest,
} from './transport.js';

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
  imports: [FormsModule, StudioViewportComponent, VoxelEditorComponent],
  templateUrl: './studio-shell.component.html',
  styleUrl: './studio-shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioShellComponent {
  readonly store = inject(STUDIO_WORKSPACE);
  readonly state = this.store.snapshot;
  readonly brushPreview = signal<VoxelBrushPreviewPresentation | null>(null);
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
    const brush = this.brushPreview();
    if (brush !== null) return brush;
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

  readonly #hostFiles = new HttpStudioHostFileBrowser();
  readonly #renderResources = new HttpStudioRenderResourceClient();
  private readonly hostPathFilterElement = viewChild<ElementRef<HTMLInputElement>>('hostPathFilter');
  #hostPathResolve: ((path: string | null) => void) | null = null;
  #restoreFocus: HTMLElement | null = null;

  constructor() {
    effect(() => {
      const snapshot = this.state();
      const canonicalRoot = snapshot.userSettings.projectRoot;
      const relativeProjectFile = snapshot.authoringDocument?.identity.relativeProjectFile;
      if (canonicalRoot !== null) this.projectRoot = canonicalRoot;
      if (relativeProjectFile !== undefined) this.projectFile = relativeProjectFile;
    });
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
    this.inspectorMode = mode;
  }

  validateVoxelPick(candidate: VoxelViewportPickCandidate): void {
    void this.store.validateVoxelViewportPick(candidate);
  }

  runVoxelAction(action: VoxelEditorAction): void {
    void this.store.runVoxelAction(action);
  }

  setBrushPreview(preview: VoxelBrushPreviewPresentation | null): void {
    this.brushPreview.set(preview);
  }

  beginSelectedPreview(tool: StudioTransformTool = 'translate'): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      this.store.beginTransformPreview(entityId, tool, this.state().settings.transformOrientation);
    }
  }

  setTransformOrientation(orientation: StudioTransformOrientation): void {
    this.store.updateSettings({ transformOrientation: orientation });
    this.store.setPreviewOrientation(orientation);
  }

  applyTransformGizmoDelta(delta: StudioTransformGizmoDelta): void {
    this.store.applyPreviewToolDelta(delta.axis, delta.delta, delta.fine, delta.toggleSnap);
  }

  finishTransformGizmoDrag(axis: StudioTransformAxis, cancelled: boolean): void {
    this.store.finishPreviewToolDrag(axis, cancelled);
  }

  canPreviewTranslation(): boolean {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    return entityId !== null && entityId !== undefined;
  }

  updateTranslation(axis: 0 | 1 | 2, raw: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTranslationPreview(entityId);
    }
    this.store.setPreviewTranslationAxis(axis, Number(raw));
  }

  updateRotation(axis: 0 | 1 | 2 | 3, raw: string): void {
    this.ensureSelectedPreview();
    this.store.setPreviewRotationAxis(axis, Number(raw));
  }

  updateScale(axis: 0 | 1 | 2, raw: string): void {
    this.ensureSelectedPreview();
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

  private ensureSelectedPreview(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTranslationPreview(entityId);
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
